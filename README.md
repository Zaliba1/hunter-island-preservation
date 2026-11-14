# Hunter Island Preservation

Inoffizielles Projekt zur Offline-Erhaltung von Hunter Island 2.02 auf Windows
und Android, basierend auf [touchHLE v0.3.0](https://github.com/touchHLE/touchHLE).
Die eigene, bereits entschlüsselte 32-Bit-IPA wird lokal importiert.

## Spielen

Lade aus [Releases](https://github.com/Zaliba1/hunter-island-preservation/releases)
das Paket für dein Gerät.

| Gerät | Datei | Voraussetzungen |
| --- | --- | --- |
| Windows-PC | HunterIsland-Windows-x64-0.2.0-preview.1.zip | Windows 10/11 x64, Python 3.11+ in 64 Bit mit Tcl/Tk, OpenGL-Treiber |
| Android | HunterIsland-Android-arm64-0.2.0-preview.1.apk | Android 8+, ARM64, OpenGL ES 3.0 |

Windows. ZIP vollständig entpacken, Start.cmd öffnen, **Eigene IPA importieren**
wählen und anschließend **Spiel starten**. Python mit Tcl/Tk gibt es bei
[python.org](https://www.python.org/downloads/windows/).

Android. APK installieren, App öffnen, **Eigene IPA importieren** wählen und
anschließend **Spiel starten**. Für den APK-Installer muss Android gegebenenfalls
die Installation aus der verwendeten Download-App erlauben.

Die ausführliche [Einrichtung](SETUP.md) erklärt Updates, Bildformate,
Spielstandübertragung und Fehlerberichte. Alle drei [optionalen Mods](MODS.md)
sind anfangs ausgeschaltet. Die Starter merken sich die gewählten Optionen.

## Entwickeln

Die gemeinsamen Emulatorquellen liegen in emulator/, der Windows-Starter in
launcher/, der Android-Starter in emulator/android/app/.
[BUILD.md](BUILD.md) beschreibt den Neuaufbau aus einer frischen Kopie, die
benötigten Werkzeuge und das passende Abhängigkeitenpaket.

Die Release-Dateien werden von passenden Quellpaketen begleitet.
[LICENSES.md](LICENSES.md) beschreibt die Lizenzzuordnung,
[LICENSE_REVIEW.md](LICENSE_REVIEW.md) die Prüfung und ihre Grenzen.
[VALIDATION.md](VALIDATION.md) nennt tatsächlich ausgeführte Prüfungen.
Dieses Projekt ist unabhängig von touchHLE, Apple und dem Spieleentwickler.
