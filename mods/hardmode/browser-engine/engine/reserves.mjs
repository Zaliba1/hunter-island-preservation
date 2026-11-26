import { initialBattleDelay, removeFromQueue, setQueueTime } from './timing.mjs';

// Normal-monster path of Team.replaceMonster/addReplacementMonster.
export function replaceCombatant({ activeIds, reserveIds, deadIds, queue, removedId, speed, randomUint32, extra = 0, returnToReserve = false }) {
  if (!activeIds.includes(removedId)) throw new Error('Das Monster steht nicht im aktiven Team.');
  const active = activeIds.filter(id => id !== removedId), reserve = [...reserveIds], dead = [...deadIds];
  let nextQueue = removeFromQueue(queue, removedId);
  const enteredId = reserve.shift() ?? null;
  if (returnToReserve) reserve.push(removedId);
  else dead.push(removedId);
  let delay = null, requestedTime = null, scheduledTime = null;
  if (enteredId !== null) {
    active.push(enteredId);
    delay = initialBattleDelay({ speed, randomUint32, extra });
    // Battle.notifyTeam:addedMonster: adds as float32 BEFORE truncating.
    requestedTime = Math.trunc(Math.fround(Math.fround(queue.clock) + delay.float));
    nextQueue = setQueueTime(nextQueue, enteredId, requestedTime);
    scheduledTime = nextQueue.entries.find(entry => entry.id === enteredId).time;
  }
  return { activeIds: active, reserveIds: reserve, deadIds: dead, queue: nextQueue,
    enteredId, initialDelay: delay?.float ?? null, requestedTime, scheduledTime };
}

export { replaceCombatant as replaceFallen };

export function onField(state, combatant) {
  return !state.activeIds || state.activeIds[combatant.side].includes(combatant.id);
}
export function livingOnField(state) {
  if (!state.activeIds) return state.combatants.filter(c => c.hp > 0);
  return ['player', 'enemy'].flatMap(side => state.activeIds[side].map(id => state.combatants.find(c => c.id === id))).filter(c => c.hp > 0);
}

export function sacrificeMagnification(level, levelMultiplier = .03, base = 0) {
  if (!Number.isInteger(level) || level < 1 || level > 150 || !Number.isFinite(levelMultiplier) || levelMultiplier < 0 || !Number.isFinite(base)) throw new Error('Ungültige Opferverstärkung.');
  return Math.fround(Math.fround(Math.fround(level) * Math.fround(levelMultiplier)) + Math.fround(base));
}
export function compileReserveSacrifice(ref, catalog) {
  if (ref?.type !== 'Utility.Sacrifice' || catalog.components[ref.type]?.class !== 'SacrificeAbilityComponent' ||
      Object.keys(ref).some(key => key !== 'type' && key !== 'arguments') ||
      Object.keys(ref.arguments ?? {}).some(key => key !== 'levelMultiplier')) return null;
  const levelMultiplier = ref.arguments?.levelMultiplier ?? 0;
  return Number.isFinite(levelMultiplier) && levelMultiplier >= 0 ? { levelMultiplier } : null;
}
export function sacrificeLastReserve({ reserveIds, deadIds, level, levelMultiplier = .03 }) {
  if (!reserveIds.length) throw new Error('Fressen benötigt ein Reservemonster.');
  const reserve = [...reserveIds], sacrificedId = reserve.pop();
  return { reserveIds: reserve, deadIds: [...deadIds, sacrificedId], sacrificedId,
    magnification: sacrificeMagnification(level, levelMultiplier) };
}

// The UI can retry the same pure command with one freshly supplied entry roll.
// A missing roll never commits any HP, queue, history or reserve changes.
export class EntryRandomRequired extends Error {
  constructor(combatantId) { super('Eintrittszufall fehlt für ' + combatantId); this.name = 'EntryRandomRequired'; this.combatantId = combatantId; }
}
