# Review und Umsetzungsplan für Windows, Wine/Proton und Dedicated Server

Stand: 5. November 2026. Dies ist ein Vorschlag nach einer gezielten Quellcodeprüfung. Die unten beschriebenen Produktänderungen sind noch nicht implementiert.

## Entscheidung

Ein gemeinsamer Windows-Produktpfad bleibt die Grundlage. Auf Linux läuft dieser unter Wine beziehungsweise Proton. Client und Dedicated Server teilen sich Modplanung, Assetvorbereitung, Transaktionen, Konfiguration und Updater. Unterschiede werden dort behandelt, wo sie tatsächlich wirken: Spielziel im Loader, Fensterkompatibilität im Fenstermodul und Wartungskoordination an der Grenze zum externen Prozessverwalter.

PR #73 sollte inhaltlich übernommen werden. PR #97 sollte mit begrenzten Anpassungen übernommen werden. Die alternativen Fassungen #74 und #76 dürfen nicht zusätzlich eingespielt werden. #93 bleibt ein unabhängiges UI-Feature und braucht eigene Korrekturen. Keiner dieser PRs begründet einen zweiten Linux-Installer, ein zweites Asset-Backend oder das Abschalten der Wiederherstellung.

## 1. Geprüfter Stand und Übernahmeentscheidung

Lokaler Branch: `1.8.0`. Geprüfter Commit: `823823a01f76526cd580be9369e1d43bc39f5f7e`. Die GitHub-Basis von #73 und #97 heißt zum Prüfzeitpunkt `1.7.9` und zeigt auf denselben Commit. Die Branchbezeichnungen sind deshalb nicht mit unterschiedlichen Implementierungsständen gleichzusetzen.

Während der Prüfung wurde der lokale HEAD extern auf `85f4d53f9908a9400505303b3962b647f3ae5926` weitergeführt. Der Vergleich beider Commits enthält ausschließlich die bereits zuvor lokal geänderten Website-Dateien `website/src/filters.js` und `website/src/render-page.mjs`. Die hier untersuchten Produktpfade blieben unverändert.

| PR | Geprüfter Head | Entscheidung | Vor Übernahme |
| --- | --- | --- | --- |
| [#73](https://github.com/bonsaibauer/shroudforge/pull/73) | `f0d48cd7d80a5800ddebf4c32ad2f25d60b67e8a` | Reader-/Writer-Freigabe und Logausschluss übernehmen | Changelog-Konflikt lösen, Zielrelease einordnen, aktuellen integrierten Stand prüfen |
| [#97](https://github.com/bonsaibauer/shroudforge/pull/97) | `fc4f181e701e6b42a2d8f69301bb51c0292f4b5c` | Schattenfix, Wine-Probe und Cursorbrücke übernehmen | Gemeinsamen Fensterbesitzer verwenden, Cursoraktualisierung vervollständigen, neu integrieren |
| [#76](https://github.com/bonsaibauer/shroudforge/pull/76) | `08ece8f739dfe15007384328c7e401c978c6a0ea` | Als alternative Implementierung nicht übernehmen | Server-Testbelege berücksichtigen. Geschlossen und nicht gemergt |
| [#74](https://github.com/bonsaibauer/shroudforge/pull/74) | `c6b26fba2cd00af47a7cc6033ca7979a38ed248b` | Nicht übernehmen | Zurückgezogen, Beschreibung bittet ausdrücklich um Nichtbeachtung |
| [#93](https://github.com/bonsaibauer/shroudforge/pull/93) | `aff57fc4fe0b07952a4d433971044484878e421e` | Separat weiterführen | Kategorienfehler, Suchverhalten und Lokalisierung korrigieren, Basis aktualisieren |

GitHub meldete #73 und #97 als nicht mergebar. Eine lokale Merge-Simulation für #73 bestätigte ausschließlich einen Konflikt in `changelog/1.7.8.md`. Der Produktcode hatte in dieser Simulation keine Konflikte. Es wurde kein Merge durchgeführt.

#93 zeigt auf GitHub gegen seine ältere Basis `1.7.8` einen stark aufgeblähten Diff. Gegen den gemeinsamen Vorfahren des aktuellen HEAD sind seine drei zusätzlichen Commits auf 23 Dateien mit 280 Ergänzungen und 48 Löschungen begrenzt. Diese Featureänderungen gezielt neu integrieren. Ein kompletter Ersatz aktueller Dateien durch den PR-Branch würde neuere Repositoryänderungen zurücknehmen.

## 2. Fehleranalyse: Was belegt ist

### 2.1 Asset-Publishing scheitert an offenen eigenen Handles

Der aktuelle Kontrollfluss ist:

```text
Loader prüft Ziel, Vorbereitung und Baseline
  → alte unterbrochene Transaktion wiederherstellen
  → AppState öffnet KFC-Reader und aktive Companion-Dateien
  → Lua berechnet Änderungen
  → Writer schreibt in privates Stage-Verzeichnis
  → writer.finalize() konsumiert Writer und schließt seine Handles
  → transaction::commit veröffentlicht den Dateisatz
  → erst danach wird AppState normalerweise zerstört
```

Der `KFCCursor<KFCReader>` hält die aktive `<stem>.kfc_resources` und zwischengespeicherte `.dat`-Dateien offen. `commit_assets()` veröffentlicht heute, während dieser Reader noch lebt. Im Fehlerfall läuft `Drop::drop` vor dem automatischen Freigeben der Felder. Deshalb versucht auch `abort()` seine Arbeit noch mit eigenen offenen Reader-/Writer-Handles.

PR #73 korrigiert genau diese Lebensdauer. `reader` wird optional, vor Commit geschlossen und zusammen mit dem Writer vor Abort freigegeben. `reader()` liefert anschließend einen Lua-Fehler, wenn ein Zugriff nach Freigabe erfolgt. Das ist der robustere Vertrag gegenüber dem `expect` aus #76.

Quellen: [Session-Erzeugung](../../modules/mod-runtime/session/create_mod_session.rs), [Assetzugriff und Commit](../../modules/mod-runtime/session/asset_resources.rs), [Session-Drop](../../modules/mod-runtime/session/mod.rs), [Writer-Finalisierung](../../modules/mod-runtime/startup/commit_asset_changes.rs), [Transaktion](../../modules/game-files/game_file_transaction.rs) und der tatsächliche Reader in `kfc-parser/crates/kfc-base/src/container/reader.rs`.

Die Korrektur gilt für beide Spielziele und für Windows genauso wie Wine. Sie erhält dieselbe Transaktion. Ein `if wine` um das Schließen wäre die falsche Zuständigkeit.

[Issue #72](https://github.com/bonsaibauer/shroudforge/issues/72) beschreibt den Fehler auch im Launch-Modus ohne laufendes Spiel. [Issue #75](https://github.com/bonsaibauer/shroudforge/issues/75) und #76 ergänzen erfolgreiche Fremdtests desselben Fixes auf einem Wine-Dedicated-Server. Dort wurden ein erfolgreicher Commit und die tatsächliche Skilländerung durch einen verbundenen Client bestätigt. Das ist konkrete Server-Evidenz, aber keine von uns ausgeführte Debian-Abnahme.

### 2.2 Der Debian-Bericht enthält Beobachtungen und Hypothesen

Der vollständig gelesene Anhang von Leshton berichtet reproduzierbare Commit-/Abort-Fehler auf Debian 13 unter GE-Proton und gewöhnlichem Wine, funktionierende Runtime-Mods und erfolgreiche Vorbereitung unter Windows. Diese Beobachtungen passen zur Handle-Lebensdauerklasse.

Die Aussagen zu `ReplaceFileW`, atomarem blockweisem Schreiben, dem entscheidenden Engine-Handle und einem kausalen `NtLockFile`-Fehler sind dadurch nicht bewiesen. Die aktuelle Assettransaktion verwendet `remove_file` und `rename`. Sie ist journalgestützt wiederherstellbar und veröffentlicht den Index zuletzt. Der ganze Dateisatz wird nicht in einer einzigen atomaren Dateisystemoperation ausgetauscht.

Die aktuelle Bootstrap-Sperre hält außerdem den Spieleinstieg bis nach der Assetvorbereitung zurück. Eine Wine-`fixme`-Zeile allein identifiziert keine fehlgeschlagene Operation. Der Konfigurationslock verwendet an anderer Stelle tatsächlich `fs2::FileExt::lock_exclusive()`. Asset- und Konfigurationsfehler müssen deshalb nach Operation und Pfad unterschieden werden.

Auch [Issue #101](https://github.com/bonsaibauer/shroudforge/issues/101) zeigt den Commit-/Abort-Symptomkomplex unter Windows Server 2019. Baseline-Restore und spätere Runtimeinitialisierung funktionieren dort. Das beweist nicht dieselbe Ursache, widerlegt aber die pauschale Diagnose eines ausschließlich Linux-spezifischen Schreibrechteproblems.

Die leeren `.dat`-Paddingdateien sind kein belegter Fehler. Sie dürfen nicht als Reparaturmaßnahme entfernt werden.

### 2.3 Erfolgreich vorbereitete Mods werden durch ihre eigenen Logs blockiert

Der zweite Teil von #73 ist ebenfalls sinnvoll. Der Fingerprint erfasst bisher sämtliche sichtbaren Paketdateien. Eine native DLL erzeugt danach ihr `.log` im Paketordner. Der nächste Vergleich sieht damit andere vermeintliche Eingaben und blockiert bereits vorbereitete Assetmods.

[calculate_preparation_fingerprint.rs](../../modules/modloader/calculate_preparation_fingerprint.rs) ist bereits der gemeinsame Besitzer. Loader-Check, Apply, Runtimeplanung und UI verwenden dieselbe Funktion. `*.log` dort einmal ohne Beachtung der Groß-/Kleinschreibung auszunehmen behebt die belegte Fehlblockierung. `.ini`, Lua und andere tatsächliche Eingaben bleiben erfasst. Keine allgemeine Ignore-Liste oder neue Manifestoption für diesen Fix einführen.

Ein bloßer Austausch der Runtime-DLL reicht als Produktlieferung nicht. Die Host-EXE berechnet den Fingerprint ebenfalls. Das Release muss die zusammengehörigen Binärdateien enthalten.

### 2.4 Wine-Fensterfehler sind eine andere Zuständigkeit

[#96](https://github.com/bonsaibauer/shroudforge/issues/96) und #97 betreffen Tao-/WebView2-Fenster: Schatten-Inset, zusätzliche Dekoration, Klickversatz und Cursorübergabe zwischen Prozessen. Der begrenzte Wine-Nachweis über `ntdll!wine_get_version`, das Ausschalten des undekorierten Schattens und die validierte Cursorübergabe sind schlüssig. Die berichteten Spieltests stammen von den PR-Autoren.

Ein konkreter Restfehler liegt in der vorgeschlagenen Cursorbrücke: Sie reagiert nur auf `pointerover` und `pointerdown`. Wird ein Update-Button nach einem Snapshot deaktiviert, wechselt sein CSS-Cursor, während die Maus auf demselben Element bleibt. Ohne neues Ereignis behält der Host die alte Form. Die Brücke muss nach solchen Zustandswechseln am letzten Pointer-Ort begrenzt neu auswerten. Das ist aus dem Code ableitbar und wurde hier nicht unter Wine reproduziert.

Debug Console und World Editor bauen ebenfalls undekorierte Tao-Fenster. Der PR behandelt nur den Modloader. Das ist eine Umfangs-/Strukturlücke. Der dekorierte Updater braucht dagegen keinen pauschalen Schattenfix.

Die größere Tab-Klickfläche aus #97 ist eine allgemeine UI-Verbesserung für alle Systeme. Sie wird separat übernommen und geprüft. #93 ist dafür keine technische Voraussetzung.

### 2.5 Eine fehlende WebView2 Runtime bleibt ein Installationsproblem

[#94](https://github.com/bonsaibauer/shroudforge/issues/94) behandelt `0x80070002` bei fehlender WebView2 Runtime im richtigen Wine-/Proton-Prefix. #97 installiert diese Runtime nicht. [#45](https://github.com/bonsaibauer/shroudforge/issues/45) enthält einen erfolgreichen Proton-Experimental-Weg und einen gescheiterten GE-Proton-Versuch. Daraus lässt sich kein exklusiver GE-Proton-Support ableiten. #69 enthält einen ähnlichen Startfehler, aber keinen bestätigten abschließenden Ursachennachweis.

Empfehlung: Wine und Proton als gemeinsame Kompatibilitätsfamilie behandeln, konkret geprüfte Versionen dokumentieren. Die UI-Voraussetzungen gehören in eine gemeinsame Installationsanleitung und hilfreiche Startdiagnose. Ein Headless-Server darf für seinen normalen Betrieb keine WebView2-Oberfläche benötigen.

## 3. Klare Ebenen und bestehende Besitzer

| Ebene | Zuständiger Besitzer | Grenze |
| --- | --- | --- |
| Linux-Host, Benutzer, Mounts, Prefix und Dienst | Betreiber, systemd, Docker oder Panel | Startet und stoppt die Instanz, erhält Prefix und Umgebung. Enthält keine Modinstallationslogik |
| Prozessstart und Spielziel | `modules/entrypoints/` | Wählt Client-/Server-EXE und Vorbereitungszeitpunkt. Injizierter Prozess bestimmt sein tatsächliches Ziel |
| Native Fensterkompatibilität | `modules/window-management/` | Wine-Probe, undekorierter Schatten, native Cursorabbildung. Keine Asset- oder Updateentscheidung |
| Gemeinsame Anzeigeelemente | `modules/ui-resources/` | Kleine gemeinsame Cursorbrücke und UI-Voraussetzungsdarstellung. Fachkommandos bleiben in den Hosts |
| Modpakete und Vorbereitungseingaben | `modules/modloader/` | Manifest, Targets, Plan und kanonischer Vorbereitungsfingerprint |
| Asset-Sitzung und Berechnung | `modules/mod-runtime/session/` und `startup/` | Reader-/Writer-Lebensdauer, Lua-Ausführung, Serialisierung und Finalisierung |
| Veröffentlichung und Wiederherstellung | `modules/game-files/` | Ein Stage-/Journal-/Commit-/Recovery-System für beide Spielziele |
| Konfiguration und Speicherorte | `modules/loader-config/` | Ein Pfadresolver und kanonische persistente Writer |
| Updates und Wartungsaufträge | `modules/updater/` | Eine Queue, ein Installer, unabhängige Worker, Migrationen, Ergebnisse |
| Laufender Spielzugriff | `modules/game-runtime/` | Windows-ABI und genaue Client-/Server-Spielprofile. Betriebssystemfamilie ersetzt keinen Buildnachweis |

Vier Begriffe bleiben unabhängig: Hostbetriebssystem, Windows-Ausführungsumgebung, Spielziel und Bedienmodus. Ein Dedicated Server kann unter Windows oder Wine laufen. Eine Serverinstallation kann ausdrücklich interaktiv verwaltet werden. Die automatische Serverinitialisierung bleibt headless. Docker ist eine Betriebsform, keine weitere Runtime-Architektur.

Unter Wine wird die Windows-Binärdatei ausgeführt. Rusts `cfg(windows)` bleibt daher aktiv. Ein neuer `cfg(target_os = "linux")`-Zweig würde die betroffenen Prozesse gerade nicht erfassen. Die vorhandene native Nicht-Windows-Bridge ist kein funktionierender Linux-Runtimepfad. Daraus folgt keine Löschfreigabe für portable Parser-/CLI-Teile.

## 4. Konkrete Umsetzungspakete in Reihenfolge

### A. Gemeinsamen Assetfehler beheben

Die vier Quellcodedateien aus #73 übernehmen:

- `modules/mod-runtime/session/mod.rs`: `reader` als `RefCell<Option<...>>`, Handles vor Abort freigeben, geschlossenen Reader sauber melden.
- `modules/mod-runtime/session/create_mod_session.rs`: Reader mit `Some(reader)` initialisieren.
- `modules/mod-runtime/session/asset_resources.rs`: fallibles `reader()`, alle betroffenen Zugriffe anpassen, Reader vor Commit freigeben.
- `modules/modloader/calculate_preparation_fingerprint.rs`: `.log` beim Paketfingerprint ausnehmen.

Den Changelog in das tatsächlich auszuliefernde Release einordnen. Beim aktuellen Arbeitsbranch wäre das ein neuer Eintrag für `1.8.0`, falls dieser Branch das Releaseziel bleibt. Die historischen Dateien nicht nur zum Auflösen eines Mergekonflikts überschreiben.

Danach in `modules/game-files/game_file_transaction.rs` Fehlerkontext ergänzen. `replace`, `sync`, Journal und Recovery sollen Phase, Operation, Quell-/Zielpfad und ursprünglichen OS-Fehler erhalten. Heute kann `recover(root, stem)?` im Commitfehlerpfad den ursprünglichen Publishfehler verdecken. Beide Ursachen müssen im Ergebnis und Log sichtbar bleiben. Dies ist eine kleine Ergänzung am Transaktionsbesitzer, kein allgemeines Dateisystemframework.

Den parallelen Kontextverlust in `modules/loader-config/write_json_atomically.rs` korrigieren. Die bestehende Tempdatei-/Sync-/Persist-Logik mit begrenzten Wiederholungen bleibt bestehen. Die Ausgabe muss das betroffene Ziel nennen.

### B. #97 an die gemeinsame Fensterzuständigkeit anbinden

Vorgeschlagene neue Dateien innerhalb bestehender Module:

```text
modules/window-management/
  read_wine_runtime.rs
  apply_window_compatibility.rs
  parse_page_cursor.rs
modules/ui-resources/
  page_cursor_bridge.js
```

`read_wine_runtime.rs` übernimmt die einmalige Probe aus #97. `apply_window_compatibility.rs` wendet die belegte Schattenanpassung ausschließlich unter Wine auf undekorierte Fenster an. `parse_page_cursor.rs` hält die endliche native Cursorabbildung. Die erforderliche LibraryLoader-Abhängigkeit liegt dann beim Fenstermodul.

Die gemeinsame JavaScript-Brücke wird über den vorhandenen WebView-Initialisierungspfad eingebunden. Sie bekommt die jeweilige bestehende IPC-Anbindung des Hosts. Die Hosts behalten ihre lokalen EventLoop-Ereignisse. Das schafft weder ein neues Fachprotokoll noch einen zweiten Snapshotkanal.

Die Brücke verarbeitet Pointerereignisse und begrenzt zusammengefasste relevante DOM-/Zustandsänderungen. Sie verwendet die letzte Pointerposition mit `elementFromPoint` und sendet nur geänderte, unterstützte Cursorformen. Auf Windows muss sie nicht aktiviert werden. Kein dauerhaftes ungebremstes Polling.

Konkrete Verbraucher:

- `modules/modloader/ui/host/window.rs` und `receive-ui-updates.ts`.
- `modules/debug-console/ui/create_console_window.rs` und `ui/window.rs`.
- `modules/world-editor/ui/create_editor_window.rs` und `ui/window.rs`.
- `modules/updater/ui/host/window.rs` nur für tatsächlich benötigte WebView-Cursorbehandlung, ohne pauschale Schattenänderung.
- `modules/window-management/{lib.rs,Cargo.toml}` und die tatsächlich betroffenen Verbraucher-Manifeste.

Die PR-lokalen Dateien `modloader/ui/host/read_wine_runtime.rs` und `mirror_page_cursor.rs` danach nicht parallel behalten. Die Cursorbeobachtung aus dem React-Snapshot-Hook entfernen. Die Änderung der Segment-Klickflächen getrennt behandeln, ohne die Frontendstacks umzubauen.

### C. Zwei konkrete Headless-Lücken schließen

Die Serverfenster werden im Bootstrap bereits übersprungen. Der Updater erhält diese Information aber nicht durchgängig.

1. `modules/entrypoints/windows-bootstrap/module_processes.cpp::run_startup_update_check` gibt beim automatischen Serverstart `--headless` weiter. `modules/updater/bin.rs` liest diesen Bedienmodus. `system-updates/headless_check.rs::startup_check` berücksichtigt ihn bei beiden Aufrufen von `open_update_window`. Headless liefert Status und Exitcode. Pflichtupdate und Reparatur dürfen den Start weiterhin sperren.
2. `modules/updater/control/execute_update_action.rs` setzt für `stageUpdate` ausdrücklich `showWindow:false`. `modules/updater/lib.rs` liest dieses Feld im Zweig `stage-system-update`, statt `request_system_update(..., true)` für die Fensteranzeige zu erzwingen. `system-updates/requests.rs` und `queue/start_update_queue.rs` besitzen den erforderlichen Parameter bereits.

Vorgeschlagener Request innerhalb des bestehenden Vertrags:

```json
{"command":"stage-system-update","waitForGame":true,"showWindow":false}
```

Ein explizit geöffnetes Verwaltungsfenster für eine Serverinstallation bleibt möglich. Deshalb nicht sämtliche UI allein bei `target == server` ausschalten. Der Aufrufer bestimmt den Bedienmodus.

### D. Serverwartung mit dem äußeren Prozessverwalter verbinden

Das ist die eigentliche fehlende Serverbetriebsgrenze. Der vorhandene Updater wartet auf Spielprozesse. systemd, Docker oder ein Panel kann die Instanz währenddessen bereits neu starten. Dieser Wettlauf ist ein Architekturproblem, hier kein reproduzierter Laufzeitfehler.

Der minimale Vertrag ist:

```text
Prozessverwalter sperrt automatische Neustarts
  → eigene Serverinstanz geordnet stoppen
  → bestehende Updaterqueue unter demselben Wine-Prefix ausführen
  → konkreten runId bis zum bestätigten Ergebnis verfolgen
  → bei Erfolg dieselbe Instanz mit denselben Argumenten starten
  → bei Fehler oder unbekanntem Ergebnis Wartungszustand anzeigen
```

Ein bloßes Prozessende des gestarteten Updaters beweist keinen Installationserfolg. `windows/worker.rs::hand_off_installed_process` lagert unter anderem `--run-queue` in eine unabhängige Kopie aus und kehrt früh zurück. Das ist erforderlich, damit die installierte EXE ersetzt werden kann.

Konkrete Erweiterung der vorhandenen Queue:

- `queue/start_update_queue.rs` gibt den unter Lock erzeugten `runId` an seinen Aufrufer zurück. Bisher melden Requests nur den Start.
- `queue/update_queue_storage.rs` bleibt die einzige persistente Quelle für den Lauf. Vorhandenen Run-Token verwenden.
- Ein fokussierter Queue-Helper, vorgeschlagen `queue/wait_for_update_queue_run.rs`, beobachtet exakt diesen Lauf mit begrenztem Warten und unterscheidet Erfolg, Fehler, Abbruch und unbekanntes Ergebnis.
- `queue/run_update_queue.rs` schreibt einen eindeutigen Endausgang zum bestehenden Lauf. Heute wird `run.state` auch nach fehlgeschlagenen Items auf `complete` gesetzt. Ein wartender Aufrufer darf daraus allein keinen Erfolg ableiten.
- Der bestätigte Wartungsabschluss umfasst zusätzlich alle erforderlichen After-install-Migrationen aus der neu installierten Version. `windows/entry.rs::run_scheduled_worker` führt sie heute nach der Queue aus, meldet ihren Fehler im Status, gibt aber weiter `queue_result` zurück. Endgültigen Erfolg erst nach diesen Jobs speichern. Migrationsfehler dem ursprünglichen `runId` zuordnen und an den wartenden Aufrufer weitergeben. Den vorhandenen Queuebesitzer und die erforderliche Freigabe des Workerlocks vor dem Migrationsaufruf erhalten.
- Die bestehende Requestroute erhält eine begrenzte Möglichkeit, auf einen Lauf zu warten. Ein über die installierte EXE ausgeführter wartender Request muss vorher ebenfalls in die vorhandene unabhängige Worker-Kopie wechseln. Er darf die zu ersetzende EXE nicht selbst offen halten.
- Startrequest und Warteanfrage bleiben getrennt. Die Warteanfrage darf nicht selbst zur `waitPid` werden, auf deren Ende die Queue wartet. `runId` und Endausgang auch in `ui/update-types.ts` und den tatsächlichen Snapshotverbrauchern mitführen.
- Der Linux-Wrapper übernimmt ausschließlich Stoppen, Aufruf, Warten und Starten. Er installiert keine Dateien und führt keine zweite Queue oder Statusdatenbank ein.

Keine systemd-, Docker- oder Bash-Aufrufe aus der injizierten Runtime. Kein globales Beenden von `wineserver`. Kein neuer dauerhafter Linux-Verwaltungsdaemon. Eine erste Integration kann eine dokumentierte Wartungsabfolge sein. Automatisierung folgt auf den belastbaren Queueabschluss.

### E. Überholte Vorbereitungsscans abbauen

`mod-runtime/startup/cache/FileStateCache` enthält noch eine eigene Dateisammlung mit sämtlichen Moddateien und Verzeichniszeitstempeln. Der normale Loader entscheidet bereits vorher über den kanonischen Fingerprint. Beide gefundenen produktiven Pregame-Aufrufer in `apply_asset_mods.rs` setzen außerdem `force_patch:true`. Die alte Patch-Skipentscheidung ist dort wirkungslos.

Nach #73 lösen Logs über diesen Legacy-Cache keinen zusätzlichen normalen Loaderpass aus. Es bleiben aber unnötige Scans und Cache-Schreibvorgänge sowie eine zweite Definition vermeintlicher Eingaben.

Konkreter Abbau nach erneutem Callerabgleich:

- Die überholte Patch-Skipentscheidung aus `startup/run_startup_mods.rs` entfernen.
- `startup/cache/track_input_files.rs` und die nur dafür gebrauchten Datei-/Zeitstempelteile aus `mod.rs` und `store_file_state.rs` entfernen.
- Die noch verwendete `BUILD_ID`-Invalidierung für Lua-Definitionsdateien beim vorhandenen Definitionscache erhalten. Sie wird in `create_mod_session.rs` verwendet und darf nicht versehentlich entfallen.
- `force_patch` und `skip_cache` erst nach Anpassung ihrer tatsächlichen Leser und Aufrufer entfernen. Öffentliche Rust-Einstiegspunkte und CLI-Optionen dabei mitführen.
- Falls danach keine Verantwortung im alten `startup/cache/` verbleibt, den Ordner löschen. Kein Ersatzcache für dieselbe Vorbereitungsentscheidung.

### F. Zielwahl und bestehende Requestaufrufe vereinheitlichen

Mehrere Helfer leiten Client oder Server aus vorhandenen Dateien ab. `entrypoints/loader/game_files.rs` bevorzugt den Client, wenn beide EXEs vorhanden sind. `asset_preparation/select_game_target.rs` berücksichtigt zusätzlich die laufende Server-EXE. Weitere Zielentscheidungen existieren in Embedded-Einstieg, Pfadauflöser und Updateraktionen.

Diese Rust-seitigen Entscheidungen auf einen fokussierten Zielresolver beim bestehenden Installationspfadbesitzer zurückführen, vorgeschlagen `modules/loader-config/resolve_game_target.rs`. Loader, Updater und Pfadauflöser können diesen vorhandenen gemeinsamen Crate nutzen. Der native Bootstrap übergibt sein bereits anhand der Prozess-EXE erkanntes Ziel. Dafür ist kein neuer Native-ABI-Vertrag erforderlich. Der gültige Vorrang ist ausdrücklich übergebenes Ziel, im injizierten Prozess dessen tatsächliche EXE, sonst genau ein erkennbares Ziel. Ein Widerspruch zur injizierten EXE oder zwei mögliche Ziele ohne Auswahl wird als Fehler gemeldet. Derselbe ausgewählte Wert bestimmt EXE, KFC-Stamm und Modplan. Keine neue persistente Linux-/Server-Targetliste parallel zum bestehenden Modvertrag.

Die wiederholte JSON-Request-/Response-Ansteuerung des installierten Updaters in Loader `embedded.rs`, Updater `windows/entry.rs` und `modloader/ui/host/invoke_updater_request.rs` kann anschließend zu einer konkreten gemeinsamen Requestfunktion beim Updater zusammengeführt werden. Migrationen behalten ihre fachlichen Wiederholungsregeln. Dafür kein allgemeines Prozessframework einführen.

### G. #93 separat korrigieren

Das Feature gehört weiterhin in den vorhandenen Modloader, nicht in eine Wine-Sonderseite. Drei konkrete Punkte sind vor Übernahme zu behandeln:

1. In `ui/edit-mod-setting.tsx` kollidiert der frei erlaubte Gruppenname `all` mit dem internen Tabwert `all`. Beide erhalten denselben React-Key und derselbe Klick setzt die ungefilterte Ansicht. Interne Tabwerte aus Index oder einem getrennten Namensraum bilden, ohne einen zulässigen Anzeigenamen zu reservieren.
2. Die Beschreibung verspricht Suche über alle Kategorien. Der Code verwirft jedoch zuerst alle Optionen außerhalb `activeCategory`. Bei einer Suche muss die Kategorie gemäß beschriebenem Verhalten zurückgesetzt oder für den Suchfilter ignoriert werden.
3. Neue Optionskategorien können über den vorhandenen Mod-Lokalisierungsvertrag nicht übersetzt werden. `mod-locale.schema.json`, `ModSettingLocale.options: BTreeMap<String,String>` und `resolve_mod_locale.rs` unterstützen bisher nur Optionslabels. Entweder Kategorieanzeigen über diesen bestehenden Vertrag ergänzen oder die Kategorieerweiterung bis dahin zurückstellen. Kein zweites Übersetzungsformat im Frontend.

Suche und Filter für lange bestehende Listen können unabhängig von der Schemaerweiterung geliefert werden. Die Kategorie-/Tab-Erweiterung muss bei Übernahme auch die veröffentlichten bilingualen Referenzen und tatsächlichen Schema-Verbraucher mitführen. Die Prüfung hier deckt die Feature-Kernänderungen und diese Integrationspunkte ab, keine vollständige Freigabe aller Übersetzungen und Schemafälle.

## 5. Berechtigungen und Diagnose im Serverbetrieb

Die Pfadkonfiguration über `resolve_installation_paths.rs` beibehalten. Die Installation benötigt je Aktion unterschiedliche schreibbare Ziele:

| Aktion | Erforderliche Ziele |
| --- | --- |
| Runtimebetrieb | Status, Runtimeanforderungen und konfigurierte Logs, gegebenenfalls Modexports |
| Modkonfiguration | Konfiguration und `extended.mod.json` im tatsächlichen Paketordner |
| Assetvorbereitung | Spielverzeichnis, `.kfc`, `.kfc_resources`, betroffene `.dat` und benachbarter Stage-/Rollbackbereich |
| System-/Modupdate | Verwaltete Programm-/Paketdateien sowie Queue, Staging, Sicherungen und Wiederherstellungsziele |

Nur Cache und Logs umzuleiten macht eine schreibgeschützte Spielinstallation nicht assetfähig. `chmod 664` auf Dateien belegt keine Lösch-/Umbenennungsberechtigung auf dem Verzeichnis. Dienstbenutzer, UID/GID, Mountoptionen und derselbe Prefix müssen in der Betriebsanleitung konkret zusammenpassen.

Eine Diagnose kann mit eigenen kleinen temporären Dateien Erstellen, Schreiben, Synchronisieren, Umbenennen und Löschen im tatsächlichen Zielverzeichnis prüfen. Sie ist eine Vorprüfung und kein Beweis, dass kein fremder Handle oder späterer Lock existiert. Sie verändert keine echten Spieldateien und repariert keine Rechte automatisch.

Restart-Manager-Kompatibilität, Overlay-/Bind-Mounts, unterschiedliche Volumes, readonly-Attribute, andere Prozesse und Workerlebensdauer bleiben bei weiteren Fehlern konkrete Untersuchungspunkte. Ein tatsächlicher Defekt dieser APIs unter der gemeldeten Umgebung ist bisher nicht nachgewiesen. Erst mit Operation und Pfad einen engen Anpassungsbedarf entscheiden. Keine pauschale Wine-Erfolgsabkürzung in der Lockprüfung.

## 6. Abnahme und Reihenfolge

Empfohlene Reihenfolge: A als unmittelbarer Fehlerfix, dann B und C als getrennte Änderungen. D schafft belastbaren Serverbetrieb. E und F räumen belegte Mehrfachzuständigkeiten auf. G bleibt außerhalb des Kompatibilitätsfixes.

| Abnahmefall | Nachweis |
| --- | --- |
| Windows-Client und Windows-Server | Vorbereitung, tatsächliche Modwirkung und unveränderte Wiederherstellung |
| Proton-Client und Wine-/GE-Proton-Server | Beide KFC-Stämme, erster Start und Wiederholungsstart, tatsächliche Modwirkung |
| Launch-Modus und frühe injizierte Vorbereitung | Erfolgreicher Commit ohne eigene offene Quellhandles |
| Fehler und Unterbrechung | Kein falscher Applied-Status, kompletter alter Satz wiederherstellbar, Publish- und Recoveryfehler erhalten |
| Fingerprint | Logs anlegen, ändern und entfernen ändern den Fingerprint nicht. Lua-/INI-/Settingänderungen bleiben wirksam |
| Alle betroffenen Fenster | Cursor, deaktivierte Buttons unter ruhender Maus, Resize, Drag, Skalierung und Klickposition |
| Headless-Server | Kein Fenster bei normaler Prüfung, Reparatur und Queueaktionen. Pflichtsperren bleiben sichtbar |
| Extern verwaltetes Update | Kein vorzeitiger Neustart, Erfolg am richtigen runId erst nach erforderlichen After-install-Migrationen, Fehler/Abbruch kein Erfolg, unabhängige EXE bleibt ersetzbar, kein waitPid-Selbstwarten |

Windows-Builds und bestehende relevante Game-Files-/Runtimeprüfungen müssen den tatsächlich integrierten Commit prüfen. Autorenseitige GNU-Checks und Spieltests sind zusätzliche Belege. Der gemeldete grüne Fork-Build von #97 bezog sich auf einen älteren Commit plus Workflowänderungen und ersetzt keine aktuelle Integration.

Die derzeitige `.github/workflows/dev_build.yml` lässt die Build-/Testjobs nur für den Releasezweig des Workflows laufen. Die PR-Freigabe der vorhandenen Pipeline allein liefert deshalb keinen vollständigen Produktbuild. Buildvalidierung vom Publikationsschalter lösen und dieselben bestehenden Buildaktionen für PRs verwenden. Veröffentlichung und externe Release-Uploads bleiben an die tatsächliche Releaseaktion gebunden. Keine kopierte zweite Buildpipeline dauerhaft etablieren.

## 7. Umfang, Quellen und tatsächliche Prüfung

Alle drei Nutzeranhänge wurden gelesen. #73 und #97 wurden mit vollständigem Diff und Diskussion geprüft. #74/#76 und die einschlägigen Issues #72/#75/#96/#94/#45/#69/#101 wurden zur Ursachen- und Umfangsabgrenzung gelesen. Bei #93 wurden die drei zusätzlichen Featurecommits, die Kernquellen und ausgewählte Integrationsverbraucher geprüft. Die übrigen offenen Feature-PRs wurden lediglich inventarisiert und erhalten hier keine fachliche Freigabe.

Vollständig gelesene Quellgruppen, jeweils bezogen auf den oben festgehaltenen Basisstand oder ausdrücklich den PR-Head:

- Assetpfad: `mod-runtime/session/{mod,asset_resources,create_mod_session}.rs`, `startup/{commit_asset_changes,run_startup_mods,open_game_assets,manage_asset_baseline}.rs`, `startup/cache/{mod,track_input_files,store_file_state}.rs`, `ingame/{plan_runtime_mods,start_ingame_runtime}.rs`, `reflection/load_type_registry.rs`, `lua/definitions/export_lua_definitions.rs`, `mod-runtime/lib.rs`, `modloader/calculate_preparation_fingerprint.rs`, `game-files/{lib,game_file_transaction}.rs`, Loader `asset_preparation/{prepare_startup_assets,apply_asset_mods,recover_failed_startup_assets,check_asset_preparation,select_game_target,lock_startup_assets}.rs`.
- Fensterpfad: `modloader/ui/{receive-ui-updates.ts,lib.rs}`, `modloader/ui/host/{window.rs,mod.rs}`, `window-management/{lib,window_geometry,choose_folder}.rs`, `ui-resources/{lib,embedded_document}.rs`, `transport/ui/send-host-command.ts`, Debug Console `ui/{create_console_window,render_console_document,mod}.rs`, World Editor `ui/{create_editor_window,render_editor_document,mod}.rs`, `updater/ui/show-system-update.tsx`.
- Server-/Updatepfad: Loader `{main,launch_game,command_line,game_files,process_modes,start_updater_controller}.rs`, Bootstrap `{module_processes,runtime_lifecycle}.cpp`, Updater `bin.rs`, `control/{run_headless_update_controller,execute_update_action}.rs`, `queue/{run_update_queue,start_update_queue,update_queue_storage}.rs`, `system-updates/{headless_check,requests}.rs`, `modloader/ui/host/invoke_updater_request.rs`, Config `{lib,read_document,write_document,resolve_installation_paths,resolve_document_path,write_json_atomically,lock_installation,runtime_is_active,read_runtime_pid,initialize_loader_settings}.rs`.
- Ergänzende UI-Featurequellen: #93-Versionen von `modloader/ui/{edit-mod-setting,show-mod}.tsx`, aktuelle `modloader/{resolve_mod_locale,read_mod_locale_catalogs}.rs`, `manifest/mod-locale.schema.json`, `ui-resources/choose-segment.tsx`, `modloader/ui/save-mod-settings.ts`.
- Verträge und Dokumentation: Workspace- und betroffene Komponentenmanifeste, Repository-Arbeitsregeln, aktuelle Ownership-Anleitung, Modul-/Loader-/Runtime-READMEs, Server-Control-Plan und die vier deutschen Serverartikel.

Nur gezielt in Ausschnitten gelesen oder nach Aufrufern durchsucht wurden insbesondere: übrige Teile von Loader `embedded.rs`, Updater `lib.rs` und `windows/{entry,worker,scheduler}.rs`, Systeminstallation und Release-Dateianwendung, Debug-Console-/World-Editor-WebView-Eventloops, Updaterfenster, UI-Konfigurationsstatus, IPC-Parser, CSS, weitere Manifest-/Schema-Verbraucher, Configdefaults, Nicht-Windows-Bridge, Workflow und Buildskripte. Deren vollständige Dateien oder Module gelten nicht als auditiert.

Der Parserreader wurde als externer Submodulquellcode vollständig gelesen, Writer-Finalisierung und Index-Lebensdauer gezielt. Die relevante Tao-Fensterbehandlung wurde als externe Abhängigkeit gezielt gelesen. Generierte Frontends, Binärartefakte, historische Profilkataloge, vollständige Übersetzungskataloge und vendorte Bibliotheken waren kein vollständiger Prüfbereich. Dies ist kein Repository-Gesamtaudit und beansprucht keinen prozentualen Leseanteil des gesamten Repositories.

Ausgeführt: Gitstatus und Referenzabgleich, Abruf der PR-Heads ohne Checkout, Diff-/Callerprüfung, `git diff --check` für #73/#97/#93 und Merge-Simulation für #73. Die drei Diffprüfungen meldeten keine Whitespacefehler. Die Simulation bestätigte den genannten Changelog-Konflikt.

Nicht ausgeführt: Produktänderungen, Merge, Workflowfreigabe, Builds, Tests, Spielstarts, Rechteänderungen oder Änderungen an einer Installation. Dieses Dokument ist die einzige neu angelegte Repositorydatei. Die bereits vorhandenen Änderungen in `website/src/filters.js` und `website/src/render-page.mjs` bleiben erhalten.

Ergänzend geprüfte Primärquellen: [Rust `fs::rename`](https://doc.rust-lang.org/std/fs/fn.rename.html), [Windows `CreateFileW` und Delete-Sharing](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew), [Windows `DeleteFileW`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-deletefilew), [Valve Proton](https://github.com/ValveSoftware/Proton) und [Microsoft WebView2-Verteilung](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution). Sie stützen die API-/Laufzeitabgrenzung und ersetzen keinen Live-Nachweis für eine bestimmte Wine-Version.
