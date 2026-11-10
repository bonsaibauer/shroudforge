---
{
  "id": "setting-groups-actions",
  "locale": "en",
  "title": "Groups and action buttons",
  "summary": "Groups organize mod settings. Action buttons call a named function in your Lua mod.",
  "navigation": {
    "path": [
      "extended.mod.json",
      "Groups and actions"
    ],
    "overview": true,
    "label": "Overview"
  }
}
---

# Groups and action buttons

A group has a visible label and can contain setting keys and actions. Set `tab: true` to show that group in its own tab on the mod page. Unmarked groups remain under the existing Settings tab, which is omitted if every group has its own tab. Group tab labels use the existing per-mod `groups[].label` translations. Groups do not change the mod's access permissions.

## Assign settings

`groups[].settings` contains keys from `settings`. A key may only be grouped once. The Modloader shows ungrouped values in a default group.

## Connect an action to Lua

The key `groups[].actions[].id` must exactly match the name passed to `shroudforge.ui.on_action`. Example:

```json
{
  "groups": [{
    "label": "Tools",
    "settings": ["enabledFeature"],
    "actions": [{"id": "resetFeature", "label": "Reset", "style": "secondary"}]
  }]
}
```

```lua
shroudforge.ui.on_action("resetFeature", function()
  shroudforge.log.info("Feature settings reset")
end)
```

The mod needs `runtime` in `mod.json`. `style` only changes the appearance. `confirm` asks the player before calling the action. See [UI actions in Lua](#doc-api-ui-actions).
