# Feld `capabilities`

Zugriffsarten, die der Mod anfordert.

## Datentyp und Pflicht

`array` · optional

## Erlaubte Werte und Grenzen

- Listenelemente: `"patch"`, `"export"`, `"runtime"`, `"runtime-register-dll"`
- Einträge müssen eindeutig sein.

## Beispiel

```json
{
  "capabilities": [
    "runtime"
  ]
}
```

## Quelle

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
