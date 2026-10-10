# Connect Modloader buttons to Lua

An action is declared under `groups[].actions[]`. Its `id` is passed to `shroudforge.ui.on_action`. The two IDs must match exactly.

```lua
shroudforge.ui.on_action("resetFeature", function()
  shroudforge.log.info("Reset action requested")
end)
```

The mod needs `runtime`. Register the callback while the mod runtime loads. `label` and `style` describe the visible button, while `confirm` adds an optional prompt.

The action only calls your callback. It does not guarantee a game change succeeded. Check return values and log the outcome. See the [API reference](#api) for function details.
