# Modding

Consulta el estado del menú principal y el nombre del personaje local seleccionado mediante el runtime de Lua. Ambas consultas usan los mismos datos nativos que el Modloader y no requieren acceso al mundo.

## Requisitos

Activa el mod y declara la capacidad `runtime` en `mod.json`:

```json
{
  "capabilities": ["runtime"]
}
```

Para estas consultas del cliente, establece `targets` en `["client"]` dentro de `extended.mod.json`. Llama a las funciones desde callbacks activos del runtime, como `on_load` y `on_update`. `runtime.phase == "ingame"` indica que el runtime del mod está iniciado, pero no confirma la entrada en un mundo. Ninguna de estas consultas necesita un perfil de ejecución. Las demás operaciones del mundo mantienen sus propios requisitos.

## Detectar el menú principal

```lua
local in_menu, reason = runtime.is_main_menu()
```

| Resultado | Significado |
| --- | --- |
| `true, nil` | Se ha confirmado el menú principal, incluidas las pantallas de selección de personaje, mundo y servidor. |
| `false, nil` | Se ha validado otro estado original del juego. |
| `nil, reason` | El estado es desconocido, está cambiando o la consulta no está disponible. |

Comprueba explícitamente `in_menu == true` o `in_menu == false`. `nil` no es un estado confirmado. `false` tampoco confirma por sí solo que se pueda leer el mundo. Comprueba por separado la operación necesaria, por ejemplo `runtime.has("runtime.ecs.read")`.

## Leer el nombre del personaje

```lua
local name, reason = runtime.player.name()
local display_name = name or "Flameborn"
```

`name, nil` devuelve como cadena UTF-8 el nombre del personaje seleccionado en el menú principal del cliente local. Los caracteres Unicode, como en `Ikiju 育樹`, se conservan. `nil, reason` significa que todavía no hay un nombre disponible, que la consulta no está disponible o que no se permite el acceso.

El mod elige su propio nombre alternativo. La API no introduce un valor predeterminado ni usa el nombre manual del Modloader. El valor no identifica a un jugador remoto y no es un SteamID ni el nombre visible de Steam. Un servidor dedicado no tiene un nombre de personaje del cliente local.

## Caché compartida

Lua y el Modloader leen la misma caché nativa. El tick existente del runtime actualiza el estado del menú. La integración existente del menú captura el nombre en la primera selección disponible y cuando cambian la selección o los metadatos del nombre. Las consultas Lua no inician otra búsqueda ni leen registros.

El último nombre capturado correctamente permanece en la caché al entrar en un mundo. Por tanto, un nombre guardado no confirma que el menú esté abierto. `runtime.has("runtime.is_main_menu")` y `runtime.has("runtime.player.name")` indican la disponibilidad de cada consulta. `runtime.status(...)` también proporciona una explicación.

## Ejemplo para `src/mod.lua`

Este ejemplo lee la caché al cargar el mod y después cada 500 ms. Elige un nombre alternativo y solo registra los cambios:

```lua
runtime.require("runtime.lifecycle")

local previous_message = nil

local function refresh()
    local in_menu = runtime.is_main_menu()
    local name = runtime.player.name()
    local state = "unknown"
    if in_menu == true then
        state = "menu"
    elseif in_menu == false then
        state = "other"
    end

    local message = state .. ": " .. (name or "Flameborn")
    if message ~= previous_message then
        previous_message = message
        shroudforge.log.info(message)
    end
end

return {
    on_load = refresh,
    on_update = function()
        refresh()
    end,
    update_interval_ms = 500,
}
```
