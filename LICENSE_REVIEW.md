# License and source review

Project source files retain MPL-2.0 and original third-party terms.
Upstream declares the combined touchHLE binary GPL-3.0-or-later.
Runtime inventories map compiled/font payloads to notices and hashes.

The review checks matching modified sources and dependencies, attribution,
full license texts, build/relink scripts, Android notice bytes and accidental
private data. MPL source availability and the GPL accompanying-source route
are documented in SOURCE.md. OpenAL is statically linked. Full source and the
work using it accompany the binaries, with a rebuild/relink route.

Authoritative references consulted on 2026-11-14.

- [MPL 2.0, section 3](https://www.mozilla.org/en-US/MPL/2.0/)
- [MPL FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)
- [GPLv3, section 6](https://www.gnu.org/licenses/gpl-3.0.html)
- [GNU Library GPL 2.0](https://www.gnu.org/licenses/old-licenses/lgpl-2.0.html)

Historical guest libraries are public touchHLE binaries at the hashes in
guest-library-provenance.json. Published Apple sources, original licenses and
historical exception headers are supplied. Their historical SDK build
environment and byte-identical regeneration remain unverified.
Independent provenance beyond those identified upstream binaries and releases
has not been established.

Emulator licenses grant no rights to Hunter Island. Users supply their own IPA.
Game content and saves are excluded. This technical inventory is not a complete
legal assessment of every jurisdiction or use.
