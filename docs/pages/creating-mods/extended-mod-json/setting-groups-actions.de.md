---
{
  "id": "setting-groups-actions",
  "locale": "de",
  "title": "Gruppen und Aktionsknöpfe",
  "summary": "Gruppen ordnen Mod-Einstellungen. Aktionsknöpfe rufen eine benannte Funktion deines Lua-Mods auf.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Gruppen und Aktionen"
    ],
    "overview": true,
    "label": "Übersicht"
  }
}
---

# Gruppen und Aktionsknöpfe

Eine Gruppe hat eine sichtbare Beschriftung und kann Einstellungsschlüssel sowie Aktionen enthalten. Mit `tab: true` erscheint die Gruppe in einem eigenen Tab der Mod-Seite. Gruppen ohne dieses Feld bleiben im vorhandenen Tab „Einstellungen“. Dieser Tab entfällt, wenn jede Gruppe einen eigenen Tab hat. Die Tab-Beschriftungen verwenden die vorhandene Übersetzung über `groups[].label`. Gruppen ändern nicht den Zugriff des Mods.

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
