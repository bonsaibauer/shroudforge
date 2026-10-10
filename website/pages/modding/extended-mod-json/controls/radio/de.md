# Steuerelement `radio`

Zeigt alle Werte aus options, von denen genau einer aktiv ist. Lege es bei einer Einstellung als `control` fest.

```json
{
  "settings": {
    "example": {"value": "easy", "label": "Optionsfelder", "control": "radio", "options": {"easy": "Einfach", "hard": "Schwer"}}
  }
}
```

Die Optionsschlüssel sind gespeicherte Werte. Bei `multiselect` muss `value` eine Liste sein. Weitere Regeln stehen unter [settings.<key>.control](#doc-field-settings-key-control).
