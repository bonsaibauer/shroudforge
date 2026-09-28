# Hello Ember: your starter mod

Welcome! This folder contains a small working ShroudForge mod. It writes messages to the ShroudForge log, shows a few settings in the Modloader, and has a button you can try. It does not change the game world.

You do not need to understand every line before you begin. First, follow the [first-mod lesson](https://bonsaibauer.github.io/shroudforge/en/#first). Then come back here to explore the example.

## What is in this folder?

~~~text
mod.json                 The mod's name, version, author, and permission
extended.mod.json        The switch, settings, and button shown in the Modloader
icon.svg          The small picture shown with the mod
src/mod.lua              The instructions the mod runs
~~~

Think of **mod.json** as the information card for the mod. **extended.mod.json** adds ShroudForge settings and buttons. The Lua file contains the behavior.

## Try the example

1. Copy this folder into the game's **mods** folder.
2. Start Enshrouded and press **F9** to open the Modloader.
3. Find **Hello Ember** and switch it on.
4. Press **F10** to see messages from ShroudForge.
5. Change the greeting or its message style, then use the button in the Modloader.

The mod is marked as a ShroudForge mod because it includes **extended.mod.json** and uses ShroudForge functions. A mod that contains only **mod.json** can be read as an EML mod. Removing the extension file does not replace ShroudForge functions in the Lua file with EML functions.

## Make it yours

Before sharing your own copy, update these values in **mod.json**:

- **id:** Give your mod a unique folder-friendly name, such as **yourname.hello-ember**. Use letters, numbers, dots, underscores, or hyphens.
- **name:** Choose the name players will see.
- **version:** Start with **1.0.0**. Use three numbers separated by dots.
- **authors:** Add the names you want players to see.
- **description:** Say in one sentence what your mod does.

In **extended.mod.json**, you can change the greeting, the message style, and the button text. The button's **id** must match the action name used in **src/mod.lua**. This lets the Modloader know which Lua instruction to run when someone presses the button.
### How the setting reaches the Lua code

The `settings` key in **extended.mod.json** is the name your Lua code asks for. In this example the key is `greeting`; the player-facing label can say “Greeting” because Lua uses the key, not the label.

```lua
local function setting(name, fallback)
    return shroudforge.settings.get(name, fallback)
end

local function log_greeting()
    local greeting = tostring(setting("greeting", "Hello from the ShroudForge runtime!"))
    shroudforge.log.info(greeting)
end
```

`value` in **extended.mod.json** is the starting value. When a player changes the text, the Modloader saves it in `settings.greeting.value`. `shroudforge.settings.get` returns the current value; the second argument is used if there is no value for that key. The code reads it again whenever `log_greeting` runs, including when the player presses the `logGreeting` button. A setting only appears in the Modloader when its key is listed in a group's `settings` list.


The sample project links point to an example address. Replace them with your own links or remove them before sharing the mod.

## Permissions

The **capabilities** list in **mod.json** tells ShroudForge which special actions this mod is allowed to use. This example only writes messages, so it asks for **runtime**. Add **patch** or **export** only when your mod needs those features.

## Keep in mind

Player setting choices are saved in **extended.mod.json**, beside the rest of the mod. Replacing the mod folder with an updated package can replace those choices too. If you need to keep a particular setting, make a copy of the file before replacing the package.

For a friendly explanation of both information files, use the [mod settings guide](https://bonsaibauer.github.io/shroudforge/en/#manifests). The [API reference](https://bonsaibauer.github.io/shroudforge/en/#api) helps when you are ready to explore more functions.
