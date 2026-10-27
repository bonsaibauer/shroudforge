---
{
  "id": "control-keybind",
  "locale": "de",
  "title": "Steuerelement `keybind`",
  "summary": "Für ein Tastatur- oder Mauskürzel, das als ein Einstellungswert gespeichert wird.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Steuerelemente",
      "Steuerelementtypen"
    ]
  }
}
---

# Steuerelement `keybind`

Definiere ein Tastenkürzel einmal als Einstellung mit `control: "keybind"`. Dieselbe Einstellung erscheint in den Mod-Einstellungen und automatisch unter **Compatibility → Hotkeys**. Beide Ansichten bearbeiten denselben Wert.

```json
{
  "settings": {
    "pasteKey": {"value": "SDL:64", "label": "Bauplan einfügen", "control": "keybind", "applyAt": "live"}
  }
}
```

Lies die deklarierte Einstellung in Lua. Wiederhole ihre Standardtaste nicht im Skript.

```lua
return {
    on_update = function()
        if shroudforge.input.was_keybind_pressed("pasteKey") then
            -- Dieselbe Aktion wie in der Mod-Oberfläche aufrufen.
        end
    end,
}
```

`was_keybind_pressed` merkt einen kurzen Tastendruck auch dann, wenn er zwischen zwei Runtime-Updates beginnt und endet. Die Funktion liefert pro Mod und Einstellung einmal `true` für einen neuen Tastendruck. Für Aktionen, die während des Haltens wiederholt werden sollen, verwende `is_keybind_down`.

Das Kürzelfeld hat zwei Spalten. Links nimmst du die vollständige Tastaturkombination auf, rechts eine optionale Mauskombination. Beide Alternativen gehören zu derselben Einstellung. Jede davon löst die Aktion aus. Live gespeicherte Änderungen erreichen die Runtime beim nächsten Konfigurationsabruf innerhalb einer Sekunde.

Während der Aufnahme sperrt ShroudForge seine Fensterkürzel und die über diese API abgefragten Mod-Kürzel prozessübergreifend. Die Sperre bleibt auch nach Abbruch oder Fokuswechsel bestehen, bis alle Tasten und Maustasten losgelassen sind und anschließend drei Sekunden ohne erneuten Tastendruck oder Mausklick vergangen sind. Jede neue Eingabe startet die Wartezeit nach dem Loslassen erneut. Escape lässt sich nicht aufnehmen, auch nicht mit Zusatztasten: Das Spiel nutzt die Taste für sein Pausenmenü und zum Schließen seiner Fenster. Ein Druck darauf bricht die Aufnahme ab und behält den bisherigen Wert.

Der Wert bleibt eine Zeichenfolge. Eine Tastaturkombination speichert einen [SDL-Scancode](https://wiki.libsdl.org/SDL3/SDL_Scancode), etwa `SDL:64` für F7, `Ctrl+SDL:29` für die Z-Position oder `Shift+SDL:100` für die zusätzliche ISO-Taste. Eine Mauskombination speichert eine SDL-Tastennummer, etwa `Mouse:4` für X1 oder `Ctrl+Mouse:4` mit Strg. Verbinde Tastatur und Maus mit `|`, zuerst die Tastatur, zum Beispiel `Ctrl+SDL:29|Mouse:4`. Pro Gerät ist höchstens eine Kombination zulässig. Optionale Zusatztastenpräfixe stehen einmalig in der Reihenfolge `Ctrl`, `Alt`, `Shift`, `Meta` und müssen genau übereinstimmen. Eine leere Zeichenfolge deaktiviert das Kürzel.

SDL liefert die angezeigten Beschriftungen. `shroudforge.input.keybind_label("pasteKey")` liefert die aktuelle Beschriftung einschließlich beider Alternativen oder `nil`, wenn das Kürzel inaktiv ist. Die Mausbeobachtung verwendet SDLs globalen Tastenzustand. Mausradkürzel werden nicht unterstützt.

ShroudForge beobachtet Kürzel im Spiel in einem eigenen unsichtbaren Prozess und übergibt Eingabezustände und Beschriftungen an die Lua-Runtime. Im Spielprozess wird keine SDL-Eingabe registriert. Zulässig sind ausschließlich die oben beschriebenen SDL-Belegungszeichenfolgen. Die frühere API `is_key_down("F1")` bis `is_key_down("F8")` wird seit ShroudForge 1.7.6 nicht mehr unterstützt. Ein Aufruf protokolliert einen Migrationshinweis und löst einen Lua-Fehler aus. Deklariere stattdessen eine `keybind`-Einstellung und weise sie im Hotkey-Manager unter **Kompatibilität → Tastenkürzel** zu.

Diese API verwendet ausschließlich deklarierte `keybind`-Einstellungen. Vorhandene `enabledWhen`-Bedingungen gelten ebenfalls. Tastenkürzel fremder nativer DLLs werden dadurch nicht automatisch gesteuert. Weitere Regeln stehen unter [settings.<key>.control](#doc-field-settings-key-control).
