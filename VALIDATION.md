# Validation of the current development source

Current source version: **0.2.0-preview.2 (unreleased)**.
Reviewed on 15 November 2026.

## Checks actually completed

| Check | Result |
| --- | --- |
| Windows GNU rebuild from the prepared source checkout | Passed |
| Engine library tests | 151 passed, 2 existing tests ignored |
| Windows launcher/package unit tests | 28 passed |
| Original arena Rewards crash | Reproduced on the preview.1 binary |
| Rewards list presentation | Visible on Windows after the framework fixes |
| Closing and reopening the list | Verified before the final NIB interaction/button patch |
| Scrolling on the final candidate | Confirmed by the user |
| Final close-button icon | Implemented; final visual verification pending |

The Windows game ran in a separate AppContainer with zero network capabilities
and a copied save. The original test seed was preserved. Private game data,
captures and detailed execution logs are excluded from the repository.
The final tested engine hash and evidence boundaries are recorded in
validation.json.

The test adds coverage for modal ownership, appearance callbacks and completion
blocks; table rows, index paths and reloads; NIB archive selection and numeric
font values; image alpha formats; scaled/nested image contexts; passive view
hit testing; and image-only button ownership.

## Android and packaging status

An intermediate Android native candidate compiled. APK assembly failed because
the build could not connect to its local Gradle daemon. It did not include the
last NIB interaction/button changes. No current APK is declared built, signed,
installed or device-tested.

The new sources and dependencies were also exported/prepared in a separate
clean directory. A complete fresh build from that final export, final package
audits and binary publication remain pending. Reusing the prepared Windows
build checkout does not establish a new clean-output rebuild.

## Previous published release

The immutable preview.1 checks, hashes and matching sources remain in
[its release assets](https://github.com/Zaliba1/hunter-island-preservation/releases/tag/v0.2.0-preview.1).
Its 141 engine tests, 36 Android host tests and APK checks apply to that release,
rather than the current development source.

Full gameplay, every UI layout, long sessions and current Android execution
remain unverified. Historical Apple guest binaries have not been independently
rebuilt byte for byte; see LICENSE_REVIEW.md.
