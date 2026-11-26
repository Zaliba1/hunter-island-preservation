export function reflectionPassive(skill, catalog) {
  if (!['Passive.Thorns', 'Passive.PercentThorns'].includes(skill.type)) return null;
  const def = catalog?.abilities?.[skill.type];
  const buff = def?.passiveBuffs?.length === 1 && catalog.buffs?.[def.passiveBuffs[0].type];
  const component = buff?.components?.length === 1 && buff.components[0];
  if (def?.class !== 'Ability' || catalog.buffComponents?.[component?.type]?.class !== 'ReflectiveBuffComponent' ||
      typeof component.arguments?.fixedReflect !== 'boolean') return null;
  return { name: skill.name, fixed: component.arguments.fixedReflect };
}

export function targetPriorityPassive(skill, catalog) {
  const args = skill.arguments;
  return skill.type === 'Passive.Generic' && catalog?.abilities?.[skill.type]?.class === 'Ability' &&
    Number.isInteger(args?.targetPriorityAdjustment) && Object.keys(args).every(key =>
      key === 'instanceName' || key === 'targetPriorityAdjustment') ? args.targetPriorityAdjustment : 0;
}

export function charismaPassive(skill, catalog) {
  const args = skill.arguments;
  const buff = args?.buffArguments?.length === 1 && args.buffArguments[0];
  if (skill.type !== 'Passive.Aura' || catalog?.abilities?.[skill.type]?.class !== 'AuraAbility' ||
      args.help !== true || args.hurt || args.buffTypes?.length !== 1 || args.buffTypes[0] !== 'StatOnly' ||
      !buff?.stats || Object.keys(buff).some(key => !['stats', 'providerScalor'].includes(key))) return null;
  const coefficient = buff.providerScalor ?? 1;
  if (!Number.isFinite(coefficient) || !Object.entries(buff.stats).every(([stat, value]) =>
    ['Attack', 'Magic', 'Defense', 'Resist'].includes(stat) && value === true)) return null;
  return Object.keys(buff.stats).map(stat => ({ stat, coefficient }));
}

function statBuff(ref) {
  const args = ref?.arguments;
  if (ref?.type !== 'StatOnly' || !args?.stats || Object.keys(args).some(key =>
    !['stats', 'providerScalor', 'stackName', 'duration'].includes(key))) return null;
  const coefficient = args.providerScalor ?? 1;
  const stats = Object.entries(args.stats);
  if (!stats.length || !Number.isFinite(coefficient) || !stats.every(([stat, value]) =>
    ['Attack', 'Magic', 'Defense', 'Resist'].includes(stat) && (value === true || Number.isFinite(value)))) return null;
  return { modifiers: stats.map(([stat, value]) => ({ stat, coefficient: Number(value) * coefficient })),
    ...(args.stackName ? { stackName: args.stackName } : {}), ...(args.duration ? { duration: args.duration } : {}) };
}

export function statReflectionPassive(skill, catalog) {
  const def = catalog?.abilities?.[skill.type];
  const ref = def?.passiveBuffs?.length === 1 && def.passiveBuffs[0];
  const buff = catalog?.buffs?.[ref?.type];
  if (def?.class !== 'Ability' || ref?.type !== 'Defensive.BuffReflection' ||
      catalog.buffComponents?.[buff?.components?.[0]?.type]?.class !== 'BuffReflectBuffComponent') return null;
  return statBuff(ref.arguments?.commonComponentArguments?.buffToReflect);
}

export function survivalPassive(skill, catalog) {
  const ref = skill.arguments?.passiveBuffs?.length === 1 && skill.arguments.passiveBuffs[0];
  const component = ref?.arguments?.components?.length === 1 && ref.arguments.components[0];
  const args = component?.arguments;
  if (skill.type !== 'Passive.Generic' || ref?.type !== 'StatOnly' ||
      catalog?.buffComponents?.[component?.type]?.class !== 'DeathTriggerBuffComponent' ||
      !args?.enemies || !args.friends) return null;
  return statBuff(args.buffToApply);
}

export function berserkPassive(skill, catalog) {
  return catalog?.abilities?.[skill.type]?.class === 'BerserkAbility';
}

export function revengePassive(skill, catalog) {
  const args = skill.arguments;
  if (skill.type !== 'Passive.Generic' || catalog?.abilities?.[skill.type]?.class !== 'Ability' ||
      !['allEnemy', 'allFriendly', 'random'].includes(args?.revengeType)) return null;
  const chance = args.revengeChance ?? 1;
  if (!Number.isFinite(chance) || chance < 0 || chance > 1) return null;
  const base = { slot: skill.slot, name: skill.name, chance, mode: args.revengeType };
  if (Number.isInteger(args.revengeStun) && args.revengeStun > 0 && !args.revengeBuffs?.length) {
    return { ...base, stun: args.revengeStun };
  }
  if (args.revengeType === 'allFriendly' && args.revengeBuffs?.length) {
    const statBuffs = args.revengeBuffs.map(statBuff);
    if (statBuffs.every(Boolean)) return { ...base, statBuffs };
  }
  const buff = args.revengeBuffs?.length === 1 && args.revengeBuffs[0];
  const stat = buff?.arguments?.commonComponentArguments?.roll?.relatedStat;
  const duration = buff?.arguments?.duration;
  if (!args.revengeStun && buff?.type === 'Offensive.Poison' && ['Attack', 'Magic'].includes(stat) &&
      Number.isInteger(duration) && duration > 0) return { ...base, poison: { stat, duration } };
  return null;
}

export class PassiveRandomRequired extends Error {
  constructor(key) {
    super('Zufallswurf für passive Fähigkeit fehlt.');
    this.key = key;
  }
}
