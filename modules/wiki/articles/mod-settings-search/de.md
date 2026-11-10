# Große Mod-Einstellungen durchsuchen und ordnen

Mit den vorhandenen Einstellungsgruppen des Modloaders kannst du eine Mod-Seite in Tabs gliedern und lange Mehrfachauswahllisten durchsuchbar machen sowie optional kategorisieren. Die Funktionen sind optional. Gespeicherte Werte bleiben unverändert, solange der Spieler keine Auswahl ändert. Für Mod-Texte kommen die vorhandenen Locale-Dateien der Mod zum Einsatz.

## Was sich auf einer großen Mod-Seite ändert

Diese Funktion gilt für Einstellungen, die eine Mod in `extended.mod.json` deklariert. Sie verwendet die vorhandene Einstellungsseite, die gespeicherten Einstellungswerte und die Locale-Dateien der Mod. Sie fügt keinen weiteren Einstellungs-Speicher hinzu.

## Eine große Mehrfachauswahl ordnen

Bei `multiselect` bleibt `value` ein Array und die verfügbaren Optionen stehen in `options`. Die Suchfunktionen erscheinen automatisch, wenn es mehr als 12 Optionen gibt. Das Beispiel gruppiert Optionen und legt die Einstellungsgruppe zusätzlich in einen eigenen Seitentab.

```json
{
  "schemaVersion": 1,
  "enabled": true,
  "settings": {
    "showEffects": {
      "value": true,
      "label": "Show effects",
      "control": "toggle"
    },
    "excludeItems": {
      "value": [],
      "label": "Excluded items",
      "description": "Choose items this mod should ignore.",
      "control": "multiselect",
      "options": {
        "Twigs": { "label": "Twigs", "group": "Materials" },
        "Stone": { "label": "Stone", "group": "Materials" },
        "Resin": { "label": "Resin", "group": "Materials" },
        "Plant Fiber": { "label": "Plant Fiber", "group": "Materials" },
        "Shroud Wood": { "label": "Shroud Wood", "group": "Materials" },
        "Fossilized Bone": { "label": "Fossilized Bone", "group": "Materials" },
        "Metal Scraps": { "label": "Metal Scraps", "group": "Materials" },
        "Fire Fly": { "label": "Fire Fly", "group": "Creatures" },
        "Fell Wispwyvern": { "label": "Fell Wispwyvern", "group": "Creatures" },
        "Hollow Halls": { "label": "Hollow Halls", "group": "Creatures" },
        "Bronze Arrows": { "label": "Bronze Arrows", "group": "Ammunition" },
        "Explosive Arrows": { "label": "Explosive Arrows", "group": "Ammunition" },
        "Poison Arrows": { "label": "Poison Arrows", "group": "Ammunition" }
      }
    }
  },
  "groups": [
    { "label": "General", "settings": ["showEffects"] },
    { "label": "Excluded items", "settings": ["excludeItems"], "tab": true }
  ]
}
```

Optionsschlüssel wie `Twigs` sind die Werte, die für die Mod gespeichert werden. Ein Textlabel wie `"Twigs": "Twigs"` wird weiterhin unterstützt. Die Objektform ergänzt ein sichtbares `label` und optional ein `group`. Kategorien ordnen die Optionen in Tabs innerhalb der Mehrfachauswahl. Die Suche prüft sowohl die sichtbare Beschriftung als auch den gespeicherten Wert und durchsucht alle Kategorien. Die Zählung zeigt sichtbare, gesamte und ausgewählte Optionen. „Nur Ausgewählte“ blendet nicht gewählte Optionen aus, ohne sie abzuwählen. Beim Ändern eines Kontrollkästchens wird wie bisher dasselbe gespeicherte Array aktualisiert. Lange Listen werden innerhalb des Einstellungsfeldes gescrollt.

## Mod-Beschriftungen und Tabs übersetzen

Texte der Mod gehören in die vorhandenen Dateien `locales/<code>.json`. Gemeinsame Bedienelemente wie Suche, „Nur Ausgewählte“ und „Alle“ verwenden die allgemeine Modloader-Sprache des Spielers.

```json
{
  "settings": {
    "excludeItems": {
      "options": {
        "Twigs": "Twigs",
        "Stone": "Stone",
        "Fire Fly": "Fire Fly",
        "Bronze Arrows": "Bronze Arrows"
      },
      "optionGroups": {
        "Materials": "Materials",
        "Creatures": "Creatures",
        "Ammunition": "Ammunition"
      }
    }
  },
  "groups": [
    { "label": "General" },
    { "label": "Excluded items" }
  ]
}
```

Die Zuordnung `options` übersetzt sichtbare Optionsbeschriftungen. `optionGroups` übersetzt die Kategorietabs. Jeder Schlüssel muss einem `group`-Namen in `extended.mod.json` entsprechen. Das Array `groups` folgt der Reihenfolge der Gruppen im Manifest und übersetzt deren Seitentabs. Im [Mod-Locale-Leitfaden](#wiki/modding-locales) findest du das bestehende Locale-Format.

## Eine Einstellungsgruppe in einen eigenen Tab verschieben

Setze in `extended.mod.json` bei einer Gruppe `"tab": true`. Das Label dieser Gruppe wird zum Seitentab, und ihre Einstellungen und Aktionen bleiben zusammen. Gruppen ohne `tab: true` bleiben unter dem vorhandenen Tab „Einstellungen“. Gibt es keine nicht markierten Gruppen oder ungruppierten Einstellungen, wird der Tab „Einstellungen“ weggelassen. Ein funktionierendes Beispiel ist die [`extended.mod.json` des ShroudForge World Editor](https://github.com/bonsaibauer/shroudforge/blob/1.8.0/mods/world-editor/extended.mod.json). Dort verwenden Keyboard shortcuts, Blueprint placement und Native entity operations jeweils `tab: true`.

## Gespeicherte Werte und Kompatibilität

Vorhandene Textlabels für Optionen, gespeicherte Arrays, Gruppen ohne `tab` und Locale-Dateien ohne `optionGroups` funktionieren weiterhin ohne Migration. Die Suchfunktionen erscheinen nur bei Mehrfachauswahlen mit mehr als 12 Optionen. Gruppierte Optionsobjekte, `tab: true` und Locale-`optionGroups` benötigen ShroudForge 1.8.0 oder neuer. Ältere Versionen lehnen unbekannte Felder ab.
