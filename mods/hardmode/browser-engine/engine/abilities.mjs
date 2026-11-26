// Original UsableAbility activation initializer, 0xa0d34..0xa0ef0 (ARMv7 2.02).
// The native brace parser reads exactly ONE UTF-16 character, not the whole tier.
export const ACTIVATION_TIERS = Object.freeze([50, 70, 100, 130, 160, 200, 250]);
function intValue(value) {
  if (typeof value === 'number' && Number.isFinite(value)) return Math.trunc(value);
  if (typeof value === 'string') {
    const match = /^\s*([+-]?[0-9]+)/.exec(value);
    return match ? Number(match[1]) : 0;
  }
  throw new Error('Nicht unterstütztes Aktivierungsargument.');
}
export function abilityActivation(argument, definition) {
  const raw = argument ?? definition ?? 100;
  if (typeof raw === 'string' && raw.startsWith('{')) {
    const tier = intValue(raw.slice(1, 2));
    return { length: ACTIVATION_TIERS[tier - 1] ?? 0, tier };
  }
  return { length: intValue(raw), tier: 0 };
}

// Inspection only: preserves the two declared component lists and their origins.
// It does not silently treat imported component classes as implemented behavior.
export function inspectAbility(skill, catalog) {
  const definition = catalog.abilities[skill.type];
  if (!definition) throw new Error('Unbekannte Fähigkeit: ' + skill.type);
  const usable = ['CombatAbility', 'NonCombatAbility', 'UsableAbility'].includes(definition.class);
  const args = skill.arguments || {};
  const components = [];
  for (const [origin, refs] of [['definition', definition.components], ['arguments', args.components]]) {
    if (refs == null) continue;
    if (!Array.isArray(refs)) throw new Error('Ungültige Komponentenliste: ' + skill.type);
    for (const ref of refs) {
      const component = catalog.components[ref.type];
      if (!component) throw new Error('Unbekannte Fähigkeitskomponente: ' + ref.type);
      components.push({ type: ref.type, className: component.class, origin });
    }
  }
  return { type: skill.type, className: definition.class, usable,
    activation: usable ? abilityActivation(args.activationLength, definition.activationLength) : null,
    targetPicker: structuredClone(definition.targetPicker || null), components,
    passiveBuffs: structuredClone(definition.passiveBuffs || []),
    // Inspection itself does not execute. damage-skills.mjs gates a narrow subset.
    executable: false };
}

export function abilityCoverage(monsters, catalog) {
  const used = new Set(), classes = new Map(), componentClasses = new Map(), anomalies = [];
  let references = 0, usable = 0, passive = 0;
  for (const monster of monsters) for (const skill of monster.skills) {
    const view = inspectAbility(skill, catalog);
    references++; used.add(skill.type);
    classes.set(view.className, (classes.get(view.className) || 0) + 1);
    for (const part of view.components) componentClasses.set(part.className, (componentClasses.get(part.className) || 0) + 1);
    if (view.usable) {
      usable++;
      if (view.activation.length <= 0) anomalies.push({ monster: monster.id, slot: skill.slot,
        type: skill.type, raw: skill.arguments?.activationLength ?? catalog.abilities[skill.type].activationLength ?? null,
        activation: view.activation });
    } else passive++;
  }
  const sorted = map => Object.fromEntries([...map].sort(([a], [b]) => a.localeCompare(b)));
  return { monsters: monsters.length, references, usable, passive, distinctUsedAbilities: used.size,
    abilityDefinitions: Object.keys(catalog.abilities).length, componentDefinitions: Object.keys(catalog.components).length,
    abilityClasses: sorted(classes), componentClasses: sorted(componentClasses), anomalies,
    fullyExecutableReferences: 0,
    scope: 'Definition inspection and activation arithmetic; narrow unbuffed damage execution is reported separately.' };
}
