# Feld `icon`

Icon-Datei im Paketstamm oder unter assets/.

## Datentyp und Pflicht

`string | null` · optional

## Erlaubte Werte und Grenzen

- Muster: `^(?:assets/[A-Za-z0-9._/-]+|[A-Za-z0-9_-][A-Za-z0-9._-]*)$`
- Optionaler Dateiname des Mod-Icons im Paketordner oder ein Pfad unter assets/. Verwende null, wenn der Mod kein Icon hat.

## Beispiel

```json
{
  "icon": "example"
}
```

## Quelle

Schema: [mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/manifest.schema.json). Der Paketleser prüft zusätzliche Beziehungen zwischen Feldern.
