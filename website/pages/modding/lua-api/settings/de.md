# Mod-Einstellungen in Lua lesen

Die Beschreibung und der Startwert stehen in `extended.mod.json`. Der Lua-Code liest den aktuellen Wert mit `shroudforge.settings.get(key, fallback)`.

```lua
local speed = shroudforge.settings.get("flightSpeed", 1.0)
```

Der Schlüssel muss exakt übereinstimmen. Der Fallback sollte denselben Datentyp haben wie `value`. Rufe `get` im Callback auf, wenn eine Spielerauswahl ohne Neustart berücksichtigt werden soll. Ein beim Laden einmal gelesener lokaler Wert wird nicht von selbst aktualisiert.

Ein Mod, der diese Laufzeitfunktion verwendet, braucht `runtime` in `mod.json`. Die Funktion selbst speichert keine Einstellung, das erledigt der Modloader. Siehe [Einstellungen und Steuerelemente](#doc-setting-controls).
