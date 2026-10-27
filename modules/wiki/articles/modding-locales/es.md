# Traducir los textos de un mod

Los textos visibles del Modloader y los textos de Lua usan los mismos archivos de idioma del mod. El idioma elegido en el Modloader se obtiene con `shroudforge.locale()`, incorporada en el PR #42.

## Archivos de idioma

Añade un archivo `locales/<código>.json` por cada idioma. Los códigos actuales del selector de idioma del Modloader son `de`, `de-AT`, `de-BAY`, `de-CH`, `en-GB`, `en-US`, `es`, `fr`, `hi`, `it`, `ja`, `nl`, `pl`, `sv` y `zh`. El nombre del archivo debe coincidir exactamente con el código. Estos archivos se guardan junto a `mod.json` y `extended.mod.json`.

El esquema está publicado en [`mod-locale.schema.json`](https://bonsaibauer.github.io/shroudforge/schemas/mod-locale.schema.json). El catálogo puede traducir `mod.name`, `mod.description`, las etiquetas de ajustes y opciones, los grupos, las acciones, las notas de versión y las cadenas Lua bajo `strings`. Los catálogos pueden estar incompletos. El texto original se conserva si falta una traducción.

```json
{
  "$schema": "https://bonsaibauer.github.io/shroudforge/schemas/mod-locale.schema.json",
  "settings": {
    "severity": {
      "label": "Nivel del registro",
      "options": { "info": "Información", "warn": "Aviso" }
    }
  },
  "groups": [
    { "label": "Registro", "actions": { "write": { "label": "Escribir mensaje" } } }
  ],
  "strings": { "overlay.ready": "Listo" }
}
```

Las claves de ajustes y opciones se mantienen iguales que en `extended.mod.json`. Los grupos siguen el mismo orden que en ese archivo. Las traducciones de acciones usan su identificador actual. Los valores guardados no cambian.

## Usar los mismos textos en Lua

```lua
local locale = shroudforge.locale()
local ready = shroudforge.i18n.get("overlay.ready", locale, "Ready")
```

`shroudforge.i18n.get(key, locale, fallback)` usa el mismo catálogo cargado que el Modloader. Busca primero el código regional, como `de-AT`, y después el idioma `de`. Si falta un texto, devuelve el texto alternativo o la clave. La consulta no vuelve a leer archivos. El PR #42 aporta la API de idioma y esta función la conecta con el catálogo de cada mod.

Si `shroudforge.locale()` devuelve `en` porque no se puede leer la configuración, la búsqueda consulta primero `en-GB.json` y después `en.json`. `en-GB` es el inglés predeterminado del Modloader. `en-US` sigue disponible como variante seleccionable explícitamente.

Los archivos de idioma no requieren el permiso `runtime`. Los textos propios de la interfaz de ShroudForge permanecen en las traducciones compartidas del Modloader.
