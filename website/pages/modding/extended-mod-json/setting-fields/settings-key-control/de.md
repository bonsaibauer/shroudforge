# Feld `settings.<key>.control`

Art des Eingabefelds, das der Modloader zeigt.

## Datentyp und Pflicht

`string` · optional

## Erlaubte Werte und Grenzen

- Werte: `"toggle"`, `"checkbox"`, `"text"`, `"textarea"`, `"number"`, `"slider"`, `"select"`, `"radio"`, `"segmented"`, `"multiselect"`, `"keybind"`, `"color"`

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

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/extended.mod.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
