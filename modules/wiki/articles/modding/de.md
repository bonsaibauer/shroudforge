# Modding

Lies den Hauptmenüzustand und den ausgewählten lokalen Charakternamen mit der Lua-Runtime. Beide Abfragen nutzen dieselben nativen Daten wie der Modloader und benötigen keinen Weltzugriff.

## Voraussetzungen

Aktiviere die Mod und deklariere in `mod.json` die Berechtigung `runtime`:

```json
{
  "capabilities": ["runtime"]
}
```

Setze für diese Client-Abfragen in `extended.mod.json` das Feld `targets` auf `["client"]`. Verwende die Funktionen in aktiven Laufzeit-Callbacks wie `on_load` und `on_update`. `runtime.phase == "ingame"` bezeichnet die gestartete Mod-Laufzeit und bestätigt keinen Weltbeitritt. Für diese beiden Abfragen ist kein Ausführungsprofil nötig. Andere Weltoperationen behalten ihre eigenen Voraussetzungen.

## Hauptmenü erkennen

```lua
local in_menu, reason = runtime.is_main_menu()
```

| Rückgabe | Bedeutung |
| --- | --- |
| `true, nil` | Das Hauptmenü einschließlich Charakter-, Welt- und Serverauswahl wurde bestätigt. |
| `false, nil` | Ein anderer ursprünglicher Spielzustand wurde bestätigt. |
| `nil, reason` | Der Zustand ist unbekannt, wechselt gerade oder die Abfrage ist nicht verfügbar. |

Prüfe ausdrücklich `in_menu == true` oder `in_menu == false`. `nil` ist kein bestätigter Zustand. Auch `false` bestätigt für sich allein keinen lesbaren Weltzustand. Prüfe dafür die benötigte Operation, zum Beispiel `runtime.has("runtime.ecs.read")`.

## Charakternamen lesen

```lua
local name, reason = runtime.player.name()
local display_name = name or "Flameborn"
```

`name, nil` liefert den im lokalen Hauptmenü ausgewählten Charakternamen als UTF-8-String. Unicode-Zeichen wie in `Ikiju 育樹` bleiben erhalten. `nil, reason` bedeutet, dass noch kein Name vorliegt oder der Zugriff nicht verfügbar beziehungsweise nicht erlaubt ist.

Die Mod bestimmt ihren Ersatznamen selbst. Die API setzt keinen Standardnamen ein und übernimmt nicht die manuelle Namenswahl des Modloaders. Der Wert bezeichnet keinen entfernten Spieler, keine SteamID und keinen Steam-Anzeigenamen. Ein Dedicated Server besitzt keinen lokalen Client-Charakternamen.

## Gemeinsamer Cache

Lua und Modloader lesen denselben nativen Cache. Der vorhandene Runtime-Tick aktualisiert den Menüstatus. Die bestehende Menüanbindung erfasst den Namen bei der ersten verfügbaren Auswahl und erneut bei einer geänderten Auswahl oder geänderten Namensmetadaten. Lua-Abfragen starten keine eigene Suche und lesen keine Logs.

Der zuletzt erfolgreich erfasste Name bleibt beim Weltbeitritt im Cache. Ein vorhandener Name bestätigt daher keinen Menüaufenthalt. `runtime.has("runtime.is_main_menu")` und `runtime.has("runtime.player.name")` melden die jeweilige Verfügbarkeit. `runtime.status(...)` ergänzt eine Begründung.

## Beispiel für `src/mod.lua`

Das Beispiel liest den Cache beim Laden und danach alle 500 ms. Es verwendet einen eigenen Ersatznamen und protokolliert nur Änderungen:

```lua
runtime.require("runtime.lifecycle")

local previous_message = nil

local function refresh()
    local in_menu = runtime.is_main_menu()
    local name = runtime.player.name()
    local state = "unknown"
    if in_menu == true then
        state = "menu"
    elseif in_menu == false then
        state = "other"
    end

    local message = state .. ": " .. (name or "Flameborn")
    if message ~= previous_message then
        previous_message = message
        shroudforge.log.info(message)
    end
end

return {
    on_load = refresh,
    on_update = function()
        refresh()
    end,
    update_interval_ms = 500,
}
```
