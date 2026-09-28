# 🎮 Enshrouded Mod Loader (EML): User 1 (Regular Player Guide)

This guide provides step-by-step instructions for regular players installing and using mods via the **Enshrouded Mod Loader (EML)**.

> 💡 **Looking for EML Mods?**
> Be sure to download the standalone **EML Ecosystem Directory** from the **Files** tab on Nexus Mods alongside this guide for a list of mods available for download.

---

⚡ **TL;DR:** Back up save files (`<Steam>\userdata\<your_id>\1203620\remote`). `dbghelp.dll` + `mods` folder ➔ Enshrouded dir. Extract structured mod folder to `mods`. Launch game.

---

### Step-by-Step Installation

1. **Back Up Your Saves First:**
   * Before installing any mods, back up your save files located at:
     `<Steam>\userdata\<your_id>\1203620\remote`

2. **Open your main Enshrouded Directory** (the folder containing `enshrouded.exe`):
   * Open **Steam** > **Library**.
   * Right-click **Enshrouded** > **Properties** > **Installed Files** > **Browse...**

3. **Download the Loader:**
   * Download the latest `dbghelp.dll` file from [EML Releases](https://github.com/Brabb3l/kfc-parser/releases).

4. **Install the Loader:**
   * Place `dbghelp.dll` directly into your main **Enshrouded Directory**.

5. **Create your Mods Folder:**
   * Create a new folder named `mods` inside your main **Enshrouded Directory**.

6. **Obtain and Unpack Your Mod:**
   * **a.** Download a mod from a trusted site (such as NexusMods).
   * **b.** If zipped (`.zip`, `.7z`, or `.rar`), extract it using a tool like [7-Zip](https://www.7-zip.org/).
   * **c.** Verify that the unpacked mod folder contains `mod.json` and a `src` folder at the top level.
   * **d.** Move that top-level mod folder into your new `mods` folder.

7. **Launch the Game:**
   * Start Enshrouded normally. The mod loader runs automatically in the background.

---

### 📌 Notes & Alternatives

* 📜 **Source:** Instructions adapted from the [Enshrouded Mod Loader Documentation](https://brabb3l.github.io/kfc-parser/eml/index.html).
* 🐧 **Linux Users:** Additional configuration is required. Refer to the official [Linux Setup Instructions](https://brabb3l.github.io/kfc-parser/eml/usage.html#linux-users).
* 🔧 **CLI Alternative:** This guide uses the recommended Proxy DLL method. To load mods via the command-line interface instead, see [Loading Mods via CLI](https://brabb3l.github.io/kfc-parser/eml/usage.html#loading-mods).

---

### 🙏 Credits & Acknowledgments

A massive thank you to **Brabb3l** for creating and maintaining the **Enshrouded Mod Loader (EML)** and the `kfc-parser` toolkit. Without their hard work reverse-engineering game assets and building the Lua execution runtime, community data mining and modding for Enshrouded wouldn't be where it is today.

* 📂 **GitHub Repository:** [Brabb3l/kfc-parser](https://github.com/Brabb3l/kfc-parser)
* 📖 **Official EML Documentation:** [brabb3l.github.io/kfc-parser](https://brabb3l.github.io/kfc-parser/eml/index.html)
