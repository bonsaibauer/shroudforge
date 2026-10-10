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

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
