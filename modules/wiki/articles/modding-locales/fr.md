# Traduire les textes d’un mod

Les textes visibles dans le Modloader et les textes Lua utilisent les mêmes fichiers de langue du mod. La langue choisie dans le Modloader se lit avec `shroudforge.locale()`, ajoutée par la PR #42.

## Fichiers de langue

Ajoutez un fichier `locales/<code>.json` par langue. Les codes actuels du sélecteur du Modloader sont `de`, `de-AT`, `de-BAY`, `de-CH`, `en-GB`, `en-US`, `es`, `fr`, `hi`, `it`, `ja`, `nl`, `pl`, `sv` et `zh`. Le nom du fichier reprend exactement le code. Ces fichiers accompagnent `mod.json` et `extended.mod.json`.

Le schéma est publié sous [`mod-locale.schema.json`](https://bonsaibauer.github.io/shroudforge/schemas/mod-locale.schema.json). Le catalogue peut traduire `mod.name`, `mod.description`, les libellés des paramètres et des options, les groupes, les actions, les notes de version et les chaînes Lua sous `strings`. Les fichiers peuvent être incomplets. Le texte source est conservé pour les traductions manquantes.

```json
{
  "$schema": "https://bonsaibauer.github.io/shroudforge/schemas/mod-locale.schema.json",
  "settings": {
    "severity": {
      "label": "Niveau du journal",
      "options": { "info": "Information", "warn": "Avertissement" }
    }
  },
  "groups": [
    { "label": "Journal", "actions": { "write": { "label": "Écrire un message" } } }
  ],
  "strings": { "overlay.ready": "Prêt" }
}
```

Les clés des paramètres et des options restent identiques à celles de `extended.mod.json`. Les groupes suivent le même ordre que dans ce fichier. Les traductions d’action utilisent leur identifiant existant. Les valeurs enregistrées ne changent pas.

## Utiliser les mêmes textes dans Lua

```lua
local locale = shroudforge.locale()
local ready = shroudforge.i18n.get("overlay.ready", locale, "Ready")
```

`shroudforge.i18n.get(key, locale, fallback)` utilise le même catalogue chargé que le Modloader. Le code régional, comme `de-AT`, est recherché avant sa langue `de`. Si le texte manque, la fonction renvoie le texte de remplacement ou la clé. La recherche ne relit pas les fichiers. La PR #42 fournit l’API de langue, puis cette fonctionnalité la relie au catalogue de chaque mod.

Si `shroudforge.locale()` renvoie `en` parce que la configuration est illisible, la recherche consulte d’abord `en-GB.json`, puis `en.json`. `en-GB` est la langue anglaise par défaut du Modloader. `en-US` reste disponible comme variante sélectionnée explicitement.

Les fichiers de langue ne nécessitent pas la capacité runtime. Les textes de l’interface ShroudForge restent dans les traductions communes du Modloader.
