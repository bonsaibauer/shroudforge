# Estándar de carga DInput8 chain

Muchos cargadores de mods de Enshrouded y mods con DLL quieren ocupar el nombre `dinput8.dll` del juego, pero solo puede existir un archivo con ese nombre. El cargador que lo ocupa inicia las demás DLL indicadas en `dinput8-chain.ini`, junto al ejecutable del juego. EMF, Kiln, ShroudForge, XHL-Core y Auto Loot Plus leen este archivo. Esta página recoge las reglas para que todos los cargadores lo lean igual y la lista del jugador funcione con independencia de cuál ocupe `dinput8.dll`.

Esta página es de dominio público bajo CC0. Puedes copiarla en la documentación de tu proyecto y modificarla sin necesidad de dar crédito. Las propuestas aceptadas por varios cargadores son bienvenidas como pull requests en el repositorio de EMF.

Las palabras «debe» y «debería» indican el comportamiento de un cargador que sigue esta especificación.

## El archivo

```ini
; my mods
[Chain]
mods\mymod\mymod.dll
autolootplus.dll

[EMF]
creative.dll
```

- Ubicación: `dinput8-chain.ini` en la carpeta del ejecutable del juego (`enshrouded.exe` o `enshrouded_server.exe`).
- Codificación: el cargador debe leer UTF-8 con o sin BOM y UTF-16 LE con BOM. También debería leer la página de códigos ANSI del sistema cuando los bytes no sean UTF-8 válido, como ocurre con archivos guardados por una versión antigua del Bloc de notas. Una herramienta que escriba el archivo debe conservar la codificación y los saltos de línea encontrados.
- Líneas: se omiten las líneas vacías. Las que empiezan por `;`, `#` o `//` son comentarios. Si se antepone `; ` a una línea de DLL, esta queda desactivada y el jugador puede apagarla sin borrarla. Se puede añadir un comentario después del nombre, precedido de un espacio o tabulador (`mymod.dll ; my mod`), y el nombre puede ir entre comillas dobles.
- Secciones, sin distinguir mayúsculas y minúsculas: `[Chain]` es compartida. Todos los cargadores que siguen esta especificación inician sus DLL. Cualquier otra sección pertenece a una herramienta y lleva su nombre, por ejemplo `[EMF]` o `[Kiln]`. Los demás cargadores deben ignorarla. Las líneas anteriores a cualquier sección pertenecen a `[Chain]`. Los archivos antiguos pueden no tener secciones.
- Líneas `key=value`: una línea `name=value` cuenta como entrada de DLL cuando su valor nombra un archivo `.dll`. Las demás claves son ajustes de la herramienta que las escribió y los otros cargadores las ignoran.

## Las DLL

- Las rutas son relativas a la carpeta del ejecutable. `/` se interpreta como `\`. El cargador debe rechazar rutas absolutas, letras de unidad, rutas UNC (`\\server\...`) y cualquier parte `..`. La cadena no puede salir de la carpeta del juego.
- Solo se admiten archivos `.dll`. Si una línea nombra otro tipo de archivo, se omite y se informa de ello.
- Nunca se debe cargar la propia `dinput8.dll`, ni una segunda copia del cargador. Por ejemplo, EMF reconoce su propio cargador aunque tenga otro nombre y no carga esa copia.
- Cada DLL se inicia una sola vez. Si aparece repetida en `[Chain]` y en la sección de una herramienta, o dos veces en una sección, solo se inicia en su primera posición.
- Orden: las DLL se inician siguiendo sus líneas de arriba abajo. Se tienen en cuenta `[Chain]` y la sección propia del cargador en el orden en que aparecen en el archivo.
- Momento: el cargador usa `LoadLibrary` desde su propia `DllMain` o justo después, antes de que se ejecute código del juego. Es el momento en que se habría iniciado una DLL encadenada si ella misma fuera `dinput8.dll`.
- Se cuentan como máximo 256 entradas. Las demás se notifican y se omiten.
- Errores: si falta una DLL o no se puede cargar, se omite y se informa. Esto incluye el error 126 de Windows y el 1114 cuando falla su `DllMain`. El juego continúa.

## Informes

El cargador debería registrar una línea por cada DLL encadenada: iniciada, omitida por estar desactivada, rechazada o ya cargada, o fallida con el número de error de Windows. EMF también escribe la misma información en forma de tabla en `emf\chain_status.txt` para que los jugadores puedan compartirla.

## Al mover la dinput8.dll de otra herramienta

Si un instalador encuentra la `dinput8.dll` de otra herramienta, debería cambiarle el nombre, nunca borrarla, añadir el nuevo nombre a su propia sección de `dinput8-chain.ini` y restaurarla al desinstalar. EMF usa `<tool>.dll` si reconoce la herramienta. Si no, usa la información de versión de la DLL o el nombre `dinput8-previous.dll`, y anota lo que movió en `emf\moved.txt`.

## Exportaciones

El juego llama a `DirectInput8Create` del archivo que ocupa `dinput8.dll`. El cargador debe reenviar esa llamada a la `dinput8.dll` original de Windows (`%SystemRoot%\System32\dinput8.dll`). Las DLL encadenadas no reciben llamadas de DirectInput. Un mod que realice su trabajo en `DirectInput8Create` debe trasladarlo a `DllMain` o a un hilo que inicie.

## Quién sigue esta especificación

| Herramienta | Lee `[Chain]` | Sección propia |
| --- | --- | --- |
| EMF | Sí, también las líneas anteriores a cualquier sección | `[EMF]`, para las DLL que renombra su instalador |
| ShroudForge | Sí. Cada entrada debe estar dentro de `mods\<folder>\`. Las demás se notifican. | |
| Kiln | Sí, también `[EMF]` para las DLL renombradas por instaladores antiguos de EMF | Ninguna. Su lista está en `kiln\chain.ini`. Los nombres entre comillas y los comentarios después del nombre están previstos. |
| XHL-Core, Auto Loot Plus | Sí | |

Avísanos si tu herramienta lee el archivo o lo interpreta de otra manera para actualizar esta tabla.

## Fuente

[Especificación compartida de EMF para dinput8-chain.ini](https://github.com/matenm/emf-enshrouded/blob/main/docs/modding/dinput8-chain.md). La especificación original está dedicada al dominio público bajo CC0.
