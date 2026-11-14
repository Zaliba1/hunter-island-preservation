# Prüfung für 0.2.0-preview.1

Geprüft am 14. November 2026 auf dem Windows-Build-PC.

| Prüfung | Ergebnis |
| --- | --- |
| Quellen und Abhängigkeiten in frischer Kopie vorbereitet | bestanden |
| Windows-GNU-Releasebuild in neuem Buildordner | bestanden |
| Engine-Bibliothekstests | 141 bestanden, 2 vorhandene Tests ignoriert |
| Windows-Starter und Paketprüfungen | 28 bestanden |
| Frischer Android-ARM64-Releasebuild | bestanden |
| Android-Robolectric/JUnit-Prüfungen auf API 26 und 35 | 36 bestanden |
| APK-Signatur, ARM64 und 16-KiB-Ausrichtung | bestanden |
| Windows-AppContainer und Titelbild aus ZIP | bestanden |

Windows wurde mit einem separaten Testprofil und einer lokal bereitgestellten
IPA gestartet. Das Titelbild war bei 1024 × 768 sichtbar. Der begrenzte
25-Sekunden-Test wurde absichtlich durch sein Zeitlimit beendet.
Private Testprofile und Bilder liegen außerhalb dieses Repositorys.

23 Quellarchive und 9.974 Dateien aus 148 Rust-Paketen wurden gegen die
bereitgestellten Prüfsummen geprüft. Die zugehörigen Quellen und
kontrollierenden Buildskripte sind in den Source-Assets enthalten.

Die Code-Dateien in emulator/src und die Android-App-Quellen stimmen mit den
Eingaben der frischen Builds überein. Die Prüfsummen der aktuellen Runtime
stehen in validation.json sowie in den Release-Inventaren.

## Grenzen

Die neue APK wurde in dieser Prüfung nicht auf einem Handy ausgeführt.
Das Pixel war nicht per ADB verfügbar. Frühere Pixel-Tests gelten für die
jeweils damaligen APKs.

Der vollständige Spielverlauf, sämtliche Monsterfähigkeiten, alle GPUs und
lange Sitzungen sind nicht verifiziert. Historische Apple-Gastbibliotheken
wurden nicht unabhängig bytegleich neu gebaut. LICENSE_REVIEW.md beschreibt
diese Herkunftsgrenze. Erfolgreiche Builds belegen die dokumentierte
Rebuild-Route, keine plattformübergreifende Bytegleichheit.
