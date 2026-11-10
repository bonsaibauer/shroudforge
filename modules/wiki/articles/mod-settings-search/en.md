# Search and organize large mod settings

Use the Modloader's existing setting groups to organize a mod page into tabs, and make long multi-select lists searchable with optional categories. The controls are opt-in, keep saved values unchanged unless the player changes a choice, and use the mod's existing locale files.

## What changes in a large mod page

This feature is for settings declared by a mod in `extended.mod.json`. It uses the existing mod setting page, saved setting values, and mod locale files. It does not add another settings store.

## Organize a large multi-select

For a `multiselect`, keep `value` as an array and declare the available choices in `options`. Search controls appear automatically when there are more than 12 choices. The example also groups choices and puts the setting group in its own page tab.

```json
{
  "schemaVersion": 1,
  "enabled": true,
  "settings": {
    "showEffects": {
      "value": true,
      "label": "Show effects",
      "control": "toggle"
    },
    "excludeItems": {
      "value": [],
      "label": "Excluded items",
      "description": "Choose items this mod should ignore.",
      "control": "multiselect",
      "options": {
        "Twigs": { "label": "Twigs", "group": "Materials" },
        "Stone": { "label": "Stone", "group": "Materials" },
        "Resin": { "label": "Resin", "group": "Materials" },
        "Plant Fiber": { "label": "Plant Fiber", "group": "Materials" },
        "Shroud Wood": { "label": "Shroud Wood", "group": "Materials" },
        "Fossilized Bone": { "label": "Fossilized Bone", "group": "Materials" },
        "Metal Scraps": { "label": "Metal Scraps", "group": "Materials" },
        "Fire Fly": { "label": "Fire Fly", "group": "Creatures" },
        "Fell Wispwyvern": { "label": "Fell Wispwyvern", "group": "Creatures" },
        "Hollow Halls": { "label": "Hollow Halls", "group": "Creatures" },
        "Bronze Arrows": { "label": "Bronze Arrows", "group": "Ammunition" },
        "Explosive Arrows": { "label": "Explosive Arrows", "group": "Ammunition" },
        "Poison Arrows": { "label": "Poison Arrows", "group": "Ammunition" }
      }
    }
  },
  "groups": [
    { "label": "General", "settings": ["showEffects"] },
    { "label": "Excluded items", "settings": ["excludeItems"], "tab": true }
  ]
}
```

Option keys such as `Twigs` are the values saved for the mod. A string label such as `"Twigs": "Twigs"` remains supported. The object form adds a visible `label` and an optional `group`. Category tabs divide the choices inside the multi-select. Search checks both the visible label and stored value across every category. The count shows visible, total, and selected choices. “Only selected” hides unchecked choices without unselecting them. Changing a checkbox updates the same saved array as before, and a long list scrolls inside its setting.

## Translate mod labels and tabs

Labels authored by the mod belong in its existing `locales/<code>.json` files. Shared controls such as Search, Only selected, and All use the player's general Modloader language.

```json
{
  "settings": {
    "excludeItems": {
      "options": {
        "Twigs": "Twigs",
        "Stone": "Stone",
        "Fire Fly": "Fire Fly",
        "Bronze Arrows": "Bronze Arrows"
      },
      "optionGroups": {
        "Materials": "Materials",
        "Creatures": "Creatures",
        "Ammunition": "Ammunition"
      }
    }
  },
  "groups": [
    { "label": "General" },
    { "label": "Excluded items" }
  ]
}
```

The `options` map translates visible choice labels. `optionGroups` translates category tabs, and each key must match a `group` name in `extended.mod.json`. The `groups` array follows the order of the manifest groups, so its labels translate the page tabs. See the [mod locale guide](#wiki/modding-locales) for the existing locale format.

## Put a setting group in its own tab

Set `"tab": true` on a group in `extended.mod.json`. That group's label becomes its page tab, and its settings and actions stay together. Groups without `tab: true` remain under the existing Settings tab. If there are no unmarked groups or ungrouped settings, the Settings tab is omitted. For a working example, the [ShroudForge World Editor's `extended.mod.json`](https://github.com/bonsaibauer/shroudforge/blob/1.8.0/mods/world-editor/extended.mod.json) uses `tab: true` for Keyboard shortcuts, Blueprint placement, and Native entity operations.

## Saved values and compatibility

Existing string option labels, saved arrays, groups without `tab`, and locale files without `optionGroups` continue to work without migration. The search controls appear only for multi-selects with more than 12 choices. Grouped option objects, `tab: true`, and locale `optionGroups` require ShroudForge 1.8.0 or newer. Older versions reject fields they do not recognize.
