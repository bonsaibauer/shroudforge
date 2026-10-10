# Feld `dependencies[].optional`

Ob eine fehlende oder inkompatible Abhängigkeit den Start blockiert.

## Datentyp und Pflicht

`boolean` · optional

## Erlaubte Werte und Grenzen

- Wenn true, darf dieser Mod auch starten, wenn die Abhängigkeit fehlt. Standard ist false.

## Beispiel

```json
{
  "dependencies": [
    {
      "id": "helper.mod",
      "version": "^1.0.0",
      "optional": true
    }
  ]
}
```

## Quelle

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
