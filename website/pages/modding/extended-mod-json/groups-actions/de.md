# Gruppen und Aktionsknöpfe

Eine Gruppe hat eine sichtbare Beschriftung und kann Einstellungsschlüssel sowie Aktionen enthalten. Gruppen ändern nicht den Zugriff des Mods.

## Einstellungen zuordnen

`groups[].settings` enthält die Schlüssel aus `settings`. Jeder Schlüssel darf nur einmal gruppiert werden. Nicht gruppierte Werte zeigt der Modloader in einer Standardgruppe.

## Aktion mit Lua verbinden

Der Schlüssel `groups[].actions[].id` muss genau dem Namen entsprechen, den dein Mod an `shroudforge.ui.on_action` übergibt. Beispiel:

```json
{
  "groups": [{
    "label": "Werkzeuge",
    "settings": ["enabledFeature"],
    "actions": [{"id": "resetFeature", "label": "Zurücksetzen", "style": "secondary"}]
  }]
}
```

```lua
shroudforge.ui.on_action("resetFeature", function()
  shroudforge.log.info("Feature settings reset")
end)
```

Die Mod benötigt `runtime` in `mod.json`. `style` ändert nur die Darstellung. `confirm` zeigt vor dem Aufruf eine Rückfrage. Siehe [UI-Aktionen in Lua](#doc-api-ui-actions).
