// Serializable team simulator with a separately bounded quest setup.
import { unbuffedStat, unbuffedStats, originalDice, originalHit, storedStars, STAT_ORDER } from './original-rules.mjs';
import { bonusActionHits } from './bonus-actions.mjs';
import { startTactic, mayGuard, questDamage, questStrength, advanceTactic } from './quest-tactics.mjs';
import { referenceAttack } from './battle.mjs';
import { compileCombatSkill } from './combat-skills.mjs';
import { healWithSkill } from './healing-skills.mjs';
import { finishAbility } from './finish-ability.mjs';
import { barrierModifier, afterHurt } from './barriers.mjs';
import { replaceCombatant, onField, livingOnField, EntryRandomRequired, sacrificeLastReserve, sacrificeMagnification } from './reserves.mjs';
import { initialBattleDelay, setQueueTime, addQueueTime, removeFromQueue, advanceQueue, adjustByModifier } from './timing.mjs';
import { poisonTick, statModifierAmount, statusHits, teamActionCost } from './status-skills.mjs';
import { matchingEnemyTargets, randomTeamTarget, redirectionPool, redirectRollCount, redirectTargets } from './team-targets.mjs';
import { ORIGINAL_ACTIVE_SIZE, SIMULATOR_MAX_RESERVE } from './team-limits.mjs';
import { damageMagnification, groupMagnification } from './damage-magnification.mjs';
import { berserkPassive, charismaPassive, PassiveRandomRequired, reflectionPassive, revengePassive, statReflectionPassive,
  survivalPassive, targetPriorityPassive } from './passive-skills.mjs';

function randomValue(value) {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) throw new Error('Ungültiger Testzufall.');
}
function definition(monsters, id) {
  const result = monsters.get(id);
  if (!result) throw new Error('Unbekanntes Kampfmonster: ' + id);
  return result;
}

function equippedStat(state, monster, level, stat) {
  const bonus = state.ocarina?.statBonuses[stat];
  return unbuffedStat(monster, stat, level, bonus ? [bonus] : []);
}

function passiveState(monster, catalog) {
  if (!catalog) return {};
  const targetPriority = monster.skills.reduce((sum, skill) => sum + targetPriorityPassive(skill, catalog), 0);
  const revenge = monster.skills.map(skill => revengePassive(skill, catalog)).filter(Boolean);
  const charisma = monster.skills.flatMap(skill => charismaPassive(skill, catalog) ?? []);
  const survival = monster.skills.flatMap(skill => {
    const buff = survivalPassive(skill, catalog);
    return buff ? [{ ...buff, name: skill.name }] : [];
  });
  const berserk = monster.skills.some(skill => berserkPassive(skill, catalog));
  return { ...(targetPriority ? { targetPriority } : {}), ...(revenge.length ? { revenge } : {}),
    ...(charisma.length ? { charisma } : {}), ...(survival.length ? { survival } : {}), ...(berserk ? { berserk } : {}) };
}
export function teamStats(state, member, monsters) {
  const monster = definition(monsters, member.monsterId);
  const stats = unbuffedStats(monster, member.level);
  const adjustments = {};
  for (const [stat, amount] of Object.entries(state.ocarina?.statBonuses ?? {})) adjustments[stat] = [amount];
  for (const ally of state.combatants) {
    if (!ally.charisma || ally.side !== member.side || ally.hp <= 0 || !onField(state, ally)) continue;
    for (const modifier of ally.charisma) (adjustments[modifier.stat] ??= []).push(statModifierAmount(modifier, ally.level));
  }
  for (const effect of member.statEffects ?? []) {
    if (effect.until !== null && effect.until <= state.queue.clock) continue;
    if (effect.modifiers) for (const { stat, amount } of effect.modifiers) (adjustments[stat] ??= []).push(amount);
    else if (effect.stat) (adjustments[effect.stat] ??= []).push(effect.amount);
    else {
      (adjustments[effect.sourceStat] ??= []).push(-effect.amount);
      (adjustments[effect.destinationStat] ??= []).push(effect.amount);
    }
  }
  for (const [key, amounts] of Object.entries(adjustments)) stats[key] = unbuffedStat(monster, key, member.level, amounts);
  for (const [key, amount] of Object.entries(member.nativeStatDelta ?? {})) stats[key] = Math.max(0, stats[key] + amount);
  return stats;
}

export function teamHit(state, source, target, monsters, { plan, stat, activation, randomUint32, legacy, reserveMagnification,
  modifiersApplied, sharedMagnification, targetIds, statsFor = teamStats }) {
  const attacker = definition(monsters, source.monsterId);
  const defender = definition(monsters, target.monsterId);
  let magnification;
  if (plan?.magnification) {
    if (sharedMagnification !== undefined) magnification = sharedMagnification;
    else if (plan.targetCount === 1) magnification = damageMagnification(plan.magnification, state, source, target, monsters);
    else {
      const field = livingOnField(state);
      const allies = plan.friendlyDamage ? field.filter(member => member.side === source.side) : [];
      const enemies = plan.targetCount === 'all' ? field.filter(member => member.side !== source.side) :
        targetIds?.map(id => field.find(member => member.id === id)) ?? [target];
      const selected = target.side === source.side ? allies : [...allies, ...enemies];
      magnification = groupMagnification(plan.magnification, state, source, selected, monsters);
    }
  }
  if (plan?.reserveSacrifice) {
    const reserve = state.reserveIds?.[source.side]?.at(-1);
    magnification = reserveMagnification ?? (reserve ? sacrificeMagnification(state.combatants.find(c => c.id === reserve).level,
      plan.reserveSacrifice.levelMultiplier) : 0);
  }
  let noHit = false;
  let defenderForStats = target;
  if (plan?.modifiers && !modifiersApplied) {
    defenderForStats = { ...target, statEffects: [...(target.statEffects ?? [])] };
    for (const modifier of plan.modifiers) addTimedEffect(defenderForStats, 'statEffects', {
      ...modifier, amount: statModifierAmount(modifier, source.level), until: state.queue.clock + modifier.duration,
    });
  }
  const hit = referenceAttack(attacker, defender, {
    attackerLevel: source.level, defenderLevel: target.level, hp: target.hp,
    kind: plan?.kind ?? stat, activation: plan?.activation ?? activation, targets: plan?.diceTargets ?? 1,
    abilityScale: plan?.abilityScale ?? 1,
    attackerStats: statsFor(state, source, monsters), defenderStats: statsFor(state, defenderForStats, monsters),
    randomUint32, legacy, modifyDamage: barrierModifier(target.buffs),
    modifyStrength: amount => {
      const boost = state.ocarina?.elementBoosts[attacker.element];
      const base = boost ? Math.trunc(Math.fround(amount * Math.fround(1 + boost))) : amount;
      const adjusted = magnification === undefined ? base : Math.trunc(adjustByModifier(base, magnification));
      noHit = adjusted < 1;
      return questStrength(state.objective, source, target, adjusted);
    },
  });
  if (noHit) hit.noHit = true;
  hit.damage = questDamage(state, source, target, Math.min(target.hp, Math.round(hit.damage * (source.force || 1))));
  return hit;
}
function event(state, kind, fields = {}) {
  const sequence = state.eventSequence ?? state.events.length;
  state.events.push({ sequence, clock: state.queue.clock, kind, ...fields });
  if (state.eventLimit !== undefined) {
    state.eventSequence = sequence + 1;
    if (state.events.length > state.eventLimit) state.events.splice(0, state.events.length - state.eventLimit);
  }
}
function removeSleep(state, member, cause) {
  if (!member.sleepEffects?.length) return;
  delete member.sleepEffects;
  event(state, 'statusRemoved', { targetId: member.id, status: 'Sleep', cause });
}

function fixedHit(state, source, target, monsters, { amount, kind, element, legacy }) {
  const stats = teamStats(state, target, monsters);
  return originalHit({ amount, damageElement: element, defenderElement: definition(monsters, target.monsterId).element,
    sourceStat: kind, defense: stats.Defense, resist: stats.Resist, hp: target.hp,
    attackerLevel: source.level, defenderLevel: target.level, legacy, modifyDamage: barrierModifier(target.buffs) }).damage;
}

function afterHit(state, target) {
  removeSleep(state, target, 'hit');
  if (target.buffs?.length) {
    target.buffs = target.buffs.filter(buff => {
      const next = afterHurt(buff.remainingHits); buff.remainingHits = next.remainingHits;
      if (next.expired) event(state, 'buffExpired', { actorId: target.id, abilityName: buff.name });
      return !next.expired;
    });
  }
}

function reflectHit(state, reflector, source, element, hit, command, monsters, catalog, legacy) {
  const attacker = state.combatants.find(member => member.id === source.id);
  for (const skill of definition(monsters, reflector.monsterId).skills) {
    const statEffect = statReflectionPassive(skill, catalog);
    if (statEffect && attacker.hp) applyStatBuff(state, reflector, attacker, statEffect, skill.name);
    const passive = reflectionPassive(skill, catalog);
    if (!passive || !attacker.hp) continue;
    const amount = passive.fixed ? reflector.level : Math.trunc(Math.fround(Math.fround(hit.damage) * .75));
    const damage = fixedHit(state, reflector, attacker, monsters, { amount, kind: hit.kind, element, legacy });
    attacker.hp -= damage;
    event(state, 'reflected', { actorId: reflector.id, targetId: attacker.id, abilityName: passive.name, damage });
    afterHit(state, attacker);
    if (!attacker.hp) leaveField(state, attacker, command, monsters, 'reflection', reflector);
  }
}

function addPoison(state, source, target, effect, rolled, abilityName) {
  const until = state.queue.clock + effect.duration;
  const rate = Math.fround(Math.fround(rolled) / Math.fround(effect.duration));
  (target.poisonEffects ??= []).push({ until, rate, carry: 0, sourceId: source.id, stat: effect.stat });
  event(state, 'poison', { actorId: source.id, targetId: target.id, abilityName, until });
}

function addTimedEffect(member, key, effect) {
  if (effect.stackName && member[key]?.some(active => active.stackName === effect.stackName)) return false;
  (member[key] ??= []).push(effect);
  return true;
}

function applyStatBuff(state, source, target, buff, abilityName) {
  const until = buff.duration ? state.queue.clock + buff.duration : null;
  const modifiers = buff.modifiers.map(modifier => ({ stat: modifier.stat, amount: statModifierAmount(modifier, source.level) }));
  if (!addTimedEffect(target, 'statEffects', { stackName: buff.stackName, until, modifiers })) return;
  for (const { stat, amount } of modifiers) event(state, 'statModified',
    { actorId: source.id, targetId: target.id, abilityName, stat, amount, until });
}

function addSummon(state, source, plan, command, monsters, catalog) {
  const { side, level } = source;
  const { copy, monsterId: fixed, minStars, maxStars } = plan.summon;
  let id = copy ? source.monsterId : fixed;
  if (!id) {
    randomValue(command.summonRoll);
    const pool = [...monsters.values()].filter(monster => {
      const stars = storedStars(monster.stars).integer;
      return monster.category === 'Normal' && stars >= minStars && stars <= maxStars;
    });
    if (!pool.length) throw new Error('Kein passendes Monster für die Beschwörung.');
    id = pool[command.summonRoll % pool.length].id;
  }
  const monster = definition(monsters, id), maxHP = equippedStat(state, monster, level, 'MaxHP');
  const actor = state.combatants.find(member => member.id === source.id);
  const hp = copy === 'one' ? 1 : copy === 'split' ? Math.min(maxHP, Math.floor(actor.hp / 2)) : maxHP;
  if (hp < 1) return;
  if (copy === 'split') actor.hp -= hp;
  if (!state.activeIds) {
    state.activeIds = { player: [], enemy: [] };
    state.reserveIds = { player: [], enemy: [] };
    state.deadIds = { player: [], enemy: [] };
    for (const member of state.combatants) (member.hp > 0 ? state.activeIds : state.deadIds)[member.side].push(member.id);
  }
  const summonedId = `${side}-summon-${state.turn}`;
  state.combatants.push({ id: summonedId, monsterId: monster.id, side, level, hp, maxHP, summoned: true,
    ...(copy ? { extraEntryDelay: 100 } : {}), ...passiveState(monster, catalog) });
  state.reserveIds[side].push(summonedId);
  event(state, 'summoned', { actorId: actor.id, targetId: summonedId, abilityName: plan.name });
  if (state.activeIds[side].length >= ORIGINAL_ACTIVE_SIZE) return;
  const incomingId = state.reserveIds[side].shift();
  if (!Object.hasOwn(command.entryRolls ?? {}, incomingId)) throw new EntryRandomRequired(incomingId);
  const incoming = state.combatants.find(member => member.id === incomingId);
  const roll = command.entryRolls[incomingId]; randomValue(roll);
  const speed = equippedStat(state, definition(monsters, incoming.monsterId), incoming.level, 'Speed');
  const delay = initialBattleDelay({ speed, randomUint32: roll, extra: incoming.extraEntryDelay ?? 0 });
  state.activeIds[side].push(incomingId);
  incoming.enteredAt = state.queue.clock;
  const requestedTime = Math.trunc(Math.fround(Math.fround(state.queue.clock) + delay.float));
  state.queue = setQueueTime(state.queue, incomingId, requestedTime);
  event(state, 'entered', { actorId: incomingId, initialDelay: delay.float, requestedTime,
    scheduledTime: state.queue.entries.find(entry => entry.id === incomingId).time, randomUint32: roll });
}

function triggerRevenge(state, victim, killer, command, monsters) {
  const random = (...parts) => {
    const key = JSON.stringify(parts);
    if (!Object.hasOwn(command.passiveRolls ?? {}, key)) throw new PassiveRandomRequired(key);
    const roll = command.passiveRolls[key]; randomValue(roll); return roll;
  };
  for (const passive of victim.revenge ?? []) {
    if (!statusHits(random(victim.id, passive.slot, 'chance'), passive.chance)) continue;
    const side = passive.mode === 'allFriendly' ? victim.side : killer.side;
    let targets = livingOnField(state).filter(member => member.side === side);
    if (!targets.length) continue;
    if (passive.mode === 'random') targets = [targets[random(victim.id, passive.slot, 'target') % targets.length]];
    for (const target of targets) {
      if (passive.stun) stunTarget(state, victim.id, target.id, passive.stun, passive.name);
      for (const buff of passive.statBuffs ?? []) applyStatBuff(state, victim, target, buff, passive.name);
      if (passive.poison) {
        const stat = teamStats(state, victim, monsters)[passive.poison.stat];
        const rolled = originalDice({ stat, randomness: .2, targets: 1, usable: false,
          randomUint32: random(victim.id, passive.slot, target.id) }).roll;
        addPoison(state, victim, target, passive.poison, rolled, passive.name);
      }
    }
  }
}

function selectNext(state, monsters, command = {}, legacy = false) {
  for (;;) {
    const clock = state.queue.clock;
    const result = advanceQueue(state.queue); state.queue = result.queue;
    event(state, 'time', { elapsed: result.elapsed });
    for (const member of livingOnField(state)) {
      for (const effect of member.poisonEffects ?? []) {
        const elapsed = Math.min(result.elapsed, effect.until - clock);
        if (elapsed <= 0 || !member.hp) continue;
        const tick = poisonTick(effect, elapsed);
        effect.carry = tick.carry;
        if (!tick.amount) continue;
        const source = state.combatants.find(combatant => combatant.id === effect.sourceId) ?? member;
        const damage = fixedHit(state, source, member, monsters, { amount: tick.amount, kind: effect.stat,
          element: definition(monsters, member.monsterId).element, legacy });
        member.hp -= damage;
        event(state, 'poisonDamage', { actorId: source.id, targetId: member.id, damage });
        afterHit(state, member);
        if (!member.hp) leaveField(state, member, command, monsters, 'poison', source);
      }
      for (const effect of member.sentenceEffects ?? []) {
        if (!member.hp || effect.until > state.queue.clock) continue;
        const source = state.combatants.find(combatant => combatant.id === effect.sourceId);
        if (source?.hp > 0 && onField(state, source)) {
          member.hp = 0;
          leaveField(state, member, command, monsters, 'sentence');
        }
      }
      for (const key of ['hasteEffects', 'guardEffects', 'sleepEffects', 'poisonEffects', 'sentenceEffects', 'statEffects']) {
        if (!member[key]) continue;
        member[key] = member[key].filter(effect => effect.until === null || effect.until > state.queue.clock);
        if (!member[key].length) {
          delete member[key];
          if (key === 'sleepEffects' || key === 'poisonEffects') event(state, 'statusRemoved',
            { targetId: member.id, status: key === 'sleepEffects' ? 'Sleep' : 'Poison', cause: 'expiry' });
        }
      }
    }
    const livingSides = new Set(livingOnField(state).map(c => c.side));
    if (state.objective?.integrity === 0) { livingSides.clear(); livingSides.add('enemy'); }
    if (livingSides.size < 2) {
      state.winner = [...livingSides][0] || 'draw'; state.actorId = null; state.phase = 'finished';
      event(state, 'finished', { winner: state.winner }); return;
    }
    const front = state.queue.entries[0];
    if (front.time > state.queue.clock) continue;
    const actor = state.combatants.find(member => member.id === front.id);
    if (actor.sleepEffects?.length) {
      const cost = teamActionCost(actor, state.queue.clock, 130); // Utility.LooseTurn, tier 4
      state.queue = addQueueTime(state.queue, actor.id, cost);
      delete actor.stunGifts;
      delete actor.venomGifts;
      if (state.bonusActionRank) delete actor.bonusActionChain;
      event(state, 'turnSkipped', { actorId: actor.id, cost, status: 'Sleep' });
      continue;
    }
    state.actorId = front.id;
    event(state, 'ready', { actorId: state.actorId });
    return;
  }
}
export function createTeamBattle({ player, enemy, playerReserve = [], enemyReserve = [], initialRolls, eventLimit,
  questBattle = false, tactic = '', catalog, bonusActionRank, ocarina }, monsters) {
  if (eventLimit !== undefined && (!Number.isInteger(eventLimit) || eventLimit < 16 || eventLimit > 4096)) throw new Error('Ungültige Protokollgrenze.');
  const activeLimit = ORIGINAL_ACTIVE_SIZE;
  if ([player, enemy].some(team => !Array.isArray(team) || team.length < 1 || team.length > activeLimit)) {
    throw new Error(`Der Kampf braucht je 1–${activeLimit} aktive Teilnehmer.`);
  }
  if (!Array.isArray(playerReserve) || playerReserve.length > (questBattle ? ORIGINAL_ACTIVE_SIZE : SIMULATOR_MAX_RESERVE) ||
      !Array.isArray(enemyReserve) || enemyReserve.length > (questBattle ? 69 : SIMULATOR_MAX_RESERVE)) {
    throw new Error('Zu viele Reservemonster für diesen Kampf.');
  }
  const activeCount = player.length + enemy.length;
  const input = [...player.map(c => ({ ...c, side: 'player', reserve: false })), ...enemy.map(c => ({ ...c, side: 'enemy', reserve: false })),
    ...playerReserve.map(c => ({ ...c, side: 'player', reserve: true })), ...enemyReserve.map(c => ({ ...c, side: 'enemy', reserve: true }))];
  if (!Array.isArray(initialRolls) || initialRolls.length !== activeCount) throw new Error('Startzufall fehlt.');
  const ids = new Set();
  const state = { schema: 1, mode: 'limited-team-test', phase: 'awaiting-command', turn: 0,
    queue: { clock: 0, entries: [] }, combatants: [], actorId: null, winner: null, events: [],
    ...(questBattle ? { allowRetreat: false } : {}),
    ...(catalog && !questBattle ? { runtimeEffects: true } : {}),
    ...(eventLimit === undefined ? {} : { eventLimit, eventSequence: 0 }) };
  if (bonusActionRank) state.bonusActionRank = bonusActionRank;
  if (ocarina) {
    // Original Gem.applyBuffsToMonsters adds the Ocarina's stats as a permanent buff.
    const stats = Object.entries(ocarina.stats), level = ocarina.level;
    state.ocarina = {
      id: ocarina.id,
      statBonuses: Object.fromEntries(stats.filter(([key, value]) => STAT_ORDER.includes(key) && value !== 0)
        .map(([key, value]) => [key, Math.fround(value * level * (key === 'Defense' || key === 'Resist' ? 1.5 : 1))])),
      elementBoosts: Object.fromEntries(stats.filter(([key]) => key.endsWith('Boost'))
        .map(([key, value]) => [key.slice(0, -5), Math.fround(value * level * .01)])),
      tierBonus: (ocarina.stats.TierBonus ?? 0) * level,
    };
  }
  if (questBattle && tactic) state.objective = startTactic(tactic);
  if (playerReserve.length || enemyReserve.length) {
    state.activeIds = { player: player.map(c => c.id), enemy: enemy.map(c => c.id) };
    state.reserveIds = { player: playerReserve.map(c => c.id), enemy: enemyReserve.map(c => c.id) };
    state.deadIds = { player: [], enemy: [] };
  }
  for (const [index, entry] of input.entries()) {
    if (typeof entry.id !== 'string' || !entry.id.length || ids.has(entry.id)) throw new Error('Kampf-ID fehlt oder ist doppelt.');
    if (!Number.isInteger(entry.level) || entry.level < 1 || entry.level > 150) throw new Error('Ungültiges Testlevel.');
    ids.add(entry.id); if (!entry.reserve) randomValue(initialRolls[index]);
    const monster = definition(monsters, entry.monsterId);
    const vitality = questBattle && entry.side === 'enemy' ? entry.vitality ?? 1 : 1;
    const force = questBattle && entry.side === 'enemy' ? entry.force ?? 1 : 1;
    if (![vitality,force].every(n => Number.isFinite(n) && n >= 1)) throw new Error('Ungültige Queststärke.');
    const maxHP = Math.max(1, Math.ceil(equippedStat(state, monster, entry.level, 'MaxHP') * vitality));
    state.combatants.push({ id: entry.id, side: entry.side, monsterId: entry.monsterId, level: entry.level, hp: maxHP, maxHP,
      ...(force === 1 ? {} : { force }), ...passiveState(monster, questBattle ? null : catalog) });
    if (entry.reserve) continue;
    const delay = initialBattleDelay({ speed: equippedStat(state, monster, entry.level, 'Speed'), randomUint32: initialRolls[index] }).queueTime;
    state.queue = setQueueTime(state.queue, entry.id, delay);
    event(state, 'entered', { actorId: entry.id, initialDelay: delay });
  }
  selectNext(state, monsters);
  return state;
}

function leaveField(state, victim, command, monsters, cause, killer = null) {
  if (killer && state.runtimeEffects) {
    const credited = state.combatants.find(member => member.id === killer.id);
    credited.kills = (credited.kills ?? 0) + 1;
    triggerRevenge(state, victim, killer, command, monsters);
  }
  if (victim.hp === 0) for (const survivor of livingOnField(state)) {
    for (const buff of survivor.survival ?? []) applyStatBuff(state, survivor, survivor, buff, buff.name);
  }
  if (state.activeIds) {
    const incomingId = state.reserveIds[victim.side][0];
    let speed, roll, extra;
    if (incomingId !== undefined) {
      roll = command.entryRolls && Object.hasOwn(command.entryRolls, incomingId) ? command.entryRolls[incomingId] : undefined;
      if (roll === undefined) throw new EntryRandomRequired(incomingId);
      randomValue(roll);
      const incoming = state.combatants.find(c => c.id === incomingId);
      speed = equippedStat(state, definition(monsters, incoming.monsterId), incoming.level, 'Speed');
      extra = incoming.extraEntryDelay;
    }
    const displaced = cause === 'landslide' || cause === 'rotation';
    const transition = replaceCombatant({ activeIds: state.activeIds[victim.side], reserveIds: state.reserveIds[victim.side],
      deadIds: state.deadIds[victim.side], queue: state.queue, removedId: victim.id, speed, extra, randomUint32: roll, returnToReserve: displaced });
    state.activeIds[victim.side] = transition.activeIds; state.reserveIds[victim.side] = transition.reserveIds;
    state.deadIds[victim.side] = transition.deadIds; state.queue = transition.queue;
    if (displaced) for (const key of ['buffs', 'hasteEffects', 'guardEffects', 'stunGifts', 'venomGifts', 'sleepEffects', 'poisonEffects', 'sentenceEffects', 'statEffects']) delete victim[key];
    event(state, displaced ? 'displaced' : 'died', { actorId: victim.id, ...(cause ? { cause } : {}) });
    if (transition.enteredId !== null) {
      const incoming = state.combatants.find(member => member.id === transition.enteredId);
      if (state.runtimeEffects) {
        incoming.enteredAt = state.queue.clock;
        delete incoming.kills;
      }
      event(state, 'entered', { actorId: transition.enteredId, replacing: victim.id,
        initialDelay: transition.initialDelay, requestedTime: transition.requestedTime, scheduledTime: transition.scheduledTime, randomUint32: roll });
    }
  } else {
    state.queue = removeFromQueue(state.queue, victim.id);
    event(state, 'died', { actorId: victim.id, ...(cause ? { cause } : {}) });
  }
}

function stunTarget(state, actorId, targetId, delay, abilityName) {
  state.queue = addQueueTime(state.queue, targetId, delay);
  event(state, 'stun', { actorId, targetId, delay, abilityName,
    scheduledTime: state.queue.entries.find(entry => entry.id === targetId).time });
}

export function commandTeamBattle(previous, command, monsters, { legacy = false, catalog = null, aiDecision = null, statuses = false } = {}) {
  if (previous.phase !== 'awaiting-command' || command.turn !== previous.turn || command.actorId !== previous.actorId) {
    throw new Error('Dieser Kampfbefehl gehört nicht zum aktuellen Zug.');
  }
  if (command.kind === 'guard' && mayGuard(previous.objective) && previous.combatants.find(c => c.id === previous.actorId).side === 'player') {
    const state = structuredClone(previous);
    state.objective.cover = 3; state.objective.playerTurns++;
    state.queue = addQueueTime(state.queue,state.actorId,120);
    event(state,'guard',{actorId:state.actorId}); state.turn++; selectNext(state, monsters, command, legacy);
    return state;
  }
  if (command.kind === 'delay') {
    const actor = previous.combatants.find(c => c.id === previous.actorId);
    const opponents = new Set(livingOnField(previous).filter(c => c.side !== actor.side).map(c => c.id));
    const nextEnemy = previous.queue.entries.find(entry => opponents.has(entry.id));
    const cost = Math.max(1, nextEnemy ? nextEnemy.time - previous.queue.clock : 99999);
    const state = structuredClone(previous);
    const waiting = state.combatants.find(member => member.id === actor.id);
    if (waiting.stunGifts) delete waiting.stunGifts;
    if (waiting.venomGifts) delete waiting.venomGifts;
    if (state.bonusActionRank) delete waiting.bonusActionChain;
    state.queue = addQueueTime(state.queue, actor.id, cost);
    if (aiDecision) event(state, 'aiDecision', { actorId: actor.id, ...aiDecision });
    event(state, 'cost', { actorId: actor.id, cost });
    state.turn++;
    selectNext(state, monsters, command, legacy);
    return state;
  }
  if (!['testAttack', 'useSkill'].includes(command.kind)) throw new Error('Diese Aktion ist im Teamtest nicht unterstützt.');
  if (command.kind === 'testAttack' && (!['Attack', 'Magic'].includes(command.stat) || !Number.isInteger(command.activation) || command.activation < 50 || command.activation > 800)) {
    throw new Error('Ungültiger Prüfangriff.');
  }
  randomValue(command.randomUint32);
  const source = previous.combatants.find(c => c.id === command.actorId);
  const target = previous.combatants.find(c => c.id === command.targetId);
  if (!source || source.hp <= 0 || !target || target.hp <= 0 || !onField(previous, source) || !onField(previous, target)) throw new Error('Ungültiges Angriffsziel.');
  const sourceDef = definition(monsters, source.monsterId);
  if (command.kind === 'useSkill' && !catalog) throw new Error('Fähigkeitskatalog fehlt.');
  let plan;
  if (command.kind === 'useSkill') {
    const skill = sourceDef.skills.find(s => s.slot === command.slot);
    if (!skill) throw new Error('Diese Fähigkeit gehört nicht zum Monster.');
    plan = compileCombatSkill(skill, catalog, { statuses: statuses && !previous.objective });
    if (!plan.supported) throw new Error('Fähigkeit noch gesperrt: ' + plan.reasons.join(', '));
  }
  const selfBarrier = plan?.effect === 'barrier' && plan.target === 'self';
  const healing = plan?.effect === 'heal';
  const groupHeal = healing && plan.target === 'friends';
  const friendlyStatus = ['haste', 'stunGift', 'venomGift', 'guardian', 'cleanse', 'invert', 'shift'].includes(plan?.effect);
  const doesDamage = !plan || plan.effect === 'damage' || (plan.stun && plan.kind);
  const multiEnemy = plan?.target === 'enemies';
  let damageTargets = [target];
  if (plan?.randomTarget) {
    if (!Array.isArray(command.targetRolls) || command.targetRolls.length !== 2) throw new Error('Zufallswerte für die Zielwahl fehlen.');
    command.targetRolls.forEach(randomValue);
    damageTargets = [randomTeamTarget(livingOnField(previous), source, command.targetRolls)];
  }
  if (multiEnemy) {
    const enemies = livingOnField(previous).filter(c => c.side !== source.side);
    const ids = command.targetIds;
    const prescribed = plan.targetCount === 'all' ? enemies.map(member => member.id)
      : plan.targetCount === 'matching' ? matchingEnemyTargets(enemies, command.targetId, monsters) : null;
    const needed = prescribed?.length ?? Math.min(plan.targetCount, enemies.length);
    if (!Array.isArray(ids) || ids.length !== needed || new Set(ids).size !== needed ||
        ids[0] !== command.targetId || ids.some(id => !enemies.some(c => c.id === id))) throw new Error('Ungültige Schadensziele.');
    if (prescribed && ids.some((id, i) => id !== prescribed[i])) throw new Error(plan.targetCount === 'all'
      ? 'Alle Gegner müssen in Teamreihenfolge angegeben werden.' : 'Zielgruppe entspricht nicht dem Element des Hauptziels.');
    if (doesDamage) {
      if (!Array.isArray(command.randomUint32s) || command.randomUint32s.length !== needed) throw new Error('Schadenszufall pro Ziel fehlt.');
      command.randomUint32s.forEach(randomValue);
    }
    damageTargets = ids.map(id => enemies.find(c => c.id === id));
  } else if (command.targetIds != null) throw new Error('Diese Fähigkeit erlaubt keine Zielgruppe.');
  const healTargets = groupHeal ? livingOnField(previous).filter(c => c.side === source.side && c.id !== source.id) : [target];
  const validTarget = plan?.target === 'self' || plan?.target === 'team' || groupHeal ? source.id === target.id
    : plan?.target === 'friend' ? source.side === target.side && source.id !== target.id : source.side !== target.side;
  if (!validTarget) throw new Error('Ungültiges Angriffsziel.');
  if (groupHeal) {
    if (!healTargets.length) throw new Error('Kein lebendes Heilziel im Team.');
    if (!Array.isArray(command.randomUint32s) || command.randomUint32s.length !== healTargets.length) throw new Error('Heilzufall pro Ziel fehlt.');
    command.randomUint32s.forEach(randomValue);
  }
  if (selfBarrier && source.buffs?.length) throw new Error('Schutz bereits aktiv; erneutes Anwenden ist noch nicht freigegeben.');
  if (plan?.reserveSacrifice && !previous.reserveIds?.[source.side]?.length) throw new Error('Fressen benötigt ein Reservemonster.');
  if (plan?.escape && previous.allowRetreat === false) throw new Error('In diesem Kampf ist Rückzug gesperrt.');
  if (plan?.rotation && !previous.reserveIds?.[target.side]?.length) throw new Error('Der Wechsel benötigt ein Reservemonster.');
  if (plan?.summon?.copy && ((previous.reserveIds?.[source.side]?.length ?? 0) ||
      livingOnField(previous).filter(member => member.side === source.side).length >= ORIGINAL_ACTIVE_SIZE)) {
    throw new Error('Für die Kopie muss ein aktiver Platz frei und die eigene Reserve leer sein.');
  }
  if (plan?.reserveElement && !hasElementReserve(previous, source.side, plan.reserveElement, monsters)) throw new Error('Kein Reservemonster des passenden Elements.');
  const opposingSide = source.side === 'player' ? 'enemy' : 'player';
  const reserveToScramble = plan?.scramble ? previous.reserveIds?.[opposingSide] ?? [] : [];
  if (plan?.scramble) {
    if (reserveToScramble.length < 2) throw new Error('Die gegnerische Reserve braucht mindestens zwei Monster.');
    if (!Array.isArray(command.scrambleRolls) || command.scrambleRolls.length !== reserveToScramble.length) throw new Error('Zufallswerte für die Reserve fehlen.');
    command.scrambleRolls.forEach(randomValue);
  }
  const sleepAllies = plan?.sleep?.friendly ? livingOnField(previous).filter(member => member.side === source.side) : [];
  const friendlyDamageTargets = plan?.friendlyDamage ? livingOnField(previous).filter(member => member.side === source.side) : [];
  if (friendlyDamageTargets.length) {
    if (!Array.isArray(command.friendlyDamageRolls) || command.friendlyDamageRolls.length !== friendlyDamageTargets.length) {
      throw new Error('Schadenszufall für die eigenen Ziele fehlt.');
    }
    command.friendlyDamageRolls.forEach(randomValue);
  }
  if (plan?.stun || plan?.sleep) {
    if (!Array.isArray(command.statusRolls) || command.statusRolls.length !== damageTargets.length + sleepAllies.length) throw new Error('Statuszufall pro Ziel fehlt.');
    command.statusRolls.forEach(randomValue);
  }
  if (plan?.poison) {
    if (!Array.isArray(command.poisonRolls) || command.poisonRolls.length !== damageTargets.length) throw new Error('Giftwurf pro Ziel fehlt.');
    command.poisonRolls.forEach(randomValue);
  }
  if (doesDamage && source.venomGifts?.length) {
    if (!Array.isArray(command.venomRolls) || command.venomRolls.length !== damageTargets.length * source.venomGifts.length) throw new Error('Zufallswurf für Giftgabe fehlt.');
    command.venomRolls.forEach(randomValue);
  }
  if (plan?.sacrificeChance > 0) randomValue(command.sacrificeRandomUint32);
  else if (command.sacrificeRandomUint32 !== undefined) randomValue(command.sacrificeRandomUint32);
  const requestedIds = damageTargets.map(member => member.id);
  let redirected = null;
  if (damageTargets[0]?.side !== source.side && (doesDamage || plan?.stun || plan?.sleep || plan?.poison || plan?.sentence ||
      plan?.rotation || plan?.modifiers)) {
    const field = livingOnField(previous);
    const pool = redirectionPool(field, source.side, requestedIds, previous.queue.clock);
    const rolls = command.redirectRolls ?? [];
    if (!Array.isArray(rolls) || rolls.length !== redirectRollCount(pool, requestedIds.length)) throw new Error('Zufallswerte für Zielumleitung fehlen.');
    rolls.forEach(randomValue);
    if (pool.length) {
      const ids = redirectTargets(pool, requestedIds, rolls);
      damageTargets = ids.map(id => field.find(member => member.id === id));
      if (ids.some(id => !requestedIds.includes(id))) redirected = ids;
    }
  }
  const state = structuredClone(previous), victim = state.combatants.find(c => c.id === target.id);
  if (statuses && !previous.objective) state.runtimeEffects = true;
  if (aiDecision) event(state, 'aiDecision', { actorId: source.id, ...aiDecision });
  if (redirected) event(state, 'targetRedirected', { actorId: source.id, requestedIds, targetIds: redirected, abilityName: plan?.name });
  const activation = plan?.activation ?? command.activation;
  if (plan?.escape) {
    state.turn++;
    state.actorId = null;
    state.phase = 'finished';
    state.winner = opposingSide;
    event(state, 'escaped', { actorId: source.id, abilityName: plan.name });
    event(state, 'finished', { winner: state.winner, cause: 'escape' });
    return state;
  }
  let survivingHitTarget, magnification = 0;
  if (plan?.reserveSacrifice) {
    const sacrificedId = state.reserveIds[source.side].at(-1), reserve = state.combatants.find(c => c.id === sacrificedId);
    if (reserve.hp <= 0) throw new Error('Dieses Reservemonster ist bereits besiegt.');
    const sacrificed = sacrificeLastReserve({ reserveIds: state.reserveIds[source.side], deadIds: state.deadIds[source.side],
      level: reserve.level, levelMultiplier: plan.reserveSacrifice.levelMultiplier });
    state.reserveIds[source.side] = sacrificed.reserveIds; state.deadIds[source.side] = sacrificed.deadIds;
    reserve.hp = 0; magnification = sacrificed.magnification;
    event(state, 'reserveSacrificed', { actorId: source.id, targetId: sacrificedId, abilityName: plan.name,
      reserveLevel: reserve.level, magnification });
  }
  if (plan?.stun) {
    for (const [index, member] of damageTargets.entries()) {
      if (statusHits(command.statusRolls[index], plan.stun.chance)) stunTarget(state, source.id, member.id, plan.stun.duration, plan.name);
      else event(state, 'statusMiss', { actorId: source.id, targetId: member.id, abilityName: plan.name });
    }
  }
  if (plan?.barrier) {
    const recipients = selfBarrier ? [victim] : livingOnField(state).filter(member => member.side === source.side);
    for (const member of recipients) {
      const buff = { ...plan.barrier, id: `${source.id}:${state.turn}:${plan.slot}`, name: plan.name, sourceId: source.id };
      (member.buffs ??= []).push(buff);
      event(state, 'buffAdded', { actorId: source.id, targetId: member.id, abilityName: plan.name, activation });
    }
  }
  if (plan?.modifiers) {
    const targets = plan.target === 'team' ? livingOnField(state).filter(member => member.side === source.side) : damageTargets;
    for (const target of targets) {
      const member = state.combatants.find(member => member.id === target.id);
      for (const modifier of plan.modifiers) {
        const until = state.queue.clock + modifier.duration;
        const amount = statModifierAmount(modifier, source.level);
        if (addTimedEffect(member, 'statEffects', { ...modifier, amount, until })) {
          event(state, 'statModified', { actorId: source.id, targetId: member.id, abilityName: plan.name, stat: modifier.stat, amount, until });
        }
      }
    }
  }
  if (plan?.sleep) {
    for (const [index, target] of [...damageTargets, ...sleepAllies].entries()) {
      const member = state.combatants.find(member => member.id === target.id);
      if (statusHits(command.statusRolls[index], plan.sleep.chance)) {
        const until = state.queue.clock + plan.sleep.duration;
        (member.sleepEffects ??= []).push({ until });
        event(state, 'sleep', { actorId: source.id, targetId: member.id, abilityName: plan.name, until });
      } else event(state, 'statusMiss', { actorId: source.id, targetId: member.id, abilityName: plan.name, status: 'Sleep' });
    }
  }
  if (friendlyStatus) {
    const targets = plan.target === 'team' ? livingOnField(state).filter(c => c.side === source.side) : [victim];
    const shiftAmount = plan.shift ? teamStats(state, source, monsters)[plan.shift.sourceStat] : 0;
    for (const member of targets) {
      if (plan.shift) {
        const until = state.queue.clock + plan.shift.duration;
        (member.statEffects ??= []).push({ ...plan.shift, amount: shiftAmount, until });
        event(state, 'statShift', { actorId: source.id, targetId: member.id, abilityName: plan.name, ...plan.shift, amount: shiftAmount, until });
      } else if (plan.invert) {
        const hp = member.hp;
        member.hp = Math.max(1, member.maxHP - hp);
        event(state, 'hpInverted', { actorId: source.id, targetId: member.id, abilityName: plan.name, hp: member.hp, change: member.hp - hp });
      } else if (plan.cleanse) {
        for (const [status, key] of [['Sleep', 'sleepEffects'], ['Poison', 'poisonEffects']]) {
          if (!plan.cleanse.includes(status) || !member[key]?.length) continue;
          delete member[key];
          event(state, 'statusRemoved', { actorId: source.id, targetId: member.id, abilityName: plan.name, status, cause: 'cleanse' });
        }
      } else if (plan.haste) {
        const until = state.queue.clock + plan.haste.duration;
        if (addTimedEffect(member, 'hasteEffects', { ...plan.haste, until })) {
          event(state, 'haste', { actorId: source.id, targetId: member.id, abilityName: plan.name, until });
        }
      } else if (plan.guardian) {
        const until = state.queue.clock + plan.guardian.duration;
        (member.guardEffects ??= []).push({ priority: plan.guardian.priority, until });
        event(state, 'guardian', { actorId: source.id, targetId: member.id, abilityName: plan.name, until });
      } else if (plan.venomGift) {
        (member.venomGifts ??= []).push({ ...plan.venomGift, sourceId: source.id });
        event(state, 'venomGift', { actorId: source.id, targetId: member.id, abilityName: plan.name });
      } else {
        (member.stunGifts ??= []).push(plan.stunGift);
        event(state, 'stunGift', { actorId: source.id, targetId: member.id, abilityName: plan.name, delay: plan.stunGift });
      }
    }
  } else if (plan?.summon) {
    addSummon(state, source, plan, command, monsters, catalog);
  } else if (plan?.scramble) {
    const reordered = [];
    for (const [i, id] of reserveToScramble.entries()) reordered.splice(command.scrambleRolls[i] % (i + 1), 0, id);
    state.reserveIds[opposingSide] = reordered;
    event(state, 'reserveOrder', { actorId: source.id, abilityName: plan.name });
  } else if (plan?.reserveElement) {
    const ids = state.reserveIds[source.side], belongs = id => definition(monsters, state.combatants.find(member => member.id === id).monsterId).element === plan.reserveElement;
    state.reserveIds[source.side] = [...ids.filter(belongs), ...ids.filter(id => !belongs(id))];
    event(state, 'reserveOrder', { actorId: source.id, abilityName: plan.name });
  } else if (plan?.rotation) {
    for (const member of damageTargets) leaveField(state, state.combatants.find(target => target.id === member.id), command, monsters, 'rotation');
  } else if (plan?.sentence) {
    for (const target of damageTargets) {
      const member = state.combatants.find(member => member.id === target.id);
      const until = state.queue.clock + plan.sentence.duration;
      (member.sentenceEffects ??= []).push({ until, sourceId: source.id });
      event(state, 'sentence', { actorId: source.id, targetId: member.id, abilityName: plan.name, until });
    }
  } else if (healing) {
    for (const [index, healTarget] of healTargets.entries()) {
      const roll = groupHeal ? command.randomUint32s[index] : command.randomUint32;
      const healed = healWithSkill(sourceDef, plan, { level: source.level, hp: healTarget.hp, maxHP: healTarget.maxHP,
        randomUint32: roll, targets: healTargets.length, magnification, magic: teamStats(state, source, monsters).Magic });
      state.combatants.find(c => c.id === healTarget.id).hp = healed.hp;
      if (healed.amount === 0) continue; // The original component skips healAmount for a zero group result.
      event(state, 'heal', { actorId: source.id, targetId: healTarget.id, abilityName: plan.name,
        skillSlot: plan.slot, abilityType: plan.type, activation, randomUint32: roll,
        amount: healed.amount, healed: healed.healed });
    }
  } else if (doesDamage) {
    let dealtDamage = 0;
    const groupMag = plan?.magnification && multiEnemy;
    const allyBonus = groupMag ? groupMagnification(plan.magnification, previous, source, friendlyDamageTargets, monsters) : undefined;
    const sharedMagnification = groupMag ? groupMagnification(plan.magnification, previous, source, damageTargets, monsters, allyBonus) : undefined;
    for (const [targetIndex, damageTarget] of [...friendlyDamageTargets, ...damageTargets].entries()) {
      const friendly = targetIndex < friendlyDamageTargets.length;
      const index = friendly ? targetIndex : targetIndex - friendlyDamageTargets.length;
      const victim = state.combatants.find(c => c.id === damageTarget.id);
      if (!victim.hp) continue;
      const roll = friendly ? command.friendlyDamageRolls[index] : multiEnemy ? command.randomUint32s[index] : command.randomUint32;
      const liveActor = state.combatants.find(member => member.id === source.id);
      const hit = teamHit(state, liveActor, victim, monsters, { plan, stat: command.stat,
        activation: command.activation, randomUint32: roll, legacy, reserveMagnification: magnification,
        modifiersApplied: true, sharedMagnification: friendly ? allyBonus : sharedMagnification });
      victim.hp -= hit.damage;
      dealtDamage += hit.damage;
      if (hit.damage > 0 && victim.hp > 0) survivingHitTarget ??= victim;
      event(state, 'attack', { actorId: source.id, targetId: damageTarget.id, stat: hit.kind, activation,
        randomUint32: roll, damage: hit.damage,
        ...(multiEnemy ? { targetCount: damageTargets.length, diceTargetCount: plan.diceTargets } : {}),
        ...(plan ? { skillSlot: plan.slot, abilityType: plan.type, abilityName: plan.name } : {}) });
      if (!friendly && hit.damage > 0 && victim.hp > 0 && source.stunGifts?.length) {
        const delay = source.stunGifts.reduce((sum, value) => sum + value, 0);
        stunTarget(state, source.id, victim.id, delay, 'Stun Gift');
      }
      if (!friendly && hit.damage > 0 && victim.hp > 0) {
        for (const [giftIndex, gift] of (source.venomGifts ?? []).entries()) {
          const giver = state.combatants.find(member => member.id === gift.sourceId);
          const stat = teamStats(state, giver, monsters)[gift.stat];
          const rolled = originalDice({ stat, activation: gift.activation, abilityScale: gift.abilityScale,
            randomness: .2, targets: 1, randomUint32: command.venomRolls[index * source.venomGifts.length + giftIndex] }).roll;
          addPoison(state, giver, victim, gift, rolled, 'Venom Gift');
        }
      }
      if (!hit.noHit) {
        afterHit(state, victim);
        if (statuses && !previous.objective && victim.id !== source.id) {
          reflectHit(state, victim, source, sourceDef.element, hit, command, monsters, catalog, legacy);
        }
      }
      if (!victim.hp) {
        leaveField(state, victim, command, monsters, undefined, source);
      }
    }
    const member = state.combatants.find(member => member.id === source.id);
    if (plan?.drain && dealtDamage && member.hp > 0) {
      const hp = member.hp;
      const amount = Math.trunc(Math.fround(Math.fround(dealtDamage) * Math.fround(plan.drain)));
      member.hp = Math.max(0, Math.min(member.maxHP, hp + amount));
      event(state, 'drain', { actorId: member.id, abilityName: plan.name, change: member.hp - hp });
      if (!member.hp) leaveField(state, member, command, monsters, 'recoil');
    }
  }
  if (plan?.poison || plan?.slow) {
    const stat = plan.poison ? teamStats(state, source, monsters)[plan.poison.stat] : null;
    for (const [index, target] of damageTargets.entries()) {
      const member = state.combatants.find(member => member.id === target.id);
      if (!member.hp) continue;
      if (plan.poison) {
        const rolled = originalDice({ stat, randomness: .2, activation, abilityScale: plan.abilityScale,
          targets: 1, randomUint32: command.poisonRolls[index] }).roll;
        addPoison(state, source, member, plan.poison, rolled, plan.name);
      }
      if (plan.slow) {
        const slowUntil = state.queue.clock + plan.slow.duration;
        if (addTimedEffect(member, 'hasteEffects', { ...plan.slow, until: slowUntil })) {
          event(state, 'slow', { actorId: source.id, targetId: member.id, abilityName: plan.name, until: slowUntil });
        }
      }
    }
  }
  const actor = state.combatants.find(c => c.id === source.id);
  if (actor.stunGifts) delete actor.stunGifts;
  if (actor.venomGifts) delete actor.venomGifts;
  let cost = teamActionCost(source, state.queue.clock, activation, plan?.noHaste);
  if (actor.hp > 0 && plan?.sacrificeChance > 0) {
    const finish = finishAbility({ hp: actor.hp, chance: plan.sacrificeChance, timeCost: cost, randomUint32: command.sacrificeRandomUint32 });
    event(state, 'sacrificeCheck', { actorId: source.id, abilityName: plan.name, chance: plan.sacrificeChance,
      randomUint32: command.sacrificeRandomUint32, killed: finish.killed });
    actor.hp = finish.hp; cost = finish.queuedCost;
    if (finish.killed) {
      leaveField(state, actor, command, monsters, 'self-sacrifice');
    }
  }
  if (actor.hp > 0 && onField(state, actor)) {
    if (state.bonusActionRank && command.bonusRandomUint32 !== undefined) {
      if (!legacy && plan && plan.sacrificeChance !== 1 && !actor.sleepEffects?.length &&
          bonusActionHits(state.bonusActionRank, command.bonusRandomUint32, actor.bonusActionChain, state.ocarina?.tierBonus)) {
        actor.bonusActionChain = (actor.bonusActionChain ?? 0) + 1;
        cost = 0;
        event(state, 'bonusAction', { actorId: actor.id });
      } else delete actor.bonusActionChain;
    }
    state.queue = addQueueTime(state.queue, source.id, cost);
  }
  event(state, 'cost', { actorId: source.id, cost });
  const displaced = advanceTactic(state,source,survivingHitTarget);
  if (displaced) leaveField(state,displaced,command,monsters,'landslide');
  state.turn++;
  selectNext(state, monsters, command, legacy);
  return state;
}

export function hasElementReserve(state, side, element, monsters) {
  return state.reserveIds?.[side]?.some(id => monsters.get(state.combatants.find(member => member.id === id).monsterId).element === element) ?? false;
}
