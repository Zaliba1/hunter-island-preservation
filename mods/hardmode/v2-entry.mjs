// SPDX-License-Identifier: MPL-2.0
import { buildBattleContent } from '@browser/import/battle-content.mjs';
import { createTeamBattle, teamStats } from '@browser/engine/team-battle.mjs';
import { rankHardmodeTeamActions } from '@browser/engine/team-ai.mjs';
import { chooseSearchTeamAction } from '@browser/engine/team-search.mjs';
import { EVOLVED_HARDMODE_WEIGHTS, EVOLVED_HARDMODE_V2_SEARCH } from '@browser/engine/evolved-hardmode.mjs';
import { combatSkillOptions } from '@browser/engine/combat-skills.mjs';

let model;
globalThis.hunterAiLoad = text => {
  const content = buildBattleContent(JSON.parse(text));
  const monsters = new Map(content.monsters.map(monster => [monster.id, monster]));
  const skillPlans = new Map(content.monsters.map(monster => [monster.id, combatSkillOptions(monster, content.catalog, { statuses: true })]));
  model = { monsters, skillPlans, catalog: content.catalog, legacy: content.legacy };
  return 'ok';
};

function assemble(snapshot) {
  const { monsters } = model;
  const [player, enemy] = snapshot.teams;
  const reserve = team => [...team.active.slice(3), ...team.reserve];
  const state = createTeamBattle({ player: player.active.slice(0, 3), enemy: enemy.active.slice(0, 3),
    playerReserve: reserve(player), enemyReserve: reserve(enemy), initialRolls: Array(player.active.slice(0, 3).length +
      enemy.active.slice(0, 3).length).fill(0x80000000), eventLimit: 16, catalog: model.catalog }, monsters);
  const live = new Map(snapshot.teams.flatMap(team => [...team.active, ...team.reserve]).map(member => [member.id, member]));
  const split = name => Object.fromEntries(snapshot.teams.map((team, index) => [index === 0 ? 'player' : 'enemy', team[name].map(m => m.id)]));
  state.activeIds = split('active');
  state.reserveIds = split('reserve');
  state.deadIds = { player: Array(player.dead).fill(null), enemy: Array(enemy.dead).fill(null) };
  state.actorId = snapshot.actorId;
  state.queue = { clock: 0, entries: snapshot.order.map(id => ({ id, time: live.get(id).wait })) };
  state.events = [];
  state.eventSequence = 0;
  state.allowRetreat = snapshot.allowRetreat;
  for (const member of state.combatants) {
    const current = live.get(member.id);
    const { hp, stats, enteredAt, kills, hasteEffects, sleepEffects, guardEffects, poisonEffects, buffs, stunGifts, venomGifts } = current;
    Object.assign(member, { hp, maxHP: Math.max(hp, stats.MaxHP), enteredAt, kills, targetPriority: current.targetPriority ?? member.targetPriority,
      hasteEffects, sleepEffects, guardEffects, poisonEffects, buffs, stunGifts, venomGifts });
  }
  for (const member of state.combatants) {
    const current = live.get(member.id);
    const computed = teamStats(state, member, monsters);
    member.nativeStatDelta = Object.fromEntries(Object.entries(current.stats).map(([key, value]) => [key, value - computed[key]]));
  }
  return state;
}

globalThis.hunterAiChoose = text => {
  if (!model) throw new Error('AI catalog not initialized');
  const snapshot = JSON.parse(text);
  const state = assemble(snapshot);
  const { monsters, skillPlans, catalog, legacy } = model;
  const rules = { catalog, legacy, statuses: true, weights: EVOLVED_HARDMODE_WEIGHTS, skillPlans, explain: false };
  const selected = chooseSearchTeamAction(state, monsters, rules, EVOLVED_HARDMODE_V2_SEARCH);
  const alternatives = rankHardmodeTeamActions(state, monsters, rules).map(row => row.action);
  const moves = [], seen = new Set();
  for (const { plan, targetIds } of [selected, ...alternatives]) {
    if (!/^usable[1-9]\d*$/.test(plan.slot)) continue;
    const slot = Number(plan.slot.slice(6));
    if (!snapshot.slots.includes(slot)) continue;
    const key = JSON.stringify([slot, targetIds]);
    if (seen.has(key)) continue;
    seen.add(key);
    moves.push({ slot, targetIds, name: plan.name });
  }
  return JSON.stringify(moves);
};
