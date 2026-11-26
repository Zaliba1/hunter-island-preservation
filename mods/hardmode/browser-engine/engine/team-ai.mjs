import { teamHit, teamStats } from './team-battle.mjs';
import { combatSkillOptions } from './combat-skills.mjs';
import { healWithSkill } from './healing-skills.mjs';
import { livingOnField } from './reserves.mjs';
import { addQueueTime } from './timing.mjs';
import { poisonPressure, statusProbability, teamActionCost } from './status-skills.mjs';
import { matchingEnemyTargets, randomTeamTarget, redirectionPool, redirectedTargetChances, redirectRollCount } from './team-targets.mjs';

const FALLBACK = { name: 'Prüfangriff', effect: 'damage', target: 'enemy', kind: 'Attack', activation: 200 };
const ROLL_RANGE = 2 ** 32;
const MEDIAN = ROLL_RANGE / 2;
export const SAMPLE_ROLLS = Object.freeze(Array.from({ length: 9 }, (_, i) => Math.floor(ROLL_RANGE * (i + .5) / 9)));
const damages = plan => plan.effect === 'damage' || Boolean(plan.stun && plan.kind);

function targetGroups(plan, actor, allies, opponents, monsters) {
  switch (plan.target) {
    case 'enemy': return opponents.map(target => [target.id]);
    case 'self': return [[actor.id]];
    case 'friend': return allies.map(target => [target.id]);
    case 'friends': return allies.length ? [allies.map(target => target.id)] : [];
    case 'team': return [[actor.id, ...allies.map(target => target.id)]];
    case 'enemies': {
      if (plan.targetCount === 'matching') {
        const primaries = new Map();
        for (const target of opponents) {
          const element = monsters.get(target.monsterId).element;
          if (!primaries.has(element)) primaries.set(element, target.id);
        }
        return [...primaries.values()].map(id => matchingEnemyTargets(opponents, id, monsters));
      }
      if (opponents.length <= 2 || plan.targetCount === 'all') return [opponents.map(target => target.id)];
      return opponents.flatMap((first, i) => opponents.slice(i + 1).map(second => [first.id, second.id]));
    }
    default: return [];
  }
}

export function teamAiActions(field, actor, monsters, catalog, { statuses = false, allSupported = false, reserveAvailable = true, skillPlans } = {}) {
  const allies = field.filter(member => member.side === actor.side && member.id !== actor.id);
  const opponents = field.filter(member => member.side !== actor.side);
  const plans = (skillPlans ?? combatSkillOptions(monsters.get(actor.monsterId), catalog, { statuses })).filter(plan => plan.supported &&
    (reserveAvailable || !plan.reserveSacrifice) &&
    (allSupported || plan.effect === 'damage' || (plan.effect === 'heal' && !plan.sacrificeChance && !plan.reserveSacrifice) ||
      (statuses && (plan.stun || plan.haste || plan.stunGift || plan.guardian || plan.sleep || plan.poison || plan.cleanse ||
        plan.rotation || plan.invert || plan.sentence || plan.venomGift || plan.shift || plan.modifiers ||
        (plan.barrier && plan.target === 'team')))));
  if (!plans.some(plan => damages(plan) && !plan.randomTarget)) plans.push(FALLBACK);
  return plans.flatMap(plan => targetGroups(plan, actor, allies, opponents, monsters).map(targetIds => ({ plan, targetIds })));
}

export function chooseStandardTeamAction(state, monsters, catalog) {
  const field = livingOnField(state);
  const actor = field.find(member => member.id === state.actorId);
  return teamAiActions(field, actor, monsters, catalog).find(action => action.plan.effect === 'damage');
}

// Both the benchmark and UI execute choices through this command builder.
// Each roll is drawn only when committing the decision, as with a manual command.
export function teamActionCommand(state, { plan, targetIds }, nextUint32) {
  if (plan.effect === 'delay') return { turn: state.turn, actorId: state.actorId, kind: 'delay' };
  const base = { turn: state.turn, actorId: state.actorId, targetId: targetIds[0] };
  const actor = state.combatants.find(member => member.id === state.actorId);
  const field = livingOnField(state);
  let selectedTargets = targetIds;
  if (plan.randomTarget) {
    base.targetRolls = [nextUint32('target', 0), nextUint32('target', 1)];
    selectedTargets = [randomTeamTarget(field, actor, base.targetRolls).id];
  }
  if (plan.scramble) base.scrambleRolls = (state.reserveIds?.[actor.side === 'player' ? 'enemy' : 'player'] ?? [])
    .map((_, index) => nextUint32('scramble', index));
  if (plan.summon && !plan.summon.copy && !plan.summon.monsterId) base.summonRoll = nextUint32('summon', 0);
  if (plan.friendlyDamage) base.friendlyDamageRolls = field.filter(member => member.side === actor.side)
    .map((_, index) => nextUint32('friendlyHit', index));
  if (damages(plan) || plan.stun || plan.sleep || plan.poison || plan.sentence ||
      ((plan.rotation || plan.modifiers) && ['enemy', 'enemies'].includes(plan.target))) {
    const pool = field.find(member => member.id === selectedTargets[0])?.side !== actor.side ?
      redirectionPool(field, actor.side, selectedTargets, state.queue.clock) : [];
    const count = redirectRollCount(pool, selectedTargets.length);
    if (count) base.redirectRolls = Array.from({ length: count }, (_, index) => nextUint32('redirect', index));
  }
  if (damages(plan) && actor.venomGifts?.length) base.venomRolls = Array.from({ length: targetIds.length * actor.venomGifts.length },
    (_, index) => nextUint32('venomGift', index));
  if (plan.slot === undefined) return { ...base, kind: 'testAttack', stat: plan.kind,
    activation: plan.activation, randomUint32: nextUint32('hit', 0) };
  const multiple = plan.target === 'enemies' || plan.target === 'friends';
  const damagingOrHealing = damages(plan) || plan.effect === 'heal';
  const allyCount = plan.sleep?.friendly ? livingOnField(state).filter(member =>
    member.side === actor.side).length : 0;
  const statusRolls = plan.stun || plan.sleep ? Array.from({ length: targetIds.length + allyCount }, (_, index) => nextUint32('status', index)) : null;
  const poisonRolls = plan.poison ? targetIds.map((_, index) => nextUint32('poison', index)) : null;
  const rolls = multiple && damagingOrHealing ? targetIds.map((_, index) => nextUint32('hit', index)) : null;
  const randomUint32 = rolls?.[0] ?? (damagingOrHealing ? nextUint32('hit', 0) : 0);
  return { ...base, kind: 'useSkill', slot: plan.slot,
    targetId: ['friends', 'team'].includes(plan.target) ? state.actorId : base.targetId,
    randomUint32, ...(statusRolls ? { statusRolls } : {}),
    ...(poisonRolls ? { poisonRolls } : {}),
    ...(multiple ? { randomUint32s: rolls } : {}),
    ...(plan.target === 'enemies' ? { targetIds } : {}),
    sacrificeRandomUint32: nextUint32('sacrifice', 0) };
}

export function koProbability(damageAt, hp) {
  if (damageAt(0) >= hp) return 1;
  if (damageAt(ROLL_RANGE - 1) < hp) return 0;
  // Damage in the supported direct-hit path is monotone in the unsigned roll.
  // Find the first lethal roll, including the original float32 rounding boundaries.
  let low = 0, high = ROLL_RANGE - 1;
  while (high - low > 1) {
    const middle = Math.floor((low + high) / 2);
    if (damageAt(middle) >= hp) high = middle;
    else low = middle;
  }
  return (ROLL_RANGE - high) / ROLL_RANGE;
}

const mean = values => values.reduce((sum, value) => sum + value, 0) / values.length;
const detail = value => Math.round(value * 10) / 10;

export const HARDMODE_WEIGHTS = Object.freeze({
  knockout: 1, prevention: 1, followUp: 1, haste: 1, guardian: 1, control: 1,
  support: 1, tempo: 1, exposure: 1, sacrifice: 1, reserve: 1, sleepTactics: 0, retaliation: 0,
});

export function rankHardmodeTeamActions(state, monsters, { catalog, legacy = false, statuses = false,
  weights = HARDMODE_WEIGHTS, explain = true, skillPlans } = {}) {
  const field = livingOnField(state);
  const members = new Map(field.map(member => [member.id, member]));
  const actor = members.get(state.actorId);
  const opponents = field.filter(member => member.side !== actor.side);
  const queue = state.queue.entries;
  const times = new Map(queue.map(({ id, time }) => [id, time]));
  const clock = state.queue.clock;
  const options = new Map(field.map(member => [member.id, teamAiActions(field, member, monsters, catalog, {
    statuses, reserveAvailable: Boolean(state.reserveIds?.[member.side]?.length), skillPlans: skillPlans?.get(member.monsterId),
  })]));

  const statCache = new Map();
  function statsFor(battle, member) {
    if (member !== members.get(member.id)) return teamStats(battle, member, monsters);
    if (!statCache.has(member.id)) statCache.set(member.id, teamStats(battle, member, monsters));
    return statCache.get(member.id);
  }

  function damage(source, target, plan, randomUint32, targetIds) {
    return teamHit(state, source, target, monsters, { plan, randomUint32, legacy, targetIds, statsFor }).damage;
  }

  function heal(source, target, plan, count, randomUint32) {
    return healWithSkill(monsters.get(source.monsterId), plan, { level: source.level, hp: target.hp,
      maxHP: target.maxHP, targets: count, randomUint32, magic: statsFor(state, source).Magic }).healed;
  }

  function targetDistribution(member, targetIds, extra = null) {
    return redirectedTargetChances(redirectionPool(field, member.side, targetIds, clock, extra), targetIds);
  }

  // Rate a visible opponent's best immediate attack or heal. Keep the estimate
  // independent of which move the player actually picks on their next turn.
  function medianAttack(member, action, extra = null) {
    let total = 0;
    for (const [id, chance] of targetDistribution(member, action.targetIds, extra)) {
      const target = members.get(id), hp = damage(member, target, action.plan, MEDIAN, action.targetIds);
      total += chance * (100 * hp / target.maxHP + (hp >= target.hp ? 45 : 0));
    }
    return total;
  }
  const threats = new Map(opponents.map(opponent => {
    const highest = Math.max(0, ...options.get(opponent.id).map(action => {
      const { plan, targetIds } = action;
      if (plan.effect === 'heal') return targetIds.reduce((total, id) => {
        const target = members.get(id);
        return total + 80 * heal(opponent, target, plan, targetIds.length, MEDIAN) / target.maxHP;
      }, 0);
      const damageValue = damages(plan) ? medianAttack(opponent, action) : 0;
      const stunValue = plan.stun ? 20 * statusProbability(plan.stun.chance) * Math.min(1, plan.stun.duration / 100) * targetIds.length : 0;
      return (plan.haste ? 14 * targetIds.length : damageValue + stunValue) * (plan.sacrificeChance ? .6 : 1);
    }));
    return [opponent.id, Math.min(100, highest)];
  }));
  const peakThreat = Math.max(1, ...threats.values());
  const sleepingThroughTurn = member => !member.poisonEffects?.length &&
    member.sleepEffects?.some(effect => effect.until > times.get(member.id));
  const incoming = new Map();
  if (weights.retaliation) for (const foe of opponents) {
    let threat = 0;
    for (const option of options.get(foe.id)) {
      if (!damages(option.plan)) continue;
      const chance = targetDistribution(foe, option.targetIds).get(actor.id) ?? 0;
      if (!chance) continue;
      const amount = damage(foe, actor, option.plan, MEDIAN, option.targetIds);
      threat = Math.max(threat, chance * (22 * amount / actor.maxHP + (amount >= actor.hp ? 44 : 0)));
    }
    incoming.set(foe.id, threat);
  }

  const nextAlly = queue.slice(1).find(entry => members.get(entry.id)?.side === actor.side);
  const followUps = new Map();
  for (const target of opponents) {
    if (!nextAlly || nextAlly.time >= times.get(target.id) || target.buffs?.length) continue;
    const helper = members.get(nextAlly.id);
    let bestPlan, bestTargets, bestChance = 0, bestDamage = 0;
    for (const { plan, targetIds } of options.get(helper.id)) {
      if (!damages(plan) || plan.sacrificeChance) continue;
      const chance = targetDistribution(helper, targetIds).get(target.id) ?? 0;
      if (!chance) continue;
      const estimate = damage(helper, target, plan, MEDIAN, targetIds) * chance;
      if (estimate > bestDamage) { bestPlan = plan; bestTargets = targetIds; bestDamage = estimate; bestChance = chance; }
    }
    if (bestPlan) {
      const damageAt = roll => damage(helper, target, bestPlan, roll, bestTargets);
      const ko = bestChance * koProbability(damageAt, target.hp);
      followUps.set(target.id, { ko, chance: bestChance, samples: SAMPLE_ROLLS.map(damageAt) });
    }
  }

  const bestAttacks = new Map();
  function bestAttack(member) {
    if (!bestAttacks.has(member.id)) {
      let best;
      for (const action of options.get(member.id)) {
        if (!damages(action.plan) || action.plan.sacrificeChance) continue;
        const impact = medianAttack(member, action);
        const rate = impact / (action.plan.activation + 50);
        if (!best || rate > best.rate) best = { ...action, impact, rate };
      }
      bestAttacks.set(member.id, best);
    }
    return bestAttacks.get(member.id);
  }

  function delayValue(id, delay, projected, sacrifice) {
    const wait = times.get(id) - clock;
    const targetTime = times.get(id);
    const crossing = projected.entries.filter(entry => entry.id !== (sacrifice ? actor.id : null) &&
      members.get(entry.id)?.side === actor.side && entry.time >= targetTime && entry.time < targetTime + delay).length;
    const advance = Math.min(1.2, .35 * Math.min(1, delay / 100) + .45 * crossing);
    return threats.get(id) * (100 / (100 + Math.max(0, wait))) * advance;
  }

  function hasteValue(member, plan, projected) {
    const attack = bestAttack(member);
    if (!attack) return 0;
    const first = projected.entries.find(entry => entry.id === member.id).time;
    const until = clock + plan.haste.duration;
    const horizon = Math.min(until, clock + 340);
    const buffed = { ...member, hasteEffects: [...(member.hasteEffects ?? []), { ...plan.haste, until }] };
    function opportunities(fighter) {
      let value = 0;
      for (let time = first; time <= horizon; time += teamActionCost(fighter, time, attack.plan.activation)) {
        value += 100 / (100 + time - clock);
      }
      return value;
    }
    return Math.min(75, (opportunities(buffed) - opportunities(member)) * Math.min(120, attack.impact));
  }

  function guardianValue(plan, id) {
    const extra = { id, priority: plan.guardian.priority };
    return opponents.reduce((points, opponent) => {
      const wait = times.get(opponent.id) - clock;
      if (wait >= plan.guardian.duration) return points;
      const attacks = options.get(opponent.id).filter(action => damages(action.plan));
      const before = Math.max(0, ...attacks.map(action => medianAttack(opponent, action)));
      const after = Math.max(0, ...attacks.map(action => medianAttack(opponent, action, extra)));
      return points + Math.max(0, before - after) * 100 / (100 + wait);
    }, 0);
  }

  const forecastCache = new Map();
  const ranked = [];
  for (const action of options.get(actor.id)) {
    const { plan, targetIds } = action;
    if (plan.randomTarget) continue;
    if (plan.effect === 'heal' && targetIds.every(id => members.get(id).hp === members.get(id).maxHP)) continue;
    if (plan.cleanse && !targetIds.some(id => ['sleepEffects', 'poisonEffects'].some(key => members.get(id)[key]?.length))) continue;
    if (plan.rotation && !state.reserveIds?.[members.get(targetIds[0]).side]?.length) continue;
    const cost = teamActionCost(actor, clock, plan.activation, plan.noHaste);
    const projected = addQueueTime(state.queue, actor.id, cost);
    const actorIndex = projected.entries.findIndex(entry => entry.id === actor.id);
    const readyAgain = projected.entries[actorIndex].time - clock;
    const targets = [];
    const sacrifice = statusProbability(plan.sacrificeChance ?? 0);
    const sacrificeCost = sacrifice ? sacrifice * (field.some(member => member.side === actor.side && member.id !== actor.id) || state.reserveIds?.[actor.side]?.length
      ? 26 + 30 * actor.hp / actor.maxHP + .22 * Math.min(120, bestAttack(actor)?.impact ?? 0) : 10000) : 0;
    let impact = 0, knockouts = 0, interruption = 0, teamFollowUp = 0, statusValue = 0;
    const giftCoverage = new Map();
    const hostile = ['enemy', 'enemies'].includes(plan.target);
    const distribution = hostile ? targetDistribution(actor, targetIds) : new Map(targetIds.map(id => [id, 1]));
    for (const [id, inclusion] of distribution) {
      const target = members.get(id), wait = times.get(id) - clock;
      if (plan.shift) {
        const stats = statsFor(state, target);
        const points = Math.min(28, 35 * statsFor(state, actor)[plan.shift.sourceStat] /
          Math.max(1, stats[plan.shift.destinationStat])) - 20 * (1 - target.hp / target.maxHP);
        statusValue += points;
        targets.push({ id, wait, statusValue: detail(points) });
        continue;
      }
      if (plan.modifiers && !damages(plan)) {
        const points = plan.modifiers.reduce((sum, modifier) => target.statEffects?.some(effect => effect.stackName === modifier.stackName)
          ? sum : sum + Math.min(14, 5 * Math.abs(modifier.coefficient)), 0);
        statusValue += inclusion * points;
        targets.push({ id, wait, statusValue: detail(points) });
        continue;
      }
      if (plan.venomGift || (plan.barrier && plan.target === 'team')) {
        const points = plan.venomGift ? target.venomGifts?.length ? 0 : 14
          : 16 * opponents.filter(member => plan.barrier.allowedElements.includes(monsters.get(member.monsterId).element)).length;
        statusValue += target.id === actor.id && sacrifice ? points * (1 - sacrifice) : points;
        targets.push({ id, wait, statusValue: detail(points) });
        continue;
      }
      if (plan.invert || plan.rotation) {
        const points = plan.invert ? 80 * (Math.max(1, target.maxHP - target.hp) - target.hp) / target.maxHP
          : hostile ? inclusion * (threats.get(id) * 100 / (100 + wait) * .45 - 8) : (1 - target.hp / target.maxHP) * 45 - 10;
        statusValue += points;
        targets.push({ id, wait, statusValue: detail(points) });
        continue;
      }
      if (plan.cleanse) {
        const points = (plan.cleanse.includes('Sleep') && target.sleepEffects?.some(effect => effect.until > clock + wait) ? 40 : 0) +
          (plan.cleanse.includes('Poison') ? poisonPressure(target, clock) : 0);
        statusValue += points;
        targets.push({ id, wait, statusValue: detail(points) });
        continue;
      }
      if (plan.effect === 'heal') {
        const amount = mean(SAMPLE_ROLLS.map(roll => heal(actor, target, plan, targetIds.length, roll)));
        impact += 80 * amount / target.maxHP;
        targets.push({ id, wait, hp: detail(amount) });
        continue;
      }
      if (plan.haste || plan.stunGift || plan.guardian) {
        let points = 0;
        if (plan.haste) points = hasteValue(target, plan, projected);
        else if (plan.guardian) points = guardianValue(plan, target.id);
        else if (id !== actor.id) {
          const attack = bestAttack(target);
          for (const [affected, chance] of attack ? targetDistribution(target, attack.targetIds) : []) {
            const foe = members.get(affected);
            const hitAt = roll => damage(target, foe, attack.plan, roll, attack.targetIds);
            const nonlethalHit = koProbability(hitAt, 1) - koProbability(hitAt, foe.hp);
            const existing = target.stunGifts?.reduce((sum, delay) => sum + delay, 0) ?? 0;
            const gain = delayValue(affected, existing + plan.stunGift, projected, true) - delayValue(affected, existing, projected, true);
            const value = chance * nonlethalHit * gain * (times.get(affected) > times.get(id) ? 1 : .24);
            const prior = giftCoverage.get(affected) ?? 0;
            points += Math.max(0, value - prior);
            giftCoverage.set(affected, Math.max(prior, value));
          }
        }
        statusValue += points;
        targets.push({ id, wait, statusValue: detail(points), ...((plan.haste || plan.guardian)
          ? { duration: (plan.haste ?? plan.guardian).duration } : { delay: plan.stunGift }) });
        continue;
      }
      const key = JSON.stringify([plan.slot, id, plan.magnification && plan.targetCount === 2 ? targetIds : null]);
      if (!forecastCache.has(key)) {
        const damageAt = roll => damages(plan) ? damage(actor, target, plan, roll, targetIds) : 0;
        const samples = SAMPLE_ROLLS.map(damageAt);
        forecastCache.set(key, { samples, amount: mean(samples), ko: koProbability(damageAt, target.hp),
          hitChance: actor.stunGifts?.length ? koProbability(damageAt, 1) : 0 });
      }
      const { samples, amount, ko, hitChance } = forecastCache.get(key);
      const threat = threats.get(id);
      // HP and a KO on a low-impact target are worth less than the same progress
      // against an opponent capable of turning the current battle.
      const targetValue = .4 + .6 * threat / peakThreat;
      const urgency = 100 / (100 + Math.max(0, wait));
      const followUp = followUps.get(id);
      // If an ally can already finish the target in time, the current KO is
      // still useful but saves less danger. Credit only newly enabled follow-ups.
      const prevented = .9 * threat * urgency * ko * (1 - (followUp?.ko ?? 0));
      let setup = 0;
      if (followUp && ko < 1 && followUp.ko < 1) {
        const addedKOs = samples.reduce((count, hit) => hit >= target.hp ? count : count +
          followUp.samples.filter(nextHit => nextHit < target.hp && hit + nextHit >= target.hp).length, 0);
        setup = .5 * threat * urgency * followUp.chance * addedKOs / SAMPLE_ROLLS.length ** 2;
      }
      impact += inclusion * 100 * amount / target.maxHP * targetValue;
      if (plan.drain) impact += 70 * inclusion * (plan.drain < 0 ? Math.max(-actor.hp, amount * plan.drain)
        : Math.min(actor.maxHP - actor.hp, amount * plan.drain)) / actor.maxHP;
      knockouts += inclusion * 45 * ko * targetValue;
      interruption += inclusion * prevented;
      teamFollowUp += inclusion * setup;
      const stunChance = plan.stun ? statusProbability(plan.stun.chance) : 0;
      const giftDelay = damages(plan) ? (actor.stunGifts?.reduce((sum, delay) => sum + delay, 0) ?? 0) : 0;
      const giftChance = giftDelay ? hitChance - ko : 0;
      const ownDelay = plan.stun?.duration ?? 0;
      const delayPoints = delay => delay ? delayValue(id, delay, projected, Boolean(sacrifice)) : 0;
      let statusPoints = (1 - ko - giftChance) * stunChance * delayPoints(ownDelay) +
        giftChance * ((1 - stunChance) * delayPoints(giftDelay) + stunChance * delayPoints(giftDelay + ownDelay));
      if (plan.sleep && wait < plan.sleep.duration && !target.sleepEffects?.some(effect => effect.until > clock + wait)) {
        statusPoints += (1 - ko) * statusProbability(plan.sleep.chance) * threat * urgency * .75;
        if (weights.sleepTactics && target.poisonEffects?.length && wait > 0) {
          statusPoints -= weights.sleepTactics * (1 - ko) * statusProbability(plan.sleep.chance) * threat * urgency * .45;
        }
      }
      if (weights.sleepTactics && damages(plan) && amount > 0 && sleepingThroughTurn(target)) {
        statusPoints -= weights.sleepTactics * (1 - ko) * threat * urgency * .55;
      }
      if (plan.poison) statusPoints += (1 - ko) * (target.poisonEffects?.length ? 8 : 20) * targetValue;
      if (plan.slow && wait < plan.slow.duration) statusPoints += (1 - ko) * threat * urgency * .16;
      if (plan.sentence && !target.sentenceEffects?.length) statusPoints += 45 * targetValue * actor.hp / actor.maxHP;
      statusValue += inclusion * statusPoints;
      targets.push({ id, wait, hp: detail(inclusion * amount), ko: inclusion * ko, threat: detail(threat), value: detail(targetValue),
        ...(inclusion < 1 ? { inclusion } : {}), prevented: detail(inclusion * prevented), followUp: detail(inclusion * setup), ...(plan.stun || giftDelay ? {
          stunChance: inclusion * (plan.stun ? stunChance : giftChance), delay: plan.stun?.duration ?? giftDelay, statusValue: detail(inclusion * statusPoints),
        } : {}) });
    }
    if (plan.sleep?.friendly) {
      statusValue -= statusProbability(plan.sleep.chance) * field.filter(member => member.side === actor.side &&
        (member.id === actor.id ? !sacrifice && readyAgain < plan.sleep.duration : times.get(member.id) - clock < plan.sleep.duration)).length * 22;
    }
    if (plan.friendlyDamage) {
      for (const target of field.filter(member => member.side === actor.side)) {
        const amount = damage(actor, target, plan, MEDIAN, targetIds);
        impact -= 100 * amount / target.maxHP + (amount >= target.hp ? 55 : 0);
      }
    }
    // Penalize pressure from survivors still queued before this actor can act again.
    // Another queued ally can respond first, so only count full pressure before any ally.
    let exposure = 0, retaliation = 0;
    for (const target of opponents) {
      const enemyIndex = projected.entries.findIndex(entry => entry.id === target.id);
      if (enemyIndex < actorIndex) {
        const beforeResponse = !projected.entries.slice(0, enemyIndex).some(entry => members.get(entry.id)?.side === actor.side);
        const survival = 1 - (targets.find(row => row.id === target.id)?.ko ?? 0);
        exposure += (beforeResponse ? .14 : .04) * threats.get(target.id) * survival;
        if (weights.retaliation) {
          const wakesTarget = damages(plan) && targets.some(row => row.id === target.id && row.hp > 0);
          if (!sleepingThroughTurn(target) || wakesTarget) {
            retaliation += (beforeResponse ? 1 : .25) * (incoming.get(target.id) ?? 0) * survival;
          }
        }
      }
    }
    const tempoCost = weights.tempo * .15 * cost * (1 - sacrifice);
    exposure = (weights.exposure * exposure + weights.retaliation * retaliation) * (1 - sacrifice);
    const statusWeight = plan.haste ? weights.haste : plan.guardian ? weights.guardian
      : plan.stun || plan.sleep || plan.stunGift || plan.slow ? weights.control : weights.support;
    statusValue *= statusWeight;
    const score = impact + weights.knockout * knockouts + weights.prevention * interruption + weights.followUp * teamFollowUp +
      statusValue - tempoCost - exposure - weights.sacrifice * sacrificeCost - (plan.reserveSacrifice ? 25 * weights.reserve : 0);
    if (!explain) {
      ranked.push({ action, score });
      continue;
    }
    ranked.push({ action, score, summary: { ability: plan.name, effect: plan.effect, targets, cost, readyAgain,
      ...(targets.some(row => row.inclusion !== undefined) ? { requestedIds: targetIds } : {}),
      score: detail(score), impact: detail(impact), knockouts: detail(weights.knockout * knockouts), interruption: detail(weights.prevention * interruption),
      teamFollowUp: detail(weights.followUp * teamFollowUp), tempoCost: detail(tempoCost), exposure: detail(exposure),
      ...(statusValue || sacrifice ? { statusValue: detail(statusValue), sacrifice, sacrificeCost: detail(weights.sacrifice * sacrificeCost) } : {}) } });
  }

  ranked.sort((a, b) => b.score - a.score);
  return ranked;
}

export function chooseHardmodeTeamAction(state, monsters, options = {}) {
  const ranked = rankHardmodeTeamActions(state, monsters, options);
  const best = ranked[0];
  if (options.explain === false) return best.action;
  return { ...best.action, explanation: { ...best.summary, alternatives: ranked.slice(1, 3).map(row => row.summary) } };
}
