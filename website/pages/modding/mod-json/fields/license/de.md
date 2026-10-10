# Feld `license`

Lizenzkennung, falls der Mod eine Lizenz erklärt.

## Datentyp und Pflicht

`string | null` · optional

## Erlaubte Werte und Grenzen

- Optionale Lizenzkennung, zum Beispiel MIT. Verwende null, wenn keine Lizenz angegeben wird.

## Beispiel

```json
{
  "license": "example"
}
```

## Quelle

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
