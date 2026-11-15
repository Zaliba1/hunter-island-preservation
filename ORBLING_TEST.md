# Orbling encounter and capture verification

Verified on Windows on 15 November 2026 with Hunter Island 2.02.

## Rule and isolation

The opt-in `--hunter-test-orbling` diagnostic uses the existing verified wild
weighted-draw interception. For the original Leafy pool (`over-28x92`, north
of Ark Island), it returns `Crystaldragon01` in the first encounter slot.
Other slots use the game's normal draw. Other encounter pools are left alone
unless the separate existing starter-dragon mod is also explicitly enabled.
The hook checks the bundle/version, guest implementation and caller addresses,
original call-site bytes, and the original four-entry encounter-pool signature.

The original IPA and monster resources were not modified. In particular,
Orbling's `bonusMonster` property and the original capture routines remain
unchanged. The test ran with a separate save copy in an AppContainer with
zero capabilities and no network access. The source save remained byte-identical.

## Observed results

1. Leave Ark Island and visit the Leafy road field to the north. The hook
   reports selecting `Crystaldragon01`. Tap the current road field to enter
   its wild battle.
2. Orbling appears visibly with its name, sprite and original B quality.
   The tested group also contained Seaquill.
3. Open Orbling's capture-card control. The original capture screen offers
   3%, 7%, and 100% chances in this particular party/health/level situation.
   These are observed values, not fixed probabilities for every encounter.
4. Use the original 100-gold capture card. Orbling is captured, and the gold
   balance falls from 750 to 650. No chance override or capture bypass is used.
5. Complete the battle and collect the silver reward. Orbling is available
   on the bench in town. Its detail view shows the name Orbling, level 4,
   Dragon type, and the Hoarfrost and Whirlpool abilities. Its card shows
   5.5 stars. Transfer it from the bench into the battle line-up successfully.
6. After the game writes the save, intentionally stop only the isolated QA
   process. Load that same save using the ordinary preview.3 runtime, without
   `--hunter-test-orbling`. Orbling remains in the line-up at level 4 with
   5.5 stars. This verifies ordinary save persistence after an explicit process
   stop, rather than relying on an emulator savestate or a shutdown save.

The modified engine passed all 151 existing library tests (2 existing tests
remain ignored). No guest panic occurred during the observed path. The first
QA process was deliberately stopped for the restart test; its forced-stop
exit status must not be classified as a game crash.

## Limits

Follow-up: the user also confirmed a successful silver-card capture during
the separate Orbling silver test. That test used the original capture rules;
neither card probabilities nor random values were overridden. This is a human
confirmation, not a measured estimate of the capture success rate.

Only this monster and this original encounter pool were tested. A successful
gold-card capture does not establish the long-run statistics of silver-card
attempts. Orbling's abilities were displayed but not exhaustively exercised
in combat; evolution and long-session behavior remain untested.
The diagnostic has not been built or tested on Android. It is not enabled in
the normal Windows or Android launcher and is not included in the previously
published release binaries. This result does not establish that every other
bonus/event monster can be spawned and captured without additional work.

Private screenshots, original resources, saves and detailed runtime logs stay
outside the repository and distributed source packages.
