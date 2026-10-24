# Tastenkürzel

Hier siehst und änderst du Fensterkürzel von ShroudForge sowie Mod-Tastenkürzel, die ein Mod ausdrücklich als `control: "keybind"` in seiner `extended.mod.json` deklariert.

## Ein Kürzel belegen

Klicke auf das Feld und drücke die gewünschte Taste oder Kombination. Die Eingabe verwendet physische SDL3-Tastencodes, zum Beispiel `SDL:64` für F7, `Ctrl+SDL:29` oder `Shift+SDL:100`. Dadurch bleibt die Tastenposition über verschiedene Tastaturlayouts hinweg gleich. SDL liefert die lesbare Beschriftung passend zum aktuellen Betriebssystemlayout. Zusatztasten wie Ctrl, Alt, Shift und Meta müssen genau übereinstimmen.

Während der Aufnahme sperrt ShroudForge seine Fensterkürzel und die deklarierten Mod-Kürzel prozessübergreifend. Die Sperre bleibt auch nach Abbruch oder Fokuswechsel bestehen, bis alle Tasten und Maustasten losgelassen sind und anschließend drei Sekunden ohne erneuten Tastendruck oder Mausklick vergangen sind. Jede neue Eingabe startet die Wartezeit nach dem Loslassen erneut. Escape lässt sich nicht aufnehmen, auch nicht mit Zusatztasten: Das Spiel nutzt die Taste für sein Pausenmenü und zum Schließen seiner Fenster. Ein Druck darauf bricht die Aufnahme ab und behält das bisherige Kürzel.

Mit **Löschen** deaktivierst du ein Kürzel. Für ShroudForge-Fenster kannst du den eingebauten Standard wiederherstellen. Mod-Tastenkürzel bleiben Teil der eigenen Mod-Einstellungen. Die Felder in den Mod-Einstellungen und unter **Kompatibilität → Tastenkürzel** bearbeiten denselben gespeicherten Wert.

## Tastenkürzel in einem ShroudForge-Mod

Ein Mod-Autor deklariert das Kürzel als Einstellung in `extended.mod.json`:

```json
{
  "settings": {
    "pasteKey": {
      "value": "SDL:64",
      "label": "Bauplan einfügen",
      "control": "keybind",
      "applyAt": "live"
    }
  }
}
```

Der Mod liest diese Einstellung über die ShroudForge-Lua-API aus. Damit reagiert er auf den aktuell eingestellten Wert, statt eine feste Taste im Skript zu verwenden:

```lua
local was_down = false

return {
    on_update = function()
        local down = shroudforge.input.is_keybind_down("pasteKey")
        if down and not was_down then
            -- Die Aktion ausführen.
        end
        was_down = down
    end,
}
```

`shroudforge.input.keybind_label("pasteKey")` liefert die aktuelle lesbare SDL-Beschriftung für das Betriebssystemlayout. Die Tastenabfrage funktioniert bei fokussiertem Spielfenster. Eine Deklaration allein ändert keine fest im Skript hinterlegte Taste. Der Mod muss den Wert über diese API abfragen und für seine Aktion verwenden. Mod-Autoren sollen den Tastendruck nur beim Übergang von nicht gedrückt zu gedrückt auslösen, damit eine gehaltene Taste keine Aktion in jedem Update wiederholt.

## Konflikte und fremde Mods

ShroudForge weist auf Konflikte zwischen aktiven, von ShroudForge verwalteten Fensterkürzeln und deklarierten Mod-Keybinds hin. Aktive Kürzel mit überlappender Taste und Modifikatorkombination können beim Speichern nicht neu in Konflikt gebracht werden.

Eigenständige EML-Mods und DLL-Mods, die ihre eigenen Tastenkürzel verwalten, lassen sich hier nicht zuverlässig umbelegen oder abfangen. Stelle solche Kürzel in den Einstellungen des jeweiligen Mods oder in dessen eigener Konfiguration ein, sofern der Mod das anbietet. Auch ein EML-Mod kann hier erscheinen, wenn er ausdrücklich eine passende `keybind`-Einstellung in `extended.mod.json` deklariert und die ShroudForge-Lua-API dafür verwendet. Seine unabhängig verwalteten EML- oder DLL-Kürzel werden dadurch nicht übernommen.
