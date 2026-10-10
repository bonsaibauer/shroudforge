# Feld `dependencies[].version`

SemVer-Bereich, der für die Version des benötigten Mods akzeptiert wird.

## Datentyp und Pflicht

`string` · erforderlich

## Erlaubte Werte und Grenzen

- SemVer-Versionsbereich, zum Beispiel ^1.2.0 oder >=1.2.0, <2.0.0.

## Beispiel

```json
{
  "dependencies": [
    {
      "id": "helper.mod",
      "version": "^1.2.0"
    }
  ]
}
```

## Quelle

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
