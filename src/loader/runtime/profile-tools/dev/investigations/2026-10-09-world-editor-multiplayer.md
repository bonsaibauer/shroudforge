# World Editor im Multiplayer: Prüfung vom 09.10.2026

## Ergebnis

Eine Client-Oberfläche mit Ausführung auf dem Dedicated Server ist der passende
Weg für den vollständigen Editor. Native World-Bindings existieren auf beiden
Targets. Ein fertiger Multiplayer-Editor besteht damit noch nicht: Modtransport,
Zuordnung zum anfragenden Spieler und der Nachweis von Replikation/Persistenz
fehlen. `targets: ["client", "server"]` allein reicht nicht.

Ein Host-Client ist gesondert zu behandeln: Wenn er die autoritative Welt
ausführt, benötigt er für seine eigenen Editoraktionen möglicherweise keinen
zusätzlichen Modtransport. Ob die vorhandenen Eingriffe andere Spieler und
Spielstände erreichen, ist trotzdem zu testen. `runtime.is_server` unterscheidet
Prozesstypen, nicht zuverlässig Host-Autorität und Remote-Client.

## Neue Belege aus den Originaldateien

Geprüfte EXEs:

| Target | KFC-Build | SHA-256 |
| --- | --- | --- |
| Client | 1076226 | `af2f5a1227911d8aa06b3908d6bd0211838211cae14ea91099cb57d0df990781` |
| Server | 1024233 | `001c1b40ed091d8c1aee583adde3800d7c858ae2c7f4dff54fca2938b2be1637` |

Die drei bisher nur über ihre Hashes betrachteten Ereignisse sind über
`qualifiedHash`, Größe und die geschriebenen Feldoffsets eindeutig zugeordnet:

| Qualified Hash | Originalname | Größe | Erzeuger-RVA Client / Server |
| --- | --- | --- | --- |
| `0xb6fcf706` | `keen::ecs::BuildingPlaceEvent` | 72 | `0x3ebb70` / `0x1c71c0` |
| `0x474baeed` | `keen::ecs::BuildingTearDownEvent` | 72 | `0x3ebcb0` / `0x1c7300` |
| `0xbbaebaf7` | `keen::ecs::UiBuildingEvent` | 24 | `0x3e77f0` / `0x1c2dc0` |

Die ersten acht Bytes stammen vom Basistyp `GameEvent.timeStamp`; sie sind keine
Spielerkennung. Place schreibt `material: MaterialFeedbackId` bei +60,
`trackingItemId: ItemId` bei +64 und `ownerId: EntityId` bei +68. TearDown schreibt
`material` bei +60 und `ownerId` bei +64. UiBuildingEvent enthält unter anderem
`playerEntityId`, `materialItemId` und Erfolgs-/Aktionsflags. Der Finish-Aufruf
erzeugt dieses UI-Ereignis und führt weitere Zustandsoperationen aus; ihn als
bewiesenen Netzwerk-Commit zu betrachten wäre falsch.

**Korrigierter Fehler:** Der native Place-Adapter hatte die beiden letzten
Argumente vertauscht. Originalcode auf beiden Builds schreibt Argument 4 nach
`material` und Argument 5 nach `trackingItemId`. Der Adapter übergibt jetzt
`feedback, tracking`. Die öffentliche Lua-/C-Reihenfolge bleibt kompatibel.
Die beiden Produktionsprofile und der Client-Funktionskatalog beschreiben die
native Reihenfolge nun korrekt.

Der neue Diagnosetest `tools/enshrouded/verify-world-event-abi.py` führt die
originalen Place-/TearDown-Instruktionen isoliert in Unicorn aus. Er ersetzt
nur Queue-Helfer, Allokation und Zeitstempel durch Testdaten. Acht Fälle auf
beiden EXEs bestätigen Feldzuordnung, Owner, Position und Zeitstempel. Die
Reflexionsdatei muss zur SHA-256 der EXE passen. Dieser Test prüft ausdrücklich
keine Netzwerkübertragung oder Speicherung.

Auch der Voxelpfad wurde tiefer verfolgt: Server `0x722d20` → `0x722990` →
`0x722140` → `0x7311b0`. Der letzte Helfer aktualisiert Chunkstrukturen und
trägt unter Bedingungen einen räumlichen Schlüssel in eine Sammlung bei
Store+`0x5100` ein. Das ist noch kein Nachweis, welche Netzwerk-/Save-Verbraucher
diese Änderungen erhalten. Es wäre ebenso unbewiesen, schon jetzt einen
zusätzlichen Dirty-Aufruf als zwingend erforderlich zu behaupten.

## Konkrete Lücken im aktuellen Code

- `mods/world-editor/extended.mod.json` wählt nur Client. `mod.lua` bindet
  Cursor, Hotkeys, UI, lokale Bibliothek, Capture/Paste und Undo zusammen.
- Unter `src/loader` existiert keine Mod-RPC-/Socket-Implementierung. Gleiche
  `runtime.world.*`-Namen adressieren jeweils den eigenen Prozess.
- `EntityRequest` und `RemovalRequest` im nativen World-Adapter tragen keine
  anfragende Spielerkennung. Der nächste passende Actor-Hook kann den Auftrag
  übernehmen; geprüft wird die Welt, aber nicht die Identität des Anfragenden.
  Die `ownerId` kommt aus diesem Actor-Kontext. Vor Mehrspieleraufträgen muss
  die Zuordnung überprüft werden, einschließlich des Destroy-Fallbacks.
- Der Server-Weltkontext verlangt eine Actor-Beobachtung aus den letzten
  500 ms. Verfügbarkeit ohne Spieler beziehungsweise ohne passende laufende
  Bau-Callbacks ist noch nicht belegt. Eine Netzwerkverbindung allein behebt
  das nicht.
- `set_entity_scale_on_game_thread` schreibt direkt in `CurrentTransform`.
  Es gibt dabei keinen expliziten Replikationsaufruf. Ob die Engine diese
  Änderung selbst erfasst, ist offen.
- Paste/Undo speichern lokale Entity-Handles und Vorzustände im Client-Lua.
  Diese Handles dürfen nicht als Server-Handles übertragen werden.
- Der derzeitige Erfolgstext bestätigt lokale ECS-/Voxel-Rücklesung und sagt
  ausdrücklich, dass Save-Persistenz nicht geprüft ist.
- Beide Spielprozesse liefen bei der Prüfung. Der installierte Server-Loader
  meldete zuletzt `voxel_context=waiting`; die neuen World-Bindings wurden
  in dieser Prüfung nicht installiert oder live ausgeführt. Die verschiedenen
  KFC-Builds sind kein Beleg kompatibler Spiel-Netzwerkprotokolle.

## Ein gemeinsamer Ausführungspfad

1. Der Client behält Cursor, Auswahl, Vorschau und Blueprint-Dateien. Er sendet
   semantische Aufträge: Bereich erfassen, Blueprint platzieren, Auftrag rückgängig
   machen. Keine Adressen, Registry-Pointer oder lokalen Entity-Handles senden.
2. Ein wiederverwendbarer Modtransport im Loader verbindet Client-Mod und
   Server-Mod. Zunächst ist ein separater authentifizierter Transport technisch
   möglich. Die Nutzung des bestehenden Spielkanals setzt dessen weitere
   Auflösung voraus; aus ECS-Ereignishashes folgt kein frei nutzbarer RPC-Kanal.
3. Der Server ordnet die Verbindung dem tatsächlichen Spieler und dessen
   Baurechten zu. Eine vom Client behauptete `ownerId` reicht nicht. Er löst
   Templates/Rezepte selbst auf und führt begrenzte Jobs im passenden
   Game-Thread-/Actor-Kontext aus. Wiederholte Nachrichten müssen denselben
   Auftrag erkennen, statt einen Blueprint doppelt zu platzieren.
4. Capture, Änderungen und Undo-Journal liegen bei der autoritativen Welt.
   Undo prüft zwischenzeitliche Änderungen anderer Spieler. Große Blueprints
   werden über mehrere Ticks verarbeitet; die aktuelle Obergrenze von bis zu
   einer Million Props ist kein akzeptables synchrones Netzwerkjob-Limit.
5. Bestehende `runtime.world.*`-Bindings bleiben der lokale Engine-Adapter.
   Der World Editor erhält einen gemeinsamen Auftragskern für lokale und
   serverseitige Ausführung. Profile enthalten weiter ausschließlich
   buildabhängige Bindings, keine Netzwerk- oder Editorlogik.

Unmodifizierte Mitspieler könnten Änderungen über die normale Spielreplikation
sehen, sofern die serverseitigen Eingriffe diesen Pfad vollständig bedienen.
Bewiesen ist das noch nicht. Der bedienende Remote-Client braucht die
Editor-Oberfläche und den Modtransport. Ein rein serverseitiger Importjob ohne
Client-UI ist ebenfalls möglich, ersetzt aber keinen interaktiven Editor.

## Nächster belastbarer Spieltest

Zunächst auf einer Weltkopie den Serveradapter lokal ansteuern, bevor ein
vollständiges Editorprotokoll gebaut wird: je ein Prop erzeugen, platzieren,
löschen, skalieren und einen kleinen Voxelbereich ändern. Für jede Aktion
separat Server-Rücklesung, Sicht eines verbundenen Clients, erneutes Verbinden
und Save/Neustart prüfen. Danach zwei Spieler gleichzeitig testen: korrekter
Owner, getrennte Aufträge und konfliktbewusstes Undo. Der Serverkontext ist
zusätzlich mit und ohne aktiven Bauvorgang zu prüfen.

Erst das beantwortet, welche nativen Publikationswege noch ergänzt werden
müssen. Ein reiner Transporttest würde diese zentrale Frage offenlassen.

## Validierung dieser Änderung

- Acht isolierte Originalcode-ABI-Fälle erfolgreich, vier je EXE.
- 16 bestehende Python-Tests und beide Profilvalidierungen erfolgreich.
- Nativer Release-Build erfolgreich; bestehende Padding-Warnung bei
  `RemovalRequest` bleibt.
- Keine Schreibzugriffe auf die laufenden Spiele, kein Neustart, keine
  Installation. Die erzeugte DLL ist noch kein Ingame-Multiplayernachweis.
