---
{
  "id": "control-multiselect",
  "locale": "de",
  "title": "Steuerelement `multiselect`",
  "summary": "Erlaubt mehrere Werte. value ist eine Liste und options muss die möglichen Elemente enthalten.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Steuerelemente",
      "Steuerelementtypen"
    ]
  }
}
---

# Steuerelement `multiselect`

Erlaubt mehrere Werte. `value` ist eine Liste und `options` enthält die möglichen Elemente. Lege es bei einer Einstellung als `control` fest. Listen mit mehr als 12 Optionen erhalten Suche, einen Filter für ausgewählte Einträge, eine Anzahl und einen begrenzten Scrollbereich. Die Suche berücksichtigt alle Kategorien.

```json
{
  "settings": {
    "example": {"value": ["easy"], "label": "Mehrfachauswahl", "control": "multiselect", "options": {"easy": {"label": "Einfach", "group": "Allgemein"}, "hard": {"label": "Schwer", "group": "Erweitert"}}}
  }
}
```

Die Optionsschlüssel sind gespeicherte Werte. Optionsbeschriftungen können Strings bleiben oder `{ "label": "Einfach", "group": "Allgemein" }` verwenden, um Kategorie-Tabs anzuzeigen. Bei `multiselect` muss `value` eine Liste sein. Mod-Locale-Dateien übersetzen Kategorienamen mit `settings.<key>.optionGroups`. Weitere Regeln stehen unter [settings.<key>.control](#doc-field-settings-key-control).
