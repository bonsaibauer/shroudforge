# Projektrefactor: klare Produktgrenzen und ein gemeinsamer Ausführungsweg

Stand: 9. Oktober 2026. **Vorschlag für einen tatsächlichen Architekturumbau.**

Dieser Plan ersetzt den vorherigen Plan. Insbesondere war dessen Entscheidung falsch, `world-editor-ui` als eingebauten Fensteradapter zu behalten: Dieser Code besitzt auch Blueprintverwaltung, Screenshotzuordnung und eine zweite Oberfläche. Solche Mod-Sonderfälle müssen aus dem Produktkern verschwinden.

Das Ziel ist: **Eine Funktion hat einen fachlichen Besitzer. Eine Mod ist als Paket vollständig. UI, CLI und Dateibefehle verwenden dieselbe Ausführung. Konfiguration, Formulare und Referenz beschreiben denselben Vertrag.**

## 1. Befund und Prüfumfang

Das [Dateiinventar](refactor-file-inventory.csv) enthält 466 Einträge einschließlich zwei Submodulen. Darunter sind 234 eigene Quelltextdateien mit rund 60.000 Zeilen. Jede erfasste Datei hat einen vorgesehenen Besitzer, eine Behandlung und einen Zielpfad bzw. benannte Teilziele. Für Dateien sind SHA-256 und Zeilenzahl erfasst.

**Eine vollständige manuelle Zeilenprüfung ist nicht abgeschlossen.** Alle eigenen Textdateien wurden maschinell auf die erfassten Architekturverweise durchsucht. 28 Dateien wurden vollständig gelesen, 21 weitere in Abschnitten bzw. an ihren Übergängen. `review` und `review_detail` unterscheiden das ausdrücklich. Generierte Spielkataloge, Drittanbieterquellen und mitgelieferte DLLs sind ebenfalls nicht als manuell geprüft markiert. Bei DLLs fehlt hier der Quelltext. Die Befunde unten beruhen auf den benannten Implementierungen; sie sind keine Aussage über die Fehlerfreiheit des Gesamtprojekts.

Der Arbeitsbaum enthält bereits Änderungen an vielen dieser Dateien. Das Inventar beschreibt den während der Untersuchung erfassten Bestand. Vor einem Umbau werden Hash und aktueller Diff erneut abgeglichen; vorhandene Änderungen werden übernommen, nicht zurückgesetzt.

### Nachgewiesene Grenzverletzungen

| Befund | Konkrete Stelle heute | Konsequenz |
|---|---|---|
| World Editor ist Mod und eingebautes Produktfeature zugleich | `src/loader/Cargo.toml` importiert sein UI-Crate; `bootstrap.cpp::start_world_editor_ui/stop_world_editor_ui`; `workflow/main.rs` kennt `--world-editor-ui` | Mod-UI-Host und Paketdeklaration ersetzen die Editorzweige. |
| Der Produktbuild benötigt eine Datei einer konkreten Mod | `world-editor-ui/src/main.rs` bindet `mods/world-editor/icon.svg` per `include_bytes!` ein | UI/Icon aus dem installierten Modpaket laden. |
| Editorfachlogik steckt im Fensterbackend | `view_state`, `apply_screenshot`, `backup_cover`, `undo_screenshot`; feste Blueprint-/`editor-state.txt`-Pfade | Bibliothek, Coverzuordnung und Undo gehören zur Mod; nur allgemeine Fenster-/Bilddienste zum Host. |
| Zwei Implementierungen derselben Editoransicht | `world-editor-ui/ui/index.html` und Rust-`evaluate_script` mit `__sfFallbackRenderSignature`, Blueprintkarten und Screenshotmanager | Eine Oberfläche im Modpaket; kein zweiter Fachrenderer im Host. |
| Modaktionen verwenden verschiedene Regeln | `modloader-ui::queue_mod_action` prüft `groups.actions`; Editor-`dispatch_action` schreibt direkt `{modId,action,value}`; API-`dispatch_ui_actions` liest Dateien | Eine Befehlsdeklaration unabhängig von sichtbaren Buttons, ein Dispatcher. |
| Zwei Editoraktionen verwenden diesen Unterschied | 28 registrierte `on_action`-Namen, 26 in Gruppen; `newBlueprint` und `selectBlueprint` fehlen dort | Beide regulär deklarieren; UI ruft dieselbe API wie andere Bedienwege auf. |
| Serversteuerung ist unvollständig | 20 `control.actions` im Default, 12 im Headless-`SUPPORTED`; es fehlen `diagnostics`, `refresh`, `refreshMod`, `reloadSettings`, `removeMod`, `runModAction`, `searchCatalog`, `setNewsRead` | Gemeinsame Handler und ein vollständiger Befehlskatalog. |
| UI und Headless konkurrieren um dieselben Aufträge | Zwei `claim_*control_actions` verändern dieselben Felder. UI-Recovery setzt allgemeine `running`-Einträge auf `failed` | Ein Besitzer für Annahme/Ergebnisse; Fensterstart darf fremde Arbeit nicht als abgebrochen erklären. |
| GitHub-Updates doppelt implementiert | UI-`fetch_latest_github/parse_release_tag/is_update_available` und Updater-`headless_check_system_updates` | Ein Releaseclient; aktuell unterscheiden sich schon Konfiguration und Zeitlimits. |
| Commands-Modul ist ein Platzhalter | `modules/commands/src/main.rs` meldet nur Verfügbarkeit | Echter gemeinsamer Dispatcher und CLI. |
| Settingregeln mehrfach gepflegt | Default-JSON, Schema, `save_setting/save_settings`, React-Listen | Ein fachlicher Vertrag pro Besitzer; Defaults/Validierung/Formular daraus ableiten. |
| Package-Crate kennt einzelne Produktfeatures | `config.rs` bindet fremde Schemas per relativem Pfad ein und kennt `request_world_editor_module_settings`, `worldEditorSettings`, feste Fensternamen | Besitzer registrieren Verträge; allgemeine Speicherung kennt keine Editor-/Diagnose-/Parserdetails. |
| Website-Editor widerspricht Loaderformat | `website/app.js::validateManifests` lehnt `extended.mod.json.targets` ab, das Loader-Schema erlaubt es | Gemeinsames Schema statt manueller Feldlisten. |
| Website erfindet andere API-Namen | `website/tools/build.mjs::publicName` macht aus `io.*`, `buffer.*`, `loader.*` Namen unter `shroudforge.*`; `env/mod.rs::register` registriert sie als `io`, `buffer`, `loader`, `shroudforge::create` ergänzt keine solchen Tabellen | Tatsächlich aufrufbare Namen publizieren. Die Websiteprüfung erwartet einige umgeschriebene Namen sogar ausdrücklich. |
| Anwendungszeitpunkt von Settings wird geraten | `manifest_reader` liest rekursiv Lua; `infer_api_contract` setzt für alle Settings gemeinsam `live` oder `restart` aus Textmustern | Neue Pakete deklarieren dies je Setting; v1-Heuristik bleibt ausschließlich im Kompatibilitätsimport. |
| Parsergrenze ist nicht durchgezogen | Parser und API hängen direkt von `kfc` ab; API-Werte/Reflection verwenden KFC-Typen | Zusammenhängender Spieladapter; tatsächliche Typabhängigkeiten beim Umbau entfernen. |

Der Websitebefund wurde zusätzlich ausgeführt: Die vorhandene Funktion `validateManifests` aus `website/app.js` wurde mit den unveränderten World-Editor-Manifesten aufgerufen. Ergebnis: `extended.mod.json: unbekanntes Feld “targets”.` Das Loader-Schema enthält `properties.targets`. Diese Prüfung war lokal und hat keine Projekt- oder Benutzerdatei geändert.

## 2. Verbindliche Begriffe

| Begriff | Bedeutung | Erweiterungsregel |
|---|---|---|
| Anwendung / EXE | Prozessstart, Argumente, Zusammenstecken der Komponenten | Eine neue Ansicht erfordert nicht automatisch ein neues Modul oder eine neue EXE. |
| Core | Pakete, Mod-Lifecycle, Befehle, Einstellungen, Zustand, allgemeiner UI-Host | Keine Namen oder Fachdateiformate einzelner Mods. |
| Eingebautes Modul | Produktfunktion wie Updater, Logansicht, Diagnose; besitzt Fachbefehle, Settings und eigene Ansichten | Statische Registrierung bei der Anwendung. Ein `module.json` allein ist kein dynamischer Pluginloader. |
| Mod | Vollständiges Verzeichnis/ZIP mit Manifest, Lua, optionaler UI, Ressourcen und optionalem nativen Code | Paket installieren; keine neue Cargo-Abhängigkeit, Bootstrap-Sonderbehandlung oder globaler Config-Key. |
| Native Mod-Erweiterung | DLL innerhalb eines Modpakets mit dokumentiertem Lifecycle | OS-/ABI-Abhängigkeit und gegebenenfalls Neustartpflicht bleiben sichtbar. |
| Spieladapter | Enshrouded-Dateien, EXE-Erkennung, KFC, Profile, native Hooks, Spielthread | Nur öffentliche API delegiert hierher; Mods kennen keine internen Adressen oder Profile. |
| Öffentliche API / SDK | Echte Lua-Aufrufe, Paket-/UI-/Befehlsverträge, Werkzeuge und Beispiele | Jede Fähigkeit nennt Ziel, Phase, Parameter, Ergebnis und Verfügbarkeit. |
| Website / Editor | Verbraucher derselben SDK-Verträge und Formularkomponenten | Keine zweite Definition gültiger Felder, Capabilities und API-Namen. |

**World Editor bleibt eine normale Mod. Seine komplette Oberfläche und Fachfunktion gehören in dasselbe installierbare Paket. Fremde Mods erhalten dieselben Hostmöglichkeiten.**

## 3. Zielstruktur nach fachlichem Besitz

```text
src/
  apps/                         # CLI, Desktop, Updater-EXE, Runtime-FFI, Verdrahtung
  core/
    mods/                       # Paket lesen, planen, aktivieren, Zustand
    runtime/                    # Lua-Lifecycle, native Mod-DLLs, Berechtigungen
    control/                    # ein Befehlsweg, Annahme, Routing, Ergebnisse
    settings/                   # Werte, Validierung, Revisionen
    storage/                    # Pfade, atomisches Schreiben, Cache
    ui_host/                    # beliebige Modansicht; Fenster/Bridge/Dateidialog
  api/
    eml/                        # bestehende EML-Lua-Bindings
    shroudforge/                # SF-Bindings: Runtime, Befehle, UI, Settings
  modules/
    updater/                    # Releases, Katalog, Download, Installation, Queue, UI
    diagnostics/                # Diagnosefunktion und ihre Bedienung
    logs/                       # Loganzeige und deren Einstellungen
  games/enshrouded/
    assets/                     # Parser, KFC-Transaktion, Vorbereitung, Backup
    runtime/                    # native ABI-Anbindung und C++-Provider
    bootstrap/                  # Windows-Proxy/Startbrücke
    profiles/                   # geprüfte Buildprofile
mods/<id>/                      # vollständige Modpakete inklusive ihrer UI
eml-mods/<id>/                  # externe EML-Beispiele/Kompatibilitätsfälle
sdk/
  schema/                       # gemeinsame Paket-/Befehlsverträge
  lua/                          # tatsächliche öffentliche API-Deklarationen
  operations/                   # Operationen und Voraussetzungen
  web/                          # Bridgeclient, Settingscontrols, Modvorschau
  templates/                    # dieselben Pakete für CLI, Website und Doku
website/                        # Seiten und Autorenwerkzeuge als SDK-Verbraucher
tools/                          # Build, SDK-Erzeugung und Maintainerwerkzeuge
vendor/                         # gepinnte KFC-/JSON-Submodule
docs/                           # Anleitungen und datierte Untersuchungsnachweise
```

Diese Verzeichnisse sind Besitzergrenzen, **keine Forderung nach einem Cargo-Crate pro Unteraufgabe**. Crates trennen wir bei tatsächlichen Abhängigkeits-, Plattform- oder Artefaktgrenzen. Bestehende Crates dürfen vorübergehend Fassaden bleiben; interne Rust-Importe werden umgebaut.

Dateien heißen nach ihrer Arbeit: `github_releases.rs`, `install_mod.rs`, `library.lua`. Keine zusätzliche Kette `manager → service → handler → provider` für dieselbe Aktion. Kleine zusammengehörige Funktionen bleiben zusammen.

```text
Anwendungen → Core + Module + API + Spieladapter
Module      → Core-Verträge und SDK-Datentypen
API         → Core-Dienste und Spieladapter
Spieladapter→ gemeinsame Kontext-/Statusverträge, keine UI oder einzelne Mod
Core        → gemeinsame Datentypen, keine konkreten Mods oder Feature-UIs
Modpaket    → öffentliche API und eigene Paketdateien
Website     → SDK-Verträge, Vorschaukomponenten, versionierte Datenexporte
```

Die Anwendung verdrahtet Implementierungen ausdrücklich. Diagnose beobachtet beispielsweise den Runtime-Lifecycle über einen Anschluss; der Core importiert nicht das Diagnosemodul. Spielbezogene Typen dürfen nicht über allgemeine Lua-Helfer unbemerkt in den Core zurücklaufen. Diese Grenzen müssen kompilierbar hergestellt werden.

Eine wichtige echte Buildgrenze: `src/core` bleibt ohne WebView-Abhängigkeit. Der Fensterhost ist ein separates Paket unter `src/core/ui_host`, das den Core verwendet. Das Runtime-FFI-Paket unter `src/apps/game_runtime` hängt von Core, API und Spieladapter ab; das CLI-/Desktop-/Updater-Hostpaket unter `src/apps` darf zusätzlich UI-Host und eingebaute Featuremodule verwenden. Der Spielprozess braucht dadurch keinen konkreten Modfenstercode. Übrige Cratezusammenlegungen werden im jeweiligen Arbeitsauftrag mit ihren Verbrauchern umgesetzt.

Zur Laufzeit gibt es ausdrücklich benannte Rollen: eine Control-Instanz je Installation, eine Runtime je Spielprozess, einen allgemeinen Fensterhost bei Bedarf und den vorhandenen Installationsworker für Arbeit über das Spielende hinaus. Der Control-Host kann als Modus von `shroudforge.exe` starten und besitzt eine Installationssperre; keine zusätzliche EXE pro Mod. Paketdeklarationen bestimmen, welche Modansichten angelegt werden. Fensterstart, Controllerstart und Modaktivierung sind getrennte Ereignisse.

## 4. World Editor als vollständige Mod mit eigener UI

### Paket und Funktionsbesitz

```text
mods/world-editor/
  mod.json                      # EML-Identität und bestehende Capabilities
  extended.mod.json             # SF-Vertrag: Settings, Befehle, Ansicht, Ziele
  icon.svg
  src/
    mod.lua                     # Lifecycle und Registrierung
    commands.lua                # alle Aktionen, unabhängig von sichtbaren Buttons
    selection.lua               # Auswahl und Cursor
    library.lua                 # Format, Laden/Speichern, Namen, Bibliothek
    covers.lua                  # Screenshotzuordnung und Cover-Undo
    paste.lua                   # Platzierungsablauf
    undo.lua                    # fachliche Rücknahme und Konfliktprüfung
    building_plan.lua           # vorhandene Rezept-/Bauplanung
    game_building.lua           # vorhandener Spieleingabemodus
  ui/
    index.html
    app.js
    styles.css
  README.md
```

Die Aufteilung von `mod.lua` folgt den fachlichen Zuständen. Ein expliziter Editorzustand gehört der Modinstanz und wird den Funktionen übergeben. Capture-/Paste-/Undo-Zustand wird nicht als verstreute globale Variablen auf Dateien verteilt.

| Heute | Ziel |
|---|---|
| `world-editor-ui/ui/index.html`, `styles.css` | Modpaket; JavaScript aus HTML/Rust in eine `ui/app.js` vereinigen |
| `view_state` und `editor-state.txt`-Parser | Mod veröffentlicht strukturierten Zustand. Host interpretiert keine Blueprintdateien oder Editor-Textschlüssel. |
| `apply_screenshot`, `backup_cover`, `undo_screenshot` | Fachablauf nach `src/covers.lua`; allgemeine Bilddekodierung und Thumbnailerzeugung als Host-API |
| Blueprintnamen, Bibliothekspfade, Zuordnung von Steam-Screenshots | Modbibliothek; allgemeine Dateiauswahl/Spielmedien-Funktion nur soweit tatsächlich wiederverwendbar |
| Fensterposition, WebView, Hotkeys/Fokus, Stop-Ereignis | `core/ui_host`, parametrisiert durch Ansicht und Settings |
| `dispatch_action`, spezielle DOM-Events, Tab-getrennte Payloads | Gemeinsamer Bridgeclient mit strukturierten Parametern und Ergebnis-ID |
| Bootstrap-Editorstart, Cargo-Abhängigkeit, eingebettetes Modicon | Entfallen nach Migration zum generischen Host |
| `modules.worldEditor`, `worldEditorSettings`, feste Window-Schemafelder | Modsettings und `modId + viewId`; alte Werte ausdrücklich übernehmen |

### Der fehlende allgemeine Vertrag

Dafür muss ein **neuer dokumentierter Mod-UI-Vertrag implementiert** werden. Zum Funktionsumfang gehören:

1. Paketrelative UI-Einstiege/Ressourcen in Verzeichnis und ZIP. Geladen wird die installierte Paketversion.
2. Ansichts-Lifecycle: öffnen, schließen, Mod deaktiviert/aktualisiert, Spiel beendet. Eine entladene Mod kann nicht über ein altes Fenster weiter aufgerufen werden.
3. Gemeinsame Befehle mit Parametern, Ergebnis, Fortschritt, Fehler und Abbruch.
4. Modzustand mit Revision, lesbar für UI und Status-/Diagnoseabfrage.
5. Allgemeine Dateiauswahl, paketbezogene Daten und Bildkonvertierung. Die Mod bestimmt die Zuordnung eines Bildes zu einem Blueprint.
6. Identität vom Host: Ein Fenster erhält Mod-/Ansichtsidentität aus dem geöffneten Paket. Ein frei gesendetes `modId` ersetzt sie nicht. Paketpfade bleiben im Paketbereich; externe Dateien werden ausdrücklich ausgewählt. Native DLLs bleiben nativer Prozesscode und sind keine vollständig sandboxed Lua-Funktion.
7. Einheitliche Hotkeyregistrierung mit Konflikt-/Fokusbehandlung. F2–F8 des Editors bleiben als Bedienfunktionen erhalten.

**Vertragsentwurf, heute noch nicht unterstützt:** Eine optionale SF-Erweiterung v2 deklariert Einstiegspunkte, Ansichten, Befehle und Anwendungszeitpunkte einzelner Settings. Die vier EML-Capabilitynamen in `mod.json` bleiben gültig; UI-Rechte werden nicht als unbekannte fünfte EML-Capability hineingeschrieben. Die genaue Schemaänderung entsteht in A3 zusammen mit Import, SDK und Beispielpaket.

```json
{
  "schemaVersion": 2,
  "targets": ["client"],
  "entrypoints": { "runtime": "src/mod.lua" },
  "views": {
    "main": { "entry": "ui/index.html", "kind": "window", "hotkey": "F2" }
  },
  "commands": {
    "selectBlueprint": {
      "execution": "mod-runtime",
      "input": {
        "type": "object",
        "required": ["name"],
        "properties": { "name": { "type": "string" } },
        "additionalProperties": false
      }
    }
  }
}
```

Dieses verkürzte Beispiel zeigt den Besitz. Das vollständige Schema ergänzt Standardaktivierung, Settings, Ausgabe-/Fehlerschema und Fähigkeiten. Ein Befehl ist unabhängig davon deklariert, ob das allgemeine Modloaderformular einen Button dafür zeigt. `shroudforge.ui.on_action` erhält einen klar benannten v1-Adapter zum selben Dispatcher.

Vorhandene Blueprints, Cover und Undo-Dateien bleiben lesbar. Einen nötigen Import des alten Editorzustands besitzt die Mod. Der Core bekommt keinen Editor-Altformatparser. Screenshotfunktion, Speicherschritte, Bibliothek, Auswahl, Rotation, beide Platzierungsmodi und Undo gehören zur Funktionsabnahme.

**Architekturbeweis:** World Editor als ZIP auf einem unveränderten generischen Loader installieren. Eine zweite kleine Mod mit eigener UI muss ebenfalls ohne Bootstrap-, Cargo- oder globale Configänderung funktionieren. Ohne installiertes Editorpaket gibt es keine Editoroberfläche im Host. Konkrete Modnamen dürfen dort höchstens in ausdrücklich benannten Migrationen und Testfixtures vorkommen.

## 5. UI, CLI und Dedicated Server: ein Befehl, eine Implementierung

```text
Modloader-Button ───────┐
Updater-Fenster ────────┤
Modansicht ────────────┤
CLI ──────────────────┼→ CommandRequest → Validierung/Router → zuständiger Handler
Dateiauftrag am Server ┘                          │                      │
                                          CommandResult ← Fortschritt/Ergebnis
```

`core/control` besitzt Annahme, Zuordnung und Ergebnisse. Das Fachmodul besitzt die Ausführung. Der gemeinsame Vertrag enthält: stabile ID, Besitzer, Eingabe-/Ausgabeschema, Ausführungsort, Voraussetzungen an Prozess/Spielzustand, Berechtigung, Abbruch und Wiederholungsverhalten. Die Registrierung verbindet ihn mit genau einem Handler. Derselbe Datensatz speist CLI-Hilfe, UI-Aktionen, Dateivorlagen und Dokumentation.

Geplante CLI, heute noch nicht vorhanden:

```text
shroudforge commands list --root <installation>
shroudforge commands describe updater.check --root <installation>
shroudforge command updater.check --root <installation> --args-file request-args.json
shroudforge command mod.world-editor.selectBlueprint --instance <id> --args-file selection.json
```

Bei reinem Dateizugriff wird derselbe Auftrag atomar abgelegt:

```json
{
  "schemaVersion": 1,
  "requestId": "admin-20261009-001",
  "command": "updater.check",
  "target": { "installation": "server-installation" },
  "arguments": {}
}
```

Die Umsetzung muss folgende Punkte lösen:

- Ein Controller pro Installation besitzt die Annahme. Sperre und Besitzerkennung verhindern konkurrierende UI-/Headless-Controller. Der allgemeine Host startet ihn; kein Fenster oder Updater besitzt die übrige Administration.
- Offlineaktionen laufen ohne Spiel und WebView. Runtimebefehle gehören zu einer aktiven Spielinstanz. Bei mehreren passenden Instanzen muss das Ziel angegeben werden.
- Instanzidentität enthält Prozessziel, PID und Startkennung. Ergebnisse einer alten Sitzung dürfen nicht in eine neue geraten.
- Lua läuft im vorgesehenen Lifecycle; native Weltoperationen bleiben auf dem Spielthread. Ein Dateipoller ruft keine Spieloperation direkt auf.
- Annahme, Ausführung und fachlicher Abschluss sind eigene Zustände. Workerübergabe ist noch keine erfolgreiche Installation.
- Request-IDs verhindern doppelte Annahme. Bei einem Absturz garantieren sie allein keine genau einmal erfolgte Nebenwirkung. Handler stellen anhand ihres Journals wieder her oder melden einen unklaren Ausgang; nicht idempotente Spielaktionen werden nicht automatisch erneut gesendet.
- Alte `control.actions.<name> = true` übersetzt **ein** v1-Adapter. UI und Updater lesen die Booleschen Schalter danach nicht selbst.
- Alle bisherigen 20 Aktionen erhalten einen gemeinsamen Handler. UI-/CLI-/Dateizugriff verwenden dieselben Regeln für Aktionen, die auf dem Ziel tatsächlich möglich sind. Ein Dedicated Server bekommt dadurch keinen erfundenen lokalen Cursor oder Dateidialog.

Damit entfallen getrennte Headless-, UI-Control- und Mod-Sonderlisten. Eine Aktion darf wegen ihres Zielkontexts ungeeignet sein, aber nicht wegen einer fehlenden Kopie ihres Handlers im Servercontroller.

## 6. Konfiguration: derselbe Wert und dieselbe Regel überall

„1:1“ heißt: Jeder bearbeitbare UI-Wert hat einen eindeutigen Configpfad und denselben Typ, dieselben Grenzen, denselben Standardwert und Anwendungszeitpunkt. Jeder Button hat einen eindeutigen Befehl. Jeder Statuswert kommt vom ausführenden Besitzer.

| Sache | Besitzer | Speicherung/Anzeige |
|---|---|---|
| Einstellung | Fachmodul/Mod deklariert; Core validiert und speichert Werte | Config → effektiver Wert → dasselbe Formular; CLI/Datei verwenden dieselbe Validierung |
| Aktion | Deklarierter Fachbefehl | Request mit ID und Argumenten über Button, CLI oder Datei |
| Ergebnis/Fortschritt | Ausführender Handler | Schreibgeschützter Status; keine als Einstellung getarnte aktuelle Releaseantwort |
| Fensterpräferenz | Allgemeiner UI-Host pro Ansicht | Mod-/Viewidentität; kein globales `worldEditor`-Sonderfeld |

Ein Modul hält seine Beschreibung bei sich, etwa `src/modules/updater/contract.json`. Daraus entstehen Defaults, der entsprechende Abschnitt des Gesamtschemas, UI-Metadaten und Hilfe. Handgeschriebene Ausführung liegt in `commands.rs`; generierte IDs verbinden Vertrag und Handler. Zusätzliche Regeln zwischen mehreren Feldern gehören zum selben fachlichen Besitzer und werden von allen Eingängen verwendet.

Die bestehenden Configpfade bleiben zunächst erhalten, etwa `/modules/updates/system/...`. Der Vertrag nennt den Pfad ausdrücklich; die UI verwendet denselben. Ein anderes Wording erfordert keine gleichzeitige Umbenennung installierter Benutzerdateien. Eine spätere Formatänderung hat eine Version und Migration.

Für Mods werden Paketdefaults und Benutzerwerte getrennt. Das Paket liefert Definitionen; ein benutzerseitiger Store hält Aktivierung/Werte. Ein Update kann dadurch keine neuere Nutzereinstellung beiläufig überschreiben. Bestehendes `extended.mod.json` v1 wird nachvollziehbar importiert. Weitere externe v1-Bearbeitung braucht während der Übergangszeit eine festgelegte Konfliktregel und Revision; zwei gleichrangige beschreibbare Wahrheiten sind unzulässig.

Neue Deklarationen enthalten `apply = live | next-start` je Setting. Laufzeitänderungen quittieren gewünschte und angewendete Revision. Die alte Lua-Erkennung bleibt nur im v1-Import, vergibt keine Capabilities und ist keine allgemeine Planungslogik. Validierungsfehler, Defaults und notwendige Neustarts stimmen zwischen UI, CLI und Status überein. Core-Speicherung enthält dafür keine Modnamen oder Listen konkreter Featurefelder.

## 7. Updater als zusammenhängendes Fachmodul

```text
src/modules/updater/
  mod.rs                        # fachlicher Einstieg
  contract.json                 # Settings und öffentliche Befehle
  commands.rs                   # check, enqueue, start, cancel, install
  github_releases.rs            # GitHub lesen, Tags/Builds vergleichen, Release wählen
  shroudedit_catalog.rs          # Modkatalog und Downloadmetadaten
  download.rs                   # Download, Prüfsumme, Abbruch
  queue.rs                      # System-/Modupdate-Queue und Zustände
  install_system.rs             # Systemdateien, Backup, Rollback
  install_mod.rs                # Paket, Nutzerwerte, Runtime-Unload/Reload, Rollback
  worker.rs                     # Prozessübergabe und Lebensdauer der Installation
  ui/
    UpdaterPanel.tsx             # gemeinsame Ansicht der Queue
    styles.css
```

`src/apps/updater.rs` besitzt EXE-Argumente und Startmodus. Desktop und kompaktes Updaterfenster binden dieselbe `UpdaterPanel` ein. Releaseprüfung, Queuezustände und Regeln kommen vom selben Fachmodul.

| Auftrag | Hauptdatei | Wirkung auf andere Eingänge |
|---|---|---|
| GitHub-Releaseauswahl ändern | `github_releases.rs` | Beide Ansichten und Server erhalten dasselbe Ergebnis |
| Katalogpaket laden | `shroudedit_catalog.rs`, `download.rs` | Gemeinsamer Downloadauftrag/Fortschritt |
| Systemdateien ersetzen | `install_system.rs` | Gleicher Worker-/Recoveryvertrag |
| Modpaket ersetzen | `install_mod.rs` | Paketstore und Runtime-Lifecycle, kein WebView erforderlich |
| Updatezeile darstellen | `ui/UpdaterPanel.tsx` | Vorhandener Status-/Befehlsvertrag |
| Verhalten der EXE ändern | `apps/updater.rs`, bei Übergabe `worker.rs` | Bestehende Argumente/Exitcodes berücksichtigen |
| Einstellung ergänzen | `contract.json` plus gegebenenfalls Fachhandler | UI, Schema und Hilfe entstehen aus demselben Feld |

Zusammengeführt werden `fetch_latest_github` und `headless_check_system_updates`. Aus dem UI wandern `fetch_catalog`, `download_catalog_version`, `install_mod_from_catalog`, `replace_catalog_mod`, Katalogregistrierung und ZIP-/Hashprüfung zum Updater. Allgemeine atomische Speicherung/Paketpfadprüfung kommt aus dem Core; Releasefachregeln bleiben im Modul.

Die Updatequeue bleibt fachlich beim Updater. Sie ist nicht die allgemeine Befehlsannahme: `updater.start` startet gewählte Queueeinträge und liefert deren Job-IDs. Ein Queuebesitzer führt die Zustandsübergänge aus. Der Dispatcher erfindet keine zweite Updateplanung.

## 8. Laden, Phasen, Prozessziele und Multiplayer

### Genau ein Ladeplan

1. Anwendung legt Installation und Prozessrolle fest. Client/Server werden nicht an mehreren Stellen mit unterschiedlicher EXE-Priorität erraten.
2. Core entdeckt Verzeichnis-/ZIP-Pakete und importiert jedes Format in ein normalisiertes Modell. Revisionen erlauben Caching; Statuspolling durchsucht nicht erneut sämtliche Lua-Dateien.
3. Planer prüft Aktivierung, Ziel, Version, Abhängigkeiten und Konflikte. Ergebnis: Plan mit Gründen je Paket und getrennten Aufgaben für Assetphase, Runtime und Ansichten.
4. Assetphase läuft im vorgesehenen Startupfenster mit Backup/Lock/Transaktion. Runtime-Mods mit zusätzlichem `export` werden dadurch nicht zu Assetmods.
5. Native Runtime wird für den tatsächlichen Gamebuild eingerichtet. Core erzeugt Lua-Umgebungen; API wird registriert; die vorgesehenen Mod-Lifecycle-Einstiege laufen.
6. UI-Host erhält die Ansichten aktiver, geeigneter Pakete. Der Dedicated Server lädt keine WebView wegen einer installierten Mod.
7. Deaktivieren/Update/Beenden folgt demselben Mod-Lifecycle: Befehle stoppen, Ansichten schließen, Arbeit abschließen/abbrechen, Ressourcen freigeben. Native DLLs behalten ihre tatsächlichen Unload-/Neustartgrenzen.

| Begriff | Aussage | Daraus nicht ableitbar |
|---|---|---|
| Prozessziel `client` / `server` | EXE und Modziel | Netzwerkautorität oder automatische Übertragung |
| Phase `pregame` / `ingame` | Vorgesehene Lebensphase und Zugriffsarten | Eine konkrete Welt ist schon verfügbar |
| Capability | Was die Mod deklariert und verwenden darf | Profil/Provider/Operation sind gerade bereit |
| Buildunterstützung | Operation für diese EXE geprüft | Aktueller Cursor oder benötigtes Spielobjekt existiert |
| Welt-/Netzwerkkontext | Beobachteter Kontext, gegebenenfalls `unknown` | Singleplayer/Host allein aus `is_client` |
| Ausführungsbestätigung | Ein definierter Schritt wurde ausgeführt | Serverannahme, Replikation und Spielstandspeicherung ohne entsprechenden Nachweis |

Singleplayer und lokales Hosting bleiben Clientprozesse. Ein beigetretener Client hat denselben Prozessnamen. Der Editor-Spieleingabepfad bleibt eine normale Engineeingabe mit eigenen Annahme-/Beobachtungsgrenzen; er ist keine neue allgemeine Mod-RPC-Verbindung. Direkte Weltänderung und Spieleingabe bleiben verschiedene Operationen.

### Capabilities und tatsächliche Verfügbarkeit

Heute verteilen sich Prüfungen über Manifest, Runner, `available`, `operation_status`, `runtime_denial_reason`, GameContract und native Readiness. Künftig besitzt jede Stelle einen klaren Fakt:

- Paketmodell: deklarierte Fähigkeiten und Prozessziele.
- Planer: für diese Instanz vorgesehen oder mit stabilem Fehlercode ausgeschlossen.
- Gemeinsame Aufrufprüfung: deklarierte Fähigkeit, aktive Mod, Phase, Prozess-/Weltkontext.
- Spieladapter: Buildunterstützung, aktueller Provider, benötigte Objekte und Spielthreadausführung.

Ein strukturiertes Ergebnis nennt Unterstützung, Berechtigung, Bereitschaft und Grund. `has`, Aufrufprüfung, UI und CLI verwenden daraus ihre zugesicherten Aussagen. Bestehende Lua-v1-Rückgaben erhalten explizite Wrapper; unterschiedliche bisherige Bedeutungen von „supported“ und „ready“ dürfen nicht stillschweigend gleichgesetzt werden. Nativer Zustand wird bei der Ausführung erneut geprüft; ein zuvor aktivierter Button ersetzt das nicht.

## 9. API, KFC, Website, Editor und Dokumentation

`src/api` implementiert die öffentliche Lua-Oberfläche; `sdk/lua` enthält echte Deklarationen/Beispiele. Der Spieladapter besitzt KFC-Dateien, Reflection, Layouts und ABI-Aufrufe. Core besitzt den Lebenszyklus der Modinstanz.

```text
Mod-Lua → öffentliche API → gemeinsame Berechtigungs-/Kontextprüfung
        → Assetadapter ODER Runtimeadapter
        → Parser/KFC-Datei ODER nativer Spielthread
```

Die direkte KFC-Verwendung in heutigen API-Werten wird tatsächlich herausgelöst: Spielbezogene Konvertierungen gehören in API/Spieladapter; generische Lua-VM, Paketplan und UI-Host hängen nicht von KFC-Implementationstypen ab. Native Symbollader aus `loader.rs`, `runtime_building.rs` und weiteren Bindings werden im Spieladapter vereinigt. C-ABI und Profilprüfung bleiben erhalten.

EML ist ein unterstützter Import-/API-Vertrag. Native DLLs, `native-plugin.ini`, Archivextraktion und verzögertes DLL-Laden bleiben kompatibel. Benannte Adapter ersetzen implizite Sonderwege; funktionierende EML-Mods werden nicht als Altlast entfernt.

| Heute getrennt | Ziel |
|---|---|
| Loader-Schema und manuelle Website-Feldlisten | Ein versioniertes Schema; Rust und Browser validieren denselben Vertrag |
| Modloadercontrols und Website-Mockup | SDK-Webkomponenten mit Transportadapter: Vorschauzustand im Editor, echte Befehle im Loader |
| CLI-Vorlage, Website-ZIP und `templates/mod` | Ein Templatebestand für alle Paketgeneratoren |
| Website-Aliastabelle und Lua-Registrierung | Tatsächlich aufrufbarer Name als Identität; Anzeigenamen verändern keinen API-Namen |
| Einige erwartete Symbole in `validate.mjs` | Vollständiger Abgleich registrierter öffentlicher Symbole mit Referenz und ausführbaren Beispielen |
| API-Liste und verstreute Anleitung | Pro Operation Voraussetzungen, Parameter, Ergebnis, Fehler, Beispiel und Grenzen |
| Aktuelle API und eingefrorene Spieltypen | Getrennte versionierte Datensätze mit Gamebuild, Prozessziel und Erzeugungsstand |

Verträge erzeugen Typen, Standardformulare, Hilfe und Referenztabellen. Fachlogik wird nicht durch ein neues universelles Schemasystem ersetzt. Beispiele und Anleitungen werden beim Besitzer gepflegt und gegen echte Aufrufe geprüft.

Der gemeinsame Beispielbestand enthält: vollständiges Basispaket, Assetmod, Runtimemod, Mod mit eigener UI, parametrisierte Aktion und native EML-Erweiterung. Jedes Beispiel erklärt Dateien, Ladezeitpunkt, Client-/Server-Eignung, Voraussetzungen, erwartetes Ergebnis und Fehlerdiagnose. World Editor ist ein umfangreicher Verbraucher; eine kleine UI-Beispielmod bleibt zusätzlich nötig.

Der Website-Editor unterstützt sämtliche Vertragsfelder einschließlich Prozesszielen, Settings, Aktionen, Ansichten und Paketdateien. Vereinfachte Formulare erhalten weitere unterstützte Daten beim Import/Export verlustfrei. JSON-Ansicht und Formular bearbeiten denselben Zustand.

## 10. Datei- und Funktionszuordnung

Das CSV bildet jede erfasste Datei ab. `split` bezeichnet einen tatsächlichen Import-/Aufrufumbau. Die folgenden Aufträge präzisieren die großen Dateien. Kurzformen `apps/core/api/modules/games` liegen unter `src/`.

| Quelle heute | Ziel | Aufgabe |
|---|---|---|
| `workflow/main.rs` | `apps/cli.rs`, `desktop.rs`, `compose.rs` | Argumente und Verdrahtung, keine Featurefachlogik |
| `workflow/lib.rs` | `apps/game_runtime/lib.rs`, `core/runtime/lifecycle.rs` | FFI und Modsession |
| `workflow/pregame.rs` | `games/enshrouded/assets/prepare.rs`, `recovery.rs` | Assetablauf mit explizitem Paketplan |
| `package/env.rs`, `registry/mod.rs`, `registry/fs.rs` | `core/mods/{discover,plan,registry,files}.rs` | Ein Paket-/Planmodell für alle Verbraucher |
| `package/registry/manifest_reader.rs` | `core/mods/import.rs`, `compat/eml_v1.rs`, `compat/extension_v1.rs` | Formatimport und isolierte Altheuristik |
| `package/config.rs` | `core/settings/store.rs`, `core/storage/json.rs`, `core/control/legacy_config.rs`, `core/ui_host/state.rs` | Werte, atomisches Schreiben, Altbefehle, allgemeine Fensterzustände |
| `package/status.rs` | `core/mods/status.rs`, `apps/desktop/snapshot.rs` | Fachstatus einmal ermitteln und für Darstellung zusammenstellen |
| `api/lib.rs`, `runner/*` | `core/runtime/session.rs`, `runner/*`, `api/lib.rs`, `games/enshrouded/assets/execute.rs` | Runtime, Fassade und Assetausführung |
| `api/env/app_state.rs` | `core/runtime/context.rs`, `native_plugins.rs`, `games/enshrouded/assets/context.rs` | Mod-/DLL-Lifetime und KFC-Zustand |
| `api/env/loader.rs` | `api/shroudforge/{runtime,ecs,world,patch}.rs`, `core/runtime/permissions.rs`, `games/enshrouded/runtime/bindings.rs` | Lua-Bindings, gemeinsame Prüfung und ABI-Anbindung |
| `api/shroudforge/v1/shroudforge.rs` | `api/shroudforge/{settings,commands,input,notifications}.rs` | Allgemeine Hostfunktionen; Aktionen über Core |
| `api/env/game/*`, Buffer-/Image-/Integer-/IO-Bindings | `api/eml/` mit bisherigen Fachgruppen | EML-Wertsemantik; Spielkonvertierungen ausdrücklich halten |
| `api/runtime_resolution/*` | `games/enshrouded/runtime/resolution/*` | Spieltypen und Funktionsauflösung |
| `parser/src/*`, API-KFC-Ladecode, `package/backups.rs` | `games/enshrouded/assets/` | KFC, Transaktion, Backup, Export |
| `compatibility/src/lib.rs` | `games/enshrouded/compatibility.rs` | Buildunterstützung; Modkonflikte bleiben getrennt im Core |
| `runtime/native/*`, `profiles/*` | `games/enshrouded/runtime/native/*`, `profiles/*` | Zusammenhängende native Verantwortungen |
| Modloader-UI-Backend | `apps/desktop`, `core/{control,settings,mods,ui_host}`, `modules/updater` | Fachhandler aus der Fensterereignisschleife herauslösen |
| Modloader-`main.tsx` | Desktopnavigation, Featurepanels, `sdk/web` | Darstellungen beim Besitzer; gemeinsame Controls/Vorschau |
| Updater-`main.rs` | Abschnitt 7 und allgemeiner Controller in `core/control` | Ein Updatefeature, ein allgemeiner Befehlsdienst |
| Editor-UI-Crate und Editor-Mod | Abschnitt 4 plus allgemeine Hostdienste | Vollständig installierbare Mod |
| `modules/commands` | `core/control`, CLI | Statusplatzhalter ablösen |
| Debug Console und Diagnose | `modules/logs`, `modules/diagnostics` | Fachzustand und Ansicht zusammen; Fenstertechnik gemeinsam |
| `website/app.js` | `website/{editor,reference,navigation}.js`, `sdk/web` | Autorenwerkzeug, Referenz, Navigation; Doppelregeln entfernen |
| `build.ps1`, API-/Websitegeneratoren | stabiler Root-Einstieg, `tools/build`, `tools/sdk` | Explizite Artefakte und gemeinsamer Vertragsbuild |
| Profiletools/Untersuchungen | `tools/enshrouded/` | Maintainercode außerhalb der ausgelieferten Runtime |
| Kleine eigene Mods und EML-Pakete | bleiben je ein vollständiges Paket | Keine künstliche Zerlegung kleiner zusammenhängender Mods |

Rust-Integrationstests bleiben als solche im Cargo-Testharness registriert. Build-, Include-, Fixture- und Websitepfade sind Teil jedes Umzugs. Die C++-Game-Unterbereiche sind bereits nach ECS/World/Patch/Dispatcher aufgeteilt; ihre Implementierungen werden fachlich geprüft, aber nicht allein wegen Zeilenzahlen in neue Schichten zerlegt.

KFC- und JSON-Submodule werden als gepinnte Abhängigkeiten behandelt. KFC-Crate und Proxybuild sollen denselben Checkout konsumieren; bisher existieren Git-Cargo-Abhängigkeit und Submoduleinstieg für unterschiedliche Verbraucher. Die Vereinheitlichung erhält den Commit und aktualisiert Workspaceausschlüsse und Lockdatei. Fremdcode wird nicht als eigener Altcode gelöscht.

## 11. Arbeitsaufträge mit Ergebnis und Löschung

Zuerst belastbare Schnittstellen umsetzen, dann vollständige Verbraucher migrieren, anschließend alte Wege entfernen. Jede Stufe bleibt baubar. Ein Vertrag wird mit seinem ersten echten Verbraucher geliefert.

### A0 — Bestand und Funktionen sichern

Inventar, aktueller Diff, vorhandene Tests und Releaseverträge in einen reproduzierbaren Quellstand einschließlich offener Änderungen übernehmen. Funktionsmatrix und Datenkopien für Update-/Mod-/Configmigration erstellen. Ein Snapshot genügt; ein geänderter Arbeitsbaum ist kein Grund, den Refactor insgesamt zu verweigern. Keine aktive Installation für Strukturtests verwenden.

**Abnahme:** Alle vorhandenen Änderungen enthalten; vorherige Testfehler und fehlende Spielnachweise ausdrücklich dokumentiert.

### A1 — Gemeinsame Befehle ausführen

Beide `claim_*control_actions`, UI-Handler, `dispatch_ui_actions` und Commands-Platzhalter in `core/control`, CLI und Befehlsvertrag überführen. Zuerst harmlose Abfrage und Modaktion vollständig von UI/CLI/Datei bis zum selben Ergebnis; danach alle 20 Altaktionen.

**Entfällt:** Zweiter Controller, UI-Aktionsnamensliste, Headless-`SUPPORTED`-Sonderliste. Ein v1-Configadapter bleibt einziger Übersetzer.

**Abnahme:** Gleiche Aktion ohne geöffnete UI; eindeutige Runtimeinstanz; UI-Neustart verwirft keine fremde Arbeit.

### A2 — Updater und Katalog aus der UI lösen

Beide Releaseclients, Katalog-/Installerlogik und Queue/Worker nach Abschnitt 7 zusammenführen. Alle Eingänge verwenden sofort denselben Handler. Vorhandene Updatezustände mit gemeinsamen Befehls-/Jobresultaten verbinden.

**Entfällt:** UI-GitHub-Client, UI-Kataloginstaller, doppelte Versionsauswahl, allgemeiner Configcontroller im Updater.

**Abnahme:** System-/Modupdates, Prüfsummen, Auswahl, Warten auf Spielende, Downloadabbruch, Installation, Rollback und beide Fenster erhalten. EXE-/Worker-Argumente bleiben über benannte Adapter erreichbar.

### A3 — Modvertrag für UI/Befehle bauen

EML-/SF-Schema, Modaktionen, Settings und Paket-FS zu Erweiterung v2 und normalisiertem Modmodell ergänzen. SDK-Bridge und `sdk/templates/mod-with-ui` gemeinsam liefern. Entwurf aus Abschnitt 4 präzisieren: Ergebnisse, Lifecycle, Fähigkeiten, Wertebesitz, ZIP/Verzeichnis.

**Entfällt:** Keine aktive Altformatunterstützung; sie bleibt im Importadapter.

**Abnahme:** Ein Modautor installiert das Beispiel, ändert seine UI-Datei und ruft eine parametrisierte Aktion auf, ohne Produktbuild.

### A4 — Allgemeinen UI-Host implementieren

Wiederverwendbare WebView-/Fenster-/Dateidialog-/Bildtechnik nach `core/ui_host` bringen. Paketidentität, Ressourcen, Zustandsabonnement, Befehlsantwort und Unload zusammen implementieren. Alte Fenster delegieren, sobald ihr benötigter Umfang vorhanden ist.

**Entfällt:** Parallel gepflegte allgemeine Fenstertechnik; kein neuer Ersatz-Fachrenderer.

**Abnahme:** Verzeichnis-/ZIP-Modansichten, Update/Unload, korrekte Modidentität und Pfadbegrenzung; keine Serverfenster.

### A5 — World Editor vollständig herauslösen

Gesamtes Editor-UI-Crate, Mod und Bootstrap-/Config-/Schemaspezialfälle nach Abschnitt 4 migrieren. Bibliothek, Cover, Zustandsmodell und UI werden Modcode; generische Bild-/Dateidienste übernehmen native Hilfsarbeit. Bestehende Nutzerwerte und Blueprintdateien übernehmen.

**Entfällt:** `shroudforge-world-editor-ui`, Bootstrap-Editorprozess, eingebettetes Icon, `worldEditorSettings`, globaler Editor-Settingsabschnitt nach Migration, Editor-Dateiformatparser und zweiter DOM-Renderer im Host.

**Abnahme:** Sämtliche bisherigen Aktionen, Screenshot-Undo, Save-Fortschritt und beide Platzierungsmodi. Zweite fremde Mod-UI ohne Produktsonderbehandlung. Vorhandene unbewiesene Multiplayerwirkungen werden nicht als nachgewiesen dargestellt.

### A6 — Settings, Werte und Status vereinheitlichen

Default-/Schema-JSON, `save_*settings`, Frontendfelder und Statusberechnung in Besitzerverträge, `core/settings` und SDK-Controls überführen. Benutzerwerte aus Modpaketen versioniert importieren.

**Entfällt:** Weitere UI-Whitelists, Fensterdefaults, eigene Settingstypauswertung und doppelte Statusentscheidungen.

**Abnahme:** Jeder öffentliche Wert hat überall denselben Pfad/Typ/Default/Grenzen/Anwendungszeitpunkt. Paketupdate erhält Nutzerwerte; ungültige alte Werte bleiben diagnostizierbar.

### A7 — Ladeplan und Lifecycle zusammenführen

Packageplan, Pregame, `IngameRuntime`, Runner/AppState, DLL-Manager und Reload-Dateien in `core/mods`, `core/runtime` und Assetvorbereitung umsetzen. Asset-, Runtime- und Viewaufgaben kommen aus einem Plan mit getrennten Ausführungszuständen.

**Entfällt:** Wiederholte Ziel-/Planentscheidungen an UI-/CLI-Einstiegen, Lua-Heuristik außerhalb v1-Import, modbezogene Sonderstarts.

**Abnahme:** Client/Server, Abhängigkeiten, Live-Settings, Deaktivierung/Update, Recovery, native Neustartfälle. Runtime+Export startet keinen ungeeigneten Pregame-Lifecycle.

### A8 — API und Spielintegration technisch trennen

`api/lib.rs`, `env/loader.rs`, AppState, KFC-Spielwerte, Symbollader und Compatibility in API, Core-Policy und Spieladapter aufteilen. Generische Lua-Infrastruktur von spielbezogenen Userdata-Konvertierungen lösen.

**Entfällt:** Doppelte Availabilityentscheidungen und Symbollader, implizite Abhängigkeit allgemeiner Dienste von Feature-UI.

**Abnahme:** Lua-Aufrufe und native ABI erhalten; Core ohne KFC-Implementationstypen; Capability-/Phasen-/Providerstatus erklärt auch Ablehnungen.

### A9 — Website und Autorenwerkzeuge ans SDK anbinden

Editor, API-Aliasse, Schema-Kopien, Modloadercontrols und alle Vorlagen auf den gemeinsamen Vertrag/Komponentenbestand umstellen. `targets`, Ansichten und UI-Dateien gehören zum Autorenwerkzeug.

**Entfällt:** Manuelle Schema-Whitelist, separat nachgebaute Modvorschau, virtuelle API-Umbenennungen, unterschiedliche Starterpakete.

**Abnahme:** Loader/Editor validieren dieselben Pakete gleich; Import/Export ist verlustfrei; dokumentierte Aufrufe existieren; Beispiele nennen tatsächliche Voraussetzungen und laufen gegen die API.

### A10 — Nebenmodule, Build und Werkzeuge einordnen

Logfenster, Diagnose, Nachrichten, Build, Proxy/CMake, Profiletools und Crates an ihre Zielbesitzer verschieben. `tools/build`, `tools/enshrouded`, `vendor` aufbauen. Physische Wurzelumzüge abschließen; während der funktionalen Migration dürfen alte Ordner als Zwischenstand bestehen.

**Entfällt:** Modulmanifeste ohne Verbraucher, leere Fassaden, alte Pfadkopien und doppelte Generatoraufrufe nach Verbrauchernachweis.

**Abnahme:** Release mit benötigten EXE/DLLs/Profilen und vollständigen Modpaketen; keine Untersuchungscaptures oder Benutzerzustände im Release.

### A11 — Dokumentation und Altwege abschließen

Aktuelle Entwickler-, Modautor- und Serveranleitung mit dem tatsächlichen Zielsystem abgleichen. Historische Untersuchungen datieren. Jede Migration nennt Besitzer und Löschbedingung. Inventar auf tatsächlich migrierte Ziele aktualisieren.

**Entfällt:** Falsche Architekturbehauptungen und Adapter ohne zugesicherte Verbraucher. Aktive EML-Kompatibilität bleibt unterstützt.

**Abnahme:** Neue Beitragende können die Änderungsaufgaben unten anhand der Besitzer lösen; keine Anleitung verlangt undokumentierte Dateien einer fremden Komponente.

## 12. Wo eine konkrete Änderung danach hingehört

| Ich möchte … | Einstieg |
|---|---|
| GitHub-Releases anders auswählen | `modules/updater/github_releases.rs` |
| Eine Updateraktion ergänzen | Updater-`contract.json` und `commands.rs`; keine zweite Serverimplementierung |
| Editorlayout ändern | `mods/world-editor/ui/` |
| Screenshot-Cover-Undo ändern | `mods/world-editor/src/covers.lua` |
| Einer fremden Mod eine UI geben | Ihre Paketdeklaration und `ui/`-Dateien |
| Einen Editorbefehl ergänzen | Modvertrag und `src/commands.lua` |
| Ein Setting live anwenden | Besitzervertrag und dessen Handler |
| Eine Mod auf dem Dedicated Server bedienen | Deklarierter Befehl mit passender Instanz |
| KFC-Dateien anders lesen | `games/enshrouded/assets/` |
| Native World-Operation erweitern | Spieladapter und öffentliche Bindung in `api/shroudforge` |
| API-Beispiel dokumentieren | SDK-Vertrag und gemeinsames Beispielpaket |

## 13. Abnahme und tatsächlich weniger Code

1. Keine konkrete Mod benötigt Produktänderungen für Installation, eigene UI, Settings oder Befehle. World Editor erfüllt dieselben Regeln wie die kleine Beispielmod.
2. UI, CLI und Dateieingang erreichen denselben Handler mit derselben Validierung; alle bisherigen 20 Adminaktionen sind auf ihren zulässigen Zielen erreichbar.
3. Genau eine GitHub-Releaseauswahl, eine Control-Annahme, eine Editoroberfläche und eine Quelle für jedes öffentliche Settings-/Paketfeld.
4. Updates erhalten Benutzerwerte, Blueprints und wiederherstellbaren Zustand. Zugesicherte CLI-/JSON-/Lua-/ABI-Verträge bleiben über nachvollziehbare Adapter nutzbar.
5. Assettransaktionen, Lifecycle, Threadgrenzen, Prozessziele und Update-/Abbruch-/Recoveryregeln sind vor/nach dem Umbau vergleichbar.
6. Website/Loader stimmen über Schemafelder und echte API-Namen überein. Ein neues Feld muss nicht in fünf Listen gepflegt werden.
7. Core kennt kein Editoricon, Blueprintformat, Editor-Configfeld oder fachliches UI-Commandenum.

Vorhandene Tests werden pro Auftrag ausgeführt; fehlende aussagekräftige Grenztests ergänzen Paket-UI, Befehlsparität, Wertemigration, API-/Referenzabgleich und Update-Recovery. Native Spielwirkung, Replikation und Speicherung brauchen kontrollierte Spielnachweise; bestandene Strukturtests belegen diese Wirkungen nicht.

Die Reduktion entsteht durch **Löschen doppelter Implementierungen und Sonderpfade**. Dateiaufteilung allein kann die Dateizahl erhöhen. Erfolg messen wir an weniger handgepflegten Regeln und weniger fachlichen Besitzern pro Änderung. Prozentuale Laufzeit- oder Zeilenersparnis wird erst nach Messung behauptet.

Jeder Abschlussbericht nennt migrierte Funktionen, gelöschte Altimplementierung, verbleibende Adapter/Verbraucher, erhaltene Funktionsfälle und tatsächlich ausgeführte Prüfungen. Ein verschobener Monolith oder eine neue Fassade vor unveränderter Sonderlogik zählt nicht als erledigter Auftrag.
