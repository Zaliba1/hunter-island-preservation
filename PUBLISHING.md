# Release procedure

1. Freeze one clean source commit and tag for both platforms.
2. Prepare pinned dependencies and build in a fresh source/build directory.
3. Run engine, launcher and Android tests, package audits and import checks.
4. Export matching Source and SourceDependencies archives and compute hashes.
5. Record actual runtime hashes, results and limits in VALIDATION.md.
6. Upload Windows, APK, Source, SourceDependencies, license inventories and
   SHA256SUMS.txt together as a prerelease.
7. Verify the uploaded commit, files, hashes and intended visibility.

The owner has requested that this repository be public. A source push does not
publish a new binary release. preview.2 packaging is pending; preview.1 assets
retain their existing source correspondence.

Make matching source assets available to each binary recipient and retain
them for every distributed version. A build-PC test does not establish support
for every device.

Runtime inventories map compiled/font payloads to notices. Package checks scan
actual exports and compressed content for known private markers. Private keys,
game data, user profiles and private QA captures are excluded.
