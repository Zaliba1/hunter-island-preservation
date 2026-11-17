# Separate testing collection

Enable **Testspielstand erstellen / verwenden (separate Kopie)** in the Windows
or Android launcher, or **Create / use separate test save** on Mac, then start
the game. Create and save a normal game first: that checkpoint supplies the
story progress and existing party for the test copy.

The first test launch copies the normal guest directory and expands the latest
checkpoint in each occupied save slot. Later test launches resume this copy;
they do not duplicate monsters or refill spent resources. Untick the option to
see the original normal slots again. The other gameplay mods do not select the
test directory and can be used independently with either collection.

For the supported Hunter Island 2.02 catalog the test collection adds:

- Each of the 353 regular monster forms, including bonus/event species, once
  if missing. Newly added monsters are level 50; existing individual monsters,
  their levels, equipment and party order are retained.
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

## Storage and identification

Both directories are below the emulator's `touchHLE_sandbox` directory:

- Normal saves: `pockettrend.hunterisland/Documents/saves0` (and other slots).
- Test saves: `pockettrend.hunterisland-test-v1/Documents/saves0`.

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

This feature is included in preview.6. Earlier public preview.5 packages
predate it. See VALIDATION.md and the release's TEST_PROFILE_CHECKS.json for
the Windows game tests, independent save folders and marker persistence.
Mac native build/launcher/isolation/graphics evidence is supplied separately
as macos-checks.json. Actual Hunter Island gameplay on Mac and the physical
Android test-collection flow remain unverified.
