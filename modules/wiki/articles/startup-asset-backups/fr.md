# Le démarrage s’arrête à cause de sauvegardes de ressources incompatibles

Dépannage pour les anciennes versions de ShroudForge. Les versions 1.6.8, 1.7.0 et 1.7.3 peuvent interrompre le démarrage avant l’ouverture d’Enshrouded si leurs sauvegardes ne correspondent pas aux fichiers du jeu installés.

<!-- wiki-category: troubleshooting -->

## Pourquoi le démarrage s’arrête

ShroudForge conserve des copies de travail de `enshrouded.kfc` et `enshrouded.kfc_resources` afin de pouvoir restaurer les fichiers du jeu avant de préparer les mods de ressources. Ces fichiers `.bak` peuvent provenir d’une ancienne version d’Enshrouded ou d’un autre mod. Si la sauvegarde KFC ne correspond pas au fichier actuel du jeu, les anciennes versions de ShroudForge ne peuvent pas la restaurer de manière sûre et interrompent le démarrage.

## Avec ShroudForge 1.7.5 ou une version ultérieure

Lorsque la sauvegarde KFC est incompatible, ces versions peuvent déplacer les deux copies de travail vers la corbeille ShroudForge et créer de nouvelles sauvegardes à partir des fichiers actuels du jeu. Cette récupération nécessite `shroudforge/shroudforge-updater.exe` dans l’installation d’Enshrouded. Si le journal indique que le programme de mise à jour est indisponible, réparez ou réinstallez ShroudForge avec le package complet, puis relancez le jeu. Conservez les fichiers `.bak` pendant la réparation.

## Correction manuelle pour les versions 1.6.8, 1.7.0 et 1.7.3

Suivez ces étapes uniquement avec les anciennes versions de ShroudForge indiquées.

1. Fermez complètement ShroudForge et Enshrouded, y compris tout processus de serveur dédié.
2. Ouvrez le dossier d’installation d’Enshrouded. Un chemin Steam courant est `SteamLibrary\steamapps\common\Enshrouded`, mais utilisez la bibliothèque où le jeu est installé.
3. Si un autre mod a pu modifier les fichiers `.kfc` actuels, vérifiez d’abord les fichiers d’Enshrouded dans Steam. Si vous souhaitez conserver une copie des anciennes sauvegardes, copiez-les hors du dossier du jeu avant de continuer.
4. Supprimez uniquement les fichiers suivants, s’ils existent :
   - `enshrouded.kfc.bak`
   - `enshrouded.kfc_resources.bak`
5. Relancez ShroudForge. Il devrait créer de nouvelles copies de travail à partir des fichiers actuels du jeu.

Ne supprimez pas les fichiers du jeu `.kfc` ou `.kfc_resources`, ni aucun autre fichier de sauvegarde. Si le démarrage échoue encore, conservez les autres fichiers et envoyez le journal ShroudForge au support.
