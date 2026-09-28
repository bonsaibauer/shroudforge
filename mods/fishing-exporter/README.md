# 📦 Fishing Data Exporter

> **⚡ TL;DR:** This mod extracts core Enshrouded fishing spawn tables, rod attributes, bait details, and English localization text directly into clean, ready-to-use `.csv` files. It is built as a lightweight tool for wiki editors and community members to inspect and analyze game data.

> **⚠️ DEMONSTRATION & TESTING UTILITY ONLY**
> This mod is a test tool adapted from Brabb3l's `kfc-parser` examples to extract active in-memory fishing registries and localization collections. It directly resolves item name hash keys into human-readable English text for easy spreadsheet analysis. *For a deeper explanation, see the [Data & Asset Glossary](https://www.google.com/search?q=%23-data--asset-glossary) at the bottom of this page.*

---

## 🚨 HELP WANTED: Known Limitations & Gaps

> **HELP WANTED**
> If you have experience navigating Enshrouded's asset schemas or graph structures, contributions, pull requests, and insights are very welcome to help resolve the data gaps listed below!

### 1. `ConnectedBiome` Resolution Gap (`1_fish_spawn_tables_final_*`)

* **The Issue:** An attempt was made to programmatically link the `TemplateResource` to the correct `Biome` using the game's reflection data, but the direct mapping failed.
* **Current Behavior:** Because of this limitation, the `ConnectedBiome` column in `1_fish_spawn_tables_final_1076226_run_0` currently returns as all `"Unlinked / Dynamic"`.
* **The Workaround:** As a temporary fallback, a string-parsing helper (`infer_test_biome_from_name`) was implemented to guess potential biomes based on words inside the template names. Because template names are developer-defined strings rather than strict data connections, this approach is less reliable.

### 2. `FishBooster` Column in Bait Data (`3_fishing_bait_and_trash_final_*`)

* **Note on Behavior:** In `3_fishing_bait_and_trash_final_1076226_run_0`, the `FishBooster` column returns blank.
* **Clarification:** This is **not a bug**. Currently, there is no active data for `FishBooster` associated with bait types within the game files. However, this column has been intentionally included in anticipation that Keen may implement this mechanic in future game updates.

---

## 🤖 Built with AI Assistance

This mod was created with human testing and AI assistance. The script can be edited with any text editor; LuaLS uses the development configuration in `.luarc.json` for API completion.

---

## 📑 How It Works

This project runs via a single main script:

* **`mod.lua` (Fishing Exporter)**: Runs automatically on game startup, extracts localization collections, fishing spot spawn tables, rod configurations, bait metrics, and exports structured CSV reports.

---

## 🎮 Part 1: How to Export Game Files

> **⚡ TL;DR:** Enable **Mod exports** in ShroudForge settings, start the game, and find the CSV files in `<game_dir>/shroudforge/exports/` (the default export directory).

### 🔹 Step 1: Install ShroudForge

Install ShroudForge and place this mod in the game's `mods` directory.

### 🔹 Step 2: Allow Mod Exports

In the ShroudForge Modloader open **Settings → General → Mod exports** and enable **Allow mods to export files**. The mod declares the `export` capability in `mod.json`; ShroudForge only exposes `loader.features.export` and `io.export` when both that capability and this global setting are enabled.

### 🔹 Step 3: Run the Export

1. Start Enshrouded. The enabled exporter runs during startup and writes its CSV files to the configured export directory.
2. With the default directory, find them in **`<game_dir>/shroudforge/exports/`**:
    * `1_fish_spawn_tables_final_<version>_<timestamp>.csv`
    * `2_fishing_rods_final_<version>_<timestamp>.csv`
    * `3_fishing_bait_and_trash_final_<version>_<timestamp>.csv`



---

## 🛠️ Part 2: Customizing the Mod / Data Mining

> **⚡ TL;DR:** To change what gets exported, edit `src/mod.lua` and use LuaLS completion from `.luarc.json`.

> **💡 A Note on Modding Difficulty:**
> Even with modern AI tools like Gemini and GitHub Copilot, the barrier to entry for datamining remains moderate to high. AI makes writing code significantly easier, but the tricky part is understanding that not all game information is obvious at first glance. Much of the data is not kept in active memory; finding specific values requires studying the reflection file (`shroudforge/cache/types-enshrouded.json`) and tracing data relationships across files.

### Steps to Customize:

1. Open this mod's `src/mod.lua` in VS Code with the Lua Language Server extension. The included `.luarc.json` points LuaLS at the ShroudForge API definitions in this repository and at generated game definitions when available.
2. Edit the export logic, then start Enshrouded to run the mod.

---

## 📖 Data & Asset Glossary

Here is a simple breakdown of the terms used in this mod:

### 🔹 File Paths

* **`<game_dir>`**: The main installation folder on your computer where Enshrouded is stored (for example: `C:\Program Files (x86)\Steam\steamapps\common\Enshrouded`).

### 🔹 Active Memory vs. Disk Files

* **Active Memory (In-Memory / Runtime):** Data that the game currently has loaded while running on your screen. This is what the exporter reads.
* **Disk Files:** Raw game files stored permanently on your hard drive inside large game archives. This mod reads resources exposed to Lua by the loader; it does not parse the archives directly. Archive inspection requires separate parser tooling and game-format mappings.

### 🔹 Export Outputs

* **CSV (Comma-Separated Values):** Tabular data format generated by this mod, natively viewable in Excel, Google Sheets, or any text editor without needing a separate batch formatting script.

---

## 🙏 Credits & Acknowledgments

A massive thank you to **Brabb3l** for creating and maintaining the **Enshrouded Mod Loader (EML)** and the `kfc-parser` toolkit. Without their hard work reverse-engineering game assets and building the Lua execution runtime, community data mining and modding for Enshrouded wouldn't be where it is today.

* 📂 **GitHub Repository:** [Brabb3l/kfc-parser](https://github.com/Brabb3l/kfc-parser)
* 📖 **Official EML Documentation:** [brabb3l.github.io/kfc-parser](https://brabb3l.github.io/kfc-parser/eml/index.html)
