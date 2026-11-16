# Connexion P2P du Modloader

Découvrez quand le Modloader peut joindre un serveur dédié via Steam P2P et quelles autorisations des groupes d’utilisateurs d’Enshrouded permettent d’administrer le serveur.

## Se connecter dans le jeu

1. Lancez Enshrouded et entrez dans un monde hébergé par le serveur dédié.
2. Ouvrez le Modloader et sélectionnez **Serveur** comme cible.
3. Attendez que l’état du serveur affiche **Connecté**. La fenêtre du bureau et le menu principal ne confirment pas une connexion P2P dans un monde. Le champ du mot de passe serveur reste donc masqué.
4. Saisissez le mot de passe d’une entrée valide de `userGroups`. Le serveur vérifie le mot de passe via la connexion P2P.

## Autorisations requises pour le groupe serveur

Le mot de passe saisi doit correspondre exactement à un groupe dont le `name` n’est pas vide. Ce même groupe doit définir les cinq autorisations ci-dessous avec la valeur booléenne JSON `true`. Un champ absent, `false` ou une valeur d’un autre type ne permet pas d’administrer le serveur depuis le Modloader.

```json
"userGroups": [
  {
    "name": "Admin",
    "password": "REPLACE_WITH_A_PRIVATE_PASSWORD",
    "canKickBan": true,
    "canAccessInventories": true,
    "canEditWorld": true,
    "canEditBase": true,
    "canExtendBase": true,
    "reservedSlots": 0
  }
]
```

`reservedSlots` définit le nombre de places réservées aux joueurs. Ce champ ne fait pas partie de la vérification des autorisations du Modloader. ShroudForge envoie le mot de passe au serveur pour validation et ne le conserve pas. Le serveur vérifie de nouveau les autorisations du groupe lors des actions d’administration. Après cinq échecs, une attente de 60 secondes est imposée avant une nouvelle connexion.
