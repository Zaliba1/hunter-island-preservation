# Save history and compatibility

This feature records local mod history inside Hunter Island 2.02 saves. No
data is uploaded, no player/device identity is collected, and no gameplay HUD
or popup is added. Earlier release binaries may predate this implementation; use the source
archive matching the particular application build.

## What is recorded

- A permanent save-wide testing-profile marker for the separate all-inclusive
  collection, with separate full, level-one and triple-level-99 mode bits.
  The inspector reports `test_profile`, including after export or
  when the file is loaded with the option disabled. Artificial additions are
  not reported as captures, exchanges or legitimate progression events.

- Persistent "ever enabled" bits for the optional gameplay rules, set only for a
  loaded/created game profile. Launcher checkbox changes without loading a
  game do not affect an unrelated save.
- Generated eligible starter-dragon packs, selected hatchlings and species totals.
  Generated packs can exist without a battle starting. Entered affected encounters
  and hatchlings seen are counted separately, starting with this implementation.
- Successful hatchling captures while the mod was enabled, separately from
  captures originating in an affected encounter. These are not claims that
  a given monster could never have appeared naturally.
- Each collected gravestone-code monster receives its own binary lineage ID
  in the same save field as the starter dragons. The inspector reports the
  redemption event and `contains_gravestone_reward` through evolution and
  fusion. A summon has one ID in its saved spirit entry, including after
  subsequent totems. The total counts collected code rewards. The ordinary
  gravestone answer and wild capture counters keep their original meanings.
- Successful silver exchanges, total silver spent and gold obtained. Failed
  saves roll back the balances and corresponding history change together.
- Positive cooldown overrides in `beginTournamentBattle`, separately from
  repeated `timeToNextTournament` polling. This counts begin actions that
  bypass the wait, rather than polling calls or victories.
- A per-monster lineage identifier for recorded captures. It propagates to
  the separate owned object created after victory and to evolved objects;
  the current form/evolution count is updated when an owned
  monster is serialized for the actual player save, rather than by merely
  creating an evolution preview.
- Recipe fusion creates a new lineage node when at least one ingredient is
  marked. Every marked ingredient is linked to that node; all ingredient
  species are retained, including unmarked inputs. Nested fusions and later
  evolution retain the chain back to the recorded captures. Consumed ancestor
  records remain in the history. Cancelled selection, previews, ordinary bench
  additions and entirely unmarked fusions do not create marked-fusion events.
  `marked_fusions` counts these successful additions separately from captures.
- Save sequence, previous saved metadata digest, bounded event journal,
  monotonic loaded-runtime observations and the emitter version.

The record uses a versioned, compressed binary envelope in a supplementary
property-list Data field before the game's existing encryption. Monster tags
are supplementary fields in existing monster save dictionaries. The original
game serialization and atomic writer remain responsible for the actual save.
Successful writes advance the checkpoint; failed writes do not.

Unknown fields must survive the new emulator's own load/save cycle. An older
emulator can discard supplemental fields when it reconstructs a save; a later
missing record must be treated as unknown, never as an unmodified run.

## Evidence limits

This is **not authenticated anti-cheat evidence**. The player owns the client,
can modify an open-source build, roll back complete saves or recompute all
local hashes. Compression/opaque field names are not a security boundary.
Checksums, payload binding and event linkage detect inconsistent data and
simple editing, not a determined owner rewriting the whole record.
Monster evolution tagging covers `buildEvolvedVersion`. Recipe-fusion adapters
cover both the Hunter Island and base monsterpedia fusion call sites; the
Hunter Island interface has been exercised in actual gameplay. Other mechanisms
such as feeding or a future custom creation path are not claimed. The persistent
profile flag remains set regardless of what happens to an individual monster.

The lineage graph uses history schema 2. Schema 1 records are migrated without
resetting their identity, counters or prior-history status. An old schema-1-only
reader rejects the new schema instead of silently dropping fusion ancestry.

Imported legacy saves have unknown prior history. New slots first observed
by this build are distinguished from imported saves; that distinction is
still a client claim, not proof of an independently witnessed challenge start.
Missing, malformed or mismatched records must never be classified as clean.
External challenge start/periodic checkpoints are a separate future feature.

## Inspection

Build the `hunter-history` binary, then read a player's own game save using
their own compatible, already decrypted IPA:

```text
hunter-history --ipa YourGame.ipa --save sv_123 --json new-report.json
```

The optional output must be a new file; it cannot overwrite the input save.
The tool statically derives the game's existing data key from the user's IPA.
No game key, game executable, game assets or player saves are bundled.

For a graphical Windows interface, place `hunter-history.exe` beside
`tools/hunter_history_inspector.pyw` and open the Python tool. Choose the IPA
and save, then select **Inspect save**. It uses Python/Tk, like the existing
Windows launcher. It stores no previously selected paths.

## Dependency/source delivery before any future publication

This local version additionally pins RustCrypto `sha2` 0.10.6 (MIT OR Apache-2.0),
SHA-256 `82e6b795fe2e3b1e845bafcb27aa35405c4d47cdfc92af5fc8d3002f76cebdc0`.
The existing serde/serde_json/flate2 dependencies are now direct dependencies.
The license inventory includes the newly used runtime JSON/ryu dependencies
as well as sha2; their existing license texts and the new sha2 texts are included.
Regenerate the corresponding dependency source package and license inventory
before distributing a build containing this work. The previous preview.2
dependency archive alone does not contain this newly added crate.

Builds include matching source and dependency archives. The macOS workflow
verifies the added crate against Cargo.lock and includes its source in the
corresponding dependency archive.

## Local validation

- Windows/GNU: 169 emulator tests passed (two unrelated existing tests ignored),
  plus all 31 launcher tests. The graphical inspector opens successfully.
- Actual isolated gameplay on copied saves: a successful 50-silver/1-gold
  exchange persisted exactly once. Disabling all mods and reloading preserved
  the same profile history and totals.
- A separate deterministic encounter fixture selected the hatchling at 100%
  on its eligible tile. One hatchling was seen and captured; the post-victory
  owned-monster dictionary contained its lineage tag. Reloading and saving
  with all mods disabled preserved the same tag and counters.
- The final local emulator was rebuilt from the normal 5% mod implementation;
  the deterministic fixture is not the candidate executable.
- Fusion fixture: edited a copied save to provide four evolved dragons and
  recipe knowledge, retaining the actual captured hatchling's tag. Using the
  untouched IPA and all mods disabled, the original UI created Magmawyrm,
  Typhonwyrm and Omegawyrm. The saved Omegawyrm linked back through Magmawyrm
  to the hatchling; captures stayed at one and marked fusions reached two.
  A fresh session with all mods disabled preserved the Omegawyrm's owned tag,
  both ancestor nodes and the same counters after another save.
  The edited fixture correctly remained inconsistent/incomplete evidence.
- Evolution identity transfer/form tracking has automated coverage and static
  method validation; a full live evolution and arena override were not played
  through in this task. Windows gameplay coverage does not establish Android
  or macOS gameplay. Native build/package results are reported per artifact;
  physical-device gameplay testing is a separate check.
- The original settings and source save hashes remained unchanged.
