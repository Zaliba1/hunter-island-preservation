// Deliberately narrow: one damage component, 1-3 enemies, unbuffed source/target.
// A supported entry is not a claim about its actor's passives or full game parity.
import { abilityActivation } from './abilities.mjs';
import { referenceAttack } from './battle.mjs';
import { ORIGINAL_ACTIVE_SIZE } from './team-limits.mjs';
import { compileMagnification } from './damage-magnification.mjs';
import { compileReserveSacrifice } from './reserves.mjs';

const definitionKeys = new Set(['class', 'parent', 'abstract', 'activationLength', 'components',
  'targetPicker', 'eventData', 'soundTable', 'buttonType', 'aiChance', 'enemyBuffs', 'friendlyBuffs', 'selfBuffs', 'passiveBuffs']);
const argumentKeys = new Set(['instanceName', 'activationLength', 'commonComponentArguments', 'components',
  'abilityScalor', 'aiChance', 'hideBlood', 'sacrificeChance']);
export function compileDamageSkill(skill, catalog) {
  const reasons = [], def = catalog?.abilities?.[skill.type], args = skill.arguments || {};
  const result = { slot: skill.slot, type: skill.type, name: skill.name, supported: false, reasons };
  if (!def) { reasons.push('Fähigkeitsdefinition fehlt'); return result; }
  if (def.class !== 'CombatAbility' || def.abstract) reasons.push('keine aktive Kampffähigkeit');
  if (Object.keys(def).some(key => !definitionKeys.has(key))) reasons.push('weitere Fähigkeitsregeln');
  if (Object.keys(args).some(key => !argumentKeys.has(key))) reasons.push('weitere Jobargumente');
  for (const key of ['enemyBuffs', 'friendlyBuffs', 'selfBuffs', 'passiveBuffs']) {
    if (def[key] != null && (!Array.isArray(def[key]) || def[key].length)) reasons.push('Buffwirkung');
  }
  if (args.components != null && (!Array.isArray(args.components) || args.components.length)) reasons.push('zusätzliche Komponenten');
  if (args.abilityScalor != null && args.abilityScalor !== 1) reasons.push('individuelle Skalierung');
  if (args.sacrificeChance != null && args.sacrificeChance !== 0) reasons.push('Opferwirkung');
  const picker = def.targetPicker;
  let targetCount;
  if (!picker || picker.class !== 'TargetPickerGui' || ![1, 2].includes(picker.enemyTargets) ||
      Object.keys(picker).some(key => !['class', 'enemyTargets', 'allEnemies'].includes(key)) ||
      (picker.allEnemies != null && picker.allEnemies !== true)) reasons.push('andere Zielwahl');
  else targetCount = picker.allEnemies ? 'all' : picker.enemyTargets;
  let activation;
  try { activation = abilityActivation(args.activationLength, def.activationLength).length; }
  catch { reasons.push('ungültige Zeitangabe'); }
  if (!Number.isInteger(activation) || activation < 1 || activation > 100000) reasons.push('ungültige Aktivierungszeit');
  const refs = def.components;
  const ref = Array.isArray(refs) && refs.length === 1 ? refs[0] : null;
  const part = ref && catalog.components[ref.type];
  const kind = part?.class === 'AttackAbilityComponent' ? 'Attack' : part?.class === 'DamageDealingAbilityComponent' ? 'Magic' : null;
  if (!kind || !part || Object.keys(part).some(key => !['class', 'parent', 'roll'].includes(key)) ||
      !part.roll || part.roll.relatedStat !== kind || Object.keys(part.roll).some(key => key !== 'relatedStat')) reasons.push('anderer Schadensbaustein');
  if (ref && Object.keys(ref).some(key => key !== 'type')) reasons.push('Komponentenargumente');
  const common = args.commonComponentArguments || {};
  if (Object.keys(common).some(key => key !== 'attackType') ||
      (common.attackType != null && (kind !== 'Attack' || typeof common.attackType !== 'string'))) reasons.push('weitere Komponentenparameter');
  return { ...result, reasons: [...new Set(reasons)], supported: !reasons.length,
    activation, kind, targetCount, diceTargets: targetCount === 'all' ? 3 : targetCount, attackType: common.attackType ?? null };
}

export function damageSkillOptions(monster, catalog) {
  return monster.skills.map(skill => compileDamageSkill(skill, catalog));
}

// Expanded test path. Strip verified differences before validating the underlying
// damage component. Fixed all-enemy groups can share a magnification value.
export function compileRosterDamageSkill(skill, catalog) {
  const def = catalog?.abilities?.[skill.type], args = skill.arguments ?? {};
  if (!def?.targetPicker) return null;
  const { addMatchingElementMonsters: matching, randomAll, allFriends, ...targetPicker } = def.targetPicker;
  const { abilityScalor: abilityScale = 1, components, sacrificeChance = 0, ...baseArgs } = args;
  if ((matching !== undefined && matching !== true) || (randomAll !== undefined && randomAll !== true) ||
      !Number.isFinite(abilityScale) || abilityScale <= 0) return null;
  if (!Number.isFinite(sacrificeChance) || sacrificeChance < 0 || sacrificeChance > 1) return null;
  if (components != null && !Array.isArray(components)) return null;
  const magnification = components?.length === 1 ? compileMagnification(components[0], catalog) : null;
  const reserveSacrifice = components?.length === 1 ? compileReserveSacrifice(components[0], catalog) : null;
  const ref = components?.[0];
  const slowArgs = ref?.arguments;
  const slow = components?.length === 1 && ref.type === 'Utility.HasteMod' && catalog.components[ref.type]?.class === 'HasteAbilityComponent' &&
    slowArgs?.help === false && typeof slowArgs.stackName === 'string' &&
    Object.keys(slowArgs).every(key => ['help', 'stackName', 'roll'].includes(key)) ?
    { modifier: Math.fround(.3), duration: 500, stackName: slowArgs.stackName } : null;
  if (components?.length && !magnification && !reserveSacrifice && !slow) return null;
  const common = baseArgs.commonComponentArguments;
  const part = def.components?.length === 1 && catalog.components[def.components[0].type];
  if (part?.class === 'DamageDealingAbilityComponent' && common) {
    const { attackType, providerScalor, ...parameters } = common;
    if ((attackType != null && typeof attackType !== 'string') || (providerScalor != null && !Number.isFinite(providerScalor))) return null;
    baseArgs.commonComponentArguments = parameters;
  }
  const plan = compileDamageSkill({ ...skill, arguments: baseArgs },
    { ...catalog, abilities: { [skill.type]: { ...def, targetPicker } } });
  if (!plan.supported || (matching && (plan.targetCount !== 1 || magnification || randomAll || allFriends)) ||
      (randomAll && targetPicker.enemyTargets !== 1) ||
      (allFriends !== undefined && (allFriends !== true || plan.targetCount !== 'all'))) return null;
  const targetCount = matching ? 'matching' : plan.targetCount;
  return { ...plan, effect: 'damage', target: randomAll ? 'self' : targetCount === 1 ? 'enemy' : 'enemies',
    targetCount, abilityScale, sacrificeChance, ...(magnification ? { magnification } : {}),
    ...(randomAll ? { randomTarget: true } : {}),
    ...(allFriends ? { friendlyDamage: true } : {}),
    ...(reserveSacrifice ? { reserveSacrifice } : {}), ...(slow ? { slow } : {}) };
}

export function damageSkillAttack(source, target, slot, catalog, params) {
  const skill = source.skills.find(entry => entry.slot === slot);
  if (!skill) throw new Error('Diese Fähigkeit gehört nicht zum Monster.');
  const plan = compileDamageSkill(skill, catalog);
  if (!plan.supported) throw new Error('Fähigkeit noch gesperrt: ' + plan.reasons.join(', '));
  const targets = params.targets ?? 1;
  if (!Number.isInteger(targets) || targets < 1 || targets > ORIGINAL_ACTIVE_SIZE ||
      (plan.targetCount !== 'all' && targets > plan.targetCount)) throw new Error('Ungültige Schadenszielzahl.');
  // Original TargetPickerData.numberOfEnemyTargets is configured capacity:
  // allEnemies => 3, otherwise enemyTargetsNeeded. It is NOT the survivor count.
  const hit = referenceAttack(source, target, { ...params, targets: plan.diceTargets, kind: plan.kind, activation: plan.activation });
  return { ...hit, skillSlot: slot, abilityType: plan.type, abilityName: plan.name, attackType: plan.attackType };
}
