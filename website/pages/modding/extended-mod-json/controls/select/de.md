# Steuerelement `select`

Für genau eine Auswahl aus options. Lege es bei einer Einstellung als `control` fest.

```json
{
  "settings": {
    "example": {"value": "easy", "label": "Auswahlliste", "control": "select", "options": {"easy": "Einfach", "hard": "Schwer"}}
  }
}
```

Die Optionsschlüssel sind gespeicherte Werte. Bei `multiselect` muss `value` eine Liste sein. Weitere Regeln stehen unter [settings.<key>.control](#doc-field-settings-key-control).
