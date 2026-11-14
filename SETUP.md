# Einrichtung und Updates

## Eigene Spielkopie

Bestätigt unterstützt wird Hunter Island **2.02**, 32-Bit-ARM, als bereits
entschlüsselte IPA. Der Import prüft Archiv, Bundle und ausführbare Datei.
Der Starter meldet ungeeignete Dateien beim Import.

## Windows

1. Windows-x64-ZIP aus Releases herunterladen und vollständig entpacken.
   Ein kurzer Pfad wie C:/Games/HunterIsland eignet sich.
2. Python 3.11 oder neuer in 64 Bit mit Tcl/Tk über den offiziellen
   [Windows-Installer](https://www.python.org/downloads/windows/) installieren.
3. Start.cmd öffnen und **Eigene IPA importieren** auswählen.
4. Bildformat und optionale Mods auswählen, dann **Spiel starten** drücken.
5. Spielfenster schließen, um zum Starter zurückzukehren.

Zum Spielen werden Python, Grafiktreiber und Windows-Komponenten benötigt.
Rust und Compiler werden beim Entwickeln benötigt.
Windows startet das Spiel in einem AppContainer ohne Netzwerkfreigabe.
Falls die Isolationsprüfung fehlschlägt, zeigt der Starter den Fehler an.

Die Windows-Daten liegen standardmäßig unter
%LOCALAPPDATA%/HunterIslandPreservation.
Dort befinden sich Importkopie, Einstellungen, normale Spielstände und lokale
Sicherungen. Beim Update das neue Programmpaket vollständig in einen eigenen
Ordner entpacken und dessen Start.cmd verwenden. Es greift auf denselben
lokalen Datenordner zu.

## Android

1. ARM64-APK aus Releases auf das Handy übertragen oder herunterladen.
2. APK öffnen und die Installation aus der verwendeten Datei-/Download-App
   erlauben, falls Android danach fragt.
3. **Hunter Island · touchHLE** öffnen.
4. **Eigene IPA importieren** wählen und die eigene Datei im Dateidialog öffnen.
5. Bildformat und Mod-Schalter wählen, dann **Spiel starten**.

Updates mit derselben Projektsignatur können über die bestehende App
installiert werden. Ein eigener Neubau verwendet die eigene Signatur.
Vor einem Wechsel der Signatur den Spielstand exportieren. Android kann eine
APK mit anderer Signatur nicht über dieselbe bestehende Installation installieren.

Android bewahrt Import und Einstellungen im privaten App-Bereich auf.
Spielstände liegen im app-eigenen externen Dateienbereich. Eine Deinstallation
kann diese Daten entfernen. **Spielstand exportieren** erzeugt eine vom Nutzer
gewählte ZIP-Datei.

## Bildformate

Windows bietet iPad in 1024 × 768 und iPhone in 480 × 320, jeweils quer.
Android bietet zusätzlich experimentelles iPhone Retina in 960 × 640.
Das Spiel hat ein festes Seitenverhältnis. Breitere Handys zeigen deshalb
seitliche freie Flächen.

## Spielstände

Das Spiel schreibt normale Checkpoints an seinen vorgesehenen Stellen.
Beim Neuladen kann es deshalb in einer Stadt fortsetzen. Ein Emulator-Savestate
für eine laufende Kampfsitzung ist nicht implementiert.

Android kann mit **Spielstand importieren** kompatible Sicherungs-ZIPs übernehmen.
Vor einem Import legt die App eine lokale Sicherung des bisherigen Standes an.
Sie lässt sich über **Sicherung vor Import exportieren** herauskopieren.
Der Windows-Diagnoseexport kann den Spielstand auf ausdrückliche Auswahl beilegen
und erzeugt eine Android-kompatible ZIP. Der portable Windows-Starter bietet
gegenwärtig keinen Importknopf für Handy-Spielstände.

## Fehlerberichte

Version, Betriebssystem/Gerät, Bildformat, aktivierte Mods und die genauen
Schritte angeben. Diagnosepakete vor dem Teilen ansehen.
Android exportiert Diagnoseinformationen und Spielstand-ZIPs über getrennte
Schaltflächen. Windows kann auf ausdrückliche Auswahl Spielstände beilegen.
