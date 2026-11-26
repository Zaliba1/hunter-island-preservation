// Original Battle.advanceQueWithCounter cleanup decision, without callbacks,
// reserves, death reactions or original RNG generation. Inputs are explicit.
export function finishAbility({ hp, chance, randomUint32, timeCost, dead = false, belongsToTeam = true }) {
  if (!Number.isInteger(hp) || hp < 0 || hp > 0x7fffffff || !Number.isInteger(timeCost) || timeCost < 0 || timeCost > 0x7fffffff ||
      typeof chance !== 'number' || !Number.isFinite(chance) || chance < 0 || chance > 1 ||
      typeof dead !== 'boolean' || typeof belongsToTeam !== 'boolean') throw new Error('Ungültiger Aktionsabschluss.');
  if (dead || !belongsToTeam) return { hp, killed: false, randomDraws: 0, queuedCost: 0 };
  if (!Number.isInteger(randomUint32) || randomUint32 < 0 || randomUint32 > 0xffffffff) throw new Error('Opferzufall fehlt.');
  const uniform = Math.fround(Math.fround(randomUint32) * Math.fround(2 ** -32));
  const killed = uniform < Math.fround(chance);
  return { hp: killed ? 0 : hp, killed, randomDraws: 1, queuedCost: killed ? 0 : timeCost };
}
