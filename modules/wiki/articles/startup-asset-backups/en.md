# Startup stops because asset backups are incompatible

Troubleshooting for older ShroudForge releases. Versions 1.6.8, 1.7.0 and 1.7.3 can stop before Enshrouded starts when their asset backup files do not match the installed game files.

<!-- wiki-category: troubleshooting -->

## Why startup stops

ShroudForge keeps working backups of `enshrouded.kfc` and `enshrouded.kfc_resources` so it can restore the game files before preparing asset mods. These `.bak` files may come from an older Enshrouded version or another mod. If the KFC backup does not match the current game file, older ShroudForge releases cannot safely restore it and stop startup.

## If you use ShroudForge 1.7.5 or later

When the KFC backup is incompatible, these versions can move both working backups to the ShroudForge recycle bin and capture new backups from the current game files. This recovery requires `shroudforge/shroudforge-updater.exe` in the Enshrouded installation. If the log says the updater is unavailable, repair or reinstall ShroudForge with the complete release package, then start the game again. Keep the `.bak` files in place while repairing the installation.

## Manual fix for versions 1.6.8, 1.7.0 and 1.7.3

Use these steps only with the listed older ShroudForge versions.

1. Close ShroudForge and Enshrouded completely, including any dedicated server process.
2. Open the Enshrouded installation folder. A common Steam path is `SteamLibrary\steamapps\common\Enshrouded`, but use the library folder where your game is installed.
3. If another mod may have changed the current `.kfc` files, verify Enshrouded's files through Steam first. If you want to keep a copy of the old backups, copy them outside the game folder before continuing.
4. Delete only these files if they are present:
   - `enshrouded.kfc.bak`
   - `enshrouded.kfc_resources.bak`
5. Restart ShroudForge. It should create fresh working backups from the current game files.

Do not delete the `.kfc` or `.kfc_resources` game files, or any other backup files. If startup still fails, keep the remaining files and share the ShroudForge log with support.
