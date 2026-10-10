# Steam-P2P-Nachrichten zwischen Mod-Prozessen

`runtime.network` ermöglicht Nachrichten zwischen Mod-Laufzeiten auf einem Client und einem Dedicated Server. Es ist ein eigener Steam Networking Messages Transport, keine Enshrouded-Spielnachricht. Siehe auch [targets](#doc-targets).

## Nachrichten für einen bestimmten Mod

`send_mod` sendet eine UTF-8-Nachricht an eine konkrete SteamID64 und Mod-ID. Der Empfänger liest sie mit `receive_mod`. Das ist praktisch, wenn beide Seiten denselben Mod installiert haben. Es gibt keinen Broadcast.

## Eigene Kanäle

`send` und `receive` arbeiten mit binärsicheren Lua-Strings. Öffentliche Kanäle reichen von 0 bis 65534. Kanal 65535 ist für die interne Health-Prüfung reserviert. Der Kanal muss auf beiden Seiten übereinstimmen.

## Serverfreigabe und SteamID64

Der Dedicated Server akzeptiert nur aktuell von Enshrouded authentifizierte, verbundene Spieler. Eine optionale Freigabeliste unter Modloader-Einstellungen → Netzwerk kann diesen Kreis weiter einschränken. Eine leere Liste bedeutet alle aktuell authentifizierten Spieler. Der Client erkennt die Server-SteamID64 aus dem Live-Kontext und der Health-Prüfung. Die Einstellung `serverSteamId` ist ein Fallback für entfernte Server.

## Ergebnis prüfen

Ein erfolgreicher `send`-Aufruf bedeutet, dass Steam den Sendeauftrag angenommen hat. Er bestätigt nicht, dass der andere Prozess die Nachricht empfangen oder verarbeitet hat. Prüfe Protokoll, Sender, Mod-ID und Berechtigung im Empfänger. Das API-Verzeichnis dokumentiert Payload-Grenzen und Rückgabewerte.
