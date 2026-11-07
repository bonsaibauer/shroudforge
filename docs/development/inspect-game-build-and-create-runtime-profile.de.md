# Spielbuilds untersuchen und Runtimeprofile erstellen

Native Entwicklungsaktionen liegen unter `dev/profiles/`. Die Python-Implementierungen liegen gemeinsam unter `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/`. Diagnosejobs verwenden sie über denselben Jobablauf wie Lua und Modaktionen. Jede Aktionsdatei nennt ihre Handlung. Technische Nachweise werden ausdrücklich gestartet.

| Aufgabe | Implementierung |
| --- | --- |
| EXE-Metadaten lesen | `dev/profiles/cli/inspect_executable.cpp` |
| Spielbuild auswählen und seine Belege aufnehmen | `dev/profiles/cli/select_game_build.cpp` |
| Funktionskandidaten finden | `dev/profiles/cli/discover_function_candidates.cpp` |
| Funktionssignaturen suchen | `dev/profiles/cli/scan_function_signatures.cpp` |
| Funktionskatalog aus einem Profil exportieren | `dev/profiles/cli/export_profile_function_catalog.cpp` |
| Prozessmodule erfassen | `dev/profiles/cli/capture_process_modules.cpp` |
| Alte ECS-Textausgabe übernehmen | `dev/profiles/cli/import_legacy_ecs_capture.cpp` |
| Provider-ABI prüfen | `dev/profiles/cli/inspect_provider_abi.cpp` |
| Profilentwurf erstellen | `dev/profiles/cli/create_runtime_profile_draft.cpp` |
| Profil prüfen | `dev/profiles/cli/validate_runtime_profile.cpp` |
| Geprüftes Profil freigeben | `dev/profiles/cli/approve_runtime_profile.cpp` |
| Runtime und optionale Prozessdaten erfassen | `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/capture/discover_runtime_build.py` |
| ECS-Registrierungen erfassen oder exportieren | `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/capture/capture_ecs_registry.py` |
| Typ anhand von Namen oder Hash suchen | `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/analyze/find_runtime_type.py` |
| Zwei erfasste Spielbuilds vergleichen | `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/compare/compare_runtime_builds.py` |
| Vollständige Profilkopie mit Änderungs- und Nachweisbericht vorbereiten | `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/compare/prepare_profile_rebase.py` |
| Signaturen bis zu Originalfunktionen und Aufrufern verfolgen | `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/analyze/trace_function_anchors.py` |
| KFC-Ressourcen exportieren | `dev/profiles/inspect/export_kfc_resources.rs` |
| ECS-Systemabhängigkeiten aus einer Live-Erfassung auflösen | Runtime Diagnostics: `inspect.system-dependencies` nach `capture.runtime` |
| Bereits beobachtete Registrierung eindeutig exportieren | `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/capture/export_observed_registry.py` |
| Fabriken und verknüpfte Inventare erfassen | `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/capture/capture_factory_state.py` |
| Rezeptressourcen vergleichen | `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/compare/compare_recipe_resources.py` |

`dev/profiles/cli/kfc_runtime_dev.cpp` ordnet die bestehenden Konsolenbefehle ihren Implementierungen zu. Die gemeinsame PE-Verarbeitung liegt in `dev/profiles/cli/read_pe_image.cpp` und `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/inspect/read_pe_image.py`. Der Signaturmatcher liegt in `dev/profiles/cli/match_function_signatures.cpp`. Die Python-Unterordner behalten ihre bisherigen Zuständigkeiten.

## Zuerst ausdrücklich bauen

Die folgenden Befehle werden im Repository ausgeführt:

```powershell
cmake -S . -B build/native-runtime -A x64
cmake --build build/native-runtime --config Release --target kfc-runtime-dev
cmake --build build/native-runtime --config Release --target kfc-runtime-verify-component-registry
cargo build -p shroudforge-api --example export_kfc_resources --offline
```

Die native Konsole liegt danach unter `build/native-runtime/tools/kfc-runtime-dev.exe`. Die Rust-Programme liegen unter `target/debug/examples/`. Ein gesetztes `CARGO_TARGET_DIR` bestimmt deren tatsächlichen Ausgabeort.

## Eine konkrete Handlung ausführen

```powershell
$exe = 'D:/Games/Enshrouded/enshrouded.exe'
$capture = 'dev/profiles/captures/my-build'
$diagnosticPythonSources = 'modules/runtime-diagnostics/diagnostics-jobs/game_inspection'
build/native-runtime/tools/kfc-runtime-dev.exe inspect $exe --out "$capture/image-report.json"
python "$diagnosticPythonSources/capture/discover_runtime_build.py" $exe --functions --out $capture
python "$diagnosticPythonSources/analyze/find_runtime_type.py" $capture 'keen::ecs::CurrentTransform'
cargo run -p shroudforge-api --example export_kfc_resources --offline -- $exe "$capture/resources.json"
```

Der KFC-Exporter verwendet dieselbe Implementierung für die Attributauswahl, ausdrücklich gewählte `--type`-Einträge und die bisherige Baustoffauswahl mit `--building-inputs`. Die früheren Cargo-Beispielnamen bleiben verfügbar und kompilieren dieselbe Datei. Die ItemInfo-Feldprojektion der Baustoffauswahl bleibt erhalten.

Für die komplette vorhandene Client-/Servererfassung gibt es `dev/profiles/capture/capture_runtime.ps1`. Diese Aktion führt die einzelnen Erfassungsschritte in sichtbarer Reihenfolge aus. Benötigte Programme werden vorher gebaut oder mit `-NativeVerifier` und `-ResourceExporter` übergeben. Die Aktion baut keine Programme nebenbei.

Die Python-Abhängigkeiten stehen unter `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/requirements.txt`. Die [Diagnosejobs](../../modules/runtime-diagnostics/diagnostics-jobs/README.md) bieten dieselben Programme über das vorhandene Jobformat und die vorhandene Steuerung an.

Diagnosejobs zeigen ihren Ablauf im Debug Log und schreiben Berichte sowie Wiederherstellungsdateien in den konfigurierten Exportordner unter `diagnostics/<job-id>/`. Der Job `capture.factory-state` liest die benötigten `reflection.json` und `components.json` aus dem Export von `capture.runtime`. Die oben gezeigten direkten Entwicklungsaufrufe behalten ihre ausdrücklich gewählten Capturepfade.

`-VerifyBufferAdapters` startet den konkreten Differentialnachweis. Der bereits vorhandene Rust-Korpustest wird ausschließlich mit `-RunCallbackCorpusTests` oder direkt über `dev/profiles/checks/run_callback_corpus_tests.ps1` ausgewählt. Eine normale Erfassung startet keine Tests.

## Einen neuen Build automatisch vorbereiten

Die dynamische Registrierung erkennt Komponentennamen, Indizes, Größen und Originalcallbacks. Die Eigentümerketten der Spielobjekte, Speicherlayouts, Autorität und Lebensdauer der Dienste bleiben eigene Verträge. Seit Profilformat 2 enthalten die Profile auch die bislang im nativen Code festgelegten Queue- und Spawn-Layouts. Eine neue EXE-Identität erfordert deshalb weiter deren Prüfung. Die Profilpflege verwendet den externen JSON-Katalog unter dem konfigurierten `runtime/profiles`-Verzeichnis. Reine Datenänderungen erfordern keinen neuen DLL-Build.

Das optionale native Modul [Game Profile Discovery](../../modules/game-profile-discovery/README.md) wird auf der Spielprofilseite über `modules.gameProfileDiscovery.enabled` aktiviert und ist standardmäßig eingeschaltet. Ein ausdrücklich gespeichertes Aus bleibt wirksam. Es bewahrt die exakte EXE eines normalen passenden Profils im vorhandenen Cache auf. Für einen späteren unbekannten Build vergleicht es die beiden Originaldateien und prüft sämtliche benötigten Adressen und Strukturfelder. Dazu dienen übereinstimmende geladene Sektionen oder einzeln belegte Beziehungen zwischen Originalfunktionen, Datenankern und ihren tatsächlichen Zugriffen. Erst ein vollständig belegter Vertrag erzeugt ein `beta_*`-Profil im vorhandenen Profilordner. Normale Profile haben immer Vorrang. Fehlende Quell-EXE, mehrdeutige Adressen und unbelegte Layoutfelder erzeugen einen konkreten offenen Nachweis. Das Modul läuft innerhalb der vorhandenen Runtime und benötigt weder Python noch eine zusätzliche DLL. Seine Windows-Aufrufe verwenden unter Proton dieselbe Laufzeitumgebung wie das Spiel. Ein eigener Proton-Lauf ist damit noch nicht nachgewiesen.

Die Erfassungsaktion wählt bei mehreren vorhandenen Profilen zuerst ein eindeutig exakt passendes Profil anhand Zielprozess, KFC-Format, SHA-256, PE-Zeitstempel und Imagegröße. Für einen neuen Build kann `-ClientBaselineProfile` oder `-ServerBaselineProfile` eine Baseline vorgeben. Ohne Vorgabe dient die höchste numerische Build-ID desselben Zielprozesses und KFC-Formats als Vergleichsanker. Mehrdeutige exakte Profile oder höchste Baselines führen zu einem konkreten Fehler. `profile-selection.json` hält die Auswahl fest. Eine solche Baseline bestätigt keine Laufzeitfreigabe.

```powershell
pwsh -NoProfile -File dev/profiles/capture/capture_runtime.ps1 `
  -Target client -Offline `
  -ClientBaselineProfile modules/game-runtime/profiles/enshrouded/client/26320373.json `
  -OutputDirectory dev/profiles/captures/new-client-build

$inspection = 'modules/runtime-diagnostics/diagnostics-jobs/game_inspection'
python "$inspection/compare/prepare_profile_rebase.py" `
  modules/game-runtime/profiles/enshrouded/client/26320373.json `
  dev/profiles/captures/previous-client/client `
  dev/profiles/captures/new-client-build/client `
  'C:/Program Files (x86)/Steam/steamapps/common/Enshrouded/enshrouded.exe' `
  --build-id 26404966 --out dev/profiles/captures/client-rebase-review
```

Der Quellcapture muss zur exakten Baseline-EXE gehören. Der Zielcapture muss zur übergebenen neuen EXE passen. Die Aktion prüft auch das benachbarte KFC-Format und lehnt einen Wechsel zwischen Client und Server ab. Sie erstellt eine vollständige Kopie mit neuer Build-ID und Imageidentität. Alle Funktionen, Flags, zusätzlichen Felder und ursprünglichen Provenanceangaben bleiben erhalten. Historische Profile werden nicht verändert.

`<Build-ID>.candidate.json` und `rebase-review.json` entstehen ausschließlich in einem neuen Verzeichnis außerhalb des Produktionskatalogs. Der Bericht enthält den Baselinehash vor und nach dem Vorgang, jeden veränderten Blattwert, statische Funktionskandidaten und noch offene Zahlen-, Eigentümer-, Autoritäts-, Lebensdauer- und Liveprüfungen. Statische Rankings ändern keine RVAs. Übernommene Provenanceangaben beschreiben weiterhin die Baseline und bestätigen keine Freigabe des Zielbuilds.

Mit `--reviewed-changes <datei.json>` lassen sich bereits nachgewiesene Zahlenänderungen reproduzierbar übernehmen. Diese Datei enthält `source` und `target` mit den vollständigen Imageidentitäten aus `rebase-review.json` sowie ein Array `changes`. Jeder Eintrag nennt `path` als JSON Pointer, `before`, `after` und ein Array `evidence`. Ein Nachweis nennt `path`, `sha256` und eine konkrete `observation`. Nachweispfade sind relativ zur Änderungsdatei oder absolut. Die Aktion prüft Imagezuordnung, bisherigen Wert und tatsächliche Nachweisbytes. Sie erlaubt nur bestehende nichtnegative Zahlenwerte. Identität, Formatversion, Flags, Texte und Provenance lassen sich mit diesem Plan nicht ändern. Auch unveränderte Zahlen können mit identischen `before`- und `after`-Werten samt Nachweis als geprüft dokumentiert werden. Die inhaltliche Richtigkeit der Beobachtungen und tatsächliche Spieleffekte bleiben gesonderte Prüfungen.

## Quellen, Ergebnisse und Freigaben

| Ort | Inhalt |
| --- | --- |
| `catalogs/` | Gepflegte Funktionssignaturen und kleine erfasste Buildindizes. |
| `schemas/` | Verträge für Profile und Entwicklungskataloge. |
| `captures/` | Ignorierte lokale Eingaben, Originaldateien, Aufnahmen und Entwürfe. |
| `checks/` | Ausdrücklich ausführbare technische Nachweise. |
| `modules/runtime-diagnostics/tests/python/` | Entwicklungswerkzeugtests für die übernommenen Python-Implementierungen. |
| `../../modules/game-runtime/profiles/` | Ausgelieferte Runtimeprofile. |
| `../evidence/runtime/` | Untersuchungsberichte und geschlossene historische Quellbelege. |

`select-build` sucht standardmäßig die Kataloge dieses Profilbereichs. `--catalog-directory` und `--out-dir` wählen ausdrücklich andere vorhandene Eingaben beziehungsweise Ausgaben. Der Aufruf hängt nicht mehr vom zufälligen Arbeitsverzeichnis ab.

Die [frühere Runtime-Discovery-Anleitung](../evidence/runtime/previous-runtime-discovery.en.md) dokumentiert den damaligen Stand. Diese Seite beschreibt die heute vorhandenen Entwicklungsaktionen. Freigabeschritte und exakte SHA-256-Zuordnung bleiben bestehen. Ein Funktionsfund ist keine Freigabe für einen Engineaufruf.
