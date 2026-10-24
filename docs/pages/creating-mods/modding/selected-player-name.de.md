---
{
  "id": "modding-player-name",
  "locale": "de",
  "title": "Lokalen Charakternamen mit Lua abfragen",
  "summary": "runtime.player.name liest den ausgewählten Charakternamen aus dem gemeinsamen nativen Menücache. Die Mod bestimmt ihren Ersatznamen selbst.",
  "navigation": {
    "path": ["Modding"],
    "label": "Charakternamen lesen"
  },
  "apiSymbols": ["runtime.player.name", "runtime.player"],
  "apiPrefixes": ["RuntimePlayerApi."]
}
---

# Lokalen Charakternamen mit Lua abfragen

`runtime.player.name()` liefert den Namen des Charakters, der im Hauptmenü des lokalen Spielclients ausgewählt wurde. Unicode-Zeichen bleiben erhalten. Die Abfrage eignet sich zum Beispiel für eine persönliche Mod-Anzeige oder eine Mitteilung.

## Voraussetzungen

Deklariere die [Berechtigung `runtime`](#doc-capability-runtime) in `mod.json`, aktiviere die Mod und verwende einen [Laufzeit-Callback](#doc-api-lifecycle):

```json
{
  "capabilities": ["runtime"]
}
```

Setze für dieses Client-Beispiel in `extended.mod.json` [targets](#doc-field-targets) auf `["client"]`. Der Name kann bereits im Hauptmenü verfügbar sein. Weltzugriff ist dafür nicht nötig.

## Aufruf und Rückgabewerte

Die Funktion benötigt keine Argumente:

```lua
local name, reason = runtime.player.name()
local display_name = name or "Flameborn"
```

| Ergebnis | Bedeutung |
| --- | --- |
| `name, nil` | Der ausgewählte lokale Charaktername liegt als Lua-String im Cache vor. |
| `nil, reason` | Noch kein Name erfasst, native Abfrage nicht verfügbar oder Zugriff für die Mod nicht erlaubt. |

Die API fügt selbst keinen Ersatznamen ein. Im Beispiel entscheidet die Mod mit `name or "Flameborn"` über ihren Standardnamen. Die Namenseinstellung des Modloaders ist davon unabhängig.

`runtime.has("runtime.player.name")` zeigt an, ob gerade ein Name verfügbar ist. `runtime.status("runtime.player.name")` liefert zusätzlich eine Begründung, wenn die Abfrage nicht verfügbar ist.

## Vollständiges Lua-Beispiel mit erneutem Versuch

Dieses `src/mod.lua` liest den Namen beim Laden. Ist er noch nicht vorhanden, verwendet es den Standardnamen. Der bestehende Update-Callback liest den Cache alle 500 ms erneut und übernimmt einen später verfügbaren oder geänderten Namen. Eine Meldung entsteht nur bei einer Änderung:

```lua
runtime.require("runtime.lifecycle")

local display_name = nil

local function update_player_name()
  local name = runtime.player.name()
  local current_name = name or "Flameborn"

  if current_name ~= display_name then
    display_name = current_name
    shroudforge.log.info("Character name: " .. display_name)
  end
end

return {
  on_load = update_player_name,
  on_update = function(delta_seconds)
    update_player_name()
  end,
  update_interval_ms = 500
}
```

## Cache und Namensänderungen

Die native Menüanbindung erfasst den Namen bei der ersten verfügbaren Auswahl und erneut, wenn sich die Auswahl oder die Namensmetadaten ändern. Lua und Modloader verwenden denselben Cache. Ein Lua-Aufruf startet weder eine Log-Auswertung noch eine neue Suche im Spiel.

Der zuletzt erfolgreich erfasste Name bleibt nach dem Weltbeitritt im Cache. Er ist der lokal ausgewählte Charaktername. Er identifiziert keinen entfernten Spieler und ist weder eine SteamID noch ein Steam-Anzeigename. Auf einem Dedicated Server liefert die Funktion keinen lokalen Charakternamen.

Ob der Client gerade im Hauptmenü ist, beantwortet [`runtime.is_main_menu()`](#doc-modding-main-menu). Ein vorhandener Name allein bestätigt keinen Menüaufenthalt.
