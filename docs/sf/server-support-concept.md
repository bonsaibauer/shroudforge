# Dedicated-Server-Support fÃ¼r ShroudForge

## Aktueller Stand

Der Serverpfad verwendet dieselbe Runtime und denselben Modordner wie der
Client. `extended.mod.json` steuert Ã¼ber `targets` die Anzeige in der
Modloader-UI und die Auswahl fÃ¼r den laufenden Prozess. ZulÃ¤ssige Werte sind
`client` und `server`; ein Mod fÃ¼r beide Prozesse speichert beide Werte.
Alte EML-Mods ohne Erweiterungsdatei werden bei der Ãœbernahme fÃ¼r Client und
Server eingeplant. `mod.json` bleibt im unverÃ¤nderten EML-Format.

Auf einer isolierten Kopie von
`A:\Gitea\shroudedit\server_backup\server` wurden sieben Mods fÃ¼r den Server
eingeplant. `world-editor` wurde als client-only Ã¼bersprungen. Der echte
Serverordner wurde nicht fÃ¼r `prepare` verwendet.

`prepare` hat auf der Kopie den Server-KFC-Pregame-Pfad durchlaufen und eine
AssetÃ¤nderung committed. `restore` stellt jetzt sowohl
`enshrouded_server.kfc` als auch `enshrouded_server.kfc_resources` wieder her.
Beide Dateien waren nach `prepare` und `restore` bytegenau identisch mit den
Originaldateien.

## Native ServerprÃ¼fung

Das Serverprofil ist auf die geprÃ¼fte `enshrouded_server.exe` festgelegt:

- PE-Zeitstempel: `1778248905`
- PE-ImagegrÃ¶ÃŸe: `31092736`
- SHA-256 ist im Serverprofil gespeichert.
- Die benÃ¶tigten `game_thread`- und `entity_manager`-Hooks haben je einen
  Treffer in ausfÃ¼hrbaren Serverbereichen; ihre Ã¼berschriebenen Originalbytes
  stimmen mit dem Profil Ã¼berein.
- Alle sechs enthaltenen Gameplay-Patchsignaturen haben je einen eindeutigen
  Treffer. Die gefundenen Server-Funktionsgrenzen und Patchoffsets wurden ins
  Serverprofil Ã¼bernommen.
- Der Flugpatch verwendet seinen eigenen Inline-Wert; ein ursprünglicher
  EXE-relativer Displacement darf nicht in einen Trampolinpuffer kopiert werden.
- Die live gelesene Serverregistrierung bestätigt 598 Registrierungen und 517
  Entity-Speicherlayouts. Das ausgelieferte Profil enthält keine zweite Tabelle.
- World-Funktionsadressen, Finish-Aufruf und Actor-Kontext sind serverspezifisch
  aufgelöst. Ein Client-Singleton und ein Cursor-Hook sind nicht erforderlich.
  Die neue Server-Ausführung ist gebaut, aber noch nicht im Spiel getestet.

Client-Lua und Server-Lua sind getrennte Mod-Instanzen. Gemeinsame API-Namen
stellen keine Nachrichtenverbindung her. Der World Editor kann eine
Client-Oberfläche und einen Server-Teil besitzen; sein derzeitiges Paket ist
wegen der Oberfläche weiter client-only, und die Client/Server-Modkommunikation
fehlt. Details und Beispiele stehen in [runtime-registry.md](runtime-registry.md).

Der Windows-Bootstrap startet im Dedicated-Serverprozess keine Modloader-,
World-Editor- oder Debug-Console-Fenster mehr. Datei-Logging und Runtime-Logs
bleiben aktiv.

## Ablauf

1. `inspect <root>` erkennt `enshrouded_server.exe`, liest die Server-KFC-Datei
   und zeigt den Modplan, ohne Mods auÃŸerhalb des Spielprozesses auszufÃ¼hren.
2. Der Modplan liest `targets` aus `extended.mod.json`. Er prÃ¼ft das Ziel bei
   Lua-Planung, AbhÃ¤ngigkeiten und nativen DLLs.
3. `prepare <root>` verwendet `enshrouded_server.kfc` und
   `enshrouded_server.kfc_resources`. Vor der ersten AssetÃ¤nderung wird die
   Originaldatei der Ressourcen als `.bak` gesichert.
4. Im gestarteten Serverprozess wÃ¤hlt der native Provider anhand des
   Executable-Ziels das Serverprofil; Lua erhÃ¤lt `is_server=true`.

## Noch offen

- Die neu gebauten DLLs installieren, Client und Server neu starten und die
  neuen Berechnungs- und World-Bindings mit kontrollierten Spielaktionen prüfen.
- Für jeden Mod Autorität, Replikation und mögliche Korrekturen durch den Server
  testen; gleiche Codezuordnung ist kein Beleg gleicher Netzwerkwirkung.
- Den World Editor in Client-Oberfläche und Server-Ausführung mit geprüften
  Mod-Anfragen aufteilen. Das bloße Ändern von `targets` stellt dies nicht her.
- Historische Installationen ohne ursprüngliches Ressourcen-Backup können
  frühere Originalbytes nicht aus einem späteren Backup rekonstruieren.

## Relevante Dateien

- `src/loader/package/src/registry/manifest_reader.rs`: liest und normalisiert
  `targets` aus der Erweiterungsdatei.
- `src/loader/package/src/env.rs`: wÃ¤hlt Mods und AbhÃ¤ngigkeiten fÃ¼r Client
  oder Server.
- `src/loader/workflow/pregame.rs`: Server-KFC-Ziel und Asset-Backup.
- `src/loader/api/src/load.rs`: Sicherung und Wiederherstellung der
  KFC-Ressourcendatei.
- `src/loader/runtime/profiles/enshrouded/server/1024233.json`: Serverprofil (Build aus der tatsÃ¤chlichen KFC-Version)
  fÃ¼r das geprÃ¼fte Executable.
