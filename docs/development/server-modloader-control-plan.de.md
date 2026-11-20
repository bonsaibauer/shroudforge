# Masterplan: Serveransicht im Modloader-Client

**Status:** Umsetzungsstand der Serveransicht auf Basis einer gezielten Code- und Live-Server-Prüfung
**Ziel:** Den bestehenden Modloader im Client um eine geschützte Ansicht für einen Dedicated Server erweitern. Seiten und Bedienelemente bleiben vertraut. Daten und Aktionen kommen für das Serverziel vom Server.

## 1. Zielbild

Der Modloader bleibt ein gemeinsames Fenster mit derselben Navigation. Oben wählt man **Client** oder **Server**. Die aktuelle Seite, etwa Aktivität, installierte Mods oder Kompatibilität, zeigt danach die Daten des gewählten Ziels. Serverlogs werden nur nach einer erfolgreichen ShroudForge-Admin-Anmeldung und bei aktivierter Gruppenfreigabe übertragen.

Der Dedicated Server startet weiterhin ohne interaktives Fenster. Er stellt dem Client eine schmale, typisierte Verwaltungsoberfläche über den bestehenden Steam-P2P-Transport bereit. Der Client sendet keine beliebigen Datei-, Shell- oder Lua-Befehle. Der Server prüft jede Anfrage und ruft danach die vorhandene Funktion des zuständigen Moduls auf.

```mermaid
flowchart LR
    UI[Modloader-WebView] --> Host[Lokaler Modloader-Host]
    Host --> Runtime[Client-Runtime und lokaler P2P-Endpunkt]
    Runtime <--> Transport[Steam-P2P-Transport]
    Transport <--> Server[Server-Runtime und Server-Control-Endpunkt]
    Server --> ACL[Serverrechte prüfen]
    ACL --> Owners[Bestehende Modulbesitzer]
    Owners --> Result[Antwort, Snapshot, Fortschritt oder Logdaten]
    Result --> Transport
    Transport --> Runtime
    Runtime --> Host
    Host --> UI
```

**Erste Produktgrenze:** Zunächst ist ein über die vorhandene `serverSteamId` konfigurierter Server das Remote-Ziel. Mehrere gespeicherte Serverprofile können später ergänzt werden, falls der Betrieb das benötigt.

## 2. Geprüfter Ist-Zustand

### 2.1 Modloader-Oberfläche

Die Navigation des Modloaders hat die Bereiche Aktivität, News, Kompatibilität, Einstellungen, Wiki, Entdecken, Installiert, Updates, Papierkorb und Servermanagement. Installierte Mods erzeugen darunter dynamische Detailseiten. Diese Oberfläche ist bereits eine gemeinsame Shell aus [app.tsx](../../modules/modloader/ui/app.tsx) und [show-page.tsx](../../modules/modloader/ui/show-page.tsx).

Das UI spricht seinen nativen Host über typisierte WebView-Nachrichten an. Der Host liest einen `Snapshot`, verarbeitet Befehle und fragt Daten gezielt nach. [ui-snapshot.ts](../../modules/modloader/ui/ui-snapshot.ts) kennt bereits `CLIENT`, `SERVER` und `DESKTOP`. Der vorhandene Navigationseintrag und die Servermanagement-Seite bleiben bestehen. Der Featurebanner bleibt oben. Der Tab Serverrechte steht darunter.

### 2.2 Dedicated Server und Debug Console

Der [Windows-Bootstrap](../../modules/entrypoints/windows-bootstrap/module_processes.cpp) überspringt auf dem Dedicated Server sowohl das Modloader-Fenster als auch die interaktive Debug Console. Der Server schreibt stattdessen in `shroudforge-server.log`. Der heutige Debug-Console-Host liest lokal `enshrouded.log` und das lokale ShroudForge-Clientlog. Für das Serverziel liest der Serverprozess die Logs selbst und überträgt sie über den bestehenden P2P-Kanal nur nach Admin-Anmeldung und mit aktivierter Gruppenfreigabe.

### 2.3 Vorhandener Steam-P2P-Transport

[modules/transport/](../../modules/transport/) verwendet die Steam-Networking-Schnittstellen, die Enshrouded bereits initialisiert. Der Modloader soll keinen zweiten Steam-Callback-Pump starten. Der native Transport begrenzt einzelne Nachrichten auf 512 KiB. Kanal 65535 ist für Netzwerk-Healthchecks reserviert. Lua-Modnachrichten benötigen eine Runtime-Capability. Das Feld `from_mod` ist ein Routingwert und kein Nachweis für eine vertrauenswürdige Identität.

Auf dem Dedicated Server stammen verbundene Peer-IDs aus der von Enshrouded authentifizierten Spielerliste. Die optionale `modules.network.allowedClientSteamIds`-Liste wird zusätzlich angewandt. Eine nichtleere, ungültige Allowlist wird restriktiv behandelt. Diese Liste regelt den Netzwerkzugang. Sie ist kein Beleg für ein Modloader-Administratorrecht.

Das [World-Editor-P2P](../../mods/world-editor/src/p2p.lua) ist ein bestehendes Beispiel für sichere Fachdatenübertragung. Es verwendet begrenzte, geordnete Chunks mit Prüfsumme, Anfragestatus, Duplikatbehandlung und bestätigten Ergebnissen. Die Serveraktion geht danach durch die kanonische World-Editor-Implementierung in [operations.lua](../../mods/world-editor/src/operations.lua). Modloader-Verwaltung läuft über den vorhandenen typisierten Befehlssatz und den bestehenden Steam-P2P-Kanal.

### 2.4 Live-Server und Spielerrechte

Am 3. November 2026 liefen auf dem Entwicklungsrechner sowohl Enshrouded-Client als auch Dedicated Server. Die installierte Serverkonfiguration enthält die Gruppen Admin, Friend, Guest und Visitor mit unterschiedlichen Werten für `canKickBan`, `canAccessInventories`, `canEditWorld`, `canEditBase` und `canExtendBase`. Die Zugangspasswörter wurden bei der Prüfung nicht ausgegeben und gehören niemals in einen Remote-Snapshot.

Das aktive `logs/enshrouded_server.log` protokolliert beim Beitritt den Spielernamen und die effektiven Rechte. Der Dedicated-Serverprozess liest diese Einträge selbst. Er verbindet sie über die Steam-ID aus seinen eigenen Enshrouded-Peerinformationen mit dem authentifizierten Spieler. Die Namen und Rechte aus einem Client-Kompatibilitätsbericht werden für diese Serverrechteansicht nicht verwendet.

Der gemeinsame Leser in `modules/transport/read_session_peers.rs` hält Namen und Rechte in einem Datensatz pro Steam-ID. Die Rechte stammen aus Enshroudeds Log, die Gruppenvorgaben aus `enshrouded_server.json`. Die Zuordnung der Logblöcke folgt der Reihenfolge der Serverlogeinträge. Gleichzeitige Beitritte sind separat zu prüfen. Bis ein Rechteblock vorliegt, zeigt die Oberfläche die Steam-ID und unbekannte Rechte an.

Der laufende ShroudForge-Server hat Steam-P2P-Healthchecks bestätigt und protokolliert World-Editor-Anfragen. Das aktuelle ShroudForge-Log beschreibt die bestehende World-Editor-Zulassung über verbundene Enshrouded-Spieler und die Network-Beschränkung. Die effektiven Enshrouded-Rechte fließen dort noch nicht in die Freigabe ein.

Die Serverinstallation hat aktuell Netzwerk-P2P aktiviert und eine Allowlist mit einem Eintrag. Der Eintrag selbst wird hier nicht dokumentiert. Diese Prüfung betrifft nur den aktuellen lokalen Entwicklungsserver. Ein Produktionsserver kann eine andere Konfiguration haben.

## 3. Rechte-Modell

Der Server leitet die Rolle **ShroudForge Admin** aus einer `userGroups`-Gruppe ab, deren fünf Rechte `canKickBan`, `canAccessInventories`, `canEditWorld`, `canEditBase` und `canExtendBase` alle `true` sind. Der Gruppenname ist frei wählbar. Für die Serververwaltung meldet sich ein verbundener Spieler mit dem Passwort einer solchen Gruppe an. Der Server prüft das Passwort und die fünf Flags, bindet das Sitzungstoken an die authentifizierte Steam-ID und prüft es serverseitig bei jeder Verwaltungsanfrage. Die Log-Synchronisierung ist eine zusätzliche Einstellung pro ShroudForge-Admin-Gruppe.

### 3.1 Enshrouded-Spielrechte

Die Serverrechteansicht zeigt die vom Dedicated Server gelesenen Rechte des gerade verbundenen Spielers. Ein fehlender Logeintrag wird als unbekannt behandelt. Der Name des Spielers ist keine Berechtigungsgrundlage. Der Server verwendet die Enshrouded-Rechte bei Verwaltungsanmeldungen nicht aus einem Clientbericht.

### 3.2 Freigabe des Modloader-Serverbereichs

Das Passwort bleibt in `enshrouded_server.json` auf dem Server. Der Client sendet es einmal über den bestehenden Steam-P2P-Kanal. Der Server vergleicht es mit einer Gruppe, die alle fünf oben genannten Flags auf `true` hat. Der Gruppenname wird nicht fest einprogrammiert; die passende Gruppe wird zur Sitzung im Serverspeicher gehalten. Passwort und Token werden weder in eine Modloader-Konfiguration geschrieben noch an die WebView zurückgegeben.

Nach erfolgreicher Prüfung erhält ausschließlich die Client-Runtime ein zufälliges Sitzungstoken. Der Server bindet es an die authentifizierte Steam-Peer-ID und prüft es bei jeder Änderung erneut. Die Anmeldung gilt für die verbundene Spielsitzung und endet bei Trennung, Rechteentzug oder Serverneustart. Ein Wechsel der Modloader-Ansicht beendet nur das Spiegel-Abonnement und verwirft das Token nicht. Fünf fehlgeschlagene Anmeldungen sperren den Peer für eine Minute. Die bestehende Network-Allowlist bleibt eine zusätzliche Zugangsschranke und wird nicht durch die Admin-Anmeldung ersetzt.

| Freigabe | Wirkung |
| --- | --- |
| Steam-Peer ist verbunden und nach der Network-Allowlist erlaubt | P2P-Verbindung und nichtgeheime Verbindungsdaten. Noch keine Serverlogs |
| Passwort stimmt mit einer `userGroups`-Gruppe überein und alle fünf Flags sind `true` | Serververwaltung und Serverrechte lesen oder ändern |
| Die Gruppenfreigabe für Serverlogs ist aktiv | Serverlogs an die angemeldete ShroudForge-Admin-Gruppe übertragen |
| Enshrouded `PlayerPermissions` | Separate Prüfung für passende World-Editor-Spielaktionen |

`CanKickBan` allein ist damit kein Modloader-Login. Die fünf JSON-Flags werden bei schreibenden Aktionen erneut geprüft; wird eines davon entzogen, verfällt die Sitzung.

### 3.3 Durchsetzung

Die UI blendet nicht verfügbare Aktionen passend aus und zeigt die fehlende Berechtigung. Das ist Bedienhilfe. Die echte Entscheidung fällt ausschließlich im Serverprozess.

Jede schreibende Anfrage enthält eine Anfragennummer, das Sitzungstoken und den erwarteten bisherigen Modulstand. Der Server bezieht die Absender-SteamID aus Steam und verlangt eine passende, aktive Sitzung. Die Änderung wird unter dem vorhandenen Installations-Lock nur gespeichert, wenn der aktuelle Modulstand noch dem erwarteten Stand entspricht. Bei gleichzeitiger Änderung durch einen anderen Client wird die Änderung abgelehnt und der aktuelle Serverstand zurückgesendet. Das UI wiederholt sie nicht automatisch.

Client- und Serverlog erhalten dieselbe Request-ID. Der Client protokolliert Ziel, Aktion, serverbestätigten Spielernamen und Ergebnis. Der Server protokolliert die authentifizierte Peeridentität, den daraus ermittelten Enshrouded-Spielernamen, die effektiven Rechte, die Entscheidung und das Ausführungsergebnis. Roh-IDs erscheinen nur in einem für Administratoren zugänglichen Log. Damit lässt sich ein Vorgang auf beiden Seiten nachvollziehen, ohne dem Clientnamen zu vertrauen.

## 4. Funktionsmatrix

| Modloader-Funktion | Serveransicht | Schreib- oder Aktionsrecht |
| --- | --- | --- |
| **Zielumschalter** | Client oder konfigurierter Dedicated Server. Zeigt Zielname, Steam-Verbindung, Server-Runtime, ShroudForge-Version und letzten Snapshot-Zeitpunkt. | Auswahl ist lokal. Serververbindung selbst gewährt kein Recht. |
| **Aktivität** | Zeigt die Aktivitäten des gewählten Ziels. Servereinträge erhalten Ziel- und Zeitmarkierung. | Serverlogs nur nach Admin-Anmeldung und bei aktivierter Logfreigabe der angemeldeten Gruppe. Kopieren exportiert nur sichtbare Einträge. |
| **News und Benachrichtigungen** | Globale News bleiben gemeinsame Katalogdaten. Modbenachrichtigungen und Gelesenstatus können pro Ziel geladen werden. | Globale News bleiben öffentlich. Serverbezogene Modbenachrichtigungen benötigen die Admin-Sitzung. |
| **Wiki** | Artikel bleiben gemeinsam und offline verfügbar. Kontextuelle Links können auf die gewählte Zielseite führen. | Kein Remote-Schreibrecht. |
| **Entdecken** | Katalogdaten können clientseitig gesucht werden. Installieren adressiert ausdrücklich das gewählte Ziel und zeigt Serverpaket, Zielplattform und Version. | Admin-Sitzung für Installation und Paketänderung. |
| **Installierte Mods** | Serverliste mit Manifest, Aktivierung, Version, Ziel, Laufzeitstatus, Abhängigkeiten und verfügbaren Einstellungen. | Admin-Sitzung für Ansicht, Einstellungen und Aktivierung. |
| **Mod-Detailseite** | Bestehende Detailseite wird mit Server-Snapshot gerendert. Paketlinks, Beschreibung, Changelog und Abhängigkeiten bleiben sichtbar. | Admin-Sitzung für Einstellungen, Aktualisierung, Modaktionen und Entfernen. Jede Modaktion wird serverseitig geprüft. |
| **Updates** | System- und Modupdatezustand, Warteschlange, Fortschritt, Release-Notizen und Fehler des Servers. | Admin-Sitzung für Queue und Installation. Neustart und Prozessverwaltung bleiben beim bestehenden Serverbetrieb. |
| **Papierkorb** | Zeigt den serverseitigen Updater-Papierkorb samt Alter und Wiederherstellungsstatus. | Admin-Sitzung für Wiederherstellen und endgültiges Löschen. |
| **Kompatibilität: Mod-Erkennung** | Zeigt serverseitige Paketvalidierung und Konfigurationsfehler. | Admin-Sitzung. Ein Refresh bleibt eine begrenzte Serveraktion. |
| **Kompatibilität: DLL-Manager** | Zeigt Kandidaten, Filter, Reihenfolge und Profil des Serverziels. Änderungen können einen Serverneustart erfordern. | Admin-Sitzung. Keine DLL-Ausführung durch die UI. Bestehende Freigabe- und Ladeplanregeln bleiben maßgeblich. |
| **Kompatibilität: Hotkeys** | Für den Dedicated Server nicht anwendbar. Client-Hotkeys bleiben lokale Einstellungen. | Nur lokale Clientkonfiguration. |
| **Kompatibilität: Abhängigkeiten** | Berechnet den Plan aus der Serverinstallation und den aktivierten Servermods. | Admin-Sitzung. Änderungen führen über bestehende Mod-Einstellungs- oder Paketaktionen. |
| **Kompatibilität: Spiel und Runtime** | Zeigt Serverbuild, Serverprofil, Runtimebereitschaft, Provider und Serverdiagnose. Clientprofil und Serverprofil bleiben unterscheidbar. | Admin-Sitzung. Profiländerungen folgen bestehenden Profilregeln. |
| **Einstellungen: Allgemein und Darstellung** | Sprache, Zoom, Animation und Fensterverhalten gehören dem Modloader-Client und bleiben lokal. | Lokal. Keine P2P-Anfrage. |
| **Einstellungen: Module** | Zeigt serverseitige Modulpräferenzen, Logging und Netzwerkstatus. Nur servertaugliche Felder sind editierbar. | Admin-Sitzung für freigegebene Felder. Sicherheits- und Transportkonfiguration benötigt zusätzlich eine serverlokale Freigabe. |
| **Einstellungen: Pfade** | Zeigt normalisierte Serverpfade mit passenden Labels. Zugangsdaten und private Verzeichnisse werden nicht ungefiltert übertragen. | Admin-Sitzung. Erlaubt sind nur vorhandene, validierte Pfadfelder. |
| **Servermanagement** | Die vorhandene Seite und der Featurebanner bleiben bestehen. Darunter zeigt der Tab Serverrechte alle Gruppen und verbundenen Spieler mit den vom Server gelesenen Namen und Rechten. | Admin-Sitzung zum Lesen und Ändern. Der Server verwendet keine vom Client gelieferten Namen oder Rechte für diese Ansicht. |
| **Serverkonfiguration** | Zeigt und bearbeitet die Gruppenrechte aus `enshrouded_server.json`. Passwörter, Netzwerkadresse und Bannkonten bleiben verborgen. | Admin-Sitzung. Gruppenänderungen werden atomar gespeichert und beim Serverneustart übernommen. |
| **Runtime-Diagnostik** | Zeigt serverseitige Diagnosestatus und freigegebene Jobdefinitionen. Ergebnisdateien werden begrenzt und gezielt übertragen. | Admin-Sitzung für geprüfte Jobs. Schreibende oder weltverändernde Jobs brauchen eine zusätzliche Freigabe und dürfen nicht als allgemeine Diagnose angeboten werden. |
| **Debug Console** | Client- und Serverlogs bleiben als Quellen auswählbar. Serverlogs kommen live oder in begrenzten Blöcken aus dem Serverprozess. | Serverlogs nur nach Admin-Anmeldung und bei aktivierter Logfreigabe der angemeldeten Gruppe. Serverpfade werden nicht gesendet. Sichtbare Logdaten unterliegen der Aufbewahrungsgrenze. |
| **World Editor** | Bestehendes World-Editor-Fenster bleibt der Besitzer seiner Weltaktionen. Die Modloader-Zielauswahl zeigt an, zu welchem Server die P2P-Operation gehört. | Jede Weltaktion prüft die dafür nötigen effektiven Enshrouded-Rechte. `CanKickBan` allein reicht dafür nicht. |

## 5. UI- und Snapshot-Konzept

### 5.1 Gemeinsame Oberfläche

Die bestehende Navigation wird beibehalten. Ein Zielumschalter im Kopfbereich legt einen `TargetContext` fest. Alle Seiten lesen denselben ausgewählten Kontext und erhalten einen Snapshot mit Zielkennung, Zielrolle, Datenzeitpunkt, Verbindungszustand und Berechtigungen.

Jede Ansicht zeigt sichtbar **Client** oder **Server**. Der Server-Snapshot ist eine serverseitig zusammengestellte Produktansicht. Das UI bekommt keine beliebigen Pfade oder die rohe `modloader-config.json`.

Seiten mit nur lokaler Bedeutung, etwa Sprache, Zoom, Hotkeys oder Fenstereinstellungen, zeigen den Clientzustand auch dann als lokal an, wenn der Server ausgewählt ist. Serveränderungen verwenden ausdrücklich den Serverkontext und zeigen bei Neustartbedarf einen Hinweis.

### 5.2 Debug Console

Die Debug Console hat zwei Auswahlachsen:

1. **Ziel:** Client oder Server.
2. **Quelle:** Enshrouded-Log oder ShroudForge-Log.

Der Serverprozess verfolgt seine Dateien mit begrenztem Tail und liefert vollständige neue Logzeilen, Sequenznummer und Cursor. Das UI kann bei Wiederverbindung einen begrenzten neuen Tail laden. Bei Sequenzlücke wird der angezeigte Stand ersetzt, statt Zeilen still zu verlieren. Suche, Schweregradfilter, Pause, Auto-Scroll und Kopieren bleiben UI-seitig.

Übertragen werden Logzeilen, nicht Serverdateipfade. Ein bestätigter Verwaltungsrequest erscheint sowohl im Client- als auch im Serverlog unter derselben Request-ID. Der Server sendet Logs erst nach Admin-Anmeldung und nur, wenn die Gruppenfreigabe aktiv ist. Spielernamen und Rechte der Serverrechteansicht werden aus dem Serverlog gelesen und per Steam-ID zugeordnet. Ein vom Client gemeldeter Name ist dafür keine Quelle.

## 6. Transport und Befehlsfluss

Die WebView sendet einen typisierten Befehl an den vorhandenen Modloader-Host. Der Host prüft die Eingabe und reicht den Befehl an den Client-Runtime-Endpunkt weiter. Der Client-Runtime-Endpunkt verwendet den bestehenden Steam-P2P-Transport. Der Server-Runtime-Endpunkt nimmt den Request an, ermittelt den echten Steam-Absender und führt die Rechteprüfungen aus.

Der Server delegiert anschließend an vorhandene Besitzer:

- Modliste, Manifeste, Settings und Modaktivierung an Modloader und Package-Funktionen.
- Updateprüfung, Warteschlange, Installation und Papierkorb an den Updater.
- Aktivität und Laufzeitlogs an Logging und Debug Console.
- Diagnoseaufträge an Runtime Diagnostics.
- Enshrouded-Spielerrechte an den gemeinsamen Parser des Serverlogs und die serverseitig authentifizierte Steam-Peer-Liste.
- P2P-Framing, Größenlimits, Sequenzen und Peerzustand an Transport.

Der Transportvertrag wird versioniert. Jeder Befehl hat eine feste Kennung, ein validiertes Payloadschema, eine maximale Größe, Request-ID, Serverinstanzkennung und einen begrenzten Timeout. Lesen, Ergebnis, Fortschritt und Mutation sind unterscheidbare Nachrichtenarten. Serverneustart oder Verbindungsabbruch machen einen gesendeten Schreibauftrag gegebenenfalls **ungewiss**. Das UI wiederholt dann nicht blind.

Für große Logs und Snapshots gelten Obergrenzen für Gesamtdaten, Chunkanzahl, Warteschlange und Zeit. P2P-Erfolg bedeutet zunächst nur, dass Steam die Nachricht angenommen hat. Die UI zeigt Erfolg erst nach der bestätigten Serverantwort und, bei schreibenden Aktionen, dem Rücklesen des resultierenden Zustands.

## 7. Modulgrenzen für die spätere Umsetzung

| Bereich | Zuständigkeit |
| --- | --- |
| `modules/modloader/ui/` | Zielauswahl, gemeinsame Seiten, Berechtigungsanzeige, Serverstatus und Ergebnisdarstellung. |
| `modules/modloader/ui/host/` | WebView-Befehle validieren, lokale Requestzustände verwalten und Remote-Ergebnisse in den vorhandenen Snapshotfluss übergeben. |
| `modules/modloader/` | Bestehende Modloader-Settings bleiben der Eigentümer der Einstellungsvalidierung; die Admin-Sitzung autorisiert ihre Remoteänderung. Keine beliebigen Dateipfade oder Shellaktionen. |
| `modules/mod-runtime/ingame/` | Nachrichten während der vorhandenen Runtime-Lebensdauer zustellen und die Serverausführung an den richtigen Runtime-Zeitpunkt übergeben. |
| `modules/transport/` | Steam-Nachrichten, P2P-Peers, begrenzte Frames und versionierte Transportregeln. Keine zweite Steam-Callback-Pumpe und keine neue öffentliche Lua-API ohne tatsächlichen Bedarf. |
| `modules/game-runtime/` | Nur wenn die Recherche eine neue native Operation für die strukturierte Abfrage effektiver Enshrouded-Spielrechte begründet. Eine Logzeile allein ist keine native Freigabe. |
| Bestehende Featuremodule | Führen ihre jeweilige Funktion weiter aus. Der Remote-Endpunkt dupliziert keine Installations-, Log-, Diagnose- oder World-Editor-Logik. |

Der Dedicated Server bekommt keinen zweiten UI-Host. Für den Empfang und die Ausführung gibt es einen Laufzeitendpunkt, nicht eine zweite Datei- oder Befehlsinfrastruktur neben den bestehenden Besitzern.

## 8. Umsetzungsphasen

### Phase 0: Spieleridentität und Rechte beweisen

- Mit dem laufenden Server und mehreren Testgruppen die SteamID, Enshrouded-Spieleridentität und effektive Rechte strukturiert erfassen.
- Die Daten mit den Einträgen `logged in with Permissions` im echten Serverlog vergleichen.
- Join, Leave, Rejoin, gleichzeitige Spieler und Rollenwechsel abdecken.
- Ein geprüftes Ergebnis oder eine klare technische Lücke dokumentieren.
- Währenddessen keine Remote-Änderungsbefehle zulassen.

**Abnahme:** Für jede aktive Verbindung ist entweder eine aktuelle, eindeutige SteamID-zu-Rechte-Zuordnung bewiesen oder die Spielrechteintegration bleibt ausgeschaltet. Namen oder Logtext werden nie als Identitätsnachweis verwendet.

Ein zusätzlicher Negativfall ändert den Steam- oder Enshrouded-Anzeigenamen eines Nicht-Admins auf den Namen eines berechtigten Admins. Der Nicht-Admin bleibt gesperrt.

### Phase 1: Verbindung und lesende Serveransicht

- Zielumschalter mit dem einen eingerichteten Server.
- P2P-Handshake, Serveridentität, Runtimeversion und Capability-Snapshot.
- Steam-Peer und Network-Allowlist für den lesenden Zugriff prüfen. Admin-Sitzung und Gruppenflags gelten zusätzlich nur für Schreibaktionen.
- Lesender Snapshot für Status, Modliste, Abhängigkeiten, Aktivität und Netzwerkzustand.
- Debug Console für Serverlogs mit Limits und Wiederverbindung.

**Abnahme:** Ein verbundener, erlaubter Peer kann Logs und den lesenden Snapshot empfangen. Ohne gültige Admin-Sitzung kann er keine Servereinstellung ändern. Ein Serverausfall erzeugt einen klaren Offlinezustand.

### Phase 2: Begrenzte Modverwaltung

- Serverseitige Moddetails und Einstellungen lesen.
- Einstellungen speichern und Mods aktivieren oder deaktivieren.
- Serverantwort mit Rücklesewert und Neustartbedarf anzeigen.
- Modaktion nur mit gültiger Admin-Sitzung und serverseitiger Freigabe der konkreten Aktion ausführen.

**Abnahme:** Jeder Schreibbefehl bleibt an Ziel, Steam-Absender, Serverinstanz, Request-ID und Mod-ID gebunden. Veraltete oder doppelte Requests ändern keine zweite Mod.

### Phase 3: Paket- und Updateverwaltung

- Katalog und Updateverfügbarkeit für das Serverziel darstellen.
- Bestehende Queue, Paketvalidierung, Staging, Wiederherstellung und Ergebnisprotokolle serverseitig nutzen.
- Installieren, Aktualisieren und Entfernen mit getrennten Scopes versehen.
- Wartungs- und Neustartbedingungen klar anzeigen.

**Abnahme:** Ein abgebrochener oder unsicherer Auftrag lässt Zustand und Queue nachvollziehbar. Es gibt keine unbestätigte Erfolgsmeldung und keinen blinden Wiederholungsversuch einer Mutation.

### Phase 4: Diagnose, Serverkonfiguration und Wiederherstellung

- Geprüfte Diagnosejobs nach Risiko klassifizieren.
- Gruppenrechte in `enshrouded_server.json` nur über die vorhandene Serverrechteansicht ändern.
- Bekannte Gruppenfelder serverseitig validieren, atomar speichern und den erforderlichen Serverneustart anzeigen.
- DLL-Auswahl, Papierkorb, Gamefile-Backups und Wiederherstellung mit der Admin-Sitzung und expliziten Bestätigungen ergänzen.

**Abnahme:** Passwörter und andere Geheimnisse gelangen nie in UI-Snapshots oder Logs. Destruktive Aktionen zeigen Umfang und erwartete Wirkung vor dem Absenden.

### Phase 5: Spielrechte in passende Funktionen integrieren

- Effektive `PlayerPermissions` für World-Editor- und weitere bestehende Spielaktionen verwenden.
- Rechteänderungen während einer Session erneut prüfen.
- `userGroups`-Passwort plus alle fünf Admin-Flags bleibt die Freigabe für Modloader-Serveränderungen. Enshrouded-Spielrechte gelten nur für passend zugeordnete Spielaktionen.

**Abnahme:** Eine Anfrage wird vom Server abgelehnt, wenn die für genau diese Spielaktion erforderliche Enshrouded-Berechtigung fehlt.

## 9. Sicherheits- und Zuverlässigkeitsregeln

1. Steam-Transportidentität ist die Peeridentität. Payloadfelder und `from_mod` sind keine Identitätsquelle.
2. Network-Allowlist, Admin-Sitzung für Modloader-Schreibaktionen und Enshrouded-Spielrechte für Weltaktionen bleiben getrennte Prüfungen.
3. Alle Schreibaktionen werden im Serverprozess autorisiert und vor der Ausführung erneut geprüft.
4. Es gibt keine generische Schreibdatei-, Shell-, DLL-Lade- oder Lua-Eval-Nachricht.
5. Nachrichten, Logs, Warteschlangen, Chunks, Replays und Zeitfenster sind begrenzt.
6. Empfangsbestätigung und abgeschlossene Aktion sind verschiedene Zustände.
7. Nach unklarem Mutationsergebnis wird zuerst Serverzustand gelesen. Die Mutation wird nicht automatisch wiederholt.
8. Serverpasswörter, Netzwerkadresse, Bannkonten und nicht freigegebene IDs werden aus Snapshots entfernt.
9. Berechtigungsentzug oder getrennte Enshrouded-Verbindung sperren neue Aktionen sofort und laufende Aktionen nach der definierten sicheren Abbruchgrenze.
10. Bestehende native Lade-, Update-, Wiederherstellungs- und Welt-Lebenszeitregeln bleiben Eigentum ihrer Module.

## 10. Festgelegte Entscheidungen und spätere Fragen

1. Festgelegt: Für Modloader-Schreibaktionen gelten die dynamische `userGroups`-Passwortprüfung und alle fünf Admin-Flags. Besonders riskante Aktionen können in einer späteren Phase zusätzlich eine Serverbestätigung verlangen.
2. Welche Enshrouded-Rechte verlangen World-Editor-Lesen, Platzieren, Rückgängig und Abbruch jeweils? Diese Zuordnung wird getrennt pro Operation festgelegt.
3. Reicht zunächst ein fest konfigurierter Server, oder wird schon in Version eins eine Liste mehrerer Server benötigt?
4. Welche Modaktionen sind aus der Ferne erlaubt? Paketmetadaten deklarieren die Aktion, aber nicht zwingend ihr Schadenspotenzial.
5. Die fünf Rechtefelder der `userGroups`-Einträge dürfen über die Admin-Ansicht geändert werden. Passwörter und andere Serverfelder bleiben verborgen und unverändert.

Empfehlung: ein fest konfigurierter Server, Serverprüfung der `userGroups`-Zugangsdaten und der fünf Flags, Sitzungstoken nur im Speicher und konfliktgeprüfte Settingsänderungen. Die Zuordnung von SteamID zu `PlayerPermissions` bleibt eine getrennte Voraussetzung für spätere World-Editor-Aktionen.

## 11. Geprüfte Quellen und Grenzen

Gezielt geprüft wurden:

- Modloader-Navigation, Snapshot und Seitenrouting in [app.tsx](../../modules/modloader/ui/app.tsx), [show-page.tsx](../../modules/modloader/ui/show-page.tsx) und [ui-snapshot.ts](../../modules/modloader/ui/ui-snapshot.ts).
- Moddetails, Kompatibilität, Einstellungen, Updates und Debug Console in den jeweiligen UI-Modulen.
- UI-Host-Snapshot und lokaler Befehlsfluss in [modules/modloader/ui/host](../../modules/modloader/ui/host/).
- Server-Skip für Modloader- und Debug-Console-Fenster in [module_processes.cpp](../../modules/entrypoints/windows-bootstrap/module_processes.cpp).
- P2P-Vertrag in [modules/transport](../../modules/transport/), [update_ingame_runtime.rs](../../modules/mod-runtime/ingame/update_ingame_runtime.rs) und [modules/mod-runtime/README.md](../../modules/mod-runtime/README.md).
- Vorhandenes World-Editor-P2P in [p2p.lua](../../mods/world-editor/src/p2p.lua) und [operations.lua](../../mods/world-editor/src/operations.lua).
- Die laufende Dedicated-Serverkonfiguration `enshrouded_server.json`, die ShroudForge-Serverkonfiguration und aktuelle Enshrouded- sowie ShroudForge-Logs. Geheimnisse und SteamIDs wurden bei der Ausgabe redigiert.

Das war eine gezielte Architekturprüfung für Modloader, P2P und Serverrechte. Es war kein vollständiges Repository-Audit. Nicht geprüft wurden vollständige Paket-Builds, Release-Paketierung, sämtliche Runtime-APIs und die native Zuordnung von Enshrouded-Spielerhandle zu SteamID und `PlayerPermissions`. Diese Zuordnung ist das wichtigste technische Tor für die Rechteintegration.
