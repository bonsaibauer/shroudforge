---
{
  "id": "field-settings-key-control",
  "locale": "de",
  "title": "Feld `settings.<key>.control`",
  "summary": "Art des Eingabefelds, das der Modloader zeigt.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Einstellungen",
      "Einstellungsfelder"
    ]
  }
}
---

# Feld `settings.<key>.control`

Art des Eingabefelds, das der Modloader zeigt.

## Datentyp und Pflicht

`string` · optional

## Erlaubte Werte und Grenzen

- Werte: `"toggle"`, `"checkbox"`, `"text"`, `"textarea"`, `"number"`, `"slider"`, `"select"`, `"radio"`, `"segmented"`, `"multiselect"`, `"keybind"`, `"color"`

Bei `"keybind"` muss `value` eine SDL-Belegungszeichenfolge wie `"SDL:64"`, `"Mouse:4"` oder `"Ctrl+SDL:29|Mouse:4"` sein. Zulässig sind höchstens eine Tastatur- und eine Mauskombination, getrennt durch `|` mit der Tastatur zuerst. Die leere Zeichenfolge `""` deaktiviert das Kürzel. Optionale Zusatztastenpräfixe stehen je Kombination einmalig in der Reihenfolge `Ctrl`, `Alt`, `Shift`, `Meta`. Der Editor zeigt getrennte Tastatur- und Mausspalten und speichert diesen einen Wert. Die [Beschreibung von keybind](#doc-control-keybind) erklärt die Lua-API und das Bearbeiten.

## Beispiel

```json
{
  "settings": {
    "example": {
      "value": 1,
      "control": "slider"
    }
  }
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
