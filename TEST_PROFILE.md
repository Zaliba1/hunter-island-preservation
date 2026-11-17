# Separate testing collections

## Fresh triple level-99 campaign (preview.9)

Select **Neues Spiel: alle Monster je 3-mal, Level 99, Rang S** on Windows or
Android, or **New game: 3 of each monster, level 99, S rank** on Mac. Start the
game and choose a new slot. No normal save is required or copied.

Before the first checkpoint, the game creates exactly three independent copies
of each of the 353 regular monster forms: 1,059 monsters, all at level 99 and S
rarity. The original tutorial starter counts toward its form's three copies;
it is raised through the game's own job-level operations. Each additional
monster uses the original monster factory, and the monsterpedia records are
initialized to level 99. Summon-only bosses remain outside the regular collection.

No extra silver, gold, tickets, summons, ocarinas, equipment, recipes or story
progress is granted. Normal tutorial rewards still apply. This is a one-time
initial collection, not a permanent level or rarity override: later captures,
egg rolls, evolutions and gameplay use the game's original rules. Saving,
loading or restarting never refills missing copies or resets their levels.
Starting another genuinely new slot grants its own initial collection.

This fourth save area is separate from normal, full-test and level-one
campaigns. The existing local history stores its own permanent
`triple_level99` mode bit, including after export, load, evolution or fusion.
It remains combinable with the three ordinary gameplay mods; select at most
one collection mode. This feature is included from preview.9.

## Fresh level-one campaign (preview.7)

Select **Neues Spiel: alle Monster auf Level 1, ohne Zusatzboni** on Windows
or Android, or **New game: all monsters at level 1, no bonus resources** on Mac.
Start the game and choose a new in-game slot. No normal save is required or
copied: this profile starts from the game's original new-campaign defaults.

The new-player hook adds all 353 regular monster forms at level 1 and S rarity to the bench
once, using the game's own monster factory and bench storage. It adds no
silver, gold, tickets, summons, ocarinas, equipment or fusion recipes. Original
tutorial rewards and subsequent progression still apply. Monsters gain levels
normally: loading, saving and restarting never reset them or replenish the
collection. Starting another genuinely new slot grants a new initial collection.

Normal saves, the full test copy below and this fresh campaign occupy independent
directories. The three collection checkboxes are mutually exclusive;
the other three mods can be combined with any collection profile. The encoded local
save history permanently identifies level-one use separately from full-test use.

## Full testing collection (preview.6)

Enable **Testspielstand erstellen / verwenden (separate Kopie)** in the Windows
or Android launcher, or **Create / use separate full test save** on Mac, then start
the game. Create and save a normal game first: that checkpoint supplies the
story progress and existing party for the test copy.

The first test launch copies the normal guest directory and expands the latest
checkpoint in each occupied save slot. Later test launches resume this copy;
they do not duplicate monsters or refill spent resources. Untick the option to
see the original normal slots again. The other gameplay mods do not select the
test directory and can be used independently with either collection.

For the supported Hunter Island 2.02 catalog the test collection adds:

- Each of the 353 regular monster forms, including bonus/event species, once
  if missing. Newly added monsters are level 50 and S rarity. Monsters copied
  from the normal seed also receive S rarity in this new test copy; their
  levels, equipment and party order are retained.
- All monsterpedia records through `highestLevelMonsters`.
- All seven summons through their original spirit-unlock data. Summon bosses
  are not placed on the regular monster bench. Their original summon-level
  bonuses apply; existing summon experience is preserved.
- All 57 ocarinas and the remaining named unique equipment, without duplicating
  equipped or owned items.
- Every fusion recipe, 9,999,999 silver, a 9,999,999 found-gold allowance in
  addition to the preserved spending ledger (using the save's gold multiplier),
  and 9,999 tickets. Random-generator
  state is preserved; the save's `seeders` array is not a resource balance.

The catalog is read locally from the imported IPA. No monster definitions,
game catalogs, IPA or player saves are distributed with the feature. It does
not complete story quests, teleport the player or clear dungeons.

S rarity is part of these three collections' initial grants only. Existing mod
campaigns are resumed without promotion, and normal campaigns are never changed.
The original starter also receives S as part of a new level-one campaign's
starting party. Later tutorial rewards, captures and egg rolls retain their
original rules. The initial-party scope ends at the first successful game save.

## Storage and identification

All four directories are below the emulator's `touchHLE_sandbox` directory:

- Normal saves: `pockettrend.hunterisland/Documents/saves0` (and other slots).
- Test saves: `pockettrend.hunterisland-test-v1/Documents/saves0`.
- Fresh level-one saves: `pockettrend.hunterisland-level1-v1/Documents/saves0`.
- Fresh triple level-99 saves: `pockettrend.hunterisland-triple99-v1/Documents/saves0`.

On Windows/Mac this is below the launcher's `profiles/default/run`; Android
uses the app's external files directory. Creation uses a temporary sibling
directory followed by a rename, leaving the normal files untouched on failure.
Linked paths and oversized copies are rejected. Existing test directories are
never silently overwritten. A local `test-profile.json` describes the copy.

Each modified game save also carries a persistent `test_profile` marker in the
existing encoded local history. The history inspector reports `test_profile`;
test creation does not fabricate successful captures or silver exchanges. A
new testing history gets a distinct profile identifier. Normal load/save,
evolution and fusion preserve the save-wide marker. Switching the launcher
option off does not remove it from a copied/exported test save.

Windows diagnostic exports with saves and Android **Spielstand exportieren**
use the selected profile. Android imports and **Sicherung vor Import** continue
to refer to the normal profile; importing an explicitly selected test save into
the normal profile retains its test history marker. Mac backups use the selected
profile. No emulator CPU/RAM savestate is created.

The metadata is local consistency evidence, not authenticated anti-cheat proof:
the machine owner can modify the open-source client and complete save data.

## Release and validation

The full collection is included from preview.6; the fresh level-one campaign
is included from preview.7. Earlier public preview.5 packages
predate it. See VALIDATION.md and the release's TEST_PROFILE_CHECKS.json for
the Windows game tests, independent save folders and marker persistence.
Mac native build/launcher/isolation/graphics evidence is supplied separately
as macos-checks.json. Actual Hunter Island gameplay on Mac and the physical
Android test-collection flow remain unverified.
