# 📦 Item Exporter and English Translator

> **⚡ TL;DR:** This mod exports core Enshrouded item info and English localization text directly into clean, ready-to-use `.csv` files. It is built as a lightweight tool for wiki editors and community members to inspect and analyze game data.

> **⚠️ DEMONSTRATION & TESTING UTILITY ONLY**
> This mod is a test tool adapted from Brabb3l's `kfc-parser` examples to extract active in-memory item registries and localization collections. It directly resolves item name hash keys into human-readable English text for easy spreadsheet analysis. *For a deeper explanation, see the [Data & Asset Glossary](https://www.google.com/search?q=%23-data--asset-glossary) at the bottom of this page.*

---

## 🤖 Built with AI Assistance

This mod was created by combining human testing with **Google Gemini** and **GitHub Copilot**. If you want to customize or write your own EML scripts using AI, check out the *Optional AI Prompt Template for Modding* section in the **[EML Beginner's Guide](https://brabb3l.github.io/kfc-parser/eml/index.html)**.

---

## 📑 How It Works

This project runs via a single main script:

* **`mod.lua` (Base Exporter)**: Runs automatically on game startup, extracts the `En_Us` localization collection and all `keen::ItemInfo` assets, resolves name hashes, and exports structured CSV reports.

---

## 🎮 Part 1: How to Export Game Files

> **⚡ TL;DR:** Install EML, turn on exports in `eml.json`, run the game, and find your CSV files in `<game_dir>/shroudforge/exports/`.

### 🔹 Step 1: Install EML

To set up the loader and mod files properly, follow the complete step-by-step instructions in the **[Enshrouded Mod Loader (EML) Beginner's Guide: User 1 (Installing & Using Mods)](https://brabb3l.github.io/kfc-parser/eml/index.html)**.

### 🔹 Step 2: Turn On the Export Setting

1. Launch the game once so EML sets itself up, then close the game.
2. Open `eml.json` inside your main game folder (`<game_dir>`).
3. Change `"use_export_flag": false` ➔ `"use_export_flag": true`.
4. Change `"enable_console": false` ➔ `"enable_console": true`.
*(This opens a black window that shows your progress count so you know the game isn't stuck on the loading screen).*

### 🔹 Step 3: Run the Export

1. Start Enshrouded.
2. The Lua script will run automatically on startup, generating your CSV files in the export directory.
3. When finished, exit the game and open **`eml.json`** again.
4. **Restore settings back to `false**` so the mod doesn't re-run every single time you launch the game normally:
    * Change `"use_export_flag": true` ➔ `"use_export_flag": false`
    * Change `"enable_console": true` ➔ `"enable_console": false`


5. You will find your exported CSV files inside **`<game_dir>/shroudforge/exports/`**:
    * `item_info_export_FULL_<version>.csv`
    * `translation_enus_export_FULL_<version>.csv`



---

## 🛠️ Part 2: Customizing the Mod / Data Mining

> **⚡ TL;DR:** Want to edit how data exports? Follow the Data Miner steps in the EML Beginner's Guide to set up VS Code, then edit `mod.lua`.

> **💡 A Note on Modding Difficulty:**
> Even with modern AI tools like Gemini and GitHub Copilot, the barrier to entry for datamining remains moderate to high. AI makes writing code significantly easier, but the tricky part is understanding that not all game information is obvious at first glance. Much of the data is not kept in active memory; finding specific values requires studying the reflection file (`shroudforge/cache/types-enshrouded.json`) and tracing data relationships across files.

### Steps to Customize:

1. Follow the full setup instructions in the **[Data Miner steps in the EML Beginner's Guide](https://brabb3l.github.io/kfc-parser/eml/index.html)** to set up Visual Studio Code, Lua extensions, and `emm.exe`.
2. *(Optional)* Use your favorite AI assistant (like ChatGPT, Gemini, or Copilot) along with the *Optional AI Prompt Template for Modding* in the Beginner's Guide to help you write custom export functions.
3. Open `<game_dir>\mods\ItemTranslator\src\mod.lua` in VS Code to modify translation lookups or item properties.
4. Start Enshrouded to test your new settings.

---

## 📖 Data & Asset Glossary

Here is a simple breakdown of the terms used in this mod:

### 🔹 File Paths

* **`<game_dir>`**: The main installation folder on your computer where Enshrouded is stored (for example: `C:\Program Files (x86)\Steam\steamapps\common\Enshrouded`).

### 🔹 Active Memory vs. Disk Files

* **Active Memory (In-Memory / Runtime):** Data that the game currently has loaded while running on your screen. This is what the exporter reads.
* **Disk Files:** Raw game files stored permanently on your hard drive inside large game archives. Reading disk data requires `emm.exe` to have specific mappings built for those elements. For example, Brabb3l's [`get_translations.lua`](https://github.com/Brabb3l/kfc-parser/blob/main/examples/translations/get_translations.lua) script is a great demonstration of mapping directly to disk translation files.

### 🔹 Export Outputs

* **CSV (Comma-Separated Values):** Tabular data format generated by this mod, natively viewable in Excel, Google Sheets, or any text editor without needing a separate batch formatting script.

---

## 🙏 Credits & Acknowledgments

A massive thank you to **Brabb3l** for creating and maintaining the **Enshrouded Mod Loader (EML)** and the `kfc-parser` toolkit. Without their hard work reverse-engineering game assets and building the Lua execution runtime, community data mining and modding for Enshrouded wouldn't be where it is today.

* 📂 **GitHub Repository:** [Brabb3l/kfc-parser](https://github.com/Brabb3l/kfc-parser)
* 📖 **Official EML Documentation:** [brabb3l.github.io/kfc-parser](https://brabb3l.github.io/kfc-parser/eml/index.html)
