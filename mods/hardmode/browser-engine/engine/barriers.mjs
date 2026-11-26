import { ELEMENT_ORDER, roundAway } from './original-rules.mjs';
const f = Math.fround;

// Narrow data conversion for authored counters, not a general NSString API.
export function hitCount(value = 1) {
  if (typeof value === 'string' && /^[0-9]{1,10}$/.test(value)) value = Number(value);
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) throw new Error('Ungültiger Trefferzähler.');
  return value;
}
export function afterHurt(remainingHits) {
  const remaining = (hitCount(remainingHits) - 1) >>> 0;
  return { remainingHits: remaining, expired: remaining === 0 };
}
export function preventDamage(amount, element, barrier) {
  if (!Number.isInteger(amount) || amount < 0 || amount > 0x7fffffff || !ELEMENT_ORDER.includes(element)) throw new Error('Ungültiger Schutzschaden.');
  if (!barrier.allowedElements.includes(element)) return amount;
  const { flatReduction, percentReduction } = barrier;
  if (!Number.isInteger(flatReduction) || flatReduction < 0 || !Number.isFinite(percentReduction) || percentReduction < 0 || percentReduction > 1) throw new Error('Ungültiger Schadensschutz.');
  return roundAway(f(f(Math.max(0, amount - flatReduction)) * f(f(1) - f(percentReduction))));
}
export function barrierModifier(barriers = []) {
  return (amount, element) => barriers.reduce((value, barrier) => preventDamage(value, element, barrier), amount);
}
