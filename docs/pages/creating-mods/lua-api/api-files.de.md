---
{
  "id": "api-files",
  "locale": "de",
  "title": "Dateien lesen und Exporte speichern",
  "summary": "Paketdateien und Exportdateien haben unterschiedliche Orte und Berechtigungen. Verwende den passenden API-Bereich.",
  "navigation": {
    "path": [
      "Lua-API-Anleitungen",
      "Funktionsanleitungen"
    ]
  },
  "apiPrefixes": [
    "io.",
    "buffer.",
    "Buffer:",
    "game.assets.",
    "shroudforge.contents."
  ]
}
---

# Dateien lesen und Exporte speichern

Die globale `io`-API trennt mitgelieferte Paketdateien von exportierten Spielerdaten. Ein Mod sollte nicht annehmen, dass beide Bereiche denselben Pfad oder dieselbe Schreibberechtigung haben.

## Berechtigung

Für Exportfunktionen muss `export` in `mod.json` stehen. Prüfe den API-Rückgabewert, bevor du einen Erfolg meldest. Nutze die [API-Referenz](#api), um die genaue Funktion und Argumente nachzuschlagen.

## Typischer Ablauf

1. Bestimme einen relativen Exportpfad.
2. Erzeuge die Bytes mit der passenden Buffer- oder Serialisierungsfunktion.
3. Rufe die Exportfunktion auf.
4. Prüfe den Rückgabewert und schreibe eine verständliche Ergebnis- oder Fehlermeldung.

Der Loader begrenzt Pfade auf den jeweiligen Modbereich. Die Exportfunktion ist kein allgemeiner Zugriff auf beliebige Dateien des Computers.

## Große Spielinhalte ohne Warten lesen

`game.assets` liest die Bytes eines Inhalts in der Schleife der Mods, auf die jeder Mod wartet. Für große Inhalte wie die Stufen einer Textur gibt es `shroudforge.contents.read`. Es liest den angeforderten Bytebereich in einem Hintergrundthread und gibt einen Auftrag zurück, den der Mod bei seinen nächsten Updates abfragt.

```lua
local job = shroudforge.contents.read({hash = texture.content.hash, start = 0, count = 65536})

-- Bei einem späteren Update:
local result = job:result()
if result and result.data then
  -- result.data enthält die Bytes, result.seconds die Lesedauer.
elseif result then
  shroudforge.log.warn(result.error)
end
```

`hash` ist ein Inhalts-Hash, wie ihn eine Ressource enthält. `start` und `count` wählen einen Bytebereich, ohne Angabe den ganzen Inhalt. Lesbar sind nur Inhalte der Spieldateien, nicht die eines Mods.

Jede Anfrage ist auf 32 MiB begrenzt. Größere Inhalte werden in mehrere Bytebereiche aufgeteilt. Pro Runtime dürfen höchstens vier Leseaufträge offen sein. Ein fertiger Auftrag belegt weiter einen Platz, bis sein Ergebnis abgefragt oder der Auftrag freigegeben wird. Ungültige Bereiche ergeben einen Fehler und werden nicht verkürzt. Das sind erste Schutzlimits für diese API. Sie verhindern unbegrenzt viele Threads und große Speicheranforderungen und unterstützen Architect Lines' aktuelle Textur-Lesevorgänge. Wir können die Werte erhöhen, sobald die tatsächlichen Bereichsgrößen und der Speicherbedarf des Mods gemessen sind.
