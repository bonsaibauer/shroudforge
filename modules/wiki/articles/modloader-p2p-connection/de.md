# Modloader-P2P-Verbindung

Dieser Artikel erklärt, wie der Modloader einen Dedicated Server über Steam P2P erreicht, was Steam verschlüsselt, wie Nachrichten zwischen Client und Server fließen und welche Enshrouded-Benutzergruppen die Serveradministration freischalten.

## Im Spiel verbinden

1. Starte Enshrouded und betrete eine Spielwelt des Dedicated Servers.
2. Öffne den Modloader und wähle **Server** als Ziel aus.
3. Warte, bis der Serverstatus **Verbunden** anzeigt. Das bedeutet, dass der Client vor Kurzem eine Steam-P2P-Statusnachricht erhalten hat. Es bestätigt nicht, dass der Absender der gewünschte Enshrouded-Server ist. Im Desktopfenster und im Hauptmenü wird keine P2P-Verbindung in einer Spielwelt bestätigt. Dort bleibt das Serverpasswortfeld deshalb ausgeblendet.
4. Gib das Passwort eines passenden Eintrags unter <code>userGroups</code> ein. Der Server prüft das Passwort über die P2P-Verbindung.

## So funktioniert die P2P-Verbindung

Das Network-Modul läuft im Enshrouded-Client oder im Prozess des Dedicated Servers. Das Spiel initialisiert Steam. ShroudForge verwendet die in diesem Prozess bereits geladene Steam-Networking-Messages-Schnittstelle und sendet direkt an eine Steam-Identität. ShroudForge betreibt keinen Relay-Server und verpackt diese Nachrichten nicht in Enshrouded-Spielpakete. Solange das Network-Modul aktiviert ist, führt die Laufzeit die P2P-Statusprüfung und die Modloader-Dienste aus. Siehe den [Runtime-Tick](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/mod-runtime/ingame/update_ingame_runtime.rs) und den [nativen Steam-Adapter](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/native/steam_network.cpp).

## Transportverschlüsselung und Kanäle

Modloader-Anfragen und -Antworten werden als JSON-Bytes übertragen. Laufzeitmods können binäre Lua-Zeichenfolgen senden. ShroudForge ergänzt hier keine eigene TLS-Sitzung, Nachrichtenverschlüsselung oder Ende-zu-Ende-Schlüsselaushandlung. Der Code verlässt sich auf Steam Networking für die Authentifizierung des Peers und die Transportverschlüsselung. Valve dokumentiert, dass <code>ISteamNetworkingMessages</code> die zugrunde liegenden Verbindungen von <code>ISteamNetworkingSockets</code> verwendet, und beschreibt die Verschlüsselung und Authentifizierung von Steam Networking. Das schützt den Transport zur Steam-Identität. Es beweist nicht, dass der Peer der gewünschte Enshrouded-Server ist, und schützt keinen der beiden Endpunkte. Kanalnummern leiten Nachrichten nur weiter und setzen keine Berechtigungen durch. Die Lua-API akzeptiert Kanäle von 0 bis 65533 und weist die beiden reservierten Kanäle zurück. Interne Modloader-Nachrichten fordern zuverlässige Zustellung an. Das beeinflusst Zustellung und Reihenfolge, ist aber weder Verschlüsselung noch Berechtigungskontrolle. Siehe die offiziellen Dokumentationen zu [ISteamNetworkingMessages](https://partner.steamgames.com/doc/api/ISteamNetworkingMessages) und [ISteamNetworkingSockets](https://partner.steamgames.com/doc/api/ISteamNetworkingSockets) sowie die [Kanaldefinitionen des Transports](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/lib.rs) und die [Lua-Network-API](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/lua/network.rs).

| Kanal | Aktuelle Verwendung |
|---|---|
| <code>0</code> bis <code>65533</code> | Nachrichten zwischen Lua-Laufzeitmods |
| <code>65534</code> | Interne Serveranzeige und Fernsteuerung des Modloaders |
| <code>65535</code> | Steam-P2P-Statusprüfungen |

## Nachrichtenfluss zwischen Client und Server

| Richtung | Was der aktuelle Code sendet |
|---|---|
| Client zum Server | Der Client verwendet <code>modules/network/serverSteamId</code>. Ist diese Einstellung leer, greift er auf die zuletzt entdeckte Steam-ID des Remote-Servers zurück. Etwa alle 10 Sekunden sendet er eine Anmeldung. Danach kann er das Adminpasswort und anschließend mit einem Sitzungstoken Einstellungsänderungen, Mod-Listenanfragen und Modaktionen senden. Der Server entfernt Peers, die ihre Anmeldung 30 Sekunden lang nicht erneuern. |
| Server zum Client | Eine neue Anmeldung erhält sofort einen Schnappschuss der Loader-Konfiguration und die jeweils letzten 64 KiB jeder übertragenen Protokolldatei. Danach sendet der Server Konfigurationsänderungen und rohe Protokollabschnitte an angemeldete Peers. Admin-authentifizierte Anfragen erhalten außerdem Mod-Listen, Kompatibilitätsdaten, Sitzungs- und Aktionsergebnisse. |
| Laufzeitmod zu Laufzeitmod | Ein Lua-Mod sendet an eine ausdrücklich angegebene Steam-ID. <code>send_mod</code> fügt einen JSON-Umschlag hinzu und wählt anhand der Ziel-Mod-ID einen deterministischen Kanal. Der Empfänger prüft Protokoll und Ziel-Mod-ID. Der Wert <code>from_mod</code> stammt vom sendenden Mod und beweist nicht, welcher Mod die Nachricht erstellt hat. |

Der Client übergibt lokale UI-Anfragen über eine prozessinterne Brücke. Der Spielprozess führt den Steam-Versand aus. Auf dem Server muss die Steam-Identität des Absenders in der aktuell autorisierten Peer-Liste stehen. Der native Adapter erstellt diese Liste anhand von Protokollereignissen des Dedicated Servers zu Online-Peers und Steam-Authentifizierung. Die optionale Einstellung <code>allowedClientSteamIds</code> filtert diese Liste zusätzlich. Ist sie leer, entsteht keine weitere Allowlist-Einschränkung. Der Server sendet den Schnappschuss und die Protokolle bereits vor der Adminanmeldung. Die Peer-Prüfung kontrolliert daher auch den Zugriff auf diese Daten. Siehe den [Remote-UI-Dienst](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/remote_ui.rs), die [lokale Brücke](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/local_bridge.rs) und die [Network-Allowlist](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/read_network_allowlist.rs).

## Erforderliche Rechte der Servergruppe

Das eingegebene Passwort muss exakt zu einer Gruppe mit einem nicht leeren <code>name</code> passen. Dieselbe Gruppe muss alle fünf unten aufgeführten Rechte auf den JSON-Wert <code>true</code> setzen. Ein fehlendes Feld, <code>false</code> oder ein Wert mit einem anderen Datentyp qualifiziert die Gruppe nicht für die Serveradministration im Modloader.

~~~json
"userGroups": [
  {
    "name": "Admin",
    "password": "REPLACE_WITH_A_PRIVATE_PASSWORD",
    "canKickBan": true,
    "canAccessInventories": true,
    "canEditWorld": true,
    "canEditBase": true,
    "canExtendBase": true,
    "reservedSlots": 0
  }
]
~~~

<code>reservedSlots</code> legt reservierte Spielplätze fest und gehört nicht zur Rechteprüfung des Modloaders. ShroudForge sendet das eingegebene Passwort in einer JSON-Anfrage an den ausgewählten Steam-Peer und speichert es nicht. Der Server vergleicht es direkt mit dem Passwort in <code>enshrouded_server.json</code>. Dieser Code hasht oder verschlüsselt diesen Konfigurationswert nicht im Ruhezustand. Nach erfolgreicher Prüfung stellt der Server ein an die Steam-Identität gebundenes Token aus und hält es auf beiden Seiten im Arbeitsspeicher. Admin-Schreibvorgänge prüfen Token, Peer und Gruppenrechte erneut. Fünf fehlgeschlagene Anmeldeversuche lösen eine 60-sekündige Anmeldesperre aus. Siehe den [Server-Autorisierungscode](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/remote_ui.rs).

## Sicherheitshinweis

Ist <code>serverSteamId</code> leer, kann der Client den zuletzt über eine Statusnachricht entdeckten Peer auf Kanal <code>65535</code> verwenden. Diese Einstellung ist standardmäßig leer. Eine empfangene Nachricht mit dem Anfang <code>SFN1:PING:</code> aktualisiert diese entdeckte ID. **Verbunden** zeigt aktuellen Statusverkehr an, bestätigt aber keine Enshrouded-Serveridentität. Wenn ein Peer eine Steam-Nachrichtensitzung aufbauen und diese Statusnachricht senden kann, kann er als Ziel ausgewählt werden. Die Steam-Verschlüsselung würde dann die Verbindung zu diesem Peer schützen. Sie würde nicht bestätigen, dass es der Server des Benutzers ist. Prüfe oder fixiere die Steam-ID des Servers, bevor du das Passwort eingibst. Behandle angemeldete Peers außerdem als vertrauenswürdig, da sie die Konfiguration und Protokolldaten vor der Adminanmeldung erhalten. Laufzeitmods, die <code>send_mod</code> empfangen, sollten Steam-Peer, behauptete Absender-Mod-ID und Inhalt selbst prüfen. Das sind Beobachtungen aus dem Quellcode und keine Bestätigung eines live ausgenutzten Fehlers. Die [ShroudForge-Sicherheitsrichtlinie und Hinweise zur vertraulichen Meldung](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/SECURITY.md) enthalten weitere Informationen.
