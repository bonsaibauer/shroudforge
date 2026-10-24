# Raccourcis clavier

Cette page affiche et modifie les raccourcis des fenêtres ShroudForge ainsi que ceux que les mods déclarent explicitement avec `control: "keybind"` dans leur fichier `extended.mod.json`.

## Attribuer un raccourci

Cliquez sur le champ et appuyez sur la touche ou la combinaison souhaitée. La saisie utilise des codes physiques SDL3, par exemple `SDL:64` pour F7, `Ctrl+SDL:29` ou `Shift+SDL:100`. La position physique reste ainsi cohérente entre les dispositions de clavier. SDL fournit des libellés lisibles pour la disposition actuelle du système d’exploitation. Les modificateurs comme Ctrl, Alt, Shift et Meta doivent correspondre exactement.

Pendant l’enregistrement, ShroudForge bloque ses raccourcis de fenêtre et les raccourcis de mods déclarés dans tous ses processus. Cette protection reste active même après une annulation ou un changement de fenêtre active, jusqu’à ce que toutes les touches et tous les boutons de souris soient relâchés, puis pendant trois secondes sans nouvel appui. Tout nouvel appui relance cette attente après le relâchement de la touche ou du bouton. Échap ne peut pas être enregistrée, seule ou avec des touches de modification : le jeu s’en sert pour son menu de pause et pour fermer ses fenêtres. L’appuyer annule l’enregistrement et garde le raccourci précédent.

Utilisez **Effacer** pour désactiver un raccourci. Pour les fenêtres ShroudForge, vous pouvez restaurer la valeur par défaut intégrée. Les raccourcis d’un mod restent dans ses propres paramètres. Les champs des paramètres du mod et de **Compatibility → Hotkeys** modifient la même valeur enregistrée.

## Raccourcis dans un mod ShroudForge

L’auteur du mod déclare le raccourci comme paramètre dans `extended.mod.json` :

```json
{
  "settings": {
    "pasteKey": {
      "value": "SDL:64",
      "label": "Coller le plan",
      "control": "keybind",
      "applyAt": "live"
    }
  }
}
```

Le mod lit ce paramètre avec l’API Lua de ShroudForge. Il utilise ainsi la valeur actuelle au lieu d’imposer une touche dans le script :

```lua
local was_down = false

return {
    on_update = function()
        local down = shroudforge.input.is_keybind_down("pasteKey")
        if down and not was_down then
            -- Exécuter l’action.
        end
        was_down = down
    end,
}
```

`shroudforge.input.keybind_label("pasteKey")` renvoie le libellé lisible actuel de SDL pour la disposition du système d’exploitation. La lecture des touches fonctionne lorsqu’une fenêtre du jeu est active. La déclaration seule ne remplace pas une touche codée en dur dans le script. Le mod doit lire cette valeur avec l’API et l’utiliser pour son action. Les auteurs doivent déclencher l’action uniquement lors du passage de la touche relâchée à la touche enfoncée afin d’éviter de la répéter à chaque mise à jour.

## Conflits et autres mods

ShroudForge signale les conflits entre les raccourcis actifs de ses fenêtres et les raccourcis déclarés par les mods. L’enregistrement ne peut pas créer un nouveau conflit entre des raccourcis actifs dont la touche et les modificateurs se chevauchent.

Les mods EML autonomes et les mods DLL qui gèrent leurs propres raccourcis ne peuvent pas être réattribués ou interceptés de façon fiable ici. Configurez-les dans les paramètres ou le fichier de configuration du mod, si celui-ci le permet. Un mod EML peut aussi apparaître ici s’il déclare explicitement un paramètre `keybind` correspondant dans `extended.mod.json` et utilise l’API Lua de ShroudForge. Ses raccourcis EML ou DLL gérés indépendamment ne sont pas repris pour autant.
