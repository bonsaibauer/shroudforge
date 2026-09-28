# 🎮 Enshrouded Mod Loader (EML): User 2 (Data Miner Guide)

This guide provides step-by-step instructions for data miners extracting game assets and dumping internal data structures using the **Enshrouded Mod Loader (EML)**.

> 💡 **Looking for API Syntaxes & Reference Dictionaries?**
> Be sure to download the standalone **EML API Syntax Dictionary** and **example export mods** from the **Files** tab on Nexus Mods alongside this guide for complete function signatures, buffer streaming methods, and quick-copy code snippets.

## 🛠️ Part 1: Setup for Data Miners & Mod Creators

⚡ **TL;DR:** Install VS Code + Lua ext. `dbghelp.dll` ➔ game dir. `emm.exe` ➔ workspace. Run `emm.exe create -g "<game_dir>"`.

### 🔹 Step 1.1: Install Visual Studio Code

1. Download and run the installer for [VS Code](https://code.visualstudio.com/).
2. Complete the setup wizard using standard default options.
3. Launch VS Code.

### 🔹 Step 1.2: Install the Lua Extension

1. Open the **Extensions tab** in VS Code (`Ctrl + Shift + X`).
2. Type `Lua` into the search bar.
3. Locate the **Lua** extension (by *sumneko*) and click **Install**.

### 🔹 Step 1.3: Set Up a Workspace (Best Practice)

1. Create a new folder named `EnshroudedMods` anywhere on your computer.
2. In VS Code, click **File** > **Open Folder...** and select `EnshroudedMods`.
3. Click **File** > **Save Workspace As...**
4. Click **Save** with the default name `EnshroudedMods.code-workspace`.

### 🔹 Step 1.4: Download EML and Install the Loader

1. Download `emm.exe` and `dbghelp.dll` from [EML Releases](https://github.com/Brabb3l/kfc-parser/releases).
2. Place `emm.exe` inside your `EnshroudedMods` workspace folder.
3. Place `dbghelp.dll` directly into your main **Enshrouded Directory**.
4. Launch Enshrouded once so EML can generate its cache files, then close the game.

### 🔹 Step 1.5: Create a New Mod

1. In VS Code, select **Terminal** > **New Terminal** from the top menu.
2. Navigate to your workspace directory containing `emm.exe` using `cd` (wrap paths in quotes):
   ```cmd
   cd "C:\Users\YourName\Workspace\EnshroudedMods"
   ```

3. Run the mod creation command with your Enshrouded installation path:
    ```cmd
    emm.exe create -g "C:\Program Files (x86)\Steam\steamapps\common\Enshrouded"
    ```


4. Complete the interactive CLI form (use the **Spacebar** to toggle options under "Select mod capabilities").

#### 📋 Example Terminal Output:

   ```cmd
   ✔ ID · myMod
   ✔ Name · myMod
   ✔ Version · 0.0.1
   ✔ Author · your_name
   ✔ Select mod capabilities · Export
   INFO eml::lua: Type registry loaded successfully
   INFO eml::lua: Lua type definition file has been generated path="...shroudforge\cache\lua\types.lua"
   INFO eml::lua: Lua base definition file has been generated path="...shroudforge\cache\lua\base.lua"
   info: Mod has been created at C:\Program Files (x86)\Steam\steamapps\common\Enshrouded\mods\myMod

   ```

### 🔹 Step 1.6: Add the Mod Folder to Your Workspace

1. Open File Explorer and navigate to `<game directory>\mods`.
2. Drag and drop your newly generated mod folder into the VS Code workspace sidebar (`Ctrl + Shift + E`).
3. Click **Add Folder to Workspace** if prompted by VS Code.
4. Select **Yes, I trust the authors** if prompted by VS Code.

---

### 📖 Part 1 Reference

For comprehensive CLI mod creation options, see the official [EML Mod Setup Documentation](https://brabb3l.github.io/kfc-parser/eml/develop/setup.html#setup).


---

## ⛏️ Part 2: User 2 / Data Miner (Exporting Game Data)

⚡ **TL;DR:** Write script in `src/mod.lua`. Set `"enable_console": true` & `"use_export_flag": true` in `eml.json`. Launch game to dump data.

### 🔹 Step 2.1. Prepare Your Environment

Complete the **Setup for Data Miners & Mod Creators** section above.


### 🔹 Step 2.2. Open Your Workspace

In VS Code, select **File** > **Open Workspace from File...** and choose `EnshroudedMods.code-workspace`.


### 🔹 Step 2.3. Open Your Script File

Open `<game dir>/mods/<YourMod>/src/mod.lua` in VS Code to edit your code.


### 🔹 Step 2.4. Study Code Examples & Reference Documents

* 📄 **Resource Names:** Extract active, in-memory resource types (~77k+ entries) and names for function calls:

   ```lua
   local resource_types = game.assets.get_resource_types()

   for _, resource_type in ipairs(resource_types) do
      local ok, name = pcall(function() return resource_type.qualified_name or resource_type.name end)
      local qname = (ok and name) and tostring(name) or "Unknown"

      print(string.format("%s -> %s", tostring(resource_type), qname))
   end

   ```

   *💡 **Memory Limitation:** This script only reads objects currently loaded in RAM. For complete, raw asset definitions or off-memory items, cross-reference the reflection file at `<game dir>/shroudforge/cache/types-enshrouded.json` or use the buffer streaming pipeline detailed in the **EML API Syntax Dictionary**.*

* 📄 **Item Lists:** See [`kfc-parser/examples/basic/export_item_list.lua`](https://github.com/Brabb3l/kfc-parser) for item table extraction. An updated version of this script is available at **Item Exporter and English Translator** from the **Files** tab on Nexus Mods.

* 📄 **Translations (en-us):** See [`kfc-parser/examples/translations/get_translations.lua`](https://github.com/Brabb3l/kfc-parser) for extracting localized strings. An updated version of this script is available at **Item Exporter and English Translator** from the **Files** tab on Nexus Mods.

* 📖 **Other Snippets:** See [Accessing Resources](https://brabb3l.github.io/kfc-parser/eml/develop/getting_started.html#accessing-resources) and [Exporting Data](https://brabb3l.github.io/kfc-parser/eml/develop/getting_started.html#exporting-data). Also see other example scripts on the **Files** tab on Nexus Mods. 


### 🔹 Step 2.5. Configuration Flags

1. Open `<game dir>\eml.json` in VS Code.
2. Set `"enable_console": true` (enables live console logging).
3. Set `"use_export_flag": true` (outputs extracted data to `<game dir>\export`).


### 🔹 Step 2.6. Run & Extract

Launch Enshrouded to execute your script and output data files.

---

## 📌 Developer References & Constraints

* 📖 **Developer Guide:** For API definitions and hooks, see the [EML Developer Guide](https://brabb3l.github.io/kfc-parser/eml/index.html) and consult the downloaded **EML API Syntax Dictionary**.
* 📂 **EML Function Definitions:** Generated locally at `<game directory>/shroudforge/cache/lua/base.lua` or available on [GitHub](https://github.com/Brabb3l/kfc-parser) under `crates/mod-loader-lua/definitions`.
* ⚠️ **Sandbox Environment:** EML mods run in a restricted sandbox without direct external file access. Place **all** logic and helper functions directly inside `mod.lua` to avoid having to script complex loading workarounds.
* 💾 **Asset Memory Model:** Direct `game.assets` queries only inspect objects currently loaded in RAM. Unloaded or off-memory assets require resolving their GUID and streaming binary buffers via `game.assets.get_content()`.

---

### 🤖 Optional AI Prompt Template for Modding (`mod.lua`)

This prompt template may be used when generating Lua code with Copilot, Gemini, or ChatGPT:

```text
I am creating a sandboxed Lua mod for Enshrouded using the Enshrouded Mod Loader (EML).
All code MUST be fully self-contained in a single mod.lua file because external file calls are restricted.

Goal: [Describe what you want the mod to do here]

Framework Definitions & Context:
- EML function and type definitions are defined in my local file at `<game_dir>/shroudforge/cache/lua/base.lua` or online at:
  [https://github.com/Brabb3l/kfc-parser/tree/main/crates/mod-loader-lua/definitions](https://github.com/Brabb3l/kfc-parser/tree/main/crates/mod-loader-lua/definitions)
- Relevant API Dictionary / Snippets: Reference the EML API Syntax Dictionary for buffer streaming and type resolution signatures.

Requirements:
1. Syntax & Types: Use strictly valid EML API functions and types as defined in the provided EML definitions. Do not invent non-existent APIs.
2. Self-Contained: Keep all logic and helper functions strictly inside mod.lua (no external file imports or restricted system I/O).
3. Accessing Off-Memory / Disk Assets:
   - For active RAM objects, use direct asset lookup (e.g., game.assets.get_resources_by_type).
   - For off-memory/disk assets (like localization tables, raw data, or unloaded assets), use EML's buffer streaming pipeline:
     a. Extract content_hash / GUID from parent resource.
     b. Resolve GUID via game.guid.from_content_hash(hash).
     c. Read raw buffer via game.assets.get_content(guid):read_data().
     d. Unpack data using buf:read_resource("ResourceTypeName").
4. Console Logging: Include print statements so execution can be verified in real time when enable_console is true.
5. (Optional) File Export: Format and output the extracted data to disk using EML's export system so it saves to the /export directory when use_export_flag is enabled.

```

---

### 🧰 Optional Tools & Resources

* 🔌 **Lua (by Tencent):** VS Code extension providing syntax highlighting, autocomplete, and static code checks for Lua scripts.
* 📦 **json (by ZainChen):** VS Code extension that adds a side panel for inspecting JSON tree hierarchies—useful for analyzing raw exports.
* 🤖 **GitHub Copilot Chat:** AI assistant integrated into VS Code (**View** > **Appearance** > **Secondary Side Bar**). Includes a free usage tier with direct access to Claude models—allowing you to edit your workspace files without leaving the editor. *(Note: Save your workspace to keep chat sessions persistent).*
* 🧠 **Gemini (gemini.google.com):** Gemini Flash / Thinking models are effective for writing complex Lua logic or debugging EML scripts.

---

## 🙏 Credits & Acknowledgments

A massive thank you to **Brabb3l** for creating and maintaining the **Enshrouded Mod Loader (EML)** and the `kfc-parser` toolkit. Without their hard work reverse-engineering game assets and building the Lua execution runtime, community data mining and modding for Enshrouded wouldn't be where it is today.

* 📂 **GitHub Repository:** [Brabb3l/kfc-parser](https://github.com/Brabb3l/kfc-parser)
* 📖 **Official EML Documentation:** [brabb3l.github.io/kfc-parser](https://brabb3l.github.io/kfc-parser/eml/index.html)
