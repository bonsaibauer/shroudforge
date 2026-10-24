---
{
  "id": "modding-main-menu",
  "locale": "de",
  "title": "Hauptmenüzustand mit Lua abfragen",
  "summary": "runtime.is_main_menu liefert den gespeicherten Hauptmenüzustand, ohne Weltzugriff zu benötigen. Unbekannte Zustände bleiben ausdrücklich unbekannt.",
  "navigation": {
    "path": ["Modding"],
    "label": "Hauptmenü erkennen"
  },
  "apiSymbols": ["runtime.is_main_menu"]
}
---

# Hauptmenüzustand mit Lua abfragen

`runtime.is_main_menu()` beantwortet, ob der lokale Spielclient im Hauptmenü ist. Dazu gehören die Auswahlansichten für Charaktere, Welten und Server. Eine Mod kann damit eigene Anzeigen oder Arbeitsschritte an den Menüaufenthalt anpassen.

## Voraussetzungen

Deklariere in `mod.json` die [Berechtigung `runtime`](#doc-capability-runtime) und aktiviere die Mod im Modloader:

```json
{
  "capabilities": ["runtime"]
}
```

Das Beispiel läuft im Client. Setze dafür in `extended.mod.json` [targets](#doc-field-targets) auf `["client"]`. Die Abfrage funktioniert in den [Laufzeit-Callbacks](#doc-api-lifecycle), auch bevor eine Welt betreten wurde. Sie benötigt keinen Weltzugriff und kein Ausführungsprofil.

## Aufruf und Rückgabewerte

Die Funktion benötigt keine Argumente:

```lua
local in_menu, reason = runtime.is_main_menu()
```

| Ergebnis | Bedeutung |
| --- | --- |
| `true, nil` | Der ursprüngliche Spielzustand bestätigt das Hauptmenü und seine Auswahlansichten. |
| `false, nil` | Ein anderer ursprünglicher Spielzustand wurde bestätigt. |
| `nil, reason` | Der Zustand ist unbekannt, wechselt gerade oder die Abfrage ist nicht verfügbar. |

Prüfe `in_menu == true` oder `in_menu == false`. Bei `nil` ist weder Menü noch Welt bestätigt. Auch `false` bestätigt für sich allein keinen lesbaren ECS-Weltzustand. Prüfe dafür die jeweils benötigte Operation, zum Beispiel `runtime.has("runtime.ecs.read")`.

`runtime.has("runtime.is_main_menu")` und `runtime.status("runtime.is_main_menu")` beschreiben die aktuelle Verfügbarkeit der Abfrage. Eine verfügbare Abfrage kann `false` liefern.

## Vollständiges Lua-Beispiel

Dieses `src/mod.lua` liest den Cache beim Laden und danach alle 250 ms. Es meldet nur Änderungen:

```lua
runtime.require("runtime.lifecycle")

local previous_state = nil

local function update_menu_state()
  local in_menu = runtime.is_main_menu()
  local state = "unknown"
  if in_menu == true then
    state = "menu"
  elseif in_menu == false then
    state = "other"
  end

  if state ~= previous_state then
    previous_state = state
    shroudforge.log.info("Menu state: " .. state)
  end
end

return {
  on_load = update_menu_state,
  on_update = function(delta_seconds)
    update_menu_state()
  end,
  update_interval_ms = 250
}
```

## Gemeinsame Quelle und Grenzen

Die Funktion liest denselben nativen Cache wie die Modloader-UI. Der vorhandene Runtime-Tick aktualisiert die Menüerkennung. Der Lua-Aufruf startet keine weitere Erkennung und liest keine Logs.

Desktopmodus und bestätigtes Hauptmenü erwarten keinen Weltzugriff. Deshalb zeigt der Modloader dort aktivierte, fehlerfreie Mods als **Bereit**. Modfehler, Blockierungen und erforderliche Neustarts behalten ihren eigenen Status. Ein Dedicated Server hat kein Hauptmenü des lokalen Spielclients.

Den lokalen Charakternamen erhältst du über [`runtime.player.name()`](#doc-modding-player-name). Die API-Verträge stehen in der [API-Referenz](#api).
