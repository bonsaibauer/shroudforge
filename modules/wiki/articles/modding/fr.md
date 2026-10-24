# Modding

Consultez l’état du menu principal et le nom du personnage local sélectionné avec le runtime Lua. Ces deux requêtes utilisent les mêmes données natives que le Modloader et ne nécessitent aucun accès au monde.

## Prérequis

Activez le mod et déclarez la capacité `runtime` dans `mod.json` :

```json
{
  "capabilities": ["runtime"]
}
```

Pour ces requêtes du client, définissez `targets` sur `["client"]` dans `extended.mod.json`. Appelez les fonctions depuis des callbacks actifs du runtime, comme `on_load` et `on_update`. `runtime.phase == "ingame"` indique que le runtime du mod a démarré et ne confirme pas l’entrée dans un monde. Ces deux requêtes ne nécessitent aucun profil d’exécution. Les autres opérations du monde conservent leurs propres prérequis.

## Détecter le menu principal

```lua
local in_menu, reason = runtime.is_main_menu()
```

| Résultat | Signification |
| --- | --- |
| `true, nil` | Le menu principal, y compris la sélection du personnage, du monde et du serveur, a été confirmé. |
| `false, nil` | Un autre état d’origine du jeu a été validé. |
| `nil, reason` | L’état est inconnu, change actuellement ou la requête est indisponible. |

Vérifiez explicitement `in_menu == true` ou `in_menu == false`. `nil` ne représente pas un état confirmé. `false` seul ne confirme pas non plus que le monde peut être lu. Vérifiez séparément l’opération nécessaire, par exemple `runtime.has("runtime.ecs.read")`.

## Lire le nom du personnage

```lua
local name, reason = runtime.player.name()
local display_name = name or "Flameborn"
```

`name, nil` renvoie sous forme de chaîne UTF-8 le nom du personnage sélectionné dans le menu principal du client local. Les caractères Unicode, comme dans `Ikiju 育樹`, sont préservés. `nil, reason` signifie qu’aucun nom n’est encore disponible, que la requête est indisponible ou que l’accès n’est pas autorisé.

Le mod choisit son propre nom de remplacement. L’API n’insère aucune valeur par défaut et n’utilise pas le nom manuel du Modloader. Cette valeur n’identifie aucun joueur distant et n’est ni un SteamID ni un nom affiché sur Steam. Un serveur dédié n’a pas de nom de personnage du client local.

## Cache partagé

Lua et le Modloader lisent le même cache natif. Le tick existant du runtime actualise l’état du menu. L’intégration existante du menu capture le nom à la première sélection disponible, puis lorsque la sélection ou les métadonnées du nom changent. Les requêtes Lua ne lancent aucune recherche supplémentaire et ne lisent aucun journal.

Le dernier nom capturé avec succès reste en cache après l’entrée dans un monde. Un nom en cache ne confirme donc pas que le menu est ouvert. `runtime.has("runtime.is_main_menu")` et `runtime.has("runtime.player.name")` indiquent la disponibilité de chaque requête. `runtime.status(...)` fournit aussi une explication.

## Exemple pour `src/mod.lua`

Cet exemple lit le cache au chargement, puis toutes les 500 ms. Il choisit un nom de remplacement et ne journalise que les changements :

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
