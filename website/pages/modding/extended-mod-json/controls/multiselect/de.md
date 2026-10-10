# Steuerelement `multiselect`

Erlaubt mehrere Werte. value ist eine Liste und options muss die möglichen Elemente enthalten. Lege es bei einer Einstellung als `control` fest.

```json
{
  "settings": {
    "example": {"value": ["easy"], "label": "Mehrfachauswahl", "control": "multiselect", "options": {"easy": "Einfach", "hard": "Schwer"}}
  }
}
```

Die Optionsschlüssel sind gespeicherte Werte. Bei `multiselect` muss `value` eine Liste sein. Weitere Regeln stehen unter [settings.<key>.control](#doc-field-settings-key-control).
