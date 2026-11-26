import { compileDamageSkill, compileRosterDamageSkill } from './damage-skills.mjs';
import { compileBarrierSkill } from './barrier-skills.mjs';
import { compileHealingSkill } from './healing-skills.mjs';
import { compileStatusSkill } from './status-skills.mjs';
export function compileCombatSkill(skill, catalog, { statuses = false } = {}) {
  const damage = compileDamageSkill(skill, catalog);
  if (damage.supported) return { ...damage, effect: 'damage', target: damage.targetCount === 1 ? 'enemy' : 'enemies' };
  const barrier = compileBarrierSkill(skill, catalog, { teams: statuses });
  if (barrier.supported) return barrier;
  const healing = compileHealingSkill(skill, catalog);
  if (healing.supported) return healing;
  return (statuses && (compileRosterDamageSkill(skill, catalog) || compileStatusSkill(skill, catalog))) ||
    { ...damage, effect: 'unsupported', target: 'enemy' };
}
export function combatSkillOptions(monster, catalog, options) {
  return monster.skills.map(skill => compileCombatSkill(skill, catalog, options));
}
