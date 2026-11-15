# Unreleased: 0.2.0-preview.2

The current source fixes the crash when opening **Rewards / Belohnungen** from
the arena overview in Hunter Island 2.02.

- Add modal view-controller presentation and dismissal with ownership and
  completion callbacks.
- Load directory-based NIB runtime archives and decode their font metadata.
- Add basic legacy table views, row index paths and custom-cell layout.
- Support padded RGB images without alpha and UIKit bitmap image contexts,
  including scale, orientation and nested-context restoration.
- Respect noninteractive decorative views in NIB files so the reward list can
  scroll; decode image-only buttons without the placeholder title.

Windows engine checks: 151 passed, 2 existing tests ignored.
The list opened, closed and reopened in the isolated Windows test. Scrolling
on the final candidate was confirmed by the user. The final close-button icon
still needs a visual check. See VALIDATION.md for the exact scope.

There is no packaged preview.2 release yet. Existing preview.1 Windows/APK
downloads do not contain these fixes. Android execution of the current changes
remains unverified.

# Published: 0.2.0-preview.1

The previous combined Windows/Android preview remains available with its
matching source archives and dependency sources in Releases. Its binaries
and release assets are unchanged by the current source update.
