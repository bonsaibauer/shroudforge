# Rechercher et organiser les grands paramètres de mods

Utilisez les groupes de paramètres existants du Modloader pour organiser la page d'un mod en onglets et rendre les longues listes à sélection multiple consultables avec des catégories facultatives. Ces fonctionnalités sont facultatives. Les valeurs enregistrées restent inchangées tant que le joueur ne modifie pas une sélection. Les textes du mod utilisent ses fichiers de langue existants.

## Ce qui change sur une grande page de mod

Cette fonctionnalité concerne les paramètres déclarés par un mod dans `extended.mod.json`. Elle utilise la page de paramètres, les valeurs enregistrées et les fichiers de langue du mod existants. Elle n'ajoute pas un autre stockage de paramètres.

## Organiser une grande sélection multiple

Pour `multiselect`, `value` reste un tableau et les choix disponibles sont déclarés dans `options`. Les commandes de recherche apparaissent automatiquement lorsque la liste comporte plus de 12 choix. L'exemple regroupe aussi les choix et place le groupe de paramètres dans son propre onglet de page.

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

Les clés de choix comme `Twigs` sont les valeurs enregistrées pour le mod. Les libellés sous forme de texte, par exemple `"Twigs": "Twigs"`, restent pris en charge. La forme objet ajoute un `label` visible et un `group` facultatif. Les catégories répartissent les choix dans des onglets internes à la sélection multiple. La recherche porte sur le libellé visible et la valeur enregistrée, dans toutes les catégories. Le compteur indique le nombre de choix visibles, total et sélectionnés. « Sélectionnés uniquement » masque les choix non cochés sans les désélectionner. Modifier une case met à jour le même tableau enregistré qu'auparavant. Une longue liste défile à l'intérieur de son paramètre.

## Traduire les libellés et les onglets du mod

Les textes propres au mod se trouvent dans ses fichiers existants `locales/<code>.json`. Les commandes communes comme Rechercher, Sélectionnés uniquement et Tout utilisent la langue générale choisie dans le Modloader.

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

La map `options` traduit les libellés visibles des choix. `optionGroups` traduit les onglets de catégorie et chaque clé doit correspondre à un nom `group` de `extended.mod.json`. Le tableau `groups` suit l'ordre des groupes du manifeste et traduit les onglets de page. Consultez le [guide des langues des mods](#wiki/modding-locales) pour connaître le format existant.

## Placer un groupe de paramètres dans son propre onglet

Définissez `"tab": true` sur un groupe de `extended.mod.json`. Le libellé de ce groupe devient son onglet de page, et ses paramètres et actions restent ensemble. Les groupes sans `tab: true` restent dans l'onglet Paramètres existant. S'il ne reste aucun groupe non marqué ni paramètre non groupé, l'onglet Paramètres est omis. Pour voir un exemple fonctionnel, consultez le [`extended.mod.json` du World Editor de ShroudForge](https://github.com/bonsaibauer/shroudforge/blob/1.8.0/mods/world-editor/extended.mod.json), qui utilise `tab: true` pour Keyboard shortcuts, Blueprint placement et Native entity operations.

## Valeurs enregistrées et compatibilité

Les libellés texte existants, les tableaux enregistrés, les groupes sans `tab` et les fichiers de langue sans `optionGroups` restent compatibles sans migration. Les commandes de recherche n'apparaissent que pour les sélections multiples de plus de 12 choix. Les objets de choix groupés, `tab: true` et `optionGroups` dans les fichiers de langue nécessitent ShroudForge 1.8.0 ou une version ultérieure. Les anciennes versions rejettent les champs qu'elles ne reconnaissent pas.
