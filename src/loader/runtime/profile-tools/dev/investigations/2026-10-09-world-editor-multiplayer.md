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

## Aktualisierung: vorhandene Baueingabe statt zusätzlichem Modtransport

Die frühere Empfehlung eines separaten Modtransports ist für normale Bauaktionen
überholt. Der Client besitzt `ClientPlayerInputData`, der Server das gleich
aufgebaute `PlayerInput.fromClient`. Der neue, exakt auf den Clientbuild
begrenzte Adapter schreibt einen Auftrag im ursprünglichen `client_cursor`-Hook
in diesen Eingabepfad. Er verwendet den originalen Versionshelfer, trennt
Press/Release über verschiedene Eingabeversionen und lässt Baurechte sowie
Ressourcenprüfungen der Engine bestehen.

Die Lua-API ist `runtime.world.building.submit/status/cancel`; der Editor löst
ItemInfo und Ein-Zellen-Rezepte aus den aktuellen Assets auf, wartet auf Auswahl
und beobachtete Änderungen und sendet fehlgeschlagene Aufträge nicht blind
nochmals. Prop-Abbau verlangt zusätzlich ein passendes tatsächliches
Interaktionsziel. Vollständiger Netzwerkablauf und Speicherung wurden mit der
neuen DLL noch nicht ausgeführt. Die obigen Belege für den direkten Serveradapter
bleiben von diesem alternativen Client-Eingabepfad getrennt.

Aktueller Umfang, Bedienung, alle acht Mods und wiederholbare Prüfungen stehen
in [mod-multiplayer.md](../../../../../../docs/sf/mod-multiplayer.md).

## Validierung dieser Änderung

- Acht isolierte Originalcode-ABI-Fälle erfolgreich, vier je EXE.
- 16 bestehende Python-Tests und beide Profilvalidierungen erfolgreich.
- Nativer Release-Build erfolgreich; bestehende Padding-Warnung bei
  `RemovalRequest` bleibt.
- Keine Schreibzugriffe auf die laufenden Spiele, kein Neustart, keine
  Installation. Die erzeugte DLL ist noch kein Ingame-Multiplayernachweis.
