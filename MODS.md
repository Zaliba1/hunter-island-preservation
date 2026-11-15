# Optional gameplay rules

All three mods are disabled on first launch. Windows and Android remember your
choices. Changes take effect the next time you start the game.
The modifications target Hunter Island 2.02.

| Option | Effect |
| --- | --- |
| More frequent starter dragons | 5% chance per ordinary wild encounter on the four original encounter tiles |
| Exchange silver for gold | Shop action exchanging 50 silver for 1 gold |
| No 12-hour colosseum cooldown | The next local arena battle can start immediately |

## Starter dragons

| Original map tile | Catchable base form | Later final form |
| --- | --- | --- |
| over-28x92 | Leafy | Gaiawyrm |
| over-30x91 | Ashie | Pyrowyrm |
| over-31x89 | Bluey | Hydrowyrm |
| over-32x87 | Galey | Ventowyrm |

For the first monster in an ordinary wild encounter group, the mod selects
the corresponding dragon with a 5% probability. Remaining group slots are
filled from the three normal companions. The probability applies per wild
encounter, including encounters with different group sizes.

## Silver exchange

In the shop, select **Silber tauschen** (exchange silver). Depending on your
balance, the dialog offers 1, 10 or 100 gold for 50, 500 or 5,000 silver.
Selecting an offer performs the displayed exchange.
**Abbrechen** (cancel) closes the dialog.

Both the debit and credit are committed together through a normal game save.
Completed exchanges and captured monsters remain in the save after you
disable the mod.

## Colosseum

This option bypasses the 12-hour check between local tournament rounds.
The game's existing routines handle results, progression and rewards.
When you disable the option, the game again uses its recorded time of the
last arena victory.

## Developer-only Orbling encounter test

`--hunter-test-orbling` is a diagnostic option, disabled by default and not
enabled by any launcher checkbox. It replaces the first monster in every
ordinary wild group generated from the verified Leafy encounter pool
(`over-28x92`, north of Ark Island) with `Crystaldragon01` (Orbling).
Other group slots retain the original draw. This guarantees Orbling in a
generated wild group; it does not force a battle on every movement step.
It is a narrowly matched encounter test, not a general coordinate editor.

The rule leaves capture logic and `bonusMonster` unchanged. The Windows test
confirmed spawning, capture using the game's 100-gold card, bank/team transfer,
and persistence after restarting the unmodified preview.3 runtime without the
test option. See ORBLING_TEST.md for the exact scope and remaining limits.

## Developer-only Tiamat tests

`--hunter-test-tiamat` selects the original `Boss.Tiamat` in the same first
wild-group slot. Its original `Summon` category makes it uncatchable.
Adding `--hunter-test-tiamat-capture` enables a narrowly scoped exception to
that category check, only while the Tiamat spawn diagnostic is active.
Other capture restrictions, card costs and chance calculations remain in use.
Both options default to off and are absent from the normal launcher UI.
See TIAMAT_TEST.md for observed results and limits.
