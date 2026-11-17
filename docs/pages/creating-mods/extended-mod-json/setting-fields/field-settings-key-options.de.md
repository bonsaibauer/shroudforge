---
{
  "id": "field-settings-key-options",
  "locale": "de",
  "title": "Feld `settings.<key>.options`",
  "summary": "Erlaubte Auswahlwerte mit den Namen, die Spieler sehen.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Einstellungen",
      "Einstellungsfelder"
    ]
  }
}
---

# Feld `settings.<key>.options`

Erlaubte Auswahlwerte mit den Namen, die Spieler sehen.

## Datentyp und Pflicht

`object` · optional

## Erlaubte Werte und Grenzen

Keine zusätzlichen Werte oder Grenzen im Schema angegeben.

## Beispiel

```json
{
  "settings": {
    "example": {
      "value": "easy",
      "control": "select",
      "options": {
        "easy": "Easy",
        "hard": "Hard"
      }
    }
  }
}
```

## Quelle

Schema: [extended.mod.json](https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
