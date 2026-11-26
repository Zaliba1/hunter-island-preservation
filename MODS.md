# Optional gameplay rules

All gameplay mods and the test-profile option are disabled on first launch. Windows and Android remember your
choices. Changes take effect the next time you start the game.
The modifications target Hunter Island 2.02.

| Option | Effect |
| --- | --- |
| More frequent starter dragons | 5% chance per ordinary wild encounter on the four original encounter tiles |
| Roguelike | New campaign with Hardmode V3, tougher scripted enemies and rebalanced rewards |
| Gravestone codes in Orlen | Enter a Monsterpedia number (1–360) at the gravestone in Orlen to receive that entry |
| Exchange silver for gold | Shop action exchanging 50 silver for 1 gold |
| No 12-hour colosseum cooldown | The next local arena battle can start immediately |
| Additional Arena Fights | Ten historical PvP lineups in the native Colosseum menu |
| Hardmode V2 | Trained enemy AI for Hunters, story battles, arena and other scripted NPC encounters |
| Create / use separate test save | Copies normal progress once, then uses its own expanded monster/equipment collection and resources |

See [TEST_PROFILE.md](TEST_PROFILE.md) for contents, separate directories,
switching back to normal saves and the persistent testing marker.

## Roguelike

Choose **Roguelike** under **Advanced** and start a new game. The base campaign
and Zalibas Expansion each have a dedicated Roguelike save folder. Zalibas
Expansion, skipping the arena wait and Additional Arena Fights can be enabled
alongside it. The launcher disables silver exchange, gravestone codes, more
frequent starter dragons and the three testing collections while Roguelike is on.

- Ordinary wild battles award zero experience and silver. Captures and other loot follow the encounter's rules.
- Scripted story and side battles award twice the experience and silver specified by the game's battle reward logic.
- Scripted enemy levels increase by **+1, +3, +5, +7, +9, +11, +13** across the seven regions, up to the game's level cap. Towns and dungeon entrances follow nearby overworld areas.
- The enemy AI uses the same Hardmode V3 policy as Additional Arena Fights in wild, story and arena battles. The game handles turns that the policy cannot model.
- Golden egg spins land on one of the rare rewards shown on the right. The game generates those rewards for each spin.
- All authored fusion recipes in the selected campaign are available from the beginning. Ingredients are still consumed by the native fusion system.

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

## Additional Arena Fights

Enable **Additional Arena Fights** in the launcher and enter a Colosseum in
Hunter Island. Select **Additional Arena Fights** on the tournament screen.
The ten teams are ordered by their level-75, rank-S results in a V3 round robin.
Each fight uses a battle-only copy of your current party. Both sides fight
at level 75 and rank S with the Ocarina in your first license slot. The CPU
uses Hardmode V3 for these fights. The menu and team names follow the game's
English, German, French, Spanish, Italian or Japanese language selection.

The command-line flag is `--hunter-additional-arena-fights`.

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
