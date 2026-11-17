# Norme de chargement DInput8 chain

De nombreux chargeurs de mods Enshrouded et mods avec DLL veulent utiliser le nom `dinput8.dll` du jeu, mais un seul fichier peut porter ce nom. Le chargeur qui le détient lance les autres DLL indiquées dans `dinput8-chain.ini`, à côté de l'exécutable du jeu. EMF, Kiln, ShroudForge, XHL-Core et Auto Loot Plus lisent ce fichier. Cette page fixe les règles pour que tous les chargeurs le lisent de la même façon et que la liste d'un joueur fonctionne quel que soit le chargeur qui détient `dinput8.dll`.

Cette page est dans le domaine public sous CC0. Vous pouvez la copier dans la documentation de votre projet et la modifier sans mention obligatoire. Les modifications approuvées par plusieurs chargeurs sont les bienvenues sous forme de pull request dans le dépôt d'EMF.

Les termes « doit » et « devrait » décrivent ci-dessous le comportement d'un chargeur conforme à cette spécification.

## Le fichier

```ini
; my mods
[Chain]
mods\mymod\mymod.dll
autolootplus.dll

[EMF]
creative.dll
```

- Emplacement : `dinput8-chain.ini` dans le dossier de l'exécutable du jeu (`enshrouded.exe` ou `enshrouded_server.exe`).
- Encodage : un chargeur doit lire l'UTF-8 avec ou sans BOM, ainsi que l'UTF-16 LE avec BOM. Il devrait aussi lire la page de codes ANSI du système lorsque les octets ne forment pas un UTF-8 valide, par exemple pour un fichier enregistré par un ancien Bloc-notes. Un outil qui écrit le fichier doit conserver l'encodage et les fins de ligne détectés.
- Lignes : les lignes vides sont ignorées. Une ligne commençant par `;`, `#` ou `//` est un commentaire. `; ` devant une ligne de DLL désactive cette DLL afin que le joueur puisse la désactiver sans supprimer la ligne. Un commentaire peut suivre le nom après une espace ou une tabulation (`mymod.dll ; my mod`), et un nom peut être placé entre guillemets doubles.
- Sections, sans distinction de casse : `[Chain]` est partagée. Tout chargeur conforme à cette spécification démarre les DLL qu'elle contient. Toute autre section appartient à un outil et porte son nom, par exemple `[EMF]` ou `[Kiln]`. Les autres chargeurs doivent l'ignorer. Les lignes avant toute section appartiennent à `[Chain]`. Les anciens fichiers peuvent ne comporter aucune section.
- Lignes `key=value` : une ligne `name=value` compte comme entrée DLL si sa valeur désigne un fichier `.dll`. Les autres clés sont des paramètres de l'outil qui les a écrites. Les autres chargeurs les ignorent.

## Les DLL

- Les chemins sont relatifs au dossier de l'exécutable. `/` est lu comme `\`. Le chargeur doit refuser les chemins absolus, les lettres de lecteur, les chemins UNC (`\\server\...`) et toute partie `..`. Un fichier de chaîne ne doit pas permettre de sortir du dossier du jeu.
- Seuls les fichiers `.dll` sont acceptés. Toute ligne désignant un autre fichier est ignorée et signalée.
- Ne jamais charger `dinput8.dll` elle-même. Un chargeur ne doit pas non plus démarrer une seconde copie de lui-même. EMF reconnaît par exemple son propre chargeur sous un autre nom et ne démarre pas cette copie.
- Chaque DLL n'est démarrée qu'une fois. Si une DLL apparaît plusieurs fois, dans `[Chain]` et dans la section propre d'un outil ou deux fois dans la même section, elle démarre uniquement à sa première position.
- Ordre : les DLL démarrent dans l'ordre de leurs lignes, de haut en bas, avec `[Chain]` et la section propre du chargeur dans l'ordre où elles apparaissent dans le fichier.
- Moment : le chargeur utilise `LoadLibrary` depuis sa propre `DllMain` ou juste après, avant l'exécution du code du jeu. C'est le moment où une DLL chaînée aurait démarré si elle avait été elle-même `dinput8.dll`.
- Au plus 256 entrées sont prises en compte. Les suivantes sont signalées et ignorées.
- Échecs : une DLL absente ou impossible à charger est ignorée et signalée. Cela inclut l'erreur Windows 126 et l'erreur 1114 lorsque sa `DllMain` échoue. Le jeu continue.

## Rapports

Un chargeur devrait consigner une ligne par DLL chaînée : démarrée, ignorée parce qu'elle est désactivée, refusée ou déjà chargée, ou en échec avec le numéro d'erreur Windows. EMF écrit aussi les mêmes informations sous forme de tableau dans `emf\chain_status.txt`, afin que les joueurs puissent les transmettre.

## Déplacer le dinput8.dll d'un autre outil

Un programme d'installation qui trouve le `dinput8.dll` d'un autre outil devrait le renommer, jamais le supprimer, inscrire son nouveau nom dans sa propre section de `dinput8-chain.ini` et le restaurer lors de la désinstallation. EMF utilise `<tool>.dll` lorsqu'il reconnaît l'outil. Sinon, il utilise les informations de version de la DLL ou le nom `dinput8-previous.dll`, et consigne le fichier déplacé dans `emf\moved.txt`.

## Exports

Le jeu appelle `DirectInput8Create` sur le chargeur qui détient `dinput8.dll`. Celui-ci doit transférer l'appel au `dinput8.dll` natif de Windows (`%SystemRoot%\System32\dinput8.dll`). Les DLL chaînées ne reçoivent pas les appels DirectInput. Un mod qui effectue son travail dans `DirectInput8Create` doit le déplacer dans `DllMain` ou dans un thread qu'il démarre.

## Qui suit cette spécification

| Outil | Lit `[Chain]` | Section propre |
| --- | --- | --- |
| EMF | Oui, y compris les lignes avant toute section | `[EMF]`, pour les DLL renommées par son programme d'installation |
| ShroudForge | Oui. Chaque entrée doit se trouver dans `mods\<folder>\`. Les autres sont signalées. | |
| Kiln | Oui, y compris `[EMF]` pour les DLL renommées par les anciens programmes d'installation d'EMF | Aucune. Sa propre liste se trouve dans `kiln\chain.ini`. Les noms entre guillemets et les commentaires après un nom sont prévus. |
| XHL-Core, Auto Loot Plus | Oui | |

Signalez-nous si votre outil lit le fichier ou l'interprète différemment afin que nous mettions ce tableau à jour.

## Source

[Spécification commune d'EMF pour dinput8-chain.ini](https://github.com/matenm/emf-enshrouded/blob/main/docs/modding/dinput8-chain.md). La spécification originale est dédiée au domaine public sous CC0.
