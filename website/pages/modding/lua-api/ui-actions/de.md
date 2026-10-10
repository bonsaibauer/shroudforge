# Modloader-Aktionsknöpfe mit Lua verbinden

Eine Aktion steht unter `groups[].actions[]`. Die `id` wird an `shroudforge.ui.on_action` übergeben. Beide IDs müssen gleich geschrieben sein.

```lua
shroudforge.ui.on_action("resetFeature", function()
  shroudforge.log.info("Reset action requested")
end)
```

Der Mod braucht `runtime`. Die Funktion muss registriert sein, während die Mod-Laufzeit geladen wird. `label` und `style` beschreiben die sichtbare Schaltfläche, `confirm` ist eine optionale Rückfrage.

Die Aktion selbst führt nur deinen Callback aus. Sie garantiert keine erfolgreiche Spieländerung. Prüfe Rückgabewerte und logge das Ergebnis. Details der API stehen in der [API-Referenz](#api).
