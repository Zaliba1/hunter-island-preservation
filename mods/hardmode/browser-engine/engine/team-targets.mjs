const ROLL_RANGE = 2 ** 32;

export function randomTeamTarget(field, source, [teamRoll, targetRoll]) {
  const targets = field.filter(member => (member.side === source.side) === ((teamRoll & 1) === 0));
  return targets[targetRoll % targets.length];
}

export function matchingEnemyTargets(enemies, primary, monsters) {
  const target = enemies.find(member => member.id === primary);
  if (!target) return [];
  const element = monsters.get(target.monsterId).element;
  return [primary, ...enemies.filter(member => member.id !== primary &&
    monsters.get(member.monsterId).element === element).map(member => member.id)];
}

function priorityAt(member, clock) {
  return (member.guardEffects ?? []).reduce((priority, effect) => priority + (effect.until > clock ? effect.priority : 0),
    member.targetPriority ?? 0);
}

export function redirectionPool(field, side, intended, clock, extra = null) {
  const enemies = field.filter(member => member.side !== side).map(member => ({ id: member.id,
    priority: priorityAt(member, clock) + (member.id === extra?.id ? extra.priority : 0) }));
  // Preserve the existing fixed order in ordinary fights with no active priority.
  if (!enemies.some(enemy => enemy.priority !== 0)) return [];
  return enemies.map(({ id, priority }) => ({ id, weight: intended.includes(id) ? Math.max(1, 100 + priority) : priority }))
    .filter(entry => entry.weight > 0);
}

export function redirectRollCount(pool, count) {
  return Math.max(0, Math.min(count, pool.length - 1));
}

export function redirectTargets(pool, intended, rolls) {
  if (!pool.length) return intended;
  const remaining = [...pool], chosen = [];
  for (let i = 0; i < intended.length; i++) {
    let index = 0;
    if (remaining.length > 1) {
      let draw = rolls[i] % remaining.reduce((sum, entry) => sum + entry.weight, 0);
      while (draw > remaining[index].weight) draw -= remaining[index++].weight;
    }
    chosen.push(remaining.splice(index, 1)[0].id);
  }
  return chosen;
}

export function redirectedTargetChances(pool, intended) {
  if (!pool.length) return new Map(intended.map(id => [id, 1]));
  if (pool.length === intended.length) return new Map(pool.map(({ id }) => [id, 1]));
  const chances = new Map();
  function visit(remaining, count, probability) {
    const total = remaining.reduce((sum, entry) => sum + entry.weight, 0);
    const full = Math.floor(ROLL_RANGE / total), remainder = ROLL_RANGE % total;
    let cumulative = 0, start = 0;
    for (const [index, { id, weight }] of remaining.entries()) {
      cumulative += weight;
      const end = Math.min(cumulative + 1, total);
      const outcomes = (end - start) * full + Math.max(0, Math.min(remainder, end) - start);
      const chance = probability * outcomes / ROLL_RANGE;
      if (chance > 0) {
        chances.set(id, (chances.get(id) ?? 0) + chance);
        if (count > 1) visit(remaining.filter((_, i) => i !== index), count - 1, chance);
      }
      start = end;
    }
  }
  visit(pool, intended.length, 1);
  return chances;
}
