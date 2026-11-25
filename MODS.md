# Optional gameplay rules

All gameplay mods and the test-profile option are disabled on first launch. Windows and Android remember your
choices. Changes take effect the next time you start the game.
The modifications target Hunter Island 2.02.

| Option | Effect |
| --- | --- |
| More frequent starter dragons | 5% chance per ordinary wild encounter on the four original encounter tiles |
| Gravestone codes in Orlen | Enter a Monsterpedia number (1–360) at the gravestone in Orlen to receive that entry |
| Exchange silver for gold | Shop action exchanging 50 silver for 1 gold |
| No 12-hour colosseum cooldown | The next local arena battle can start immediately |
| Hardmode V2 | Trained enemy AI for Hunters, story battles, arena and other scripted NPC encounters |
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

## Gravestone codes in Orlen

Enable **Gravestone codes in Orlen** in the launcher, start your campaign and
tap the gravestone in the southwest corner of Orlen. Enter the number shown
in the Monsterpedia, press Return and collect the reward. For example, both
`1` and `001` give Spark. Each redemption from 1 to 353 grants one level-one
monster. Codes 354 to 360 use the game's spirit totems. A first totem unlocks
the summon and further totems give it experience.

The original riddle still gives its own one-time reward. Its gravestone
continues to accept numbers once solved. The option works in both the
original campaign and Zalibas Expansion. Turning it off disables new codes.
Previously granted monsters and their individual save-history tags persist.

## Silver exchange

In the shop, select **Exchange silver**. The button and dialog use the game
language. Depending on your balance, the dialog offers 1, 10 or 100 gold for
50, 500 or 5,000 silver. Selecting an offer performs the displayed exchange.
**Cancel** closes the dialog.

Both the debit and credit are committed together through a normal game save.
Completed exchanges and captured monsters remain in the save after you
disable the mod.

## Colosseum

This option bypasses the 12-hour check between local tournament rounds.
The game's existing routines handle results, progression and rewards.
When you disable the option, the game again uses its recorded time of the
last arena victory.

## Hardmode V2

Enable **Hardmode V2** in the Windows, macOS or Android launcher, or start the
emulator with `--hunter-hardmode-v2`. The trained weights and tactical search
run offline inside the emulator. Hunter Island supplies the current state
from your 2.02 IPA and executes the selected action using its normal combat
rules. Ordinary wild encounters keep their original AI. NPC turns that the
policy cannot model use the original AI. The mod works with existing saves.

## Separate collections

The full testing collection, fresh level-one campaign and fresh triple
level-99 campaign are optional, mutually exclusive profiles. Each can be
combined with the three rules above.
The fresh campaign grants all regular forms at level 1 only when creating
a new slot. Loading or restarting never resets levels. It grants no additional
resources or summon unlocks. The triple level-99 campaign starts with exactly
three copies of each regular form, all level 99 and S, without extra resources.
New full and level-one collections also start at S; existing saves are not
upgraded. All four save areas are separate. See TEST_PROFILE.md for profile paths and the
persistent local testing marker.
