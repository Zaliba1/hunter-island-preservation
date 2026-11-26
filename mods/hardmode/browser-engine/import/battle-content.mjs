const own = (obj, key) => Object.hasOwn(obj, key);
export function merge(parent, child) {
  const out = Object.assign(Object.create(null), parent);
  for (const [key, value] of Object.entries(child)) {
    out[key] = value && typeof value === 'object' && !Array.isArray(value) && !(value instanceof Uint8Array) &&
      out[key] && typeof out[key] === 'object' && !Array.isArray(out[key]) ? merge(out[key], value) : structuredClone(value);
  }
  return out;
}
export function registry(tree) {
  const entries = new Map();
  function visit(node, path = '', depth = 0) {
    if (!node || typeof node !== 'object' || Array.isArray(node)) return;
    if (depth > 64) throw new Error('Zu tiefe Datenvererbung.');
    if (path && ['parent', 'class', 'rootStats'].some(key => own(node, key))) { entries.set(path, node); return; }
    for (const [key, child] of Object.entries(node)) visit(child, path ? path + '.' + key : key, depth + 1);
  }
  visit(tree); return entries;
}
export function resolver(entries) {
  const cache = new Map();
  function resolve(path, seen = new Set()) {
    if (cache.has(path)) return cache.get(path);
    if (seen.has(path) || seen.size > 64) throw new Error('Zyklische Vererbung: ' + path);
    const node = entries.get(path); if (!node) throw new Error('Unbekannte Datenvorlage: ' + path);
    const next = new Set(seen); next.add(path);
    const value = merge(node.parent ? resolve(node.parent, next) : {}, node); cache.set(path, value); return value;
  }
  return resolve;
}
export function buildBattleContent(resources, strings = {}) {
  const monsterReg = registry(resources.Monsters), jobReg = registry(resources.Jobs);
  const monsterDef = resolver(monsterReg), jobDef = resolver(jobReg), monsters = [];
  for (const path of monsterReg.keys()) {
    const definition = monsterDef(path);
    if (path === 'RootMonster' || definition.abstract || !definition.element) continue;
    const id = path.split('.').at(-1);
    const job = jobReg.has(path) ? jobDef(path) : {}, skills = [];
    for (const [slot, ability] of Object.entries(job.jobAbilities || {})) {
      if (!ability || typeof ability !== 'object') continue;
      const args = ability.arguments || {}, name = args.instanceName || ability.type?.split('.').at(-1) || '';
      skills.push({ slot, type: ability.type, name: strings['Renderable' + name + 'NameKey'] || name,
        activation: args.activationLength, arguments: args });
    }
    for (const key of ['Attack', 'Defense', 'Magic', 'MaxHP', 'Resist', 'Speed']) {
      if (!Number.isFinite(definition.rootStats?.[key])) throw new Error('Ungültiger Monsterwert: ' + path + '.' + key);
    }
    monsters.push({ id, path, name: strings['Renderable' + id + 'NameKey'] || id, element: definition.element,
      stars: definition.stars, category: definition.category, stats: definition.rootStats,
      evolution: definition.evolveData, index: definition.index || 0, skills,
      ...(job.statGrowth ? { statGrowth: structuredClone(job.statGrowth) } : {}) });
  }
  monsters.sort((a, b) => a.index - b.index);
  if (monsters.length !== 360) throw new Error('Unerwarteter Monsterkatalog für diese Spielversion.');
  const catalog = {};
  for (const [key, name] of [['abilities', 'Abilities'], ['components', 'AbilityComponents'],
    ['buffs', 'Buffs'], ['buffComponents', 'BuffComponents']]) {
    const collection = registry(resources[name]);
    const resolve = resolver(collection);
    catalog[key] = Object.fromEntries([...collection.keys()].map(path => [path, resolve(path)]));
  }
  return { monsters, catalog, monsterDefinitions: Object.fromEntries(monsters.map(m => [m.path, monsterDef(m.path)])),
    legacy: Boolean(resources.GameSettings.legacy) };
}
