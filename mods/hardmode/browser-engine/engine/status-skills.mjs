import { abilityActivation } from './abilities.mjs';
import { actionTimeCost } from './timing.mjs';
import { divideForTargets, ELEMENT_ORDER, roundAway } from './original-rules.mjs';
import { compileBarrierSkill } from './barrier-skills.mjs';
import { compileReserveSacrifice } from './reserves.mjs';

const STUN_TYPES = new Set(['Offensive.Stun.Stun', 'Offensive.Stun.StunAll',
  'Offensive.Stun.StunMagicAttack', 'Offensive.Stun.StunMagicAttackAll',
  'Offensive.Stun.StunAttack', 'Offensive.Stun.StunAttackAll', 'Offensive.Stun.DoubleStunAttack', 'Offensive.Stun.StunAttackThrow']);
const HASTE_TYPES = new Set(['Defensive.HasteUp', 'Defensive.HasteUpAll']);
const RANGE = 2 ** 32;

export function statusHits(roll, chance) {
  return Math.fround(Math.fround(roll) / RANGE) < Math.fround(chance);
}

const chances = new Map([[0, 0]]);
export function statusProbability(chance) {
  if (!chances.has(chance)) {
    let start = 0, end = RANGE;
    while (start < end) {
      const middle = Math.floor((start + end) / 2);
      if (statusHits(middle, chance)) start = middle + 1;
      else end = middle;
    }
    chances.set(chance, start / RANGE);
  }
  return chances.get(chance);
}

export function hasteAt(member, clock) {
  return (member.hasteEffects ?? []).reduce((amount, effect) =>
    effect.until > clock ? Math.fround(amount + effect.modifier) : amount, 0);
}

export function teamActionCost(member, clock, activation, noHaste = false) {
  return Math.max(1, actionTimeCost({ activation, haste: hasteAt(member, clock), noHaste }));
}

export function sleepLength(activation, abilityScale, count) {
  const f = Math.fround;
  const base = Math.trunc(f(f(activation) * f(1.5)));
  return roundAway(divideForTargets(Math.trunc(f(f(base) * f(abilityScale))), count));
}

export function poisonTick(effect, elapsed) {
  const f = Math.fround;
  const total = f(effect.carry + f(f(elapsed) * effect.rate));
  return total < 1 ? { amount: 0, carry: total } : { amount: Math.trunc(total), carry: effect.carry };
}

export function poisonPressure(member, clock) {
  return Math.min(70, (member.poisonEffects ?? []).reduce((sum, effect) =>
    sum + 100 * Math.max(0, effect.until - clock) * effect.rate / member.maxHP, 0));
}

export function statModifierAmount({ stat, coefficient }, level) {
  const f = Math.fround;
  return f(f(f(coefficient) * f(level)) * f(['Defense', 'Resist'].includes(stat) ? 1.5 : 1));
}

export function compileStatusSkill(skill, catalog) {
  const { slot, type, name } = skill;
  const def = catalog?.abilities?.[type], args = skill.arguments ?? {};
  const common = args.commonComponentArguments ?? {};
  if (!def || def.class !== 'CombatAbility') return null;
  const reserveSacrifice = ['Offensive.Stun.StunAttackThrow', 'Utility.CallMonster'].includes(type) && args.components?.length === 1
    ? compileReserveSacrifice(args.components[0], catalog) : null;
  if (args.components?.length && !reserveSacrifice) return null;
  if (Object.keys(args).some(key => !['instanceName', 'activationLength', 'commonComponentArguments', 'soundTable', 'sacrificeChance', 'abilityScalor', 'aiChance',
    'hideBlood', ...(reserveSacrifice ? ['components'] : [])].includes(key))) return null;
  let activation;
  try { activation = abilityActivation(args.activationLength, def.activationLength).length; }
  catch { return null; }
  const sacrificeChance = args.sacrificeChance ?? 0;
  const abilityScale = args.abilityScalor ?? def.abilityScalor ?? 1;
  if (!Number.isInteger(activation) || activation < 1 || activation > 100000 || !Number.isFinite(sacrificeChance) ||
      sacrificeChance < 0 || sacrificeChance > 1 || !Number.isFinite(abilityScale) || abilityScale <= 0) return null;
  const base = { slot, type, name, supported: true, reasons: [], activation, sacrificeChance };
  const parts = def.components ?? [];
  const picker = def.targetPicker;
  const count = picker?.allEnemies === true ? 'all' : picker?.enemyTargets;
  const enemy = [1, 2, 'all'].includes(count) ? { target: count === 1 ? 'enemy' : 'enemies', targetCount: count,
    diceTargets: count === 'all' ? 3 : count } : null;
  const componentClass = part => catalog.components[part?.type]?.class;
  if (type === 'Utility.Escape' && picker?.selfOnly && parts.length === 1 &&
      componentClass(parts[0]) === 'EscapeAbilityComponent' && !Object.keys(common).length) {
    return { ...base, effect: 'escape', target: 'self', escape: true };
  }
  if (type === 'Utility.CallMonster' && picker?.selfOnly && parts.length === 1 && componentClass(parts[0]) === 'CallMonsterAbilityComponent' &&
      Object.keys(common).every(key => ['minStars', 'maxStars', 'monsterType'].includes(key))) {
    if (typeof common.monsterType === 'string') return { ...base, effect: 'summon', target: 'self',
      summon: { monsterId: common.monsterType }, ...(reserveSacrifice ? { reserveSacrifice } : {}) };
    const min = common.minStars ?? 0, max = common.maxStars ?? 9999;
    if (Number.isFinite(min) && Number.isFinite(max) && max >= min) return { ...base, effect: 'summon', target: 'self',
      summon: { minStars: Math.trunc(min), maxStars: Math.trunc(max) }, ...(reserveSacrifice ? { reserveSacrifice } : {}) };
  }
  if (type === 'Utility.AddMonster' && picker?.selfOnly && parts.length === 1 && componentClass(parts[0]) === 'AddMonsterAbilityComponent' &&
      Object.keys(common).length === 1 && ['one', 'split'].includes(common.hpStyle)) {
    return { ...base, effect: 'summon', target: 'self', summon: { copy: common.hpStyle } };
  }
  if (enemy && parts.length === 1 && componentClass(parts[0]) === 'DrainLifeAbilityComponent' &&
      Object.keys(common).every(key => key === 'healPercent') && Number.isFinite(common.healPercent)) {
    const kind = catalog.components[parts[0].type].roll?.relatedStat;
    if (['Attack', 'Magic'].includes(kind)) return { ...base, ...enemy, kind, effect: 'damage', abilityScale, drain: common.healPercent };
  }
  const guard = def.sourceBuffs?.[0];
  const friendGuard = type === 'Defensive.ScapegoatFriend' ? def.friendlyBuffs?.[0] : guard;
  if (['Defensive.Scapegoat', 'Defensive.ScapegoatFriend'].includes(type) && !sacrificeChance && !Object.keys(common).length &&
      parts.length === 0 && friendGuard?.type === 'StatOnly' && catalog.buffs?.StatOnly?.class === 'Buff' &&
      Number.isInteger(friendGuard.arguments?.duration) && Number.isInteger(friendGuard.arguments.targetPriorityAdjustment) &&
      Object.keys(friendGuard.arguments).length === 2) {
    return { ...base, effect: 'guardian', target: type.endsWith('Friend') ? 'friend' : 'self',
      guardian: { priority: friendGuard.arguments.targetPriorityAdjustment, duration: friendGuard.arguments.duration } };
  }
  if (['Utility.Knockback', 'Utility.KnockbackAll', 'Utility.Sendback', 'Utility.RetreatToBack'].includes(type) &&
      parts.length === 1 && !Object.keys(common).length &&
      ['SendMonsterToBackAbilityComponent', 'SendSelfToBackAbilityComponent'].includes(componentClass(parts[0]))) {
    const mode = type === 'Utility.RetreatToBack' ? 'self' : type === 'Utility.Sendback' ? 'friend' : enemy?.target;
    if (mode) return { ...base, ...enemy, effect: 'rotation', target: mode, rotation: true };
  }
  if (type.startsWith('Utility.CallTo') && picker?.selfOnly && parts.length === 1 &&
      componentClass(parts[0]) === 'BringElementToFrontAbilityComponent' &&
      Object.keys(common).every(key => ['element', 'buffType', 'buffArguments'].includes(key)) &&
      ELEMENT_ORDER.includes(common.element)) return { ...base, effect: 'reserveOrder', target: 'self', reserveElement: common.element };
  if (type === 'Utility.Scramble' && picker?.selfOnly && !Object.keys(common).length &&
      parts.length === 1 && componentClass(parts[0]) === 'ScrambleAbilityComponent') {
    return { ...base, effect: 'scramble', target: 'self', scramble: true };
  }
  if (['Defensive.LifeInvert', 'Defensive.LifeInvertSelf'].includes(type) && parts.length === 1 &&
      componentClass(parts[0]) === 'LifeInvertAbilityComponent' && !Object.keys(common).length) {
    return { ...base, effect: 'invert', target: picker.allFriends ? 'team' : 'self', invert: true };
  }
  if (type.startsWith('Defensive.Shift') && !parts.length && !Object.keys(common).length) {
    const side = def.sourceBuffs?.length ? 'sourceBuffs' : 'friendlyBuffs';
    const ref = def[side]?.length === 1 && def[side][0];
    const buff = catalog.buffs?.StatShift?.components?.[0];
    const shift = ref?.arguments?.commonComponentArguments;
    const duration = ref?.arguments?.duration;
    if (ref?.type === 'StatShift' && catalog.buffComponents?.[buff?.type]?.class === 'StatShiftBuffComponent' &&
        ['Defense', 'Resist'].includes(shift?.sourceStat) && ['Attack', 'Magic'].includes(shift.destinationStat) &&
        Number.isInteger(duration) && duration > 0) {
      return { ...base, effect: 'shift', target: side === 'sourceBuffs' ? 'self' : picker.allFriends ? 'team' : 'friend',
        shift: { ...shift, duration } };
    }
  }
  if (parts.some(ref => componentClass(ref) === 'StatModAbilityComponent') &&
      Object.keys(common).every(key => ['providerScalor', 'amount', 'attackType'].includes(key))) {
    const modifiers = [], attacks = [];
    for (const ref of parts) {
      const type = componentClass(ref);
      if (type === 'StatModAbilityComponent') {
        const a = ref.arguments;
        const coefficient = common.providerScalor ?? a?.providerScalor ?? 1;
        const duration = a?.duration ?? 1200;
        if (!a || Object.keys(a).some(k => !['help', 'stat', 'stackName', 'providerScalor', 'duration'].includes(k)) ||
            !['Attack', 'Magic', 'Defense', 'Resist'].includes(a.stat) || typeof a.help !== 'boolean' ||
            !Number.isFinite(coefficient) || !Number.isInteger(duration) || duration < 1) return null;
        modifiers.push({ stat: a.stat, coefficient: a.help ? coefficient : -coefficient, duration, stackName: a.stackName, helps: a.help });
      } else if (['AttackAbilityComponent', 'DamageDealingAbilityComponent'].includes(type) && Object.keys(ref).length === 1) {
        attacks.push(type === 'AttackAbilityComponent' ? 'Attack' : 'Magic');
      } else return null;
    }
    if (attacks.length > 1 || (attacks.length && componentClass(parts.at(-1)) === 'StatModAbilityComponent') ||
        !modifiers.every(mod => mod.helps === modifiers[0].helps)) return null;
    if (modifiers[0].helps && picker?.selfOnly && !attacks.length) return { ...base, effect: 'statMod',
      target: picker.allFriends ? 'team' : 'self', modifiers };
    if (enemy && !modifiers[0].helps) return { ...base, ...enemy, effect: attacks.length ? 'damage' : 'statMod', modifiers,
      kind: attacks[0] ?? null, abilityScale };
  }
  if (['Offensive.DeathSentence', 'Offensive.DeathSentenceAll'].includes(type) && enemy && !parts.length &&
      !Object.keys(common).length && def.enemyBuffs?.length === 1 && def.enemyBuffs[0].type === 'Offensive.DeathSentence') {
    const duration = def.enemyBuffs[0].arguments?.duration;
    if (Number.isInteger(duration) && duration > 0) return { ...base, ...enemy, effect: 'sentence', sentence: { duration } };
  }
  if (STUN_TYPES.has(type) && catalog.components[parts[0]?.type]?.class === 'StunAbilityComponent') {
    if (Object.keys(common).some(key => !['applyChance', 'stunDuration', 'attackType'].includes(key))) return null;
    const hitClass = componentClass(parts[1]);
    const kind = hitClass === 'DamageDealingAbilityComponent' ? 'Magic' : hitClass === 'AttackAbilityComponent' ? 'Attack' : null;
    if (parts.length !== (kind ? 2 : 1)) return null;
    const chance = common.applyChance ?? parts[0].arguments?.applyChance ?? .75;
    const duration = common.stunDuration ?? parts[0].arguments?.stunDuration ?? 50;
    if (!Number.isFinite(chance) || chance < 0 || chance > 1 || !Number.isInteger(duration) || duration <= 0) return null;
    if (!enemy) return null;
    return { ...base, ...enemy, effect: 'stun', kind, abilityScale, stun: { chance, duration },
      ...(reserveSacrifice ? { reserveSacrifice } : {}) };
  }
  if (HASTE_TYPES.has(type) && catalog.components[parts[0]?.type]?.class === 'HasteAbilityComponent' &&
      parts.length === 1 && parts[0].arguments?.help === true && Object.keys(common).every(key => key === 'amount') && !sacrificeChance) {
    return { ...base, effect: 'haste', target: type === 'Defensive.HasteUpAll' ? 'team' : 'self', noHaste: def.noHaste === true,
      haste: { modifier: Math.fround(-.6), duration: parts[0].arguments.duration ?? 500, stackName: parts[0].arguments.stackName } };
  }
  if (type.startsWith('Offensive.Sleep.') && enemy && parts.length === 1 && componentClass(parts[0]) === 'SleepAbilityComponent' &&
      Object.keys(common).every(key => key === 'applyChance')) {
    const chance = common.applyChance ?? parts[0].arguments?.applyChance ?? .75;
    const duration = sleepLength(activation, abilityScale, enemy.diceTargets);
    if (!Number.isFinite(chance) || chance < 0 || chance > 1 || duration < 1) return null;
    return { ...base, ...enemy, effect: 'sleep', sleep: { chance, duration, friendly: picker.allFriends === true } };
  }
  if (type === 'Offensive.Sleep.SleepRandom' && picker.randomAll && parts.length === 1 &&
      componentClass(parts[0]) === 'SleepAbilityComponent' && Object.keys(common).every(key => key === 'applyChance')) {
    const chance = common.applyChance ?? .75;
    if (Number.isFinite(chance) && chance >= 0 && chance <= 1) return { ...base, effect: 'sleep', target: 'self',
      randomTarget: true, sleep: { chance, duration: sleepLength(activation, abilityScale, 1) } };
  }
  if (type === 'Offensive.Poison.RussianRoulette' && picker.randomAll && !parts.length && !Object.keys(common).length) {
    const poison = buff => buff?.length === 1 && buff[0].type === 'Offensive.Poison' && buff[0].arguments?.duration === 1 &&
      buff[0].arguments?.commonComponentArguments?.roll?.relatedStat === 'Attack';
    if (poison(def.enemyBuffs) && poison(def.friendlyBuffs)) return { ...base, effect: 'poison', target: 'self',
      randomTarget: true, abilityScale, poison: { stat: 'Attack', duration: 1 } };
  }
  if (type === 'Defensive.ApplyBuffSleep' && enemy?.target === 'enemy' && picker.allFriends &&
      parts.length === 2 && componentClass(parts[0]) === 'ApplyBuffAbilityComponent' && componentClass(parts[1]) === 'SleepAbilityComponent') {
    const { applyChance: chance = 1, ...buffArguments } = common;
    const shield = compileBarrierSkill({ ...skill, arguments: { ...args, commonComponentArguments: buffArguments } },
      { ...catalog, abilities: { [type]: { ...def, components: [parts[0]], targetPicker: { class: picker.class, allFriends: true } } } }, { teams: true });
    if (shield.supported && chance >= 0 && chance <= 1) return { ...base, ...enemy, effect: 'sleep',
      sleep: { chance, duration: sleepLength(activation, abilityScale, 1), friendly: true }, barrier: shield.barrier };
  }
  if (type === 'Utility.StatusRemove.StatusRemoveAll' && picker?.allFriends && parts.length === 1 &&
      componentClass(parts[0]) === 'StatusRemoveAbilityComponent' && Object.keys(common).length === 1 &&
      Array.isArray(common.statusTypes) && common.statusTypes.length && common.statusTypes.every(status => ['Sleep', 'Poison'].includes(status))) {
    return { ...base, effect: 'cleanse', target: 'team', cleanse: common.statusTypes };
  }
  if (/^Offensive\.(Poison|DarkMist)\./.test(type) && enemy && !picker.allFriends &&
      def.enemyBuffs?.length === 1 && def.enemyBuffs[0].type === 'Offensive.Poison' &&
      Object.keys(common).every(key => key === 'attackType')) {
    const buff = def.enemyBuffs[0].arguments;
    const stat = buff?.commonComponentArguments?.roll?.relatedStat;
    const dot = catalog.buffs?.['Offensive.Poison']?.components?.[0];
    if (!['Attack', 'Magic'].includes(stat) || !Number.isInteger(buff.duration) || buff.duration < 1 ||
        catalog.buffComponents?.[dot?.type]?.class !== 'DamageOverTimeBuffComponent') return null;
    const attacks = parts.filter(ref => ['AttackAbilityComponent', 'DamageDealingAbilityComponent'].includes(componentClass(ref)));
    const slow = parts.find(ref => componentClass(ref) === 'HasteAbilityComponent' && ref.arguments?.help === false);
    if (attacks.length > 1 || attacks.length + Number(Boolean(slow)) !== parts.length) return null;
    const kind = attacks.length ? (componentClass(attacks[0]) === 'AttackAbilityComponent' ? 'Attack' : 'Magic') : null;
    return { ...base, ...enemy, effect: kind ? 'damage' : 'poison', abilityScale, kind, poison: { stat, duration: buff.duration },
      ...(slow ? { slow: { duration: slow.arguments.duration ?? 500, modifier: Math.fround(.3), stackName: slow.arguments.stackName } } : {}) };
  }
  const proc = catalog.buffs?.[common.buffType]?.components?.[0];
  const gift = common.buffArguments;
  if (type === 'Defensive.ApplyBuff' && common.buffType === 'StatOnly' && picker?.allFriends && parts.length === 1 &&
      componentClass(parts[0]) === 'ApplyBuffAbilityComponent' && Object.keys(common).length === 2 &&
      gift?.status === 'VenomGift' && gift.components?.length === 2 &&
      catalog.buffComponents?.[gift.components[0].type]?.class === 'BuffProcBuffComponent' &&
      gift.components[1].type === 'Utility.RemoveOnTurnComplete') {
    const buff = gift.components[0].arguments?.buff;
    const stat = buff?.arguments?.commonComponentArguments?.roll?.relatedStat;
    if (buff?.type === 'Offensive.Poison' && ['Attack', 'Magic'].includes(stat) && Number.isInteger(buff.arguments.duration) &&
        buff.arguments.duration > 0) return { ...base, effect: 'venomGift', target: 'team',
      venomGift: { stat, duration: buff.arguments.duration, activation, abilityScale } };
  }
  if (type === 'Defensive.ApplyBuff' && common.buffType === 'Offensive.StunProc' && def.targetPicker?.allFriends &&
      catalog.components[parts[0]?.type]?.class === 'ApplyBuffAbilityComponent' && parts.length === 1 &&
      catalog.buffComponents?.[proc?.type]?.class === 'StunProcBuffComponent' && proc.arguments?.stunDuration === 150 &&
      Object.keys(common).length === 2 && gift?.status === 'StunGift' && gift.components?.length === 1 &&
      gift.components[0].type === 'Utility.RemoveOnTurnComplete' && !gift.components[0].arguments && [0, 1].includes(sacrificeChance)) {
    return { ...base, effect: 'stunGift', target: 'team', stunGift: 150 };
  }
  return null;
}
