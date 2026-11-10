# Hello Ember: Modloader settings demo

Welcome! This is the English example mod in the repository. It writes messages to the ShroudForge log and demonstrates every Modloader setting control, the fixed mod-page tabs, custom setting tabs, and settings-related features. It does not change the game world.

You do not need to understand every line before you begin. First, follow the [first-mod lesson](https://bonsaibauer.github.io/shroudforge/en/#first). Then come back here to explore the example.

## What is in this folder?

~~~text
mod.json                 The mod's name, version, author, license, and permissions
extended.mod.json        The settings, tabs, and buttons shown in the Modloader
locales/*.json           Package translations for Modloader languages
icon.svg          The small picture shown with the mod
src/mod.lua              The instructions the mod runs
~~~

Think of **mod.json** as the information card for the mod. **extended.mod.json** adds ShroudForge settings and buttons. Files under **locales/** translate those labels and provide the same mod-specific text to Lua. The Lua file contains the behavior. Hello Ember includes German, Spanish, French, Hindi, Italian, Japanese, Dutch, Polish, Swedish, and Simplified Chinese catalogs. English uses the source text. Austrian and Swiss German use the German catalog through the Modloader's regional-locale fallback.

## Try the visual example

1. Copy this folder into the game's **mods** folder.
2. Start Enshrouded and press **F9** to open the Modloader.
3. Find **Hello Ember** and switch it on.
4. Review **Information**, **Dependencies**, and **Settings**, then open each custom tab.
5. In **Search and Categories**, search for `stone`, enable **Only switched on**, then search for `no-such-choice` to see the empty-result state.
6. In **Preset Profiles**, choose a profile, then edit a member setting to select **Custom**.
7. Use the actions under **Actions and Confirmation**, then press **F10** to see their log messages.

Because the basic settings are ungrouped, **Settings** is the initial tab and displays them together without categories. The first three page tabs are fixed. The remaining tabs come from groups marked `"tab": true`.

**Information** displays all six metadata badge slots used by the page: SF origin, version, target, asset, runtime, and MIT license. It also displays every registered link badge type: Project page, GitHub, GitLab, Codeberg, Issues, Wiki, Website, Store, Buy Me a Coffee, Patreon, PayPal, GitHub Sponsors, Ko-fi, Open Collective, and Other support. The ShroudForge website, wiki, GitHub source and issue links, Buy Me a Coffee, and Patreon point to their project destinations. GitLab, Codeberg, Store, PayPal, GitHub Sponsors, Ko-fi, and Open Collective point to provider landing pages. Project page and Other support both point to the ShroudForge website. Replace those example URLs before reusing the manifest. **Dependencies** shows the optional `sf-survival` package and a deliberately missing optional example dependency. This displays both an installed dependency and an optional missing dependency at once.

**Search and Categories** contains three multi-select lists. The two long lists have more than twelve choices, so they show search and the switched-on filter. One also shows category tabs for Materials, Tools, and Supplies, and search spans those categories. The short list has exactly twelve choices, so its category tabs remain visible while search controls are hidden.

The keybind tab includes keyboard and mouse reservation padlocks. The example does not report its `visualDemo` context as active, so it does not reserve keys or the mouse wheel from the game. Its buttons only write to the mod log.

The optional `launcher` value in **extended.mod.json** records the mod's origin: use `"EML"` to keep an EML badge after adding ShroudForge settings, and omit it or use `"SF"` for a ShroudForge mod. This metadata does not convert or rewrite **mod.json**, and it does not determine which runtime APIs the mod uses.

## Make it yours

Before sharing your own copy, update these values in **mod.json**:

- **id:** Give your mod a unique folder-friendly name, such as **yourname.hello-ember**. Use letters, numbers, dots, underscores, or hyphens.
- **name:** Choose the name players will see.
- **version:** Start with **1.0.0**. Use three numbers separated by dots.
- **authors:** Add the names you want players to see.
- **description:** Say in one sentence what your mod does.

In **extended.mod.json**, you can change settings, tabs, the changelog, and button text. An action's **id** must match the name registered in **src/mod.lua**. Add a file such as **locales/de.json** to translate the name, settings, options, groups, actions, changelog, and Lua strings without changing setting keys or saved values. The [mod locale guide](https://bonsaibauer.github.io/shroudforge/en/#doc-modding-locales) explains the schema and fallback behavior.
### How the setting reaches the Lua code

The `settings` key in **extended.mod.json** is the name your Lua code asks for. In this example the key is `greeting`. the player-facing label can say “Greeting” because Lua uses the key, not the label.

```lua
local function setting(name, fallback)
    return shroudforge.settings.get(name, fallback)
end

local function log_greeting()
    local greeting = tostring(setting("greeting", "Hello from the ShroudForge runtime!"))
    shroudforge.log.info(greeting)
end
```

`value` in **extended.mod.json** is the starting value. When a player changes the text, the Modloader saves it in `settings.greeting.value`. `shroudforge.settings.get` returns the current value. The second argument is used if there is no value for that key. The code reads it again whenever `log_greeting` runs, including when the player presses the `logGreeting` button. Settings listed in custom groups appear in those tabs. Unassigned settings are collected in the standard **Settings** tab.


The optional dependencies exist to demonstrate dependency states. They do not block Hello Ember from starting when a dependency is unavailable.

## Permissions

Follow the shared [ShroudForge logging rules](../../development/write-and-review-logs.en.md) when adding log messages. In particular, reserve INFO for meaningful operations and confirmed results. Use WARN and ERROR for actionable problems, and DEBUG or TRACE for investigation detail.

The **capabilities** list in **mod.json** demonstrates all four values accepted by the manifest: **patch**, **export**, **runtime**, and **runtime-register-dll**. Hello Ember only writes messages and does not use the patch, export, or DLL registration features. This example lists them to show the manifest choices. For a real mod, declare only the capabilities its code uses.

## Keep in mind

Player setting choices are saved in **extended.mod.json**, beside the rest of the mod. Replacing the mod folder with an updated package can replace those choices too. If you need to keep a particular setting, make a copy of the file before replacing the package.

For a friendly explanation of both information files, use the [mod settings guide](https://bonsaibauer.github.io/shroudforge/en/#manifests). The [API reference](https://bonsaibauer.github.io/shroudforge/en/#api) helps when you are ready to explore more functions.
