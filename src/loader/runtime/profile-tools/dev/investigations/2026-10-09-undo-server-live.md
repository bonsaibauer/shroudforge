# Undo und World Editor auf Dedicated Server: Live-Untersuchung

Client PID 28012, Server PID 26152. Keine Weltänderungen, kein Neustart und kein
DLL-Austausch während dieser Untersuchung. Einmalige Runtime-Diagnosesnapshots
auf beiden Prozessen; anschließend automatisch beendet. Nicht-invasives CDB
liest ausschließlich Code und Speicher der laufenden Clientinstanz.

## 1. Direkter Undo-Auftrag wird nicht abgearbeitet

Clientlog: +104.232s Paste von capture-3 erfolgreich, 1170 Voxel und ein Prop.
+110.453s Fallback nach fehlgeschlagenem direkten Destroy für lokalen Handle 8.
+114.215s Fallback abgebrochen: native ECS query failed or timed out.

Live-Zähler: actor-placement=0/0, building-dispatch=8/0, completed=0;
removal phase=0, success=0, thread=0, owner=0.

Codepfad in world_runtime.cpp:
DestroyEntityHandle -> queue_removal -> pending_removal.phase=1.
Nur OnActorPlacement konsumiert pending_removal. Dieser Hook wurde in der
Clientsession nicht aufgerufen. Nach OperationTimeoutMs=3000 wird Phase 1
wieder auf 0 gesetzt. OnBuildingDispatch wurde aufgerufen, liest aber nur
pending_entity_request und kann diesen handlebasierten Auftrag nicht finden.
Die zwei Auftragskanäle sind nicht verbunden. Hook-Installation wird außerdem
als bereit gemeldet, ohne einen tatsächlich beobachteten Aufruf zu verlangen.

mod.lua hält remove_reason zunächst in detail, verwirft ihn aber beim Start
des Fallbacks zugunsten des generischen Texts direct destroy was unavailable.
Die ursprüngliche Warnung unterschlägt somit die native Fehlerdiagnose.

## 2. Fallback-Spielersuche bricht vor dem Abbau ab

game_building.lua sucht ClientPlayerInput + NetworkCursor + SlotSelection.
EcsRuntime::query_on_game_thread scannt höchstens 256 Einträge je Aufruf.
Live: 60 unvollständige Abfragen, cursor=15360, total=131072, matches=0;
queryFailures=1, dispatcher.timeouts=1. Invoke wartet standardmäßig nur 75ms.
Bei Fehlschlag gibt KfcRuntimeEcsQuery SIZE_MAX zurück. Die Rust-Brücke macht
daraus die beobachtete Meldung. Lua toleriert nur still scanning, nicht diesen
Fehler, und stoppt die Queue. Der gespeicherte Scanstand passt exakt zu 60*256.
Das ist ein anderer Scanner als query_props_in_bounds aus dem vorigen Fix.
Ein Nulltreffer-Zwischenstand ist kein Nachweis eines fehlenden Spielers.

## 3. Nach Offline -> Dedicated Server: ungültiger Client-Entity-Manager

Clientdiagnose: layoutReady=false, layoutEpoch=4, dispatcher.ready=true,
entityManagerChanges=1. Server: layoutReady=true, dispatcher.ready=true,
queryFailures=0, timeouts=0. Beide haben eine gelesene Komponentenregistrierung.

CDB-Auswertung der geladenen DLL und anschließende reine Speicherabfrage:
GameThreadDispatcher::EntityManager() liest 0x3006CB092C0.
Die geladenen Profiloffsets sind count=0x158 und table=0x188.
[manager+0x158] = 0; [manager+0x188] = 0; layout_ready = false.
Die Adressen stimmen mit den im letzten live_layout gespeicherten Adressen
überein. Das ist kein bloßes Statusanzeigeproblem.

EcsRuntime::Tick prüft nur diesen erfassten Manager, setzt bei count=0 oder
ungültiger Tabelle layout_ready=false, löscht Handles und kehrt zurück.
Neuerfassung erfolgt ausschließlich im separaten capture_entity_manager-Hook.
Nach dem Weltwechsel wurde kein neuer Manager übernommen. Warum dieser Hook
den gültigen Client-Manager der Onlinewelt nicht erfasst, ist damit noch nicht
abschließend geklärt; blindes Ändern von Profiloffsets wäre nicht belegt.
Die API-Verfügbarkeitsprüfung sperrt damit u.a. set_scale und Prop-Abfragen.
Ein betriebsbereiter Server repariert den clientseitigen Zeiger nicht.

## 4. Serverzugang und Spielwarnungen

Server: Session accepted +228.174s, Steam authenticated +229.841s,
Player logged in +236.763s; Rechte enthalten CanEditBase, CanEditWorld,
CanExtendBase und CanAccessInventories. Client: JoinGame +226.710s,
Client_Online +226.715s, Gameplay Ready +233.577s. Danach 11-12ms Ping,
lost=0 und OperatingNormally. Die jeweiligen Zeiten sind sessionrelativ.
Der Nutzer bestätigte: Gemeint ist der Editor, nicht eine Login-Ablehnung.

AuthToken-/machine-Warnungen liegen vor dem erfolgreichen Login. Die volle
Client-Wasserqueue beim Laden und der Serverfehler SInt32/Float sind reale
separate Meldungen; sie belegen weder eine Login-Ablehnung noch die Ursache
des nativen Editorfehlers. Keine Behauptung, diese Spielwarnungen seien behoben.

## Konsequenz für die Reparatur

- Den handlebasierten Entfernungsauftrag mit einem tatsächlich laufenden,
  validierten Kontext-Hook verbinden; Identitäts-/Weltprüfungen beibehalten.
- Native Fehlerdetails nicht im Lua-Fallback verlieren.
- Spielersuche gezielt/asynchron ausführen und vor dem Senden von Eingaben
  vorübergehende Dispatcher-Ausfälle differenziert behandeln. Kein blindes
  Wiederholen bereits gesendeter Bauaktionen.
- Client-Manager nach Weltwechsel zuverlässig neu erfassen und alte
  Undo-Journale an ihre Ursprungssitzung binden.
- Im Multiplayer darf der direkte lokale Schreibweg nicht als serverseitig
  bestätigte Bearbeitung dargestellt werden; der Eingabepfad benötigt ebenfalls
  eine gültige Client-Anbindung.

Die vorherigen synthetischen Tests prüfen diese realen Hook-Aufrufe und den
Offline-Online-Wechsel nicht. Es wurde in dieser Untersuchung kein neuer
nativer Reparaturversuch installiert und keine vollständige Undo-Sicherheit
behauptet.

## Lokale Belege

build/Enshrouded-undo-diagnostics.json
build/EnshroudedServer-undo-diagnostics.json
build/editor-client-context-disassembly.txt
build/editor-client-tick-disassembly.txt
build/editor-client-manager-memory.txt
build/editor-client-manager-confirm.txt

## Reparaturstand nach dem Git-Vergleich

- 712e328 (28.09., 1.5.0) führte die allgemeine ECS-Suche mit 256 Slots
  pro Schritt und die einzelne Entity-Manager-Erfassung ein. Das sind ältere
  Einschränkungen, nicht erst Änderungen des letzten Commits.
- 9cc5c80 (30.09., 1.6.3) enthielt zwei Verbraucher für pending_removal.
  3755dbc (01.10., 1.6.3) entfernte execute_pending_removal_dispatch und
  seinen Aufruf in OnBuildingDispatch. Das erklärt die beobachtete Warteschlange
  bei actor-placement=0 und laufendem building-dispatch konkret.
- cae2230 (09.10., 1.6.4) fügte game_building.lua hinzu. Dessen Spielersuche
  verwendet den älteren allgemeinen ECS-Scanner und brach beim ersten Timeout ab.

Implementiert: zweiter Löschverbraucher wiederhergestellt, validierter Kontext
vor dem atomaren Claim, Rekursionsschutz, kein zusätzlicher native_finish-Aufruf
im Entry-Hook (die originale Engine-Funktion übernimmt die Veröffentlichung).
ResetContext verwirft nur noch nicht konsumierte handlebasierte Löschaufträge.
Native Fehlerdetails bleiben im Lua-Fallback-Log erhalten.

Die allgemeine Spielersuche hat jetzt ein Slotlimit von 8192 bei weiterhin
1 ms Zeitbudget. Lua wiederholt kurze Fehler ausschließlich bei der lesenden
Spielersuche innerhalb der bestehenden 15-s-Frist. Gesendete Bauaktionen werden
nicht wiederholt. Die Anzahl der Schritte hängt weiter von den echten Tabellen ab.

Der laufende Cursor-Hook gibt seine aktuelle Execution View an den Dispatcher
weiter. Die Kette nutzt weiterhin den profilierten lookup_manager-Offset;
Count, Table und beide Tabellenenden werden vor Übernahme geprüft.
Keine Produktions-Speichersuche und keine gespeicherten Debugger-Adressen.
Die Disassemblierung bestätigt R13 als ursprünglichen RCX-Parameter des
Cursor-Systems (RVA 0x249030, mov r13,rcx bei 0x24905a).
Der tatsächliche neue Online-Manager muss nach Neustart live überprüft werden.

runtime.world.session_id() liefert einen konservativen ECS-Sitzungsmarker
oder 0 bei fehlender/ersetzter Tabelle. Der optionale neue DLL-Export verhindert
ABI-Ladefehler mit alten DLLs; ohne Marker verweigert der neue Editor eine
Platzierung. Direkte Undo-Journale und bestätigte Input-Journale sind daran
gebunden. Andere/unverfügbare Sitzungen sperren Undo, behalten das Journal und
senden keine Änderung. Ein erneuter Beitritt hebt diese Sperre absichtlich nicht
auf: Eine dauerhafte Welt-ID und konfliktgeprüfte Wiederaufnahme sind damit
nicht implementiert. Auch laufende Input-Jobs stoppen beim Sitzungswechsel.

Validierung vor Installation:
- cargo test -p shroudforge-api --test world_editor_building: 3 bestanden,
  1 Ressourcenexport-Test unverändert ignoriert.
- Native echte ECS-Implementierung mit synthetischen Tabellen: 35 Prüfungen,
  einschließlich 131072 Slots und Ergebnisverwerfung bei Epochenwechsel.
- Native Weltkontext-Fixture: 19 Prüfungen, einschließlich zweitem Verbraucher
  ohne Actor-Hook, fremdem Kontext, Rekursion, einmaliger Veröffentlichung,
  wartender/ausgeführter Löschung und Sitzungswechsel/ungültigem Manager.
- Release-Loader und native DLL gebaut; git diff --check bestanden.

Diese Tests ersetzen keinen Live-Paste/Undo und keinen Offline-Online-Test.
Client und Server wurden vom Nutzer regulär beendet; die neue Installation
wird separat mit SHA-256-Prüfung und Wiederherstellungskopien protokolliert.

## Erster Neustart: Quellenkonflikt gefunden und in Revision 2 korrigiert

Client PID 4248, Start 06:06:06 lokal. Log zeigt zwei Host_Offline-Beitritte,
keinen Client_Online-Beitritt. Der erste neue Live-Test war damit Singleplayer.
Der neu ergänzte Cursor- und der vorhandene Lookup-Hook überschrieben ihren
Manager gegenseitig. Diagnose: entityManagerChanges=6144, layoutEpoch=1466,
layoutReady=false. Die neue Sitzungsprüfung sperrte dadurch Platzierungen
(World session is unavailable); es wurde nicht als erfolgreiche Reparatur gewertet.
Beleg: build/client-manager-source-conflict.json.

Revision 2 priorisiert für 1000 ms seit der letzten Cursor-Beobachtung die
aktuelle lokale Spielerquelle gegenüber allgemeinen (auch Preview-)Lookups.
Neue lokale Spielerwelten dürfen weiterhin übernehmen, ungültige Tabellen nie.
Die native Fixture prüft nun zusätzlich zehn konkurrierende gültige Lookups,
stabile Sitzungsidentität und Wechsel der priorisierten Quelle: 23 Prüfungen,
zusammen mit dem ECS-Test 58. Alle bestanden. Revision 2 benötigt erneut einen
Live-Test; ihre Wirksamkeit wird nicht aus dem synthetischen Test abgeleitet.

Die Prozessrolle is_server ist keine Singleplayer/Multiplayer-Erkennung.
Der Editor wählt den Schreibweg weiterhin explizit über executionMode:
direct für lokale/Host-Welten, game für gewöhnliche Eingaben über die Verbindung.
Eine automatische native Erkennung der Netzwerksitzung wurde nicht ergänzt.

## Revision 2 live und korrigierter Installationsumfang

Client PID 3576: layoutReady=true, layoutEpoch=5, entityManagerChanges=2,
entityContextReady=true, dispatcher.ready=true, timeouts=0. Der Quellenkonflikt
blieb im laufenden Singleplayer damit aus. Trotzdem erschien beim Platzieren
weiterhin World session is unavailable.

Die Modul-Liste des echten Spielprozesses zeigte den Installationsfehler:
shroudforge-runtime.dll war noch die alte Datei vom 09.10.2026 05:28.
Der Hotfix hatte kfc-runtime.dll, shroudforge.exe und Lua-Dateien enthalten,
aber nicht die im Spiel geladene Rust-CDylib. Die neue Lua-Funktion session_id
war deshalb im laufenden Spiel noch nicht vorhanden; ihr Lua-Fallback lieferte 0.
Dies war ein Fehler im Hotfix-Paket, kein weiterer Beweis eines defekten
Singleplayer-Weltkontexts. Revision 3 ergänzt zwingend die zusammen gebaute
shroudforge_modloader.dll unter dem Installationsnamen shroudforge-runtime.dll.
Buildpfad und Zielname sind mit build.ps1 und bootstrap.cpp abgeglichen.

## Echter Undo-Fehler in Revision 3 und Korrektur in Revision 4

Neuer Clientprozess 27256 installierte erfolgreich 1170 Voxels und drei Props
bei 3735,843,1400. Die Diagnose war stabil: layoutReady=true, layoutEpoch=5,
dispatcher.ready=true, timeouts=0. Log danach:
- Direct destroy: hooks_ready=true, actor_hook_entered=false,
  actor_saw_request=false, building_dispatch_entered=false,
  building_dispatch_saw_request=false.
- Der Editor startete dann den Input-Fallback, aber `game_building.lua` wartete
  in Phase `selection` auf `NetworkCursor.currentBuildingItemId == recipe.id`.
- Nach 15 Sekunden endete der Auftrag, bevor irgendeine Dismantle-Eingabe gesendet
  wurde: `phase=selection`. Das Prop ist laut verifiziertem Placement weiterhin
  im ECS; der Undo-Eintrag blieb erhalten. Dies ist die konkrete aktuelle Ursache.

Revision 4: Eine inverse `dismantle`-Aktion durchläuft die Inventar-/Bauteilauswahl
nicht mehr. Sie wartet direkt auf den Cursor, der exakt den eingefügten Entity-
Handle trifft, prüft den vollständigen Transform erneut und sendet erst danach
`ContextualAction_Hold`. Aiming darf bis zu 60 Sekunden dauern; jede falsche
Ziel-ID bleibt wartend und sendet keinen Input. Client- und Welt-Sitzung werden
weiter bei jedem Tick abgeglichen. Normale `place`- und voxel-`remove`-Aktionen
behalten die vorhandene Rezeptauswahl.

Die gezielte Lua-Regression beweist: kein vorheriger Select-Request für Undo,
keine Dismantle-Eingabe bei falscher Cursor-ID, gesendete Eingabe erst nach
exakter Handle-Übereinstimmung, und 16 Sekunden Wartezeit überschreiten das
normale 15-Sekunden-Limit ohne Abbruch. API-Testlauf: 3 bestanden, 1 unverändert
ignoriert.

## Runtime-Aussetzer während des echten Revision-4-Undo

Der nächste Liveversuch bestätigte, dass Revision 4 die Inventarauswahl korrekt
übersprang und Phase `target` erreichte. Der Auftrag brach dennoch ab, bevor
`ContextualAction_Hold` gesendet wurde. Der Client meldete währenddessen
`stale-game-thread(thread=15756,drain=over-500ms,queued=0,completed=1294,timeouts=4,rejected=0)`;
`api.ecs.query` antwortete kurzzeitig mit `KFC Runtime is not ready`. Der
Spieler-Lesezugriff behandelte diese temporäre Runtime-Sperre als endgültigen
Fehler und `stop()` beendete Undo. Der Hammer war an diesem Fehler nicht
beteiligt: Er ist nur das Zielwerkzeug für die abschließende normale
Dismantle-Eingabe, nachdem die genaue Ziel-Entity im Cursor bestätigt wurde.

Revision 5 unterscheidet den vorübergehend nicht verfügbaren Runtime-Thread von
einem normalen, noch laufenden ECS-Scan. Bei `not ready` oder einer nicht
verfügbaren Welt-Sitzung pausiert der Undo-Auftrag ohne Eingabe; bei Rückkehr
derselben Sitzung wird die Restfrist verlängert und die Zielprüfung fortgesetzt.
Ein ECS-Scan, der einfach nicht fertig wird, bleibt weiter durch die bisherigen
15/60-Sekunden-Fristen begrenzt. Der Regressionstest prüft beide Fälle sowie,
dass beim Aussetzer kein doppelter Input gesendet wird. API-Testlauf:
3 bestanden, 1 unverändert ignoriert.

## Revision 6: blueprint-Undo ohne Cursorziel und unabhängige Terrain-Rücknahme

Die Live-Logs vom 09.10. zeigen den konkreten Ablauf: F4 startete den nativen
Handle-Destroy. Der Client meldete `hooks_ready=true`, aber weder Actor- noch
Building-Dispatch-Hook sah diesen Request. Der alte Fallback wartete darauf, dass
der physische Cursor Handle 3 traf, und beendete sich mit `cursor does not target
pasted handle 3`. Beim späteren Versuch blieb der Job bei kurzzeitig nicht
verfügbarem ECS pausiert; `paste_voxels()` verweigerte F7 während des aktiven Jobs
zuvor still. Das erklärt sowohl das wirkungslose Undo als auch den fehlenden
F7-Hinweis. Der Building Hammer war nicht die Fehlerursache.

Revision 6 ändert den Ablauf gezielt auf das gewünschte F7/F4-Verhalten:

- F4 schreibt und verifiziert zuerst den ursprünglichen Terrain-Snapshot. Die
  Prop-Entfernung kann das Wiederherstellen von Wänden/Blöcken nicht blockieren.
- Für jedes kopierte Prop nutzt Undo dessen gespeicherten Handle und Transform
  und sendet die normale gehaltene Dismantle-Eingabe ohne Cursor-/Aim-Prüfung,
  Inventarauswahl oder Hammer-Auswahl. Der redundante alte Direkt-Hook-/Aim-
  Fallback ist entfernt.
- Ein aktiver Dismantle wird bei erneutem F4 nicht doppelt gesendet; der
  Terrain-Anteil kann trotzdem zurückgesetzt werden.
- Ein Runtime-Lesefehler wird nicht mehr als „Prop ist bereits weg“ gewertet.
  Das Undo wartet mit erhaltenem Journal, bis derselbe Weltkontext lesbar ist.
- F7 meldet während eines noch laufenden Undo ausdrücklich, dass keine neue
  Platzierung gestartet wurde.

Die Regression prüft F4-Dismantle ohne Cursorziel sowie einen Runtime-Aussetzer
bei der Handle-Prüfung und bei der Bestätigung. `cargo test --manifest-path
Cargo.toml -p shroudforge-api --test world_editor_building`: 3 bestanden,
1 unverändert ignoriert.

Der Git-Vergleich hat zusätzlich den Grund gefunden, warum auf dem Server sogar Wände/Blöcke nicht mehr mit F4 verschwanden: Commit `cae2230` führte am 09.10. den Game-Input-Modus ein. Ein Blueprint wird dort als geordnete Liste aus Voxelaktionen und Propaktionen gespeichert. F4 lief die ganze Liste rückwärts ab; da Props zuletzt platziert werden, wartete der Undo zuerst auf deren Dismantle und erreichte die davor platzierten Wand-/Blockzellen nicht, wenn der Prop-Schritt hängen blieb. Revision 6 ordnet nur den Undo-Batch: zuerst alle Voxel-Inversen in Rückwärtsreihenfolge, danach Prop-Inversen ebenfalls in Rückwärtsreihenfolge. Ein Test prüft diese Reihenfolge und die Zuordnung zum richtigen Journal-Eintrag, damit ein Teilerfolg keine falschen Aktionen aus dem Undo-Verlauf entfernt.

In der aktuellen Runtime ist die Stelle für normale Client-Aktionen `BuildingInput::Observe` in `src/loader/runtime/native/world/building_input.cpp`. Sie wird vom profilierten `OnCursorUpdate`-Hook aufgerufen und sendet die Spielereingabe über den normalen Client-/Serverpfad. `DestroyEntityHandle` in `world_runtime.cpp` ist dafür ungeeignet: es wartet auf Actor-/Building-Hooks, die ein eigenständiges F4 nicht auslösen. Die Runtime hat bereits getrennte normale `place`-, Voxel-`remove`- und Prop-`dismantle`-Inputs; Revision 6 nutzt für Undo genau diese Route und die gespeicherten Propkoordinaten.

## Revision 10: Ursache des stehenbleibenden Tisches im nativen Clientcode

Der Live-Log der installierten Revision 9 zeigte `queued held dismantle input`, danach aber nach 25 Sekunden `The requested change was not observed (phase=effect)`. Die Client-EXE hat denselben SHA-256 wie das Profil `enshrouded-client-1076226`, daher sind die folgenden RVA-Befunde für den aktuell laufenden Build.

Die zuvor verwendete `world_cursor`-Signatur liegt bei RVA `0x249E1D` innerhalb der Cursor-Berechnung. Der Runtime-Callback lief vor den originalen Instruktionen. Der native Spielcode berechnet anschließend den tatsächlichen Cursor-Treffer und schreibt die `NetworkCursor`-Auswahl über den nachfolgenden Aufruf bei RVA `0x24F1C0`; damit wurde das vom Mod injizierte Ziel vor der Dismantle-Systemauswertung überschrieben. Die `dispatched`-Meldung bestätigte deshalb nur den synthetischen Tastendruck, nicht den Prop-Treffer. Das erklärt, warum Wände/Terrain (eigene direkte Voxel-Inversen) verschwanden, der Tisch (Prop-Dismantle anhand `NetworkCursor`) aber stehen blieb.

Revision 10 verschiebt ausschließlich die Prop-Zielinjektion an den Einstieg des profilierten `player_building_dismantle`-Systems (Client-RVA `0x281100`). Der genaue Spielbuild enthält die neue Signatur genau einmal und die überschriebenen 12 Bytes stimmen bytegenau überein. Der Cursor kann daher zuerst normal berechnet werden; unmittelbar vor dem nativen Dismantle-System wird nur die exakte gespeicherte ECS-Entity-ID als `GameObjectId::Entity` mit Methode `TearDown` eingesetzt. `serverFlags` bleiben unberührt; der normale Server bleibt für Build-Zonen und Berechtigungen maßgeblich. Der normale gehaltene Interaktionsinput bleibt automatisiert; der Spieler muss weder zielen noch E gedrückt halten.

Verifikation Revision 10: nativer Runtime-Build erfolgreich, Profilstruktur wird von `kfc-runtime-dev validate-profile` akzeptiert, Signatur im installierten Spielbuild eindeutig und bytegenau, 34 native Building-Input-Prüfungen bestanden. Der Lua-Building-Testlauf besteht mit 3 Tests; 1 bestehender Snapshot-Test bleibt wie zuvor ignoriert. Ein Live-Erfolg ist erst nach Laden dieser DLL im neu gestarteten Client messbar.
