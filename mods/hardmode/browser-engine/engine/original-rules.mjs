// Reconstructed from the pinned Hunter Island 2.02 ARMv7 image, not DIB formulas.
// Scope: unbuffed base stats, dice, element/defense arithmetic and capture primitive.
// These functions do not implement complete abilities, buffs, eligibility or a campaign.
const f = Math.fround;
export const STAT_ORDER = Object.freeze(['MaxHP', 'Attack', 'Magic', 'Speed', 'Defense', 'Resist']);
export const RULE_PROVENANCE = Object.freeze({
  armv7: '5b9572690e5dba80c3f6244c2e45d1c50479b46ed91045f6fd036780178e3b08',
  stars: '0x000bcb00', job: '0x000b3620', capture: '0x000bc0b8', dice: '0x0009fd38',
  activation: '0x000d9174', multitarget: '0x000d9ce4',
  hit: '0x000b84e8', weakness: '0x00077790', weaknessFactor: '0x000777d4',
});

export function roundAway(value) { return value < 0 ? -Math.floor(-value + .5) : Math.floor(value + .5); }
function finite(value, name) { if (!Number.isFinite(value)) throw new Error('Ungültiger Regelwert: ' + name); return value; }
function positiveInteger(value, name) {
  if (!Number.isSafeInteger(value) || value < 1 || value > 100000) throw new Error('Ungültiger ganzzahliger Regelwert: ' + name);
  return value;
}

// Monster constructor stores an integer star count and a separate half-star flag.
export function storedStars(stars) {
  finite(stars, 'stars'); if (stars <= 0) throw new Error('Sterne müssen positiv sein.');
  const integer = Math.max(1, roundAway(f(f(stars) + f(-.1))));
  return { integer, half: f(f(stars) - f(integer)) > f(.1) };
}
export function starAdjustedBase(base, stars) {
  finite(base, 'base'); const stored = storedStars(stars);
  const count = f(f(stored.integer) + (stored.half ? f(.5) : 0));
  const multiplier = f(f(f(5) / count) + f(count / f(3)));
  return f(multiplier * f(base));
}
export function levelBonus(base, level, stat, growth = 1) {
  finite(base, 'base'); finite(growth, 'growth'); positiveInteger(level, 'level');
  if (!STAT_ORDER.includes(stat)) throw new Error('Unbekannter Statuswert.');
  return stat === 'Speed' ? 0 : f(f(f(base) * f(growth)) * f(level - 1));
}
export function unbuffedStat(monster, stat, level, adjustments = []) {
  const base = finite(monster.stats[stat], stat);
  const start = stat === 'Speed' ? f(base) : starAdjustedBase(base, monster.stars);
  const bonus = levelBonus(base, level, stat, monster.statGrowth?.[stat] ?? 1);
  return Math.max(0, roundAway(adjustments.reduce((value, amount) => f(value + f(amount)), f(start + bonus))));
}
export function unbuffedStats(monster, level) {
  return Object.fromEntries(STAT_ORDER.map(stat => [stat, unbuffedStat(monster, stat, level)]));
}

export function captureBaseChance({ card, averageLevel, targetLevel, stars, health }) {
  if (![1, 2, 3].includes(card)) throw new Error('Unbekannte Fangkarte.');
  positiveInteger(averageLevel, 'averageLevel'); positiveInteger(targetLevel, 'targetLevel');
  positiveInteger(stars, 'stored target stars'); finite(health, 'health');
  if (health < 0 || health > 1) throw new Error('HP-Anteil muss zwischen 0 und 1 liegen.');
  if (targetLevel === 1) return 1;
  let healthPercent = f(f(health) * f(100));
  if (targetLevel <= 3) healthPercent = f(healthPercent / f(4 - targetLevel));
  const healthTerm = healthPercent > 10 ? f(f(50) / f(healthPercent / f(10))) : f(50);
  const power = f(Math.pow(f(f(stars) * f(.5)), f(1.4)));
  const divisor = Math.max(f(.0001), f(power / [0, 6, 14, 200][card]));
  let percent = f(healthTerm / divisor);
  if (targetLevel > averageLevel) {
    percent = f(Number(percent) - Math.pow(targetLevel - averageLevel, f(1.85)));
  } else {
    const ratio = f(f(averageLevel) / Math.max(f(targetLevel), f(.0001)));
    percent = f(Number(percent) * Math.abs(Math.sqrt(ratio)));
  }
  return Math.max(0, Math.min(1, f(percent / f(100))));
}
export function capturePercent(params) { return roundAway(f(captureBaseChance(params) * f(100))); }

// Floating-point powf is approximated by double pow followed by fround; integer dice
// outputs are compared to the original ARM/powf harness over the committed matrix.
export function activationScale(length) {
  positiveInteger(length, 'activation length');
  return f(Math.pow(f(f(length) * f(.01)), f(.86)));
}
export function divideForTargets(value, targets) {
  finite(value, 'value'); positiveInteger(targets, 'targets');
  return targets < 2 ? f(value) : f(f(value) / f(f(targets) * f(.7)));
}
export function originalDice({ stat, randomness, activation, abilityScale = 1, targets = 1, randomUint32, usable = true }) {
  finite(stat, 'stat'); finite(randomness, 'randomness'); finite(abilityScale, 'abilityScale');
  if (randomness < 0 || !Number.isInteger(randomUint32) || randomUint32 < 0 || randomUint32 > 0xffffffff) throw new Error('Ungültiger Zufallswert.');
  const scaledStat = usable ? f(f(stat) * activationScale(activation)) : f(stat);
  const base = f(roundAway(divideForTargets(f(f(usable ? abilityScale : 1) * scaledStat), targets)));
  const spread = f(base * f(randomness));
  const lower = f(base + f(spread * f(-.5))), upper = f(spread + lower);
  const uniform = f(f(randomUint32) * f(2 ** -32));
  const roll = roundAway(f(lower + f(spread * uniform)));
  return { roll: Math.max(1, roll), min: Math.max(1, Math.trunc(lower)), max: Math.max(1, Math.trunc(upper)) };
}

export const ELEMENT_ORDER = Object.freeze(['Life', 'Death', 'Water', 'Fire', 'Earth', 'Air', 'Arcane']);
const WEAKNESSES = Object.freeze({ Life: null, Death: 'Life', Water: 'Air', Fire: 'Water', Earth: 'Fire', Air: 'Earth', Arcane: 'Arcane' });
export function weaknessOf(element) {
  if (!Object.hasOwn(WEAKNESSES, element)) throw new Error('Unbekanntes Element.');
  return WEAKNESSES[element];
}
export function weaknessFactor(element) {
  weaknessOf(element); return element === 'Death' ? f(3.5) : f(1.5);
}

// Complete arithmetic portion of Living.hitWithDamage: in the absence of modifiers
// from children/buffs. Mutation callbacks and HP/status side effects are not modeled here.
export function originalHit({ amount, damageElement, defenderElement, sourceStat,
  defense, resist, hp, defenderLevel, attackerLevel, legacy = false, modifyDamage = null }) {
  for (const [name, value] of Object.entries({ amount, defense, resist, hp })) {
    if (!Number.isSafeInteger(value) || value < 0 || value > 0x7fffffff) throw new Error('Ungültiger Schadenswert: ' + name);
  }
  positiveInteger(defenderLevel, 'defenderLevel'); positiveInteger(attackerLevel, 'attackerLevel');
  weaknessOf(damageElement); weaknessOf(defenderElement);
  if (![1, 2, 'Attack', 'Magic'].includes(sourceStat)) throw new Error('Nicht unterstützter Schadens-Statuswert.');
  let elementAmount = amount;
  if (weaknessOf(defenderElement) === damageElement) {
    elementAmount = Math.trunc(f(weaknessFactor(defenderElement) * f(amount)));
  } else if (weaknessOf(damageElement) === defenderElement) {
    elementAmount = Math.trunc(f(f(amount) / weaknessFactor(defenderElement)));
  }
  // Original child modifiers run AFTER element interaction and BEFORE mitigation.
  const modifiedAmount = modifyDamage ? modifyDamage(elementAmount, damageElement) : elementAmount;
  if (!Number.isSafeInteger(modifiedAmount) || modifiedAmount < 0 || modifiedAmount > 0x7fffffff) throw new Error('Ungültiger modifizierter Schaden.');
  const defenseStat = sourceStat === 2 || sourceStat === 'Magic' ? resist : defense;
  const level = Math.max(defenderLevel, attackerLevel);
  const threshold = Math.trunc(f(f(f(level * 9 - 9) * f(1)) + f(10)));
  let ratio;
  if (defenseStat < threshold) {
    ratio = f(f(f(f(defenseStat) + f(defenseStat)) - f(threshold)) / f(threshold));
  } else {
    ratio = legacy ? f(Math.trunc(defenseStat / threshold)) : f(f(defenseStat) / f(threshold));
  }
  const mitigation = Math.min(f(.9), f(ratio * f(.35)));
  const damage = Math.max(0, Math.min(hp, Math.trunc(f(f(modifiedAmount) - f(mitigation * f(modifiedAmount))))));
  return { damage, hpChange: -damage || 0, elementAmount };
}
