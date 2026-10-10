# Feld `version`

Version dieser Mod-Veröffentlichung im SemVer-Grundformat.

## Datentyp und Pflicht

`string` · erforderlich

## Erlaubte Werte und Grenzen

- Muster: `^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$`

## Beispiel

```json
{
  "version": "1.0.0"
}
```

## Quelle

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/main/src/loader/package/src/registry/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
