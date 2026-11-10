# Buscar y organizar ajustes grandes de mods

Usa los grupos de ajustes existentes del Modloader para organizar la página de un mod en pestañas y hacer que las listas largas de selección múltiple se puedan buscar y categorizar de forma opcional. Estas funciones son optativas. Los valores guardados no cambian a menos que el jugador modifique una selección. Los textos del mod usan sus archivos de idioma existentes.

## Qué cambia en una página de mod grande

Esta función es para los ajustes que un mod declara en `extended.mod.json`. Usa la página de ajustes, los valores guardados y los archivos de idioma del mod existentes. No añade otro almacén de ajustes.

## Organizar una selección múltiple grande

En `multiselect`, `value` sigue siendo un array y las opciones disponibles se declaran en `options`. Los controles de búsqueda aparecen automáticamente cuando hay más de 12 opciones. El ejemplo también agrupa las opciones y coloca el grupo de ajustes en su propia pestaña de la página.

```json
{
  "schemaVersion": 1,
  "enabled": true,
  "settings": {
    "showEffects": {
      "value": true,
      "label": "Show effects",
      "control": "toggle"
    },
    "excludeItems": {
      "value": [],
      "label": "Excluded items",
      "description": "Choose items this mod should ignore.",
      "control": "multiselect",
      "options": {
        "Twigs": { "label": "Twigs", "group": "Materials" },
        "Stone": { "label": "Stone", "group": "Materials" },
        "Resin": { "label": "Resin", "group": "Materials" },
        "Plant Fiber": { "label": "Plant Fiber", "group": "Materials" },
        "Shroud Wood": { "label": "Shroud Wood", "group": "Materials" },
        "Fossilized Bone": { "label": "Fossilized Bone", "group": "Materials" },
        "Metal Scraps": { "label": "Metal Scraps", "group": "Materials" },
        "Fire Fly": { "label": "Fire Fly", "group": "Creatures" },
        "Fell Wispwyvern": { "label": "Fell Wispwyvern", "group": "Creatures" },
        "Hollow Halls": { "label": "Hollow Halls", "group": "Creatures" },
        "Bronze Arrows": { "label": "Bronze Arrows", "group": "Ammunition" },
        "Explosive Arrows": { "label": "Explosive Arrows", "group": "Ammunition" },
        "Poison Arrows": { "label": "Poison Arrows", "group": "Ammunition" }
      }
    }
  },
  "groups": [
    { "label": "General", "settings": ["showEffects"] },
    { "label": "Excluded items", "settings": ["excludeItems"], "tab": true }
  ]
}
```

Las claves de opción, como `Twigs`, son los valores que se guardan para el mod. Los textos de opción simples, como `"Twigs": "Twigs"`, siguen siendo compatibles. El formato de objeto añade una etiqueta visible `label` y un `group` opcional. Las categorías dividen las opciones en pestañas dentro de la selección múltiple. La búsqueda examina tanto la etiqueta visible como el valor guardado en todas las categorías. El contador muestra las opciones visibles, totales y seleccionadas. «Solo seleccionadas» oculta las opciones no marcadas sin desmarcarlas. Al cambiar una casilla se actualiza el mismo array guardado de siempre, y las listas largas se desplazan dentro de su ajuste.

## Traducir etiquetas y pestañas del mod

Los textos creados por el mod van en sus archivos existentes `locales/<code>.json`. Los controles compartidos, como Buscar, Solo seleccionadas y Todas, usan el idioma general del Modloader elegido por el jugador.

```json
{
  "settings": {
    "excludeItems": {
      "options": {
        "Twigs": "Twigs",
        "Stone": "Stone",
        "Fire Fly": "Fire Fly",
        "Bronze Arrows": "Bronze Arrows"
      },
      "optionGroups": {
        "Materials": "Materials",
        "Creatures": "Creatures",
        "Ammunition": "Ammunition"
      }
    }
  },
  "groups": [
    { "label": "General" },
    { "label": "Excluded items" }
  ]
}
```

El mapa `options` traduce las etiquetas visibles de las opciones. `optionGroups` traduce las pestañas de categorías y cada clave debe coincidir con un nombre `group` de `extended.mod.json`. El array `groups` sigue el orden de los grupos del manifiesto y traduce las pestañas de la página. Consulta la [guía de idiomas para mods](#wiki/modding-locales) para ver el formato existente.

## Poner un grupo de ajustes en su propia pestaña

Establece `"tab": true` en un grupo de `extended.mod.json`. La etiqueta del grupo se convierte en su pestaña de página, y sus ajustes y acciones permanecen juntos. Los grupos sin `tab: true` siguen en la pestaña Ajustes existente. Si no queda ningún grupo sin marcar ni ningún ajuste sin agrupar, se omite la pestaña Ajustes. Como ejemplo funcional, el [`extended.mod.json` del World Editor de ShroudForge](https://github.com/bonsaibauer/shroudforge/blob/1.8.0/mods/world-editor/extended.mod.json) usa `tab: true` en Keyboard shortcuts, Blueprint placement y Native entity operations.

## Valores guardados y compatibilidad

Las etiquetas simples existentes, los arrays guardados, los grupos sin `tab` y los archivos de idioma sin `optionGroups` siguen funcionando sin migración. Los controles de búsqueda solo aparecen en selecciones múltiples con más de 12 opciones. Los objetos de opción agrupados, `tab: true` y `optionGroups` en los archivos de idioma requieren ShroudForge 1.8.0 o posterior. Las versiones anteriores rechazan los campos que no reconocen.
