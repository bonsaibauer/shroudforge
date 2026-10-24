# Atajos de teclado

Esta página muestra y permite cambiar los atajos de las ventanas de ShroudForge y los atajos que un mod declara explícitamente como `control: "keybind"` en su archivo `extended.mod.json`.

## Asignar un atajo

Haz clic en el campo y pulsa la tecla o combinación que quieras. La entrada usa códigos físicos de SDL3, como `SDL:64` para F7, `Ctrl+SDL:29` o `Shift+SDL:100`. Así se mantiene la misma posición física entre distribuciones de teclado. SDL proporciona etiquetas legibles para la distribución actual del sistema operativo. Los modificadores como Ctrl, Alt, Shift y Meta deben coincidir exactamente.

Durante la grabación, ShroudForge bloquea los atajos de sus ventanas y los atajos declarados de los mods en todos sus procesos. El bloqueo también se mantiene después de cancelar o cambiar el foco de la ventana, hasta soltar todas las teclas y botones del ratón y dejar pasar tres segundos sin una nueva pulsación. Cada nueva pulsación reinicia la espera después de soltar la tecla o el botón. Escape no se puede grabar, ni sola ni con modificadores: el juego la usa para su menú de pausa y para cerrar sus ventanas. Pulsarla cancela la grabación y conserva el atajo anterior.

Usa **Borrar** para desactivar un atajo. En las ventanas de ShroudForge puedes restaurar el valor predeterminado integrado. Los atajos de cada mod siguen en sus propios ajustes. Los campos de los ajustes del mod y de **Compatibility → Hotkeys** modifican el mismo valor guardado.

## Atajos en un mod de ShroudForge

El autor del mod declara el atajo como un ajuste en `extended.mod.json`:

```json
{
  "settings": {
    "pasteKey": {
      "value": "SDL:64",
      "label": "Pegar plano",
      "control": "keybind",
      "applyAt": "live"
    }
  }
}
```

El mod lee este ajuste mediante la API Lua de ShroudForge. Así responde al valor actual en lugar de fijar una tecla en el script:

```lua
local was_down = false

return {
    on_update = function()
        local down = shroudforge.input.is_keybind_down("pasteKey")
        if down and not was_down then
            -- Ejecutar la acción.
        end
        was_down = down
    end,
}
```

`shroudforge.input.keybind_label("pasteKey")` devuelve la etiqueta legible actual de SDL para la distribución del sistema operativo. La consulta de teclas funciona cuando una ventana del juego está activa. La declaración por sí sola no cambia una tecla fijada en el script. El mod debe leer este valor mediante la API y usarlo para su acción. Los autores deben ejecutar la acción solo al pasar de tecla suelta a pulsada para evitar repetirla en cada actualización mientras se mantiene pulsada.

## Conflictos y otros mods

ShroudForge informa de conflictos entre los atajos activos de sus ventanas y los keybinds declarados por los mods. Al guardar no se puede introducir un nuevo conflicto entre atajos activos cuya tecla y combinación de modificadores se solapen.

Los mods EML independientes y los mods DLL que gestionan sus propios atajos no se pueden reasignar ni interceptar de forma fiable desde aquí. Configúralos en los ajustes o en el archivo de configuración del mod, si este ofrece esa opción. Un mod EML también puede aparecer aquí si declara explícitamente un ajuste `keybind` correspondiente en `extended.mod.json` y usa la API Lua de ShroudForge. Esto no incorpora sus atajos EML o DLL gestionados por separado.
