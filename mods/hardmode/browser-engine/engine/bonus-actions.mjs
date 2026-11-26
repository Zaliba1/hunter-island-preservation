// Living.rollDoubleAction (Hunter Island 2.02 ARMv7, 0x000b89f0).
const probabilities = Object.freeze({ E: .01, D: .02, C: .04, B: .07, A: .11, S: .20 });
const f = Math.fround;

export function bonusActionHits(rank, randomUint32, consecutive = 0, tierBonus = 0) {
  if (!Object.hasOwn(probabilities, rank) || !Number.isInteger(randomUint32) || randomUint32 < 0 || randomUint32 > 0xffffffff) {
    throw new Error('Ungültiger Bonusaktionswurf.');
  }
  return consecutive < 2 && f(f(randomUint32) * 2 ** -32) < f(f(probabilities[rank]) + f(f(tierBonus) * f(.01)));
}
