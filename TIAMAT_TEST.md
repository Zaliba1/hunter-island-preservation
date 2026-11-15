# Tiamat spawn and capture-category diagnostic

Windows test, Hunter Island 2.02, 15 November 2026.

The original `Boss.Tiamat` definition has 10 stars and category `Summon`.
The game explicitly returns capture status 2 (not eligible) for that category
in `Monster.captureStatus`, even when its separate capturable flag is true.
The user reproduced the original restriction after the Tiamat spawn test.

## Opt-in exception

Use `--hunter-test-tiamat` to generate Tiamat on the original Leafy field.
The separate `--hunter-test-tiamat-capture` flag changes only the category
rejection for a Tiamat instance whose capturable flag is already true.
Both flags must be enabled. Normal launches retain the original behavior.

The hook verifies the game identifier/version, guest implementation address,
original function bytes and the two relevant original ivar-offset slots.
It preserves guest registers and flags around its own property reads. It
changes a category-rejected status to the original capture-attempt state:
eligible before an attempt, already-attempted afterward. A false capturable
flag, other monster names/categories and other original status codes are
left unchanged.

The monster definition, category, stars, stats and abilities stay unchanged.
The existing card prices, chance formula and random roll are retained. This
does not set a guaranteed silver capture chance or reset failed attempts.

## Observed verification

- 152 engine library tests passed; 2 existing tests remain ignored. The new
  policy test covers the Tiamat exception and preserved restrictions.
- A separate no-network AppContainer test spawned Tiamat. Runtime observations
  showed `name=Tiamat`, category 1, capturable true, no previous attempt, and
  capture status changing from 2 to 0.
- In the human-in-the-loop session, Tiamat subsequently appeared on the town
  bench at level 4 with 10 stars. No guest panic occurred in that observed path.
  The available capture does not establish which card was used; silver-card
  success rates for Tiamat have not been measured.
- The isolated QA checkpoint was copied to an independent test instance with
  no time limit for continued user testing. After this reload Tiamat was also
  visible as a friendly party card in battle. Original save seeds were unchanged.

## Limits

This is an experimental, default-off diagnostic, not a normal-game compatibility
fix. Tiamat's complete team behavior, abilities, long-session behavior and
persistence with all mods disabled have not been fully validated. Other bosses are not
covered. Android has not been built or tested with these options.
Private saves, game resources, screenshots and logs are not distributed.
