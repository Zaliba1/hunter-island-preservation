// Hunter 2.02: Living.initialBattleDelay, clearAbilityAndGetTimeUsed, TimedQue.
const f = Math.fround;
function integer(value, name) {
  if (!Number.isInteger(value) || value < -0x7fffffff || value > 0x7fffffff) throw new Error('Ungültige Kampfzeit: ' + name);
  return value;
}
export function adjustByModifier(value, modifier) {
  if (!Number.isFinite(value) || !Number.isFinite(modifier)) throw new Error('Ungültiger Zeitmodifikator.');
  const amount = f(value), mod = f(modifier);
  return mod < 0 ? f(amount / f(f(1) - mod)) : f(f(mod + f(1)) * amount);
}
export function initialBattleDelay({ speed, randomUint32, extra = 0 }) {
  integer(speed, 'Tempo'); integer(extra, 'Zusatz');
  if (speed < 0 || !Number.isInteger(randomUint32) || randomUint32 < 0 || randomUint32 > 0xffffffff) throw new Error('Ungültige Startverzögerung.');
  const base = adjustByModifier(150, f(f(speed) / f(-15)));
  const random = f(f(f(randomUint32) * f(2 ** -32)) * f(10));
  const value = f(f(extra) + f(base + random));
  return { float: value, queueTime: Math.trunc(value) };
}
export function actionTimeCost({ activation, haste = 0, noHaste = false }) {
  integer(activation, 'Aktivierung');
  if (activation < 0) throw new Error('Negative Aktivierungszeit.');
  return noHaste ? activation : Math.trunc(adjustByModifier(activation, haste));
}

// TimedQue does not merely sort ties: it advances the newly inserted item by one
// tick and, on a second collision, pushes consecutive occupied ticks to the right.
export function setQueueTime(queue, id, time) {
  integer(time, 'Termin');
  const entries = queue.entries.filter(entry => entry.id !== id).map(entry => ({ ...entry }));
  let candidate = time, collided = false, index = 0;
  for (; index < entries.length; index++) {
    const current = entries[index].time;
    if (current === candidate) {
      if (!collided) { candidate = integer(candidate + 1, 'Kollision'); collided = true; }
      else {
        let next = candidate + 1;
        for (let j = index; j < entries.length && (entries[j].time >>> 0) <= (next >>> 0); j++) {
          entries[j].time = integer(next++, 'Kollision');
        }
      }
    }
    if (current >= candidate) break;
  }
  entries.splice(index, 0, { id, time: candidate });
  return { ...queue, entries };
}
export function addQueueTime(queue, id, time) {
  integer(time, 'Kosten');
  const entry = queue.entries.find(entry => entry.id === id);
  if (!entry) throw new Error('Kampfteilnehmer fehlt in der Zeitplanung.');
  return setQueueTime(queue, id, integer(entry.time + time, 'Folgetermin'));
}
export function removeFromQueue(queue, id) {
  return { ...queue, entries: queue.entries.filter(entry => entry.id !== id).map(entry => ({ ...entry })) };
}
export function advanceQueue(queue) {
  const clock = queue.entries[0]?.time ?? queue.clock;
  return { queue: { ...queue, clock }, elapsed: clock - queue.clock };
}
