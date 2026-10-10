# Settings and controls

An entry under `settings` has a mod-specific key. That key connects `extended.mod.json` to your Lua code.

## Simple and detailed values

A boolean, string, number, or list can be written as a direct value. The object form adds metadata:

```json
{
  "settings": {
    "flightSpeed": {
      "value": 1.0,
      "label": "Flight speed",
      "description": "How quickly the character moves.",
      "control": "slider",
      "min": 0.2,
      "max": 3.0,
      "step": 0.1
    }
  }
}
```

## All twelve controls

The individual pages explain when a control fits and which values it expects. A boolean usually becomes a toggle, text becomes a text field, and numbers become a number field. Set `control` to choose a specific presentation.

- Toggles: [toggle](#doc-control-toggle) and [checkbox](#doc-control-checkbox).
- Text: [text](#doc-control-text) and [textarea](#doc-control-textarea).
- Numbers: [number](#doc-control-number) and [slider](#doc-control-slider).
- Choices: [select](#doc-control-select), [radio](#doc-control-radio), [segmented](#doc-control-segmented), and [multiselect](#doc-control-multiselect).
- Special inputs: [keybind](#doc-control-keybind) and [color](#doc-control-color).

## Read values in Lua

`shroudforge.settings.get("flightSpeed", 1.0)` reads the current saved value. Use the exact key and a fallback with the same type. Read it inside a callback when the current value should reflect changes during the session. See [Read settings in Lua](#doc-api-settings).

## Groups are optional

Use `groups[].settings` to choose the order. A setting not listed there appears in the default group, it is not hidden. Modloader saves changes into the same `extended.mod.json`.
