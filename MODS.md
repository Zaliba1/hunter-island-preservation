# Optional gameplay rules

All gameplay mods and the test-profile option are disabled on first launch. Windows and Android remember your
choices. Changes take effect the next time you start the game.
The modifications target Hunter Island 2.02.

| Option | Effect |
| --- | --- |
| More frequent starter dragons | 5% chance per ordinary wild encounter on the four original encounter tiles |
| Exchange silver for gold | Shop action exchanging 50 silver for 1 gold |
| No 12-hour colosseum cooldown | The next local arena battle can start immediately |
| Create / use separate test save | Copies normal progress once, then uses its own expanded monster/equipment collection and resources |

See [TEST_PROFILE.md](TEST_PROFILE.md) for contents, separate directories,
switching back to normal saves and the persistent testing marker.

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

## Separate collections

The full testing collection and fresh level-one campaign are optional,
mutually exclusive profiles. Both can be combined with the three rules above.
The fresh campaign grants all regular forms at level 1 only when creating
a new slot. Loading or restarting never resets levels. It grants no additional
resources or summon unlocks. See TEST_PROFILE.md for profile paths and the
persistent local testing marker.
