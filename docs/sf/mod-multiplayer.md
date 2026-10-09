# Mod-Ausführung: lokal, Client und Server

Stand 2026-10-09. Die ursprünglich acht Pakete wurden gegen ihren Lua-Einstieg, ihr Manifest,
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
| `sf-auto-stamina-refill` | `network_player_attributes`: `Stamina = Stamina_Max`, vor dem Netzwerk-Attributsnapshot | Binding vorhanden; ursprüngliche Verbrauchsberechnung wird damit nicht entfernt | Ändert diesen Client-Prozess; spätere Snapshots können Werte ersetzen | Separates Server-Binding vorhanden. Beide Instanzen ändern ihre eigene Ausführung; keine automatische Kopplung der Schalter |
| `sf-no-fall-damage` | `fall_damage_infliction`: Health-Schreibzugriff überspringen, Neuberechnung erhalten | Binding vorhanden | Lokale Aktivierung beweist keine Entscheidung des Hosts | Server-Binding vorhanden; für serverseitig berechneten Fallschaden ist dessen Ausführung maßgeblich |
| `sf-unlimited-flight` | `actor_rotation`: einen Skalar durch `-1.57f` ersetzen; ursprünglicher Variablenname unbekannt | Binding vorhanden, Abhängigkeit von Stamina-Mod bleibt | Bewegungsprognose und Serverkorrekturen müssen zusammen geprüft werden | Eigenes Server-Binding vorhanden. Beide installieren bedeutet zwei Modifikationen derselben Systemart, keine verdoppelte RPC-Aktion |
| `sf-no-resource-cost` | Sechstes Argument einer gemeinsam verwendeten Inventarfunktion auf null setzen | Erreicht u. a. Bauen, Crafting, Nutzung und weitere Aufrufer | Client-UI und lokale Aufrufe können reagieren; entfernt keine Prüfung in einem anderen Prozess | Der Prozess, der den Inventarvorgang abwickelt, muss den Eingriff ausführen. Wirkung ist breiter als nur Rezepte |
| `sf-infinite-item-use` | Sechstes boolesches Argument einer gemeinsamen Inventarfunktion auf false setzen | Binding vorhanden | Kein Nachweis serverseitig unendlicher Nutzung durch lokale Aktivierung allein | Server-Binding vorhanden; Nutzung, Ausrüstung und weitere Aufrufer teilen sich diesen Pfad |
| `sf-infinite-item-split` | Subtraktion vom Quellbestand in gemeinsamem Inventarhelfer unterdrücken | Binding vorhanden, nicht auf Teilen beschränkt | Inventartransaktion und Rückabgleich entscheiden über das Ergebnis | Server-Binding vorhanden. Aufrufer umfassen Crafting, Ausrüstung, Loot und andere Systeme; frühere Aussage „split-spezifisch“ korrigiert |
| `sf-unlock-blueprints` | Beim Assetstart Rezeptbedingungen auf `Unlock_Flame_Altar_PK / NPC_Flame_Hint01` umstellen | KFC-Transformation im gestarteten Spiel | Verändert Rezeptdaten dieser Installation; Anzeige ist keine serverseitige Freigabe | Für übereinstimmende Rezeptbedingungen auf beiden Installationen vorbereiten. Kein direkter Spielstand-Unlock und kein Runtime-Mod |
| `sf-production-time` | Positive `RecipeInfo.craftingDuration` auf eine feste Basisdauer setzen | Vor Spielstart vorbereiten; Weltfaktor gilt zusätzlich | Gleiche Einstellung wie beim Host für passende Anzeige verwenden | Vor Serverstart separat vorbereiten. Keine automatische Übertragung der Mod-Einstellung; standardmäßig deaktiviert |
| `world-editor` | Client-Cursor, UI und Blueprintdateien; direkte native Welt-API im Singleplayer; SFBP-V7-Transport per Steam P2P im beigetretenen Multiplayer | Direkte Platzierung und Undo bleiben lokal | Sendet denselben V7-Blueprinttext samt Cursoranker an die konfigurierte Server-Peer-ID; F4 sendet nur das Undo-Token zurück | Derselbe Mod läuft headless als Serverziel, validiert die Peer-Allowlist und führt Paste, Snapshot, Readback und Undo in seinem eigenen nativen World-Editor-Journal aus. Replikation und Persistenz bleiben Liveprüfungen |

## Tatsächlich laufender Stand der früheren Prüfung (vor dem P2P-Ziel)

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

## World Editor: integrierter P2P-Ablauf

Der Editor ist für `client` und `server` deklariert. Der Client behält Cursor,
UI, Capture und lokalen Blueprintspeicher. Er wählt den Schreibpfad automatisch
aus der nativen Weltquelle: ein bestätigter direkter Kontext verwendet die
lokale Runtime; ein bestätigter cursor-abgeleiteter read-only Kontext verwendet
Steam P2P zum Dedicated Server. Ist der Kontext nicht sicher verfügbar, sendet
F7/F4 weder einen lokalen Schreibaufruf noch einen P2P-Auftrag. Der Server lädt
den Mod headless und führt angeforderte Änderungen mit seiner nativen Runtime
aus.
P2P überträgt dieselbe `.sfbp`-V7-Datei; es gibt keinen zweiten
Zell-/Prop-Befehlskatalog und keine ECS-Handles im Netzwerk. Der Server lädt
den Mod ohne UI, prüft das vorhandene V7-Format und nutzt denselben
`paste_voxels`/`undo_voxels`-Pfad wie lokale native Änderungen.

Wenn Client und Dedicated Server in derselben Windows-Sitzung laufen, ermittelt
der Client die aktuelle Server-SteamID64 automatisch. Enshrouded kann die
Server-ID nach einem Neustart ändern; deshalb wird die Live-ID des laufenden
Servers verwendet. Für entfernte Server bleibt `serverSteamId` auf dem Client
der manuelle Fallback. Die Client-SteamID64 steht im Clientlog und muss in
`allowedClientSteamIds` auf dem Server stehen. Der Server
nimmt nur gelistete Peers und Nachrichten des `world-editor`-Modkanals an.
Steam-Peer-Autorisierung ordnet einen Prozess zu; sie beweist keine
Ingame-Spieleridentität. Transfers sind auf 32 MiB begrenzt und werden mit
Größe, Chunkfolge und Prüfsumme validiert.

Der Server hält vorübergehend genau ein Undo-Journal, wie der vorhandene lokale
Editor ebenfalls nur den letzten Paste speichert. F7 wird bis zum Abschluss
quittiert; F4 enthält ausschließlich das Server-Token. Native Readback ist
noch kein Beleg für Sichtbarkeit bei einem zweiten Client oder Persistenz nach
Wiederbeitritt. Dafür sind weiterhin ein Server-Canary, ein zweiter Client und
ein Save/Rejoin-Test nötig. Die P2P-Transport- und Server-Undo-Lua-Pfade haben
isolierte Integrationstests; ein Live-Spieltest ist damit nicht vorgetäuscht.

## Wiederholbare Prüfung

- `profile-tools/dev/tools/enshrouded/audit-mod-origins.py`: alle neun tatsächlichen
  Lua-Einstiege, Prozessziele, Modifier-Ursprünge und nativen Guards; Aufruf mit
  `--help`. Ausgabe kennzeichnet `multiplayer_gameplay_verified: false`.
- `verify-building-input.py`: Original-Eingabelayouts, Aktionsbits, Hookregister
  und originaler Versionshelfer aus EXE und Reflection.
- `cargo test -p shroudforge-api --test world_editor_p2p --offline`:
  P2P-Chunk-Reassembly, Integritäts- und Allowlistprüfungen, Deduplizierung,
  Server-Paste samt Snapshot/Undo über gemockte native World-APIs, Blueprintspeicher
  und Kompilierung aller Mod-Lua-Dateien.
- `inspect_building_inputs` liest die Ressourcen erneut aus EXE/KFC ohne Typcache.
  Mit `SF_BUILDING_RESOURCE_SNAPSHOT` und `-- --include-ignored` prüft derselbe
  Lua-Planer zusätzlich diesen frischen Export.
- CMake-Ziel `kfc-runtime-test-building-input`: tatsächlicher nativer Adapter
  gegen einen isolierten Eingabepuffer, einschließlich Mehrfachbesuch im selben
  Tick, Release-Pose, Abbruch, fremdem Spieler und falschem Build.

Die ausführbaren Profile bleiben unter `src/loader/runtime/profiles/enshrouded/`.
Diagnoseausgaben unter `target/` sind keine ausgelieferten Bindings.

## Schmelzen: aktueller Live-Nachweis

Am 09.10.2026 wurde auf dem aktiven Server ein Kupferlauf beobachtet: zehn
Ausgabebarren, unveränderte Zutatenbestände, kein Kupfererz im erfassten Eingang.
Der Produktionszyklus dauerte 300,0167 Sekunden. Die Nullkosten-Modifikation
funktionierte in diesem Lauf. Der Zeitunterschied entstand durch 320 um Faktor
400 abweichende Rezeptdauern in Client und Server, einschließlich ihrer Backups.
`sf-production-time` macht die Basisdauer nun über die vorhandene Lua-Asset-API
ausdrücklich einstellbar; gleiche Werte sind auf beiden Installationen nötig.
Der echte KFC-Schreib-/Lesezyklus wurde auf isolierten Kopien beider Builds
geprüft. Siehe [Messung und Aufrufpfad](../../src/loader/runtime/profile-tools/dev/investigations/2026-10-09-smelter-production.md).
