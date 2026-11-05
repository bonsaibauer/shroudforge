# Automatische Beta-Spielprofile

Die optionale Beta-Erkennung vergleicht einen neuen Spielbuild mit einer gespeicherten, unterstützten Version. Ein vollständiges Beta-Profil entsteht nur bei ausreichenden Belegen. Passende reguläre Spielprofile haben Vorrang.

## Vor einem Spielupdate vorbereiten

Das Modul ist standardmäßig aktiviert und lässt sich ausschalten. Ein ausdrücklich gespeichertes Aus bleibt erhalten. Du findest den Schalter unter **Einstellungen → Allgemein → Spielprofil**, direkt bei der Profilauswahl.

1. Prüfe, ob **Dynamische Spielprofilerstellung (Beta)** aktiviert ist. Schalte sie bei Bedarf ein.
2. Starte das Spiel oder den Dedicated Server vollständig neu, solange noch ein passendes reguläres Profil vorhanden ist.
3. ShroudForge prüft die genaue aktuelle EXE und speichert sie als Vergleichsbasis im Cache. Du musst keine historische EXE herunterladen.
4. Nach einem späteren Spielupdate startet die Prüfung mit dem neuen Spielprozess automatisch und verwendet die zuvor gespeicherte Vergleichsbasis.

Client und Dedicated Server speichern ihre Vergleichsbasis getrennt. Ein Client ersetzt keine Serverbasis. Wenn das Modul erst nach einem Update aktiviert wird und keine passende Vergleichsbasis gespeichert wurde, kann es die frühere Version nicht nachträglich rekonstruieren.

## Was beim Spielstart geschieht

Die Analyse läuft während des Spielstarts. Das Spiel wartet auf ihr Ergebnis, bevor es weiterstartet. Dafür ist keine geladene Welt nötig.

Zuerst sucht ShroudForge ein reguläres Profil für die genaue laufende EXE. Ist eines vorhanden, verwendet die automatische Auswahl dieses Profil. Das aktivierte Modul hält dabei die unterstützte EXE für künftige Updates im Cache fest.

Fehlt ein passendes reguläres Profil, prüft das Modul die neueste geeignete reguläre Vergleichsbasis für denselben Prozess und dasselbe KFC-Format. Die zuvor automatisch gespeicherte EXE muss genau zu dieser Basis passen. Aus den Vergleichsbildern werden Code, Verweise, registrierte Typen und die benötigten Strukturverträge verglichen. Zieladressen müssen aus dem neuen Bild hervorgehen.

Das Ergebnis übernimmt den vollständigen bisherigen Profilvertrag einschließlich seiner Funktionsschalter. Offene oder widersprüchliche Belege verhindern die Veröffentlichung des gesamten Beta-Profils. Das Modul gibt dann den Grund in der Diagnose aus. Eine längere Wartezeit oder ein Weltbeitritt beseitigt fehlende statische Belege nicht.

## Speicherort und Auswahl

Reguläre und erzeugte Profile liegen im selben konfigurierten Profilverzeichnis. Bei den Standardpfaden sind das:

| Inhalt | Speicherort |
| --- | --- |
| Clientprofile | `shroudforge/runtime/profiles/enshrouded/client/` |
| Serverprofile | `shroudforge/runtime/profiles/enshrouded/server/` |
| Gespeicherte Vergleichsbilder und Belege | `shroudforge/cache/game-profile-discovery/` |

Erzeugte Profile beginnen mit `beta_`. Die Einstellung **Automatisch** kann ein vollständig geprüftes passendes Beta-Profil verwenden, wenn kein passendes reguläres Profil verfügbar ist. Die Profilauswahl zeigt die verfügbaren Profile an. Ein später hinzugefügtes passendes reguläres Profil hat Vorrang.

Änderungen am Schalter oder an der Profilauswahl erfordern einen vollständigen Neustart des betroffenen Spiels oder Servers. Ein erneutes Öffnen des Modloader-Fensters startet die Runtime nicht neu.

## Suche, Profil und laufende Welt unterscheiden

Der orange Hinweis **Dynamische Profilsuche läuft** beschreibt eine laufende Profilprüfung. Unter **Ablauf und Hilfe** öffnest du diese Anleitung. Endet die Suche ohne passendes Profil, wird das Problem rot gemeldet. Der orange Hinweis **Experimentelles Spielprofil aktiv** nennt das tatsächlich ausgewählte Beta-Profil. Echte Runtimefehler werden weiterhin als Fehler angezeigt.

Die strukturelle EXE-Prüfung benötigt keine geladene Welt. Weltbezogene Zugriffe benötigen anschließend zusätzlich die jeweiligen laufenden Kontexte, Registrierungen und Sicherheitsprüfungen. Ein erzeugtes Profil bestätigt keine ausgeführten Spielbefehle oder sichtbaren Spieleffekte. Client und Server müssen auch bei Funktionstests getrennt geprüft werden.

Wenn kein Profil entsteht, lies den gemeldeten Grund und die technischen Details. Eine fehlende gespeicherte Vergleichsbasis, ein ungeeignetes Ausgangsprofil oder unaufgelöste Code- und Layoutbelege benötigen jeweils eine passende Grundlage oder weitere Analyse. Behalte bis dahin ein reguläres Profil beziehungsweise ein passendes ShroudForge-Update als mögliche Lösung im Blick.

## Ausführliches Protokoll

Die Debug Console zeigt die Discovery-Meldungen aus dem Hauptprotokoll unter der Quelle `game-profile-discovery`. Für alle einzelnen Vergleichs- und Rebase-Felder stellst du unter **Einstellungen → Allgemein → Protokollierung → Mindeststufe** die Stufe **Diagnose** ein. **Erweitert** zeigt die zusätzlichen Zusammenfassungen.

Der rote Runtime-Banner und der Logeintrag `[compatibility-check] check=runtime` werden aus demselben Runtime-Check erzeugt. Der bestehende UI-Refresh protokolliert den Zustandswechsel, bevor er denselben Status an den Modloader weitergibt. Die Debug Console wertet Warnungs- und Fehlerzeilen aus genau diesem Hauptprotokoll nach ihrer vorhandenen Popup-Einstellung aus.

Bei einem Fehlschlag nennt das Hauptprotokoll den konkreten Pfad zum aktuellen `*.v2.unresolved.json`- oder `*.v2.error.json`-Bericht. Bei Erfolg nennt es den Pfad zum `*.v2.proof.json`-Beleg und den vollständigen Speicherpfad des erzeugten `beta_*.json`-Profils. Die Berichte liegen im Discovery-Cache.

## Linux und Proton

Die Erkennung läuft im nativen Runtime-Modul und benötigt keine externen PowerShell- oder Python-Programme. Ein Live-Test dieses Ablaufs unter Proton steht noch aus.

Weitere Hinweise zu Runtime, Mod-API und Parser findest du unter [Engine](#wiki/engine).
