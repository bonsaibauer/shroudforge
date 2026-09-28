# 🎮 Enshrouded Mod Loader (EML) - Documentation Directory

This guide provides step-by-step instructions for four user types: regular players installing mods, data miners extracting game assets, mod creators building custom scripts, and GPORTAL server administrators managing dedicated multiplayer servers using the **Enshrouded Mod Loader (EML)**.

Use this README to determine which guide corresponds to your specific role and setup.

---

## 📂 Guide Reference Overview

### 1. Regular Players (`User_1_Regular_Player.md`)
* **Target Audience:** Players who want to install existing mods (like translations, utilities, or content adjustments) into their local game client.
* **TL;DR:** Back up save files (`<Steam>\userdata\<your_id>\1203620\remote`). `dbghelp.dll` + `mods` folder ➔ Enshrouded dir. Extract structured mod folder to `mods`. Launch game.

---

### 2. Data Miners (`User_2_Data_Miner.md`)
* **Target Audience:** Users looking to extract game assets, dump internal data structures, and inspect configuration files or reflection trees.
* **Setup TL;DR:** Install VS Code + Lua ext. `dbghelp.dll` ➔ game dir. `emm.exe` ➔ workspace. Run `emm.exe create -g "<game_dir>"`.
* **Export TL;DR:** Write script in `src/mod.lua`. Set `"enable_console": true` & `"use_export_flag": true` in `eml.json`. Launch game to dump data.

---

### 3. Mod Creators (`User_3_Mod_Creator.md`)
* **Target Audience:** Developers building custom gameplay modifications, tweaks, or scripts using the EML framework.
* **Setup TL;DR:** Install VS Code + Lua ext. `dbghelp.dll` ➔ game dir. `emm.exe` ➔ workspace. Run `emm.exe create -g "<game_dir>"`.
* **Development TL;DR:** Write code in `src/mod.lua`. Set `"enable_console": true` in `eml.json`. Launch game to test live.

---

### 4. GPORTAL Server Administrators (`User_4_GPortal_Server_Admin.md`)
* **Target Audience:** Administrators managing dedicated multiplayer servers hosted on GPORTAL and installing mods for multiplayer sessions.
* **TL;DR:** Configure GPORTAL web settings for EML. Stop server. Connect via FileZilla FTP. Back up remote directory. Upload `dbghelp.dll` if missing & drop mods into `mods`. Restart server.

---

## 🙏 Credits & Acknowledgments

A massive thank you to **Brabb3l** for creating and maintaining the **Enshrouded Mod Loader (EML)** and the `kfc-parser` toolkit. Without their hard work reverse-engineering game assets and building the Lua execution runtime, community data mining and modding for Enshrouded wouldn't be where it is today.

* 📂 **GitHub Repository:** [Brabb3l/kfc-parser](https://github.com/Brabb3l/kfc-parser)
* 📖 **Official EML Documentation:** [brabb3l.github.io/kfc-parser](https://brabb3l.github.io/kfc-parser/eml/index.html)
