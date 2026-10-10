# Einstellungen und Steuerelemente

Ein Eintrag unter `settings` hat einen mod-eigenen Schlüssel. Dieser Schlüssel ist die technische Verbindung zwischen `extended.mod.json` und deinem Lua-Code.

## Einfache und ausführliche Werte

Ein boolescher Wert, Text, Zahl oder eine Liste kann direkt als Wert stehen. Die Objektform ergänzt Metadaten:

```json
{
  "settings": {
    "flightSpeed": {
      "value": 1.0,
      "label": "Fluggeschwindigkeit",
      "description": "Wie schnell sich der Charakter bewegt.",
      "control": "slider",
      "min": 0.2,
      "max": 3.0,
      "step": 0.1
    }
  }
}
```

## Alle zwölf Steuerelemente

Die einzelnen Seiten erklären wann ein Steuerelement passt und welche Werte es erwartet. Boolean zeigt ohne Auswahl meist einen Schalter, Text einen Textbereich und Zahlen ein Zahlenfeld. Mit `control` kannst du die Darstellung ausdrücklich wählen.

- Umschalten: [toggle](#doc-control-toggle) und [checkbox](#doc-control-checkbox).
- Text: [text](#doc-control-text) und [textarea](#doc-control-textarea).
- Zahlen: [number](#doc-control-number) und [slider](#doc-control-slider).
- Auswahl: [select](#doc-control-select), [radio](#doc-control-radio), [segmented](#doc-control-segmented) und [multiselect](#doc-control-multiselect).
- Spezialfelder: [keybind](#doc-control-keybind) und [color](#doc-control-color).

## Werte in Lua verwenden

`shroudforge.settings.get("flightSpeed", 1.0)` liest den aktuellen gespeicherten Wert. Den genauen Schlüssel und einen Fallback desselben Typs angeben. Lies ihn in der Callback-Funktion, wenn Änderungen während der Sitzung berücksichtigt werden sollen. Siehe [Einstellungen in Lua lesen](#doc-api-settings).

## Gruppen sind optional

Mit `groups[].settings` legst du die Sortierung fest. Ein nicht aufgeführter Wert wird in der Standardgruppe angezeigt, nicht ausgeblendet. Der Modloader speichert Änderungen im selben `extended.mod.json`.
