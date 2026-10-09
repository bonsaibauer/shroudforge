# Mod-Ausführung: lokal, Client und Server

Stand 2026-10-09. Alle acht Pakete wurden gegen ihren Lua-Einstieg, ihr Manifest,
die aktuellen EXE-Profile, vollständige Aufrufinventare und frisch extrahierte
KFC-Ressourcen geprüft. Codezuordnung und Ingame-Multiplayerwirkung sind getrennte
Nachweise. Der Prüfer führt keine Spielaktionen aus.

## Welche Instanz macht was?

- **Lokales Spiel / selbst gehostetes Spiel:** `enshrouded.exe` ist weiterhin
  das Prozessziel `client`. Sie enthält auch Systeme für die gehostete Welt.
  `runtime.is_client` bedeutet deshalb nicht automatisch „nur Vorhersage“.
- **Einem Server beigetreten:** dieselbe EXE und dieselbe Lua-API, aber die
  Welt und serverseitige Entscheidungen werden vom verbundenen Host verwaltet.
  Direkte Änderungen im Client sind kein automatisch gesendeter Modauftrag.
- **Dedicated Server:** `enshrouded_server.exe`, eigenes Profil, eigene Lua-VM,
  eigene Handles und eigene Einstellungen. Er hat keinen lokalen Editor-Cursor.
- **Beide Prozesse auf einem PC:** zwei unabhängige Instanzen. Maßgeblich ist,
  ob der Client tatsächlich diesem Server beigetreten ist. Ein Serverprozess
  neben einer lokalen Einzelspielerwelt verändert diese lokale Welt nicht.

## Alle mitgelieferten Mods

„Beide“ in der Zielspalte bezeichnet nachgewiesene Bindings in beiden EXEs,
keine Zusicherung identischer Netzwerkwirkung. Die Zuordnung der zuständigen
Spieltransaktion muss bei verbundenem Client beobachtet werden.

| Mod | Tatsächlicher Eingriff | Lokal / Host | Beigetretener Client | Dedicated Server / beide installiert |
| --- | --- | --- | --- | --- |
| `sf-no-stamina-loss` | `network_player_attributes`: `Stamina = Stamina_Max`, vor dem Netzwerk-Attributsnapshot | Binding vorhanden; ursprüngliche Verbrauchsberechnung wird damit nicht entfernt | Ändert diesen Client-Prozess; spätere Snapshots können Werte ersetzen | Separates Server-Binding vorhanden. Beide Instanzen ändern ihre eigene Ausführung; keine automatische Kopplung der Schalter |
| `sf-no-fall-damage` | `fall_damage_infliction`: Health-Schreibzugriff überspringen, Neuberechnung erhalten | Binding vorhanden | Lokale Aktivierung beweist keine Entscheidung des Hosts | Server-Binding vorhanden; für serverseitig berechneten Fallschaden ist dessen Ausführung maßgeblich |
| `sf-unlimited-flight` | `actor_rotation`: einen Skalar durch `-1.57f` ersetzen; ursprünglicher Variablenname unbekannt | Binding vorhanden, Abhängigkeit von Stamina-Mod bleibt | Bewegungsprognose und Serverkorrekturen müssen zusammen geprüft werden | Eigenes Server-Binding vorhanden. Beide installieren bedeutet zwei Modifikationen derselben Systemart, keine verdoppelte RPC-Aktion |
| `sf-no-resource-cost` | Sechstes Argument einer gemeinsam verwendeten Inventarfunktion auf null setzen | Erreicht u. a. Bauen, Crafting, Nutzung und weitere Aufrufer | Client-UI und lokale Aufrufe können reagieren; entfernt keine Prüfung in einem anderen Prozess | Der Prozess, der den Inventarvorgang abwickelt, muss den Eingriff ausführen. Wirkung ist breiter als nur Rezepte |
| `sf-infinite-item-use` | Sechstes boolesches Argument einer gemeinsamen Inventarfunktion auf false setzen | Binding vorhanden | Kein Nachweis serverseitig unendlicher Nutzung durch lokale Aktivierung allein | Server-Binding vorhanden; Nutzung, Ausrüstung und weitere Aufrufer teilen sich diesen Pfad |
| `sf-infinite-item-split` | Subtraktion vom Quellbestand in gemeinsamem Inventarhelfer unterdrücken | Binding vorhanden, nicht auf Teilen beschränkt | Inventartransaktion und Rückabgleich entscheiden über das Ergebnis | Server-Binding vorhanden. Aufrufer umfassen Crafting, Ausrüstung, Loot und andere Systeme; frühere Aussage „split-spezifisch“ korrigiert |
| `sf-unlock-blueprints` | Beim Assetstart Rezeptbedingungen auf `Unlock_Flame_Altar_PK / NPC_Flame_Hint01` umstellen | KFC-Transformation im gestarteten Spiel | Verändert Rezeptdaten dieser Installation; Anzeige ist keine serverseitige Freigabe | Für übereinstimmende Rezeptbedingungen auf beiden Installationen vorbereiten. Kein direkter Spielstand-Unlock und kein Runtime-Mod |
| `world-editor` | Client-Cursor/Dateien/UI; wahlweise direkte Welt-API oder normale `ClientPlayerInput`-Baueingaben | Direkter Modus vorhanden; Spieleingabemodus ebenfalls clientseitig | Neuer Spieleingabemodus implementiert; folgt dem vorhandenen Baupfad und prüft beobachtete Änderungen | Kein Editor-Mod auf dem Server erforderlich für diesen Eingabepfad. Die Annahme durch die Engine, Replikation und Speicherung sind noch nicht end-to-end nachgewiesen |

## Tatsächlich laufender Stand bei der Prüfung

Client PID **12660**, Server PID **6576**: beide Loader meldeten die sechs
Funktions-Mods als aktiv, ohne Aktivierungsfehler, jeweils mit
`write-confirmed`. Das bestätigt die Modifikation der jeweiligen Codebereiche.
Der Editor war im Client aktiv und wurde auf dem Server mit
`wrong process target` abgelehnt. Dieses Paket wird dort nicht benötigt.
Blueprints laufen vor dem Spiel in der Assetphase und gehören deshalb nicht in
die Liste aktiver Runtime-Mods.

Der Client verwendet Profil **1076226**, der Server **1024233**. Die beiden
Profile wurden jeweils gegen die eigene EXE geprüft. Gleiche Lua-Namen oder
Eingabelayouts beweisen keine Netzwerk-Protokollkompatibilität dieser Builds.
Die neuen BuildingInput-Exporte sind nicht Bestandteil der bei dieser Prüfung
geladenen DLL. Eine aktive alte Installation wird durch einen Repo-Build nicht
aktualisiert.

## World Editor bedienen

Unter **World Editor → World edit execution** ist `Game building input
(multiplayer)` der neue Pfad für einen beigetretenen Client. Der Mod wählt
einen aktuellen ItemInfo-Baueintrag, wartet auf die Auswahl im NetworkCursor,
sendet eine Baueingabe und prüft danach Props oder Voxel. `dispatched` ist
ausdrücklich keine Annahmebestätigung des Servers. Direkte Diagnoseknöpfe werden
in diesem Modus nicht ausgeführt.

Der Bauhammer muss ausgestattet bleiben. Normale Reichweite, Baurechte,
Inventar- und Rezeptprüfungen gelten weiter. Ein Auftrag umfasst maximal
4096 Einzelaktionen und läuft schrittweise. F4 stoppt zuerst einen laufenden
Auftrag; danach kann es die beobachteten Änderungen durch inverse Baueingaben
rückgängig machen. Bei unklarem Ausgang wird nicht erneut gesendet. Späte
beobachtete Änderungen werden ins Journal übernommen. Bleibt eine Antwort aus,
bleibt der Auftrag unbestätigt; lokale Rücklesung ersetzt keine Serverquittung.

Für Voxel werden aus den aktuellen KFC-Daten exakt passende Ein-Zellen-Rezepte
ermittelt: im geprüften Client **66 Materialien**. Terrain-/Dichtezellen ohne
solches Rezept werden vor dem ersten Eingriff abgelehnt. Sie werden nicht
näherungsweise durch Baublöcke ersetzt. Capture sieht die im Client geladenen
Weltbereiche; es ist kein Fernzugriff auf ungeladene Server-Chunks.

Beim Entfernen muss das Spiel tatsächlich das betreffende Objekt anvisieren;
der Editor zeigt die Zielkoordinaten an und wartet auf den passenden Cursor.
Eine reine Transformänderung setzt das Interaktionsziel nicht zuverlässig um.
Für Props hält der Adapter anschließend die reflektierten Eingaben
`ContextualAction` und `ContextualAction_Hold` 1,2 Sekunden; der getrennte
`SecondaryBuildingAction` bleibt auf Voxelabbau begrenzt. Ein bereits manuell
abgebautes Paste-Prop gilt beim Undo als erledigt, während ein wiederverwendeter
oder veränderter Handle weiterhin als Konflikt stoppt. Prop-Ersetzung und
Prop-Undo bleiben wegen der Cursorzielprüfung keine unbeaufsichtigten
Massenlöschungen. Serverseitige Skalierungsannahme, Sichtbarkeit beim zweiten
Client und Bestand nach Wiederverbinden/Neustart bleiben separate Spieltests.

## Wiederholbare Prüfung

- `profile-tools/dev/tools/enshrouded/audit-mod-origins.py`: alle acht tatsächlichen
  Lua-Einstiege, Prozessziele, Modifier-Ursprünge und nativen Guards; Aufruf mit
  `--help`. Ausgabe kennzeichnet `multiplayer_gameplay_verified: false`.
- `verify-building-input.py`: Original-Eingabelayouts, Aktionsbits, Hookregister
  und originaler Versionshelfer aus EXE und Reflection.
- `cargo test -p shroudforge-api --test world_editor_building --offline`:
  Auswahlbestätigung, verzögerte Effekte, Abbruch, konfliktsensitives Undo,
  vollständige Vorprüfung, Rezeptplanung und Kompilierung aller Mod-Lua-Dateien.
- `inspect_building_inputs` liest die Ressourcen erneut aus EXE/KFC ohne Typcache.
  Mit `SF_BUILDING_RESOURCE_SNAPSHOT` und `-- --include-ignored` prüft derselbe
  Lua-Planer zusätzlich diesen frischen Export.
- CMake-Ziel `kfc-runtime-test-building-input`: tatsächlicher nativer Adapter
  gegen einen isolierten Eingabepuffer, einschließlich Mehrfachbesuch im selben
  Tick, Release-Pose, Abbruch, fremdem Spieler und falschem Build.

Die ausführbaren Profile bleiben unter `src/loader/runtime/profiles/enshrouded/`.
Diagnoseausgaben unter `target/` sind keine ausgelieferten Bindings.
