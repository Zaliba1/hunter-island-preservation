import { addQueueTime } from './timing.mjs';

const missions = {
  'blockade:1':'convoy', 'abnahme:2':'workshop', 'wurzel:0':'poison', 'wurzel:1':'displace',
  'ewigeseis:2':'charge', 'eisrezept:1':'wards',
};
export function tacticFor(id, step, path) {
  if (id === 'wurzel' && step === 2) return path === 'graben' ? 'poison' : 'displace';
  return missions[id + ':' + step] || '';
}
const guardable = ['convoy','workshop','charge','poison','displace'];
export function startTactic(kind) {
  if (!kind) return null;
  if (![...guardable,'wards'].includes(kind)) throw new Error('Unbekannte Questaufgabe.');
  return { kind, enemyTurns: 0, playerTurns: 0, integrity: 4, cover: 0 };
}
export const mayGuard = rule => guardable.includes(rule?.kind);
export function tacticHint(rule) {
  if (!rule) return '';
  switch (rule.kind) {
    case 'convoy': return 'Schleppkahn · ' + rule.integrity + '/4 Rumpf · ' + Math.min(6,rule.enemyTurns) + '/6 Gegneraktionen. Deckung fängt die nächsten drei Angriffe auf den Kahn ab.';
    case 'workshop': return 'Werkmeister · ' + rule.integrity + '/4 Deckungen. Halte ihm bis zum Sieg den Rücken frei.';
    case 'charge': return rule.enemyTurns % 2 ? '❄ Das Eis knackt! Cryowyrms nächster Angriff ist geladen. Jetzt Deckung geben.' : 'Die Laterne spiegelt sich ruhig im Eis.';
    case 'poison': return 'Giftsporen am Hang · Bei einem ungedeckten Treffer gehen zusätzlich 8 % der maximalen KP verloren.';
    case 'displace': return 'Der Hang rutscht · Bei jedem dritten Gegnerzug gerät ein getroffenes Archedion ohne Deckung in die Reserve. Fehlt Ersatz, verzögert sich sein Zug um 120 TU.';
    case 'wards': return 'Eiswächter · Schutz auf Gegnerplatz ' + (Math.min(2,Math.floor(rule.playerTurns / 2)) + 1) + '. Nach zwei und vier eigenen Aktionen wechselt der Schutz.';
  }
}
export function questStrength(rule, source, target, amount) {
  if (source.side === target.side) return amount;
  if (rule?.kind === 'charge' && source.side === 'enemy' && rule.enemyTurns % 2 && !rule.cover) return amount * 2;
  if (rule?.kind === 'wards' && source.side === 'player' && amount > 0 &&
      target.id === 'e' + Math.min(2,Math.floor(rule.playerTurns / 2))) return Math.max(1,Math.floor(amount / 3));
  return amount;
}
export function questDamage(state,source,victim,damage) {
  const r = state.objective;
  if (!r) return damage;
  let amount = damage;
  if (source.side === 'enemy' && !r.cover) {
    if (r.kind === 'poison' && amount > 0) amount += Math.ceil(victim.maxHP * .08);
  }
  if (r.kind === 'charge' && victim.side === 'enemy' && r.enemyTurns < 2) amount = Math.min(amount,victim.hp - 1);
  if (r.kind === 'convoy' && victim.side === 'enemy' && r.enemyTurns < 6 &&
      state.combatants.filter(c => c.side === 'enemy' && c.hp > 0).length === 1) amount = Math.min(amount,victim.hp - 1);
  return Math.min(victim.hp,amount);
}
export function advanceTactic(state,actor,victim) {
  const r = state.objective;
  if (!r) return;
  if (actor.side === 'player') { r.playerTurns++; return; }
  r.enemyTurns++;
  if (!r.cover && ((r.kind === 'convoy' && r.enemyTurns <= 6) || r.kind === 'workshop')) r.integrity--;
  if (!r.cover && r.kind === 'displace' && r.enemyTurns % 3 === 0 && victim?.side === 'player' && victim.hp > 0) {
    if (state.reserveIds?.player.length) return victim;
    state.queue = addQueueTime(state.queue,victim.id,120);
  }
  r.cover = Math.max(0,r.cover - 1);
}
