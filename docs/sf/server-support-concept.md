# Dedicated-Server-Support für ShroudForge

## Aktueller Stand

Der Serverpfad verwendet dieselbe Runtime und denselben Modordner wie der
Client. `extended.mod.json` steuert über `targets` die Anzeige in der
Modloader-UI und die Auswahl für den laufenden Prozess. Zulässige Werte sind
`client` und `server`; ein Mod für beide Prozesse speichert beide Werte.
Alte EML-Mods ohne Erweiterungsdatei werden bei der Übernahme für Client und
Server eingeplant. `mod.json` bleibt im unveränderten EML-Format.

Auf einer isolierten Kopie von
`A:\Gitea\shroudedit\server_backup\server` wurden sieben Mods für den Server
eingeplant. `world-editor` wurde als client-only übersprungen. Der echte
Serverordner wurde nicht für `prepare` verwendet.

`prepare` hat auf der Kopie den Server-KFC-Pregame-Pfad durchlaufen und eine
Assetänderung committed. `restore` stellt jetzt sowohl
`enshrouded_server.kfc` als auch `enshrouded_server.kfc_resources` wieder her.
Beide Dateien waren nach `prepare` und `restore` bytegenau identisch mit den
Originaldateien.

## Native Serverprüfung

Das Serverprofil ist auf die geprüfte `enshrouded_server.exe` festgelegt:

- PE-Zeitstempel: `1778248905`
- PE-Imagegröße: `31092736`
- SHA-256 ist im Serverprofil gespeichert.
- Die benötigten `game_thread`- und `entity_manager`-Hooks haben je einen
  Treffer in ausführbaren Serverbereichen; ihre überschriebenen Originalbytes
  stimmen mit dem Profil überein.
- Alle sechs enthaltenen Gameplay-Patchsignaturen haben je einen eindeutigen
  Treffer. Die gefundenen Server-Funktionsgrenzen und Patchoffsets wurden ins
  Serverprofil übernommen.
- Beim Flugpatch wurde der RIP-relativen Wert aus den Serverbytes eingetragen;
  der kopierte Clientwert war im Serverbinary anders.
- `world_prop_update` und `world_cursor` wurden im Serverbinary nicht gefunden.
  Der World Editor bleibt deshalb client-only.

`kfc-runtime.dll` baut mit dem eingebetteten Serverprofil. Das belegt Profil-
und Buildintegration sowie statische Signaturtreffer. Es belegt noch keinen
Live-Start des Dedicated Servers und keine ECS-Layout- oder Patchwirkung im
laufenden Serverprozess. Das kopierte ECS-Layout muss im Serverprozess weiter
geprüft werden, bevor ECS-Mods für den Server freigegeben werden.

Der Windows-Bootstrap startet im Dedicated-Serverprozess keine Modloader-,
World-Editor- oder Debug-Console-Fenster mehr. Datei-Logging und Runtime-Logs
bleiben aktiv.

## Ablauf

1. `inspect <root>` erkennt `enshrouded_server.exe`, liest die Server-KFC-Datei
   und zeigt den Modplan, ohne Mods außerhalb des Spielprozesses auszuführen.
2. Der Modplan liest `targets` aus `extended.mod.json`. Er prüft das Ziel bei
   Lua-Planung, Abhängigkeiten und nativen DLLs.
3. `prepare <root>` verwendet `enshrouded_server.kfc` und
   `enshrouded_server.kfc_resources`. Vor der ersten Assetänderung wird die
   Originaldatei der Ressourcen als `.bak` gesichert.
4. Im gestarteten Serverprozess wählt der native Provider anhand des
   Executable-Ziels das Serverprofil; Lua erhält `is_server=true`.

## Noch offen

- Einen echten Serverstart und kontrollierten Shutdown auf einer isolierten
  Serverkopie prüfen. Das wurde bewusst nicht gestartet, weil der Prozess
  Serverports öffnen und Weltdaten erzeugen kann.
- Im Live-Server den Profilstatus, die Dispatcher-Initialisierung, die sechs
  Patch-Auflösungen und die Lua-Lifecycle-Callbacks kontrollieren.
- ECS-Layouts und ECS-Komponentenoffsets am Dedicated Server validieren. Bis
  dahin sollten Mods mit ECS-/World-APIs nicht für `server` markiert werden.
- Die fehlenden World-Hooks im Serverprofil lassen World-Editor-Funktionen auf
  dem Server nicht verfügbar; das Paket ist aktuell client-only markiert.
- Alte Installationen, die mit einer früheren Version bereits
  `.kfc_resources` verändert haben, besitzen möglicherweise noch kein
  Ressourcen-Backup. Die neue Sicherung kann frühere Originalbytes nicht
  rekonstruieren.

## Relevante Dateien

- `src/loader/package/src/registry/manifest_reader.rs`: liest und normalisiert
  `targets` aus der Erweiterungsdatei.
- `src/loader/package/src/env.rs`: wählt Mods und Abhängigkeiten für Client
  oder Server.
- `src/loader/workflow/pregame.rs`: Server-KFC-Ziel und Asset-Backup.
- `src/loader/api/src/load.rs`: Sicherung und Wiederherstellung der
  KFC-Ressourcendatei.
- `src/loader/runtime/profiles/enshrouded/server/1076226.json`: Serverprofil
  für das geprüfte Executable.
