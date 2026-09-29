# Mod packages and settings

This page explains the files inside a mod folder. If you are building your first mod, follow the [step-by-step lesson](https://bonsaibauer.github.io/shroudforge/en/#first). The [interactive settings example](https://bonsaibauer.github.io/shroudforge/en/#manifests) lets you change an example and see how it may look in the Modloader.

## The two information files

Every mod has **mod.json**. Think of it as the mod's information card: its name, ID, version, author, dependencies, and permissions.

A mod can also have **extended.mod.json**. This optional file holds its ShroudForge on/off state, player settings, groups, buttons, links, and short update notes. Its optional `launcher` field records the mod's origin: `EML` preserves the EML badge; `SF` or an omitted value means ShroudForge.

| File | What it contains |
| --- | --- |
| **mod.json** | Mod information in the EML format. |
| **extended.mod.json** | ShroudForge state and optional extra details for one mod. |
| **shroudforge/config/modloader-config.json** | Settings for the ShroudForge loader, built-in features, and storage locations. |
| **shroudforge/version.json** | Release version and build details. |
| **shroudforge/state.json** | Default status file created by the loader, such as mod and update information. The location is configurable in Settings. |

The placeholder **<id>** means the folder name of a mod, such as **sf-world-editor**.

## Example: a small mod

The folder structure looks like this:

~~~text
my-first-mod/
├── mod.json
├── extended.mod.json
└── src/
    └── mod.lua
~~~

The mod's **mod.json** contains its basic information:

~~~json
{
  "id": "yourname.my-first-mod",
  "name": "My First Mod",
  "version": "1.0.0",
  "authors": ["Your Name"],
  "description": "My first ShroudForge mod.",
  "dependencies": [],
  "capabilities": ["runtime"]
}
~~~

- **id** is a unique name for the mod. Use letters, numbers, dots, underscores, or hyphens.
- **name** is the name players see.
- **version** is the mod's version, written as three numbers such as **1.0.0**.
- **authors** lists the people who made it.
- **dependencies** lists other mods that must be installed first. Use an empty list when there are none.
- **capabilities** lists the mod's declared access needs: **patch**, **export**, **runtime**, or **runtime-register-dll**. The last one schedules an EML mod that uses package-local native DLL registration for the runtime. The registration API checks the current phase and package path; it does not check this specific capability.

The current [manifest schema](../../src/loader/package/src/registry/manifest.schema.json) defines every accepted field.

## Add a switch or slider

The optional **extended.mod.json** file can add settings to the Modloader. For example:

~~~json
{
  "$schema": "https://bonsaibauer.github.io/shroudforge/schemas/extended.mod.schema.json",
  "schemaVersion": 1,
  "enabled": true,
  "launcher": "SF",
  "settings": {
    "allowDescent": {
      "value": false,
      "label": "Allow downward movement",
      "description": "Let the character move down during flight."
    },
    "flightSpeed": {
      "value": 1.0,
      "label": "Flight speed",
      "control": "slider",
      "min": 0.2,
      "max": 3.0,
      "step": 0.1
    }
  },
  "groups": [
    {
      "label": "Flight",
      "settings": ["allowDescent", "flightSpeed"]
    }
  ]
}
~~~

Here, **allowDescent** starts switched off. **flightSpeed** starts at **1.0** and can be changed between **0.2** and **3.0**. The group puts both settings under the heading **Flight**.

The default control follows the value: a boolean shows a switch, a string shows a text field, and a number shows a number field. In this example, `control: "slider"` makes `flightSpeed` a slider with the given limits. Adding `options` makes a single value a dropdown; an array needs `options` and becomes a multi-select. You can choose one of the 12 controls explicitly with `control`. Choice controls need `options`; use the option keys as values and their strings as the names shown to players. `min`, `max`, and `step` apply to numbers. `minLength` and `maxLength` apply to text.

A player-facing control only appears when a group lists its key in `groups[].settings`. The Lua code reads that same key with `shroudforge.settings.get(key, fallback)`. The [extension schema](../../src/loader/package/src/registry/extended.mod.schema.json) lists every accepted field; the package reader also checks that setting defaults, choices, and group references make sense together.

## Connect a Modloader setting to Lua

A setting has a definition in `extended.mod.json` and a read in `src/mod.lua`. Use the same setting key in both files. The label is only the text players see; Lua looks up the key.

This valid example creates a text field and places it under the **Greeting** heading:

```json
{
  "$schema": "https://bonsaibauer.github.io/shroudforge/schemas/extended.mod.schema.json",
  "schemaVersion": 1,
  "enabled": false,
  "settings": {
    "greeting": {
      "value": "Hello from my mod!",
      "label": "Greeting",
      "description": "Message shown by the mod.",
      "control": "text",
      "maxLength": 120
    }
  },
  "groups": [
    { "label": "Messages", "settings": ["greeting"] }
  ]
}
```

- `greeting` is the key your Lua code reads.
- `value` is the default and must match the kind of value: `true`/`false`, text, a number, or a list.
- `label` and `description` are shown to the player. They do not change the value Lua receives.
- `control` chooses the visible input. The supported controls are `toggle`, `checkbox`, `text`, `textarea`, `number`, `slider`, `select`, `radio`, `segmented`, `multiselect`, `keybind`, and `color`.
- `min`, `max`, and `step` set number limits. `minLength` and `maxLength` set text limits.
- `options` gives choices to `select`, `radio`, `segmented`, and `multiselect`. List values need `options`. A scalar value with `options` defaults to a select; a list defaults to a multi-select.
- A group must list a setting key in `settings` for that setting to appear under the group in the Modloader. Each setting can appear in one group.

Read the value in Lua with the ShroudForge API:

```lua
local greeting = shroudforge.settings.get("greeting", "Hello from my mod!")
shroudforge.log.info(greeting)
```

The first argument is the exact key from `extended.mod.json`. The second is a fallback used if that key has no value. Keep its type the same as the setting's `value`. The `runtime` permission belongs in `mod.json` for a mod using the ShroudForge runtime API.

When a player changes the text, the Modloader saves it at `settings.greeting.value` in the same `extended.mod.json`. The loader passes the saved value to the running runtime mod. Read it inside a lifecycle or action callback each time the mod needs the current value; a local variable created once at module load will keep the old value. The [starter mod](../../templates/mod/) shows this pattern with `shroudforge.settings.get`, `shroudforge.ui.on_action`, and a matching action ID.

The [extension schema](../../src/loader/package/src/registry/extended.mod.schema.json) defines the JSON fields. The loader also checks relationships such as group references and whether a default value matches its choices.

## What happens to player choices?

When someone changes a setting, the Modloader writes that choice into the mod's own **extended.mod.json**. Updating a mod replaces its package and may reset those choices. Settings migration is not supported.

Only an explicit `"launcher": "EML"` in **extended.mod.json** preserves the EML badge. The loader writes that marker when it first adds an extension to an EML package; the original **mod.json** stays untouched. `launcher: "SF"` can be written explicitly, but is optional because missing or other values are treated as SF.

## Groups, buttons, and links

Groups put related settings under one heading. A group may also contain buttons that ask the mod's Lua code to perform an action. Links open the creator's project pages. Short update notes can explain what changed in a new version.

These are ways to present a mod in the Modloader; they do not grant access. The mod must still declare its needs in **mod.json**: **patch**, **export**, **runtime**, or **runtime-register-dll** to schedule EML DLL registration during runtime.
