// Selected by the 423991 training run against the original Hardmode weights.
export const EVOLVED_HARDMODE_WEIGHTS = Object.freeze({
  knockout: .25, prevention: .626, followUp: .424, haste: .25, guardian: .382, control: .898,
  support: .802, tempo: 1.141, exposure: 1.669, sacrifice: 1.386, reserve: 1.563,
  sleepTactics: 1.417, retaliation: 0,
});
export const EVOLVED_HARDMODE_SEARCH = .65;

// Selected by the 2916047 league run with 32 fixed teams.
export const EVOLVED_HARDMODE_V2_SEARCH = Object.freeze({
  weight: .85, width: 3, margin: 42, replies: 4, samples: 2,
  material: 48, health: 85, initiative: 24, status: .7, variety: 0, independentRolls: 0,
});

// Finalist from the 26111026 round robin against V2 (64 teams and two seeds).
export const EVOLVED_HARDMODE_V3 = Object.freeze({
  weights: Object.freeze({
    knockout: .16, prevention: .626, followUp: .424, haste: .148, guardian: .714,
    control: 1.069, support: .867, tempo: 1.482, exposure: 1.451, sacrifice: 1.386,
    reserve: 1.497, sleepTactics: 1.6, retaliation: .112,
  }),
  search: Object.freeze({
    weight: .974, width: 4, margin: 77.523, replies: 4, samples: 2,
    material: 36.465, health: 104.414, initiative: 34.591, status: 2.937,
    variety: 0, independentRolls: 0, immediate: .084, risk: 0, reserveRelative: .637,
  }),
});
