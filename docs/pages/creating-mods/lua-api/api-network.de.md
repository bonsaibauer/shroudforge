---
{
  "id": "api-network",
  "locale": "de",
  "title": "Steam-P2P-Nachrichten zwischen Mod-Prozessen",
  "summary": "runtime.network stellt einen eigenen Steam-Nachrichtenkanal bereit. Er versendet keine normalen Enshrouded-Spielpakete.",
  "navigation": {
    "path": [
      "Lua-API-Anleitungen",
      "Funktionsanleitungen"
    ]
  },
  "apiPrefixes": [
    "runtime.network.",
    "RuntimeNetworkStatus.",
    "RuntimeNetworkSendOptions.",
    "RuntimeNetworkReceiveOptions.",
    "RuntimeNetworkMessage.",
    "RuntimeModNetworkMessage."
  ]
}
---

# Steam-P2P-Nachrichten zwischen Mod-Prozessen

`runtime.network` ermöglicht Nachrichten zwischen Mod-Laufzeiten auf einem Client und einem Dedicated Server. Es ist ein eigener Steam Networking Messages Transport, keine Enshrouded-Spielnachricht. Siehe auch [targets](#doc-targets).

## Nachrichten für einen bestimmten Mod

`send_mod` sendet eine UTF-8-Nachricht an eine konkrete SteamID64 und Mod-ID. Der Empfänger liest sie mit `receive_mod`. Das ist praktisch, wenn beide Seiten denselben Mod installiert haben. Es gibt keinen Broadcast.

## Eigene Kanäle

`send` und `receive` arbeiten mit binärsicheren Lua-Strings. Öffentliche Kanäle reichen von 0 bis 65533. Kanal 65534 ist für die Modloader-Dateispiegelung und Kanal 65535 für die interne Health-Prüfung reserviert. Der Kanal muss auf beiden Seiten übereinstimmen.

## Serverfreigabe und SteamID64

Der Dedicated Server akzeptiert nur aktuell von Enshrouded authentifizierte, verbundene Spieler. Eine optionale Freigabeliste unter Modloader-Einstellungen → Netzwerk kann diesen Kreis weiter einschränken. Eine leere Liste bedeutet alle aktuell authentifizierten Spieler. Der Client erkennt die Server-SteamID64 aus dem Live-Kontext und der Health-Prüfung. Die Einstellung `serverSteamId` ist ein Fallback für entfernte Server.

## Von Spielern gehostete Spiele

Ein von einem Spieler gehostetes Spiel sendet keine Health-Prüfung. Ein Client liest stattdessen die eigene Sitzung des Spiels aus `enshrouded.log`. `status().session_host_steam_id` ist die SteamID64 des Spiels, dem dieser Client beigetreten ist, also das Spiel eines Spielers oder ein Dedicated Server. Solange der Client selbst hostet, allein spielt oder im Menü ist, ist der Wert `nil`. Im hostenden Spiel liefert `connected_peers()` die Spieler seiner Sitzung, gefiltert durch dieselbe optionale Freigabeliste. Das Spiel hat diese Spieler bereits zugelassen. Ein Mod kann daher prüfen, ob eine Nachricht vom Host oder von einem dieser Spieler stammt, ohne dass der Spieler eine SteamID64 eintippen muss.

## Berechtigungen der Spieler

Im hostenden Spiel, also einem Dedicated Server oder dem Spiel eines Spielers, liefert `player_permissions(peer)` die Berechtigungen, mit denen sich der Spieler angemeldet hat, so wie Enshrouded sie in das Protokoll dieses Spiels schreibt. Beispiele sind `CanEditBase`, `CanEditWorld` oder `CanKickBan`. Ein Mod, der die Welt für andere Spieler verändert, kann so einen Spieler ablehnen, dessen Gruppe das nicht erlaubt. In einem Spiel, das nicht hostet, oder für einen Spieler, der nicht dabei ist oder sich noch nicht angemeldet hat, liefert die Funktion `nil` und einen Grund.

## Ergebnis prüfen

Ein erfolgreicher `send`-Aufruf bedeutet, dass Steam den Sendeauftrag angenommen hat. Er bestätigt nicht, dass der andere Prozess die Nachricht empfangen oder verarbeitet hat. Prüfe Protokoll, Sender, Mod-ID und Berechtigung im Empfänger. Das API-Verzeichnis dokumentiert Payload-Grenzen und Rückgabewerte.
