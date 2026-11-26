import { unbuffedStats, originalDice, originalHit } from './original-rules.mjs';

// One explicit unbuffed test attack. This is a composition of verified original
// arithmetic, not a complete original ability with buffs, animation or turn scheduling.
export function referenceAttack(attacker, defender, {
  attackerLevel, defenderLevel, hp, kind = 'Attack', activation = 200,
  randomUint32, legacy = false, modifyDamage = null, modifyStrength = null, targets = 1, abilityScale = 1,
  attackerStats, defenderStats,
}) {
  if (!['Attack', 'Magic'].includes(kind)) throw new Error('Unbekannte Angriffsart.');
  const source = attackerStats ?? unbuffedStats(attacker, attackerLevel), target = defenderStats ?? unbuffedStats(defender, defenderLevel);
  const dice = originalDice({ stat: source[kind], randomness: .2, activation,
    abilityScale, targets, randomUint32 });
  const amount = modifyStrength ? modifyStrength(dice.roll) : dice.roll;
  const hit = originalHit({ amount, damageElement: attacker.element, defenderElement: defender.element,
    sourceStat: kind, defense: target.Defense, resist: target.Resist, hp,
    defenderLevel, attackerLevel, legacy, modifyDamage });
  return { ...hit, roll: dice.roll, minRoll: dice.min, maxRoll: dice.max, kind, activation };
}
