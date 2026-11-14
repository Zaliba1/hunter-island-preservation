# Release procedure

1. Freeze one clean source commit and tag for both platforms.
2. Prepare pinned dependencies and build in a fresh source/build directory.
3. Run engine, launcher and Android tests, package audits and import checks.
4. Export matching Source and SourceDependencies archives and compute hashes.
5. Record runtime hashes, results and test limits in VALIDATION.md.
6. Create a prerelease in this private repository and upload Windows, APK,
   Source, SourceDependencies, inventories and SHA256SUMS.txt together.
7. Verify uploaded commit, assets, hashes and repository visibility.

Before sharing binaries externally, make matching source assets accessible to
each recipient. Keep sources for every distributed version.
A build-PC test does not establish support for every device.

Runtime inventories map compiled/font payloads to component notices.
The privacy gate checks actual exported files and compressed content.
Private keys, game data and user profiles are excluded.
The initial repository is private as requested.
Changing visibility is a separate owner decision.
