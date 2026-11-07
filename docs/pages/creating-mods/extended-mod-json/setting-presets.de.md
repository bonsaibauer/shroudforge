---
{
  "id": "setting-presets",
  "locale": "de",
  "title": "presets, mehrere Einstellungen auf einmal setzen",
  "summary": "Eine Voreinstellung wie Qualität setzt eine Liste anderer Einstellungen auf vorbereitete Werte. Wird eine davon von Hand geändert, wechselt die Voreinstellung auf Benutzerdefiniert.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Voreinstellungen"
    ]
  }
}
---

# `presets`, mehrere Einstellungen auf einmal setzen

Viele Spiele bieten eine Qualitätsauswahl wie Niedrig, Mittel oder Hoch, die mehrere Detailoptionen gemeinsam setzt. `presets` gibt den Einstellungen eines Mods im Modloader dasselbe Verhalten.

```json
{
  "settings": {
    "quality": {
      "value": "medium",
      "label": "Qualität",
      "options": {"low": "Niedrig", "medium": "Mittel", "custom": "Benutzerdefiniert"}
    },
    "viewRange": {"value": 128, "label": "Sichtweite", "min": 32, "max": 512},
    "detail": {"value": "medium", "label": "Detail", "options": {"low": "Niedrig", "medium": "Mittel"}}
  },
  "presets": {
    "quality": {
      "custom": "custom",
      "settings": ["viewRange", "detail"],
      "values": {
        "low": {"viewRange": 64, "detail": "low"},
        "medium": {"viewRange": 128, "detail": "medium"}
      }
    }
  }
}
```

## Verhalten im Modloader

- Die Wahl einer Voreinstellung im steuernden Feld setzt jede aufgeführte Einstellung auf den Wert dieser Voreinstellung. Wird dieselbe Voreinstellung erneut gewählt, gelten ihre Werte erneut.
- Die aufgeführten Einstellungen bleiben bearbeitbar. Wird eine davon von Hand geändert, wählt das steuernde Feld den Wert `custom`. Die übrigen Werte bleiben erhalten.
- Die Wahl von `custom` im steuernden Feld ändert sonst nichts.
- Alle geänderten Werte werden gemeinsam gespeichert, wie bei jeder anderen Änderung. Lua liest jede Einstellung wie gewohnt mit `shroudforge.settings.get`. Auch das steuernde Feld ist eine gewöhnliche Einstellung.

## Felder

| Feld | Typ | Bedeutung |
| --- | --- | --- |
| Schlüssel in `presets` | String | Schlüssel der steuernden Einstellung. Sie muss in `settings` stehen und einen String enthalten. |
| `custom` | String · optional | Wert des steuernden Felds für von Hand angepasste Einstellungen. Standard: `custom`. Hat das steuernde Feld `options`, muss dieser Wert eine davon sein. |
| `settings` | Liste von Strings | Schlüssel der Einstellungen, die die Voreinstellung setzt. Mindestens einer. |
| `values` | Objekt | Ein Eintrag pro Voreinstellung, mit dem Wert des steuernden Felds als Schlüssel. Jeder Eintrag gibt jedem Schlüssel aus `settings` einen Wert und keinem anderen. |

## Vom Paketleser geprüfte Regeln

- Die Einstellungen einer Voreinstellung müssen existieren. Sie dürfen weder das steuernde Feld selbst noch das steuernde Feld einer anderen Voreinstellung noch eine `keybind`-Einstellung enthalten. Eine Einstellung gehört höchstens zu einer Voreinstellung.
- Hat das steuernde Feld `options`, braucht jede Option außer `custom` einen Eintrag in `values`, und jeder Eintrag in `values` muss eine Option sein. Für `custom` gibt es nie einen Eintrag in `values`.
- Jeder Wert muss zu seiner Einstellung passen. Typ, `min` und `max`, `minLength` und `maxLength` sowie `options` werden wie bei einem gespeicherten Wert geprüft.

Die Einstellungsseite ist im Desktopfenster und im Fenster aus dem Eintrag **Mods** im Spielmenü dieselbe. Der Reiter **Tastenkürzel** bearbeitet Tastenbelegungen direkt. Deshalb dürfen `keybind`-Einstellungen zu keiner Voreinstellung gehören.

## Quelle

Schema: [extended.mod.json](https://github.com/bonsaibauer/shroudforge/blob/HEAD/modules/modloader/manifest/extended.mod.schema.json). Der Paketleser prüft die oben beschriebenen Beziehungen.
