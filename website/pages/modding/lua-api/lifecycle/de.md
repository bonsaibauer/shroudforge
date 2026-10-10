# Laufzeit-Callbacks

Ein Laufzeit-Mod gibt eine Tabelle mit Callback-Funktionen zurück. Der Loader ruft sie zum passenden Zeitpunkt auf. Deklariere `runtime` in `mod.json`.

```lua
runtime.require("runtime.lifecycle")

return {
  on_load = function()
    shroudforge.log.info("Mod runtime started")
  end,
  on_update = function(delta_seconds)
    -- Read current settings or process queued work here
  end,
  on_unload = function()
    shroudforge.log.info("Mod runtime stopped")
  end,
  update_interval_ms = 100
}
```

`on_load` läuft beim Start der Mod-Laufzeit. `on_update` erhält die verstrichene Zeit in Sekunden. `on_unload` läuft beim Beenden oder Entladen. Das Updateintervall ist standardmäßig 50 ms und erlaubt 8 bis 1000 ms. Verpasste Intervalle werden nicht nachgeholt.

Nutze `on_update` nicht für unnötige Arbeit pro Frame. Lies live änderbare Werte in der Callback-Funktion erneut. Prüfe bei Netzwerk- und nativen Aufrufen Rückgabewerte und Status.
