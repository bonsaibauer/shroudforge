# `targets`, Client und Server auswählen

`targets` ist eine Liste in `extended.mod.json`. Sie steuert, in welchem Prozess der Mod geladen wird.

| Wert | Wo der Mod läuft |
| --- | --- |
| `["client"]` | Im lokalen Enshrouded-Spielprozess des Spielers. |
| `["server"]` | Im Dedicated-Server-Prozess. |
| `["client", "server"]` | Im Client und im Dedicated Server. |

## Beispiele

Nur Client:

```json
{
  "targets": ["client"]
}
```

Nur Server:

```json
{
  "targets": ["server"]
}
```

Beide Prozesse:

```json
{
  "targets": ["client", "server"]
}
```

## Das bewirkt es nicht

Ein Client-Mod wird dadurch nicht automatisch mit dem Server synchronisiert. `targets` repliziert weder Lua-Zustand noch Weltänderungen und sendet keine Steam-P2P-Anfrage. Dafür braucht der Mod eine passende Netzwerk- und Serverlogik.

## Standardwerte und EML

Für neue ShroudForge-Pakete sollte `["client"]` ausdrücklich gesetzt werden. Ein EML-Paket ohne explizites `targets` läuft auf Client und Server. Das gilt auch, wenn seine Erweiterungsdatei `launcher: "EML"` enthält. Setze `targets` explizit, wenn du migrierst oder ein anderes Ziel benötigst. Siehe [EML-Migration](#doc-eml-migration).

Für den Multiplayer muss der Mod dort installiert sein, wo sein Code laufen soll. Siehe den [Multiplayer-Quickstart](#server).
