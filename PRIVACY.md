# Local data and diagnostics

The launchers import an IPA selected by the user locally.
The original filename and import path are not stored as diagnostic fields.
Windows defaults to %LOCALAPPDATA%/HunterIslandPreservation.
Android uses private app storage and its app-specific files directory.

Android requests neither Internet access nor broad storage access.
Windows starts the game in an AppContainer without network capabilities.
Data is not sent automatically. Export destinations are selected through
file dialogs.

Windows diagnostics contain technical details and selected, sanitized error
locations. Saves are included only when explicitly selected.
Android diagnostics and save exports are separate actions.
Save files may contain personal gameplay progress.

The repository and downloads contain software, license texts, provenance and
tests using synthetic data. The project owner selected the GitHub repository
address for publication.
Personal local paths, imported game copies, private saves and the private APK
signing key are excluded from the project.
SHA256SUMS.txt lists checksums for the published artifacts.
