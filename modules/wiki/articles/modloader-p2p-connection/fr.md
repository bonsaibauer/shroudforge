# Connexion P2P du Modloader

Cette page explique comment le Modloader atteint un serveur dédié via Steam P2P, ce que Steam chiffre, comment les messages circulent entre le client et le serveur et quelles autorisations des groupes d’utilisateurs d’Enshrouded permettent d’administrer le serveur.

## Se connecter dans le jeu

1. Lancez Enshrouded et entrez dans un monde hébergé par le serveur dédié.
2. Ouvrez le Modloader et sélectionnez **Serveur** comme cible.
3. Attendez que l’état du serveur affiche **Connecté**. Cela signifie que le client a récemment reçu un message d’état Steam P2P. Cela ne vérifie pas que l’expéditeur est le serveur Enshrouded prévu. La fenêtre du bureau et le menu principal ne confirment pas une connexion P2P dans un monde. Le champ du mot de passe serveur y reste donc masqué.
4. Saisissez le mot de passe d’une entrée valide de <code>userGroups</code>. Le serveur vérifie le mot de passe via la connexion P2P.

## Fonctionnement de la connexion P2P

Le module Network s’exécute dans le processus du client Enshrouded ou du serveur dédié. Le jeu initialise Steam et ShroudForge réutilise l’interface Steam Networking Messages déjà chargée dans ce processus pour envoyer directement à une identité Steam. ShroudForge n’exécute pas de serveur relais et n’intègre pas ces messages aux paquets de jeu d’Enshrouded. Tant que le module Network est activé, le runtime exécute les services d’état P2P et du Modloader distant. Voir le [cycle du runtime](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/mod-runtime/ingame/update_ingame_runtime.rs) et l’[adaptateur Steam natif](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/native/steam_network.cpp).

## Chiffrement du transport et canaux

Les requêtes et réponses du Modloader sont encodées en octets JSON. Les mods du runtime peuvent envoyer des chaînes Lua binaires. ShroudForge n’ajoute pas ici de session TLS, de chiffrement du contenu ni d’échange de clés de bout en bout. Le code s’appuie sur Steam Networking pour l’authentification du pair et le chiffrement du transport. Valve indique que <code>ISteamNetworkingMessages</code> utilise les connexions sous-jacentes de <code>ISteamNetworkingSockets</code> et décrit le chiffrement et l’authentification de Steam Networking. Cela protège le transport vers une identité Steam. Cela ne prouve pas que le pair est le serveur Enshrouded visé et ne protège aucun des deux terminaux. Les numéros de canal servent uniquement au routage et ne contrôlent pas les autorisations. L’API Lua accepte les canaux 0 à 65533 et rejette les deux canaux réservés. Les messages internes du Modloader demandent une livraison fiable. Cela concerne la livraison et l’ordre, pas le chiffrement ni l’autorisation. Voir la documentation officielle d’[ISteamNetworkingMessages](https://partner.steamgames.com/doc/api/ISteamNetworkingMessages) et d’[ISteamNetworkingSockets](https://partner.steamgames.com/doc/api/ISteamNetworkingSockets), ainsi que les [définitions des canaux de transport](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/lib.rs) et l’[API Lua Network](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/lua/network.rs).

| Canal | Utilisation actuelle |
|---|---|
| <code>0</code> à <code>65533</code> | Messages entre mods Lua du runtime |
| <code>65534</code> | Miroir interne du serveur et contrôle distant du Modloader |
| <code>65535</code> | Vérifications d’état Steam P2P |

## Circulation des messages entre client et serveur

| Direction | Messages envoyés par le code actuel |
|---|---|
| Client vers serveur | Le client sélectionne <code>modules/network/serverSteamId</code> ou, si ce réglage est vide, utilise la dernière Steam ID de serveur distant découverte. Il envoie une demande d’abonnement environ toutes les 10 secondes. Après l’abonnement, il peut envoyer le mot de passe administrateur, puis un jeton de session avec des changements de réglages, des demandes de liste de mods et des opérations sur les mods. Le serveur oublie les pairs qui ne renouvellent plus leur abonnement pendant 30 secondes. |
| Serveur vers client | Un nouvel abonnement reçoit immédiatement une copie de la configuration du loader et les 64 derniers Kio de chaque journal mis en miroir. Le serveur envoie ensuite les changements de configuration et les nouvelles portions brutes des journaux aux pairs abonnés. Les demandes authentifiées comme administrateur reçoivent aussi les listes de mods, les données de compatibilité du serveur et les résultats des sessions et opérations. |
| Mod du runtime vers un autre mod | Un mod Lua envoie le message à une Steam ID explicite. <code>send_mod</code> ajoute une enveloppe JSON et choisit un canal déterministe selon l’ID du mod destinataire. Le destinataire vérifie le protocole et l’ID de destination. La valeur <code>from_mod</code> est fournie par le mod expéditeur et ne prouve pas quel mod a créé le message. |

Le client transmet les demandes locales de l’interface par un pont interne au processus. Le processus du jeu effectue l’envoi Steam. Sur le serveur, l’identité Steam de l’expéditeur doit appartenir à l’ensemble actuel des pairs autorisés. L’adaptateur natif construit cet ensemble à partir des événements du journal du serveur dédié concernant les pairs en ligne et l’authentification Steam. Le réglage facultatif <code>allowedClientSteamIds</code> filtre davantage cet ensemble. S’il est vide, il n’ajoute aucune restriction de liste d’autorisation. Le serveur envoie la copie de configuration et les journaux avant la connexion administrateur. Ce contrôle des pairs détermine donc aussi l’accès à ces données. Voir le [service d’interface distante](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/remote_ui.rs), le [pont local](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/local_bridge.rs) et la [liste d’autorisation Network](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/read_network_allowlist.rs).

## Autorisations requises pour le groupe serveur

Le mot de passe saisi doit correspondre exactement à un groupe dont le <code>name</code> n’est pas vide. Ce même groupe doit définir les cinq autorisations ci-dessous avec la valeur booléenne JSON <code>true</code>. Un champ absent, <code>false</code> ou une valeur d’un autre type ne permet pas d’administrer le serveur depuis le Modloader.

~~~json
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
~~~

<code>reservedSlots</code> définit les places réservées aux joueurs et ne fait pas partie de la vérification des autorisations du Modloader. ShroudForge envoie le mot de passe dans une requête JSON au pair Steam sélectionné et ne le conserve pas. Le serveur le compare directement au mot de passe de <code>enshrouded_server.json</code>. Ce code ne hache ni ne chiffre cette valeur de configuration au repos. Après validation, le serveur crée un jeton lié au pair Steam et le conserve en mémoire sur les deux terminaux. Les écritures administratives vérifient de nouveau le jeton, le pair et les autorisations du groupe. Cinq échecs déclenchent une attente de 60 secondes avant une nouvelle connexion. Voir le [code d’autorisation du serveur](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/modules/transport/remote_ui.rs).

## Note de sécurité

Si <code>serverSteamId</code> est vide, le client peut utiliser le dernier pair découvert via un message d’état sur le canal <code>65535</code>. Cette valeur est vide par défaut. Un message reçu commençant par <code>SFN1:PING:</code> met à jour cet ID découvert. **Connecté** indique un trafic d’état récent et ne vérifie pas l’identité du serveur Enshrouded. Si un pair peut établir une session de messages Steam et envoyer ce contenu d’état, il peut être sélectionné comme destination. Le chiffrement Steam protégerait alors la connexion vers ce pair, sans prouver qu’il s’agit du serveur attendu. Vérifiez ou épinglez la Steam ID du serveur avant de saisir le mot de passe. Considérez aussi les pairs abonnés comme fiables pour la configuration et les journaux reçus avant la connexion administrateur. Les mods du runtime qui reçoivent des messages <code>send_mod</code> doivent vérifier eux-mêmes le pair Steam, le mod expéditeur déclaré et le contenu. Ce sont des observations du code source, pas la confirmation d’une exploitation en direct. Consultez la [politique de sécurité de ShroudForge et les consignes de signalement privé](https://github.com/bonsaibauer/shroudforge/blob/1.8.1/SECURITY.md).
