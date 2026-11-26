// Narrow healing component: single ally, self or all other living allies.
// Self-sacrifice chance follows healing; narrow Devour consumes the last reserve first.
// No resurrection, passive reactions or magnification modifiers.
import { abilityActivation } from './abilities.mjs';
import { originalDice, unbuffedStats } from './original-rules.mjs';
import { adjustByModifier } from './timing.mjs';
import { ORIGINAL_ACTIVE_SIZE } from './team-limits.mjs';

const definitionKeys = new Set(['class', 'parent', 'abstract', 'activationLength', 'components',
  'targetPicker', 'eventData', 'soundTable', 'buttonType', 'aiChance', 'SPECIAL NOTE',
  'enemyBuffs', 'friendlyBuffs', 'selfBuffs', 'passiveBuffs']);
const argumentKeys = new Set(['instanceName', 'activationLength', 'commonComponentArguments',
  'components', 'abilityScalor', 'aiChance', 'hideBlood', 'sacrificeChance', 'soundTable']);
export function compileHealingSkill(skill, catalog) {
  const reasons = [], def = catalog?.abilities?.[skill.type], args = skill.arguments || {};
  const result = { slot: skill.slot, type: skill.type, name: skill.name, effect: 'heal', supported: false, reasons };
  if (!def) { reasons.push('Fähigkeitsdefinition fehlt'); return result; }
  if (def.class !== 'CombatAbility' || def.abstract) reasons.push('keine aktive Kampffähigkeit');
  if (Object.keys(def).some(k => !definitionKeys.has(k)) || Object.keys(args).some(k => !argumentKeys.has(k))) reasons.push('weitere Heilregeln');
  for (const key of ['enemyBuffs', 'friendlyBuffs', 'selfBuffs', 'passiveBuffs']) {
    if (def[key] != null && (!Array.isArray(def[key]) || def[key].length)) reasons.push('Buffwirkung');
  }
  let reserveSacrifice = null;
  if (args.components != null && (!Array.isArray(args.components) || args.components.length)) {
    const extra = Array.isArray(args.components) && args.components.length === 1 ? args.components[0] : null;
    const part = extra && catalog?.components?.[extra.type];
    if (part?.class === 'SacrificeAbilityComponent' && Object.keys(part).every(k => k === 'class') &&
        Object.keys(extra).every(k => ['type', 'arguments'].includes(k)) && extra.arguments?.levelMultiplier === .03 &&
        Object.keys(extra.arguments).every(k => k === 'levelMultiplier')) reserveSacrifice = { levelMultiplier: .03 };
    else reasons.push('zusätzliche Komponenten');
  }
  const abilityScale = args.abilityScalor ?? 1;
  if (![1, 3].includes(abilityScale)) reasons.push('individuelle Skalierung');
  const sacrificeChance = args.sacrificeChance ?? 0;
  if (typeof sacrificeChance !== 'number' || !Number.isFinite(sacrificeChance) || sacrificeChance < 0 || sacrificeChance > 1) reasons.push('ungültige Opferchance');
  const ref = Array.isArray(def.components) && def.components.length === 1 ? def.components[0] : null;
  const part = ref && catalog?.components?.[ref.type];
  if (part?.class !== 'HealingAbilityComponent' || Object.keys(part).some(k => !['class', 'parent', 'roll'].includes(k)) ||
      part.roll?.relatedStat !== 'Magic' || Object.keys(part.roll).some(k => k !== 'relatedStat')) reasons.push('anderer Heilbaustein');
  if (ref && Object.keys(ref).some(k => !['type', 'arguments'].includes(k))) reasons.push('Komponentenparameter');
  const componentArgs = ref?.arguments || {}, common = args.commonComponentArguments || {};
  if (Object.keys(componentArgs).some(k => k !== 'allowSelf') || Object.keys(common).some(k => k !== 'allowSelf')) reasons.push('weitere Komponentenparameter');
  const allowSelf = common.allowSelf ?? componentArgs.allowSelf ?? false;
  const picker = def.targetPicker;
  let target;
  if (picker?.class === 'TargetPickerGui' && picker.selfOnly === true &&
      Object.keys(picker).every(k => ['class', 'selfOnly'].includes(k)) && allowSelf === true) target = 'self';
  else if (picker?.class === 'TargetPickerGui' && picker.friendlyTargets === 1 &&
      Object.keys(picker).every(k => ['class', 'friendlyTargets'].includes(k)) && allowSelf === false) target = 'friend';
  else if (picker?.class === 'TargetPickerGui' && picker.allFriends === true &&
      Object.keys(picker).every(k => ['class', 'friendlyTargets', 'allFriends'].includes(k)) &&
      (picker.friendlyTargets == null || picker.friendlyTargets === 1) && allowSelf === false) target = 'friends';
  else reasons.push('andere Heilzielwahl');
  if (reserveSacrifice && (target !== 'self' || sacrificeChance !== 0 || abilityScale !== 1)) reasons.push('weitere Reserveopfer-Kombination');
  let activation;
  try { activation = abilityActivation(args.activationLength, def.activationLength).length; }
  catch { reasons.push('ungültige Zeitangabe'); }
  if (!Number.isInteger(activation) || activation < 1 || activation > 100000) reasons.push('ungültige Aktivierungszeit');
  return { ...result, supported: !reasons.length, reasons: [...new Set(reasons)], target, activation, abilityScale, sacrificeChance, reserveSacrifice };
}

export function healedHP({ hp, maxHP, amount }) {
  for (const value of [hp, maxHP, amount]) {
    if (!Number.isInteger(value) || value < 0 || value > 0x7fffffff) throw new Error('Ungültiger Heilwert.');
  }
  if (hp < 1 || hp > maxHP) throw new Error('Heilung braucht ein lebendes Ziel.');
  if (hp + amount > 0x7fffffff) throw new Error('Heilwert außerhalb des unterstützten Ganzzahlbereichs.');
  const next = Math.min(maxHP, hp + amount);
  return { hp: next, healed: next - hp };
}

export function healingRoll({ stat, activation, randomUint32, targets = 1, abilityScale = 1, magnification = 0 }) {
  if (!Number.isInteger(targets) || targets < 1 || targets > ORIGINAL_ACTIVE_SIZE) throw new Error('Ungültige Heilzielzahl.');
  const dice = originalDice({ stat, randomness: .2, activation, randomUint32, targets: 1, abilityScale }).roll;
  const roll = Math.trunc(adjustByModifier(dice, magnification));
  // The component divides AFTER its integer dice roll, unlike damage's dice scaling.
  if (targets === 1) return roll;
  return Math.floor(Math.fround(Math.fround(roll) / Math.fround(Math.fround(targets) * Math.fround(.7))) + .5);
}

export function healWithSkill(source, plan, { level, hp, maxHP, randomUint32, targets = 1, magnification = 0, magic }) {
  if (!plan.supported || plan.effect !== 'heal') throw new Error('Nicht unterstützte Heilfähigkeit.');
  const amount = healingRoll({ stat: magic ?? unbuffedStats(source, level).Magic, activation: plan.activation, randomUint32, targets, abilityScale: plan.abilityScale, magnification });
  return { ...healedHP({ hp, maxHP, amount }), amount };
}
