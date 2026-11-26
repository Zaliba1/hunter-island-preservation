import { commandTeamBattle } from './team-battle.mjs';
import { chooseHardmodeTeamAction, chooseStandardTeamAction, rankHardmodeTeamActions, teamActionCommand } from './team-ai.mjs';
import { EntryRandomRequired, livingOnField } from './reserves.mjs';
import { PassiveRandomRequired } from './passive-skills.mjs';
import { hasteAt, poisonPressure } from './status-skills.mjs';

export const SEARCH_DEFAULTS = Object.freeze({ weight: .65, width: 3, margin: 42, replies: 2, samples: 2,
  material: 48, health: 85, initiative: 24, status: 0, variety: 0, independentRolls: 0 });
const ROLE_OFFSET = { hit: 0, status: 0x517cc1b7, sacrifice: 0x98cfd053, redirect: 0xb38f761d,
  poison: 0x75bc5931, venomGift: 0xeef21ce7, target: 0x245ad319, scramble: 0x60489d71,
  friendlyHit: 0xadea620b, summon: 0xc837625d };

function forecastCommand(state, action, roll, monsters, options, independentRolls) {
  const command = teamActionCommand(state, action, (role, index) =>
    (roll + Math.imul(index, 0x9e3779b9) + (independentRolls ? ROLE_OFFSET[role] ?? 0 : 0)) >>> 0);
  command.entryRolls = {};
  command.passiveRolls = {};
  for (;;) {
    try { return commandTeamBattle(state, command, monsters, options); }
    catch (error) {
      if (error instanceof EntryRandomRequired && !Object.hasOwn(command.entryRolls, error.combatantId)) {
        command.entryRolls[error.combatantId] = 0x80000000;
      } else if (error instanceof PassiveRandomRequired && !Object.hasOwn(command.passiveRolls, error.key)) {
        command.passiveRolls[error.key] = roll;
      } else throw error;
    }
  }
}

function positionValue(state, side, model) {
  if (state.phase === 'finished') return state.winner === side ? 1400 : state.winner === 'draw' ? 0 : -1400;
  let value = 0;
  const reserves = model.reserveRelative === undefined ? null : new Set(Object.values(state.reserveIds ?? {}).flat());
  for (const member of state.combatants) {
    if (member.hp) {
      const hpWeight = reserves?.has(member.id) ? model.reserveRelative : 1;
      value += (member.side === side ? 1 : -1) * (model.material + hpWeight * model.health * member.hp / member.maxHP);
    }
  }
  for (const member of livingOnField(state)) {
    const now = state.queue.clock;
    const time = state.queue.entries.find(entry => entry.id === member.id).time;
    const wait = time - now + (member.sleepEffects?.some(effect => effect.until > time) ? 100 : 0);
    const direction = member.side === side ? 1 : -1;
    value += direction * model.initiative * 100 / (100 + wait);
    if (model.status) {
      value += direction * model.status * (-18 * hasteAt(member, time) - .35 * poisonPressure(member, now) +
        7 * (member.buffs?.length ?? 0) + 6 * (member.stunGifts?.length ?? 0));
    }
  }
  return value;
}

export function chooseSearchTeamAction(state, monsters, options, search = SEARCH_DEFAULTS.weight) {
  const model = typeof search === 'number' ? { ...SEARCH_DEFAULTS, weight: search } : search;
  const ranked = rankHardmodeTeamActions(state, monsters, options);
  const best = ranked[0];
  const contenders = [], slots = new Set();
  for (const row of ranked) {
    if (contenders.length >= model.width || row.score < best.score - model.margin) break;
    if (model.variety && slots.has(row.action.plan.slot)) continue;
    contenders.push(row);
    slots.add(row.action.plan.slot);
  }
  const actor = state.combatants.find(member => member.id === state.actorId);
  const initial = { ...state, events: [], eventLimit: 16, eventSequence: 0 };
  const startValue = positionValue(initial, actor.side, model);
  const continuationOptions = { ...options, explain: false };
  let selected = best, bestScore = -Infinity;
  for (const contender of contenders) {
    let forecast = 0, worst = Infinity;
    if (contenders.length > 1) for (let sample = 0; sample < model.samples; sample++) {
      const roll = Math.floor(2 ** 32 * (sample + .5) / model.samples);
      let next = forecastCommand(initial, contender.action, roll, monsters, options, model.independentRolls);
      const immediate = model.immediate ? positionValue(next, actor.side, model) : 0;
      for (let ply = 0; ply < model.replies && next.phase !== 'finished'; ply++) {
        const replying = next.combatants.find(member => member.id === next.actorId);
        const response = replying.berserk ? chooseStandardTeamAction(next, monsters, options.catalog)
          : chooseHardmodeTeamAction(next, monsters, continuationOptions);
        next = forecastCommand(next, response, 0x80000000, monsters, options, model.independentRolls);
      }
      const value = positionValue(next, actor.side, model);
      const outcome = (model.immediate ? value + model.immediate * (immediate - value) : value) - startValue;
      forecast += outcome / model.samples;
      worst = Math.min(worst, outcome);
    }
    if (model.risk && worst !== Infinity) forecast += model.risk * (worst - forecast);
    const score = contender.score + model.weight * Math.max(-300, Math.min(300, forecast));
    if (score > bestScore) { selected = contender; bestScore = score; }
  }
  if (options.explain === false) return selected.action;
  return { ...selected.action, explanation: { ...selected.summary,
    searchScore: Math.round(bestScore * 10) / 10, alternatives: ranked.filter(row => row !== selected).slice(0, 2).map(row => row.summary) } };
}
