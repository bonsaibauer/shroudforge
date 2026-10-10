# mod.json, der Steckbrief deines Mods

Jeder Mod braucht eine `mod.json`. ShroudForge liest sie, bevor der Mod vorbereitet oder gestartet wird. Die Datei beschreibt den Mod und seine angeforderten Funktionen.

## Pflichtfelder

`id`, `name` und `version` sind erforderlich. Die anderen Felder sind freiwillig. Jede Eigenschaft hat eine eigene Seite in dieser Gruppe.

## Welche Datei wird hier beschrieben?

Das kanonische Schema liegt in [manifest.schema.json](../../../schemas/manifest.schema.json). Der Loader prüft zusätzlich Abhängigkeiten und Laufzeitbedingungen in [manifest_reader.rs](https://github.com/bonsaibauer/shroudforge/blob/HEAD/src/loader/package/src/registry/manifest_reader.rs).

## Kleines Beispiel

```json
{
  "id": "example.hello-ember",
  "name": "Hello Ember",
  "version": "1.0.0",
  "authors": ["Your Name"],
  "capabilities": ["runtime"],
  "dependencies": []
}
```

## Zugehörige Themen

- [Die optionale extended.mod.json](#doc-extended-mod-json-guide) enthält Einstellungen und Modloader-Aktionen.
- [Einen Mod Schritt für Schritt bauen](#first) zeigt ein vollständiges kleines Paket.
- [EML-Mods verstehen und migrieren](#doc-eml-migration) erklärt Herkunft und Unterschiede.
