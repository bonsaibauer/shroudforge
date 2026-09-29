# 📦 KFC Parser Mimic (KPMimic) & Asset Formatter

> **⚡ TL;DR:** This mod exports the text and data currently running inside Enshrouded into easy-to-read `.json` files. It is built as a test tool for wiki editors and community members to inspect game information.

> **⚠️ DEMONSTRATION & TESTING UTILITY ONLY**
> This mod is a test tool to see if the game's active memory matches the files extracted by the original **KFC Parser**.
> `KPMimic` only dumps obvious in-memory data chunks without any custom data mapping. In contrast, **KFC Parser** goes deeper by connecting reflection files (found in `<game_dir>/shroudforge/cache/types-enshrouded.json`) and reading raw disk data, though even KFC Parser only extracts objects that have been specifically mapped out. *For a deeper explanation, see the [Data & Asset Glossary](https://www.google.com/search?q=%23-data--asset-glossary) at the bottom of this page.*

---

## 🤖 Built with AI Assistance

This mod was created by combining human testing with **Google Gemini** and **GitHub Copilot**. If you want to customize or write your own EML scripts using AI, check out the *Optional AI Prompt Template for Modding* section in the **[EML Beginner's Guide](https://brabb3l.github.io/kfc-parser/eml/index.html)**.

---

## 📑 How It Works

This project uses two main files:

1. **`mod.lua` (KPMimic)**: Runs inside the game, generates a resource type summary CSV, and exports loaded memory assets into raw JSON files.
2. **`batch_format_assets.py` (Asset Formatter)**: An optional Python helper script that cleans up the raw text files—making them neat, organized, and easy to read in a text editor.

---

## 🎮 Part 1: How to Export Game Files

> **⚡ TL;DR:** Install EML, turn on exports in `eml.json`, run the game, and find your files in `<game_dir>/shroudforge/exports/`.

### 🔹 Step 1: Install EML

To set up the loader and mod files properly, follow the complete step-by-step instructions in the **[Enshrouded Mod Loader (EML) Beginner's Guide: User 1 (Installing & Using Mods)](https://brabb3l.github.io/kfc-parser/eml/index.html)**.

### 🔹 Step 2: Turn On the Export Setting
1. Launch the game once so EML sets itself up, then close the game.
2. Open `eml.json` inside your main game folder (`<game_dir>`).
3. Change `"use_export_flag": false` ➔ `"use_export_flag": true`.
4. Change `"enable_console": false` ➔ `"enable_console": true`.
   *(This opens a black window that shows your progress count, like `5/131`, so you know the game isn't stuck on the loading screen).*

### 🔹 Step 3: Run the Export & Clean Up
1. Start Enshrouded.
2. The Lua script will run automatically on startup. The very first thing it does is create a summary CSV file named **`0_resource_types_export_*.csv`** in your export directory, followed by the rest of the in-memory asset files.
3. When finished, exit the game and open **`eml.json`** again.
4. **Restore settings back to `false`** so the mod doesn't re-run every single time you launch the game normally:
   * Change `"use_export_flag": true` ➔ `"use_export_flag": false`
   * Change `"enable_console": true` ➔ `"enable_console": false`
5. You will find your exported files inside **`<game_dir>/shroudforge/exports/exported_assets_[game_version]/`**.

---

## 🐍 Part 2: (Optional) Make Files Easy to Read with Python

> **⚡ TL;DR:** Game exports can be squished onto a single line. This Python script uses all your computer's processor cores to quickly clean them up so they are easy to read.

When the Lua script exports game files, they can be hard to read because all the information is squished onto a single line.

Making the files look neat inside the game engine takes a long time because Lua can only use **one CPU core**. The Python script uses **all your CPU cores at once** to finish the job in seconds.

---

### 🔹 Simple Step-by-Step Python Instructions

#### 1. Download & Install Python

* Download Python from [python.org](https://www.google.com/search?q=https://www.python.org/downloads/).
* **Important:** During installation, check the box that says **"Add python.exe to PATH"**.

#### 2. Move the Script

* Copy `batch_format_assets.py` into your export folder:
`<game_dir>/shroudforge/exports/`

#### 3. Change Settings (Optional)

* Open `batch_format_assets.py` in any text editor.
* Near the top, look for:
```python
FORCE_REPROCESS = False

```


* Leave it as `False` to skip files you have already formatted. Change it to `True` if you want to clean up every file again.

#### 4. Run the Script

1. Open **Command Prompt** (`Win + R`, type `cmd`, press Enter).
2. Type `cd` followed by the path to your export folder (example below):
```cmd
cd "C:\Program Files (x86)\Steam\steamapps\common\Enshrouded\export"

```


3. Run the script:
```cmd
python batch_format_assets.py

```


*(If Command Prompt says `python` is not recognized, paste the full path to your Python program followed by the script path, for example:)*
```cmd
"C:\Users\YourName\AppData\Local\Programs\Python\Python314\python.exe" "C:\Program Files (x86)\Steam\steamapps\common\Enshrouded\export\batch_format_assets.py"

```



---

### 🔍 What the Python Script Does (Details)

If you are curious about what happens behind the scenes when you run `batch_format_assets.py`:

* **Restores Real Names:** Uses `0_resource_types_export_*.csv` (created in Part 1) to map short folder names back to full game categories (like `keen::ItemTemplate`).
* **Fixes Messy Text:** Unfolds squished single-line text blocks into normal, easy-to-read paragraphs / lines.
* **Fixes List Formatting:** Fixes broken list layouts so every item is clear.
* **Adds Headers:** Puts `$type`, `$guid`, and `$part` labels at the top of every file.
* **Neat Layout:** Adds clear spacing so you can open any file in Visual Studio Code or Notepad and read it right away.

---

## 🛠️ Part 3: Customizing the Mod / Data Mining

> **⚡ TL;DR:** Want to edit how data exports? Follow the Data Miner steps in the EML Beginner's Guide to set up VS Code, then edit `mod.lua`.

> **💡 A Note on Modding Difficulty:**
> Even with modern AI tools like Gemini and GitHub Copilot, the barrier to entry for datamining remains moderate to high. AI makes writing code significantly easier, but the tricky part is understanding that not all game information is obvious at first glance. Much of the data is not kept in active memory; finding specific values requires studying the reflection file (`shroudforge/cache/types-enshrouded.json`) and tracing data relationships across files.

### Steps to Customize:

1. Follow the full setup instructions in the **[Data Miner steps in the EML Beginner's Guide](https://brabb3l.github.io/kfc-parser/eml/index.html)** to set up Visual Studio Code, Lua extensions, and `emm.exe`.
2. *(Optional)* Use your favorite AI assistant (like ChatGPT, Gemini, or Copilot) along with the *Optional AI Prompt Template for Modding* in the Beginner's Guide to help you write custom export functions.
3. Open `<game_dir>\mods\KPMimic\src\mod.lua` in VS Code and edit settings at the top:
```lua
-- ==========================================
-- SCRIPT CONFIGURATION
-- ==========================================
local OVERWRITE_EXISTING = false  -- Set to true to overwrite old exports
local MAX_DEREF_DEPTH    = 8      -- How deep to unpack nested data folders

```


4. Start Enshrouded to test your new settings.

---

## 📖 Data & Asset Glossary

Here is a simple breakdown of the terms used in this mod:

### 🔹 File Paths

* **`<game_dir>`**: The main installation folder on your computer where Enshrouded is stored (for example: `C:\Program Files (x86)\Steam\steamapps\common\Enshrouded`).

### 🔹 Active Memory vs. Disk Files

* **Active Memory (In-Memory / Runtime):** Data that the game currently has loaded while running on your screen. This is what `KPMimic` reads.
* **Disk Files:** Raw game files stored permanently on your hard drive inside large game archives. Reading disk data requires `emm.exe` to have specific mappings built for those elements. For example, Brabb3l's [`get_translations.lua`](https://www.google.com/search?q=https://github.com/Brabb3l/kfc-parser/blob/main/examples/translations/get_translations.lua) script is a great demonstration of mapping directly to disk translation files.

### 🔹 File Names & Labels

Every game file exported by this tool uses three key pieces of information:

* **GUID (Global Unique Identifier):** A unique string of letters and numbers that acts as an ID card for an item.
* *Example:* `72980442-0433-4f5f-a68a-b46601d560dd`


* **$type:** Tells the game what kind of item or data this is.
* *Example:* `keen::category::type` or `keen::ItemTemplate`


* **$part:** The section number of the data file (usually `0`).

**How Files Are Named:**

`[GUID]_[Type Hash]_[Part]`

*Example:* `72980442-0433-4f5f-a68a-b46601d560dd_0eef4cc4_0`

---

## 🙏 Credits & Acknowledgments

A massive thank you to **Brabb3l** for creating and maintaining the **Enshrouded Mod Loader (EML)** and the `kfc-parser` toolkit. Without their hard work reverse-engineering game assets and building the Lua execution runtime, community data mining and modding for Enshrouded wouldn't be where it is today.

* 📂 **GitHub Repository:** [Brabb3l/kfc-parser](https://github.com/Brabb3l/kfc-parser)
* 📖 **Official EML Documentation:** [brabb3l.github.io/kfc-parser](https://brabb3l.github.io/kfc-parser/eml/index.html)
