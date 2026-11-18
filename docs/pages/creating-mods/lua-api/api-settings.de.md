---
{
  "id": "api-settings",
  "locale": "de",
  "title": "Mod-Einstellungen in Lua lesen",
  "summary": "Der Schlüssel in extended.mod.json ist derselbe Schlüssel, den dein Lua-Code an shroudforge.settings.get übergibt.",
  "navigation": {
    "path": [
      "Lua-API-Anleitungen",
      "Funktionsanleitungen"
    ]
  },
  "apiSymbols": [
    "shroudforge.settings.get",
    "shroudforge.settings"
  ]
}
---

# Mod-Einstellungen in Lua lesen

Die Beschreibung und der Startwert stehen in `extended.mod.json`. SF Survival deklariert die Einstellung so:

```json
{
  "settings": {
    "infiniteStamina": {
      "value": true,
      "label": "Infinite Stamina",
      "description": "Keeps stamina full and adjusts automatically when its maximum changes."
    }
  }
}
```

Der Lua-Code liest den aktuellen Wert mit `shroudforge.settings.get(key, fallback)`.

```lua
local infiniteStamina = shroudforge.settings.get("infiniteStamina", false)
```

Das Beispiel stammt aus `mods/sf-survival/extended.mod.json`. Dort ist `infiniteStamina` mit dem booleschen Paket-Standardwert `true` deklariert. Wenn der gespeicherte Wert `true` ist, liefert der Aufruf genau den Lua-Boolean `true`. Bei einem gespeicherten Wert `false` liefert er `false`. Fehlt ein gespeicherter Wert, liefert er den Fallback `false`. Ohne Fallback liefert ein fehlender Wert `nil`. Die Funktion gibt den Einstellungswert direkt zurück, nicht in einem Ergebnisobjekt.

Der Schlüssel muss exakt übereinstimmen. Der Fallback sollte denselben Datentyp haben wie `value`. Rufe `get` im Callback auf, wenn eine Spielerauswahl ohne Neustart berücksichtigt werden soll. Ein beim Laden einmal gelesener lokaler Wert wird nicht von selbst aktualisiert.

Ein Mod, der diese Laufzeitfunktion verwendet, braucht `runtime` in `mod.json`. Die Funktion selbst speichert keine Einstellung, das erledigt der Modloader. Siehe [Einstellungen und Steuerelemente](#doc-setting-controls).
