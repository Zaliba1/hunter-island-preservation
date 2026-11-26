import { abilityActivation } from './abilities.mjs';
import { ELEMENT_ORDER } from './original-rules.mjs';
import { hitCount } from './barriers.mjs';

const only = (value, keys) => value && typeof value === 'object' && !Array.isArray(value) && Object.keys(value).every(key => keys.includes(key));
export function compileBarrierSkill(skill, catalog, { teams = false } = {}) {
  const def = catalog?.abilities?.[skill.type], args = skill.arguments || {}, reasons = [];
  const result = { slot: skill.slot, type: skill.type, name: skill.name, effect: 'barrier', target: 'self', supported: false, reasons };
  if (!def || def.class !== 'CombatAbility') { reasons.push('keine aktive Schutzfähigkeit'); return result; }
  const sacrificeChance = args.sacrificeChance ?? 0;
  const group = teams && def.targetPicker?.allFriends === true && !def.targetPicker.selfOnly;
  if (!only(def, ['class', 'parent', 'targetPicker', 'components', 'eventData', 'soundTable', 'activationLength']) ||
      !only(args, ['instanceName', 'activationLength', 'commonComponentArguments', 'aiChance', 'sacrificeChance',
        ...(group ? ['abilityScalor'] : [])]) ||
      ![0, ...(group ? [1] : [])].includes(sacrificeChance)) reasons.push('weitere Fähigkeitswirkungen');
  const picker = def.targetPicker;
  if (!only(picker, ['class', 'selfOnly', 'allFriends']) || picker.class !== 'TargetPickerGui' || (!group && picker.selfOnly !== true)) reasons.push('andere Schutz-Zielwahl');
  const ref = Array.isArray(def.components) && def.components.length === 1 ? def.components[0] : null;
  if (!ref || !only(ref, ['type']) || ref.type !== 'Utility.ApplyBuff' || catalog.components[ref.type]?.class !== 'ApplyBuffAbilityComponent') reasons.push('anderer Schutzbaustein');
  const common = args.commonComponentArguments;
  if (!only(common, ['buffType', 'buffArguments', ...(group ? ['amount'] : [])]) || common.buffType !== 'StatOnly' || !only(catalog.buffs?.StatOnly, ['class']) || catalog.buffs.StatOnly.class !== 'Buff') reasons.push('anderer Bufftyp');
  const buffArgs = common?.buffArguments;
  if (!only(buffArgs, ['components', 'status']) || !Array.isArray(buffArgs.components) || buffArgs.components.length !== 2) reasons.push('weitere Buffregeln');
  const parts = Array.isArray(buffArgs?.components) ? buffArgs.components : [];
  const protect = parts.find(p => p?.type === 'Defensive.PreventDamage'), remove = parts.find(p => p?.type === 'Utility.RemoveOnHurt');
  if (!protect || !remove || !only(protect, ['type', 'arguments']) || !only(remove, ['type', 'arguments']) ||
      catalog.buffComponents?.[protect?.type]?.class !== 'PreventDamageBuffComponent' ||
      catalog.buffComponents?.[remove?.type]?.class !== 'RemoveOnHurtBuffComponent') reasons.push('weitere Buffbausteine');
  let barrier, activation;
  try {
    const p = protect.arguments, r = remove.arguments;
    if (!only(p, ['elementsAllowed', 'flatReduction', 'percentReduction']) || !only(r, ['numHitsAllowed'])) throw new Error();
    const remainingHits = hitCount(r.numHitsAllowed);
    if (!Array.isArray(p.elementsAllowed) || !p.elementsAllowed.length || (!group && p.elementsAllowed.length !== 1) ||
        !p.elementsAllowed.every(element => ELEMENT_ORDER.includes(element)) || ![1, '1'].includes(p.percentReduction) ||
        (p.flatReduction ?? 0) !== 0 || ![1, ...(group ? [2] : [])].includes(remainingHits)) throw new Error();
    barrier = { allowedElements: [...p.elementsAllowed], flatReduction: 0, percentReduction: 1, remainingHits };
    activation = abilityActivation(args.activationLength, def.activationLength).length;
    if (activation < 1 || activation > 100000) throw new Error();
  } catch { reasons.push('ungeprüfte Schutzparameter'); }
  return { ...result, target: group ? 'team' : 'self', supported: !reasons.length,
    reasons: [...new Set(reasons)], activation, barrier, ...(group ? { sacrificeChance } : {}) };
}
