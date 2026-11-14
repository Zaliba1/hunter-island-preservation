# Neubau und Quellenpakete

## Eingaben

Verwende Source-ZIP und SourceDependencies-ZIP derselben Release.
Eine Git-Kopie desselben Release-Tags kann anstelle des Source-ZIPs verwendet
werden. Zum Bauen werden Spielkopie und Spielstände nicht benötigt.

Source enthält Engine- und Starteränderungen, Buildskripte, Tests, Lizenztexte
und Herkunftsnachweise. SourceDependencies enthält gepinnte native Archive,
vollständige Cargo-Pakete, Boost und die veröffentlichten Quellfreigaben der
historischen Gastbibliotheken.

| Werkzeug | Geprüfte Version |
| --- | --- |
| Python | 3.11+ |
| Rust GNU Windows | 1.90.0-x86_64-pc-windows-gnu |
| Winlibs GNU | GCC 15.2.0 / MinGW-w64 13.0.0 UCRT r3 |
| Boost | 1.81.0, im SourceDependencies-ZIP |
| Android JDK | Eclipse Temurin 17 |
| Gradle | 8.11.1 |
| Android Gradle Plugin | 8.10.1 |
| Android SDK | Plattform 35, Build Tools 35.0.0 |
| Android NDK | 28.2.13676358 |
| Android CMake | 3.22.1 |

Werkzeuge aus ihren offiziellen Projekten installieren.
toolchain-provenance.json enthält die Winlibs-Quelle und Prüfsumme.
Visual Studio wird für diesen GNU-Buildweg nicht benötigt.
Die Windows-Isolationsprobe baut der .NET-Framework-C#-Compiler.

## Quellen vorbereiten

Eine frische Kopie nach C:/hi-src entpacken. Für Buildausgaben einen
separaten leeren Ordner verwenden. Kurze Pfade ohne Leerzeichen verwenden.\nDer Windows-Ausgabeordner darf höchstens 45 Zeichen lang sein. Das vermeidet\nzu lange GCC-Abhängigkeitspfade beim nativen CPU-Kern.

~~~powershell
rustup toolchain install 1.90.0-x86_64-pc-windows-gnu --profile minimal
rustup target add aarch64-linux-android --toolchain 1.90.0-x86_64-pc-windows-gnu
python tools/prepare_offline_sources.py C:/Downloads/HunterIsland-SourceDependencies-0.2.0-preview.1.zip
python -m venv .venv
./.venv/Scripts/python.exe -m pip install -r requirements-build.txt
~~~

Der Vorbereitungsschritt prüft die Archive und erzeugt die lokale
Cargo-Vendor-Konfiguration. Bereits vorbereitete Herstellerquellen werden
beim erneuten Aufruf nicht überschrieben.

## Windows bauen und prüfen

~~~powershell
./tools/build-emulator.ps1 -BoostRoot ./emulator/vendor/boost -GnuRoot C:/Winlibs/mingw64 -TargetDir C:/hi-build/windows -Offline
./.venv/Scripts/python.exe -m unittest discover -s tests -v
./.venv/Scripts/python.exe tools/package_release.py --emulator-exe C:/hi-build/windows/x86_64-pc-windows-gnu/release/touchHLE.exe --dependency-sources .dependencies --out dist/windows
~~~

Das Paket enthält portablen Starter, Runtime, Lizenzzuordnung und Manifest.
Der Build führt Engine-Tests aus. Das Packen prüft Quellen, Lizenzeinträge
und importierte Systembibliotheken.

## Android bauen und prüfen

JDK-/SDK-/Gradle-Pfade an die eigenen Installationen anpassen.
Der Signaturschlüssel wird außerhalb der Quellen angelegt.

~~~powershell
python tools/create_android_signing.py --java-home C:/Java17 --directory C:/hi-signing
./tools/build-android.ps1 -SdkRoot C:/Android/sdk -JavaHome C:/Java17 -GradleRoot C:/Gradle/gradle-8.11.1 -GnuRoot C:/Winlibs/mingw64 -SigningConfig C:/hi-signing/local-preview-signing.json -TargetDir C:/hi-build/android -Test
~~~

APK-Ausgabe unter emulator/android/app/build/outputs/apk/debug/app-debug.apk.
-OfflineGradle verwendet vorhandene Gradle-/Maven-Caches.
Beim ersten Build lädt Gradle die gepinnten Build- und Testabhängigkeiten
aus den im Projekt eingetragenen offiziellen Repositories.
Die nativen Cargo-Builds laufen nach der Quellenvorbereitung mit --frozen.

Ein eigener Schlüssel ist für eigene Builds ausreichend. Die Projektsignatur
wird nicht im Repository verteilt. Der Neubau mit eigenem Schlüssel lässt
sich deshalb nicht als Update über die bestehende Projekt-APK installieren.

## OpenAL ändern und neu linken

Die Quellen in emulator/vendor/openal-soft bearbeiten.
Beim jeweiligen Build zusätzlich -RebuildOpenAL angeben. Das bereinigt den
Wrapper und baut die statisch eingebundene Bibliothek mit dem Projekt neu.
SDLs Android-Anpassung wendet tools/patch_android_sdl.py an.

Ein erfolgreicher Neubau ist eine Buildprüfung. Abweichende Compilerpfade,
Signaturschlüssel und Build-Zeitpunkte können andere Binärbytes erzeugen.
VALIDATION.md nennt die tatsächlich ausgeführten Prüfungen.

## Release prüfen

Das Android-Paket lässt sich mit den bereitgestellten Werkzeugen prüfen.

~~~powershell
python tools/verify_android_apk.py --apk PATH/TO/app-debug.apk --sdk-root C:/Android/sdk --java-home C:/Java17 --out .build/android-review
python tools/android_license_inventory.py .build/android-review/HunterIsland-Android-arm64-0.2.0-preview.1.apk --out .build/android-license-inventory.json
python tools/audit_public.py .
~~~

Die Prüfung umfasst Signatur, Versionsangaben, erlaubte APK-Dateien,
ARM64-/16-KiB-Ausrichtung, Runtime-Abhängigkeiten, eingebettete Lizenztexte und
bekannte lokale Identitäts-/Pfadmarker. Das Inventar gleicht Quellen und
APK-Hinweise byteweise ab.
