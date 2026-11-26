import { livingOnField } from './reserves.mjs';

// Hunter 2.02 MagnifyAbilityComponent.applyMagnifications, in addition order.
const counters = {
  timeInCombatMultiplier: ({ state, source }) => state.queue.clock - (source.enteredAt ?? 0),
  friendlyDeadMultiplier: ({ state, source }) => deadCount(state, source.side),
  enemyDeadMultiplier: ({ state, target }) => deadCount(state, target.side),
  teamSizeMultiplier: ({ friends }) => Math.max(0, friends.length - 1),
  aloneMultiplier: ({ friends }) => Number(friends.length === 1),
  myKillsMultiplier: ({ source }) => source.kills ?? 0,
  perTUMultiplier: ({ state, target }) => (state.queue.entries.find(entry => entry.id === target.id)?.time ?? state.queue.clock) - state.queue.clock,
  perMatchingEnemyElements: ({ enemies, matchesElement }) => enemies.filter(matchesElement).length,
  perMatchingFriendElements: ({ friends, matchesElement }) => friends.filter(matchesElement).length - 1,
  perDifferentEnemyElements: ({ enemies, matchesElement }) => enemies.filter(member => !matchesElement(member)).length,
  perDifferentFriendElements: ({ friends, matchesElement }) => friends.filter(member => !matchesElement(member)).length,
  friendlyInBattingOrder: ({ state, source }) => state.reserveIds?.[source.side].length ?? 0,
  enemyInBattingOrder: ({ state, target }) => state.reserveIds?.[target.side].length ?? 0,
};
const statusFields = { Sleep: 'sleepEffects', Poison: 'poisonEffects' };

function deadCount(state, side) {
  return state.deadIds?.[side].length ?? state.combatants.filter(member => member.side === side && member.hp === 0).length;
}

export function compileMagnification(ref, catalog) {
  const args = ref?.arguments;
  if (ref?.type !== 'Utility.Magnify' || catalog.components[ref.type]?.class !== 'MagnifyAbilityComponent' ||
      Object.keys(ref).length !== 2 || !args || !Object.keys(args).length) return null;
  for (const [key, value] of Object.entries(args)) {
    if (key === 'targetMultipliers' || key === 'statusMultipliers') {
      if (!value || typeof value !== 'object' || Array.isArray(value) ||
          !Object.keys(value).length || !Object.values(value).every(Number.isFinite)) return null;
      if (key === 'statusMultipliers' && Object.keys(value).some(status => !Object.hasOwn(statusFields, status))) return null;
    } else if (!Object.hasOwn(counters, key) || !Number.isFinite(value)) return null;
  }
  return args;
}

export function damageMagnification(rules, state, source, target, monsters, before = 0) {
  const sourceElement = monsters.get(source.monsterId).element;
  const matchesElement = member => monsters.get(member.monsterId).element === sourceElement;
  const multipliers = rules.targetMultipliers;
  const segment = multipliers && monsters.get(target.monsterId).path?.split('.').find(part => Object.hasOwn(multipliers, part));
  const field = livingOnField(state);
  const context = { state, source, target, matchesElement,
    friends: field.filter(member => member.side === source.side),
    enemies: field.filter(member => member.side === target.side) };
  const f = Math.fround;
  let bonus = segment ? f(before + f(multipliers[segment])) : before;
  if (rules.statusMultipliers) {
    for (const [status, field] of Object.entries(statusFields)) {
      if (target[field]?.length && Object.hasOwn(rules.statusMultipliers, status)) bonus = f(bonus + f(rules.statusMultipliers[status]));
    }
  }
  for (const [key, count] of Object.entries(counters)) {
    if (Object.hasOwn(rules, key)) bonus = f(bonus + f(f(count(context)) * f(rules[key])));
  }
  return bonus;
}

export function groupMagnification(rules, state, source, targets, monsters, before = 0) {
  return targets.reduce((bonus, target) => damageMagnification(rules, state, source, target, monsters, bonus), before);
}
