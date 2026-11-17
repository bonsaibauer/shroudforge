---
{
  "id": "builtin-mod-sf-unlimited-flight",
  "locale": "en",
  "title": "SF Unlimited Flight",
  "summary": "Independent upward gliding, glider stamina, yaw rate, forward movement, and air resistance settings.",
  "navigation": {
    "path": [
      "Built-in mods"
    ]
  },
  "source": {
    "file": "mods/sf-unlimited-flight/extended.mod.json",
    "pointer": "/settings"
  }
}
---

# SF Unlimited Flight

SF Unlimited Flight provides six flight settings. The package targets the client and Dedicated Server and declares the `runtime` capability. Enable it on the host or dedicated server to control the world's players.

| Setting | Starting value | Range and effect |
| --- | --- | --- |
| `unlimitedFlying` | `true` | Checkbox. Steers each gliding player independently, including upward pitch. |
| `unlimitedGliderStamina` | `true` | Checkbox. Prevents stamina drain while gliding. |
| `turnSpeed` | `100` | 25 to 400 percent in steps of 5. Multiplies the original glider yaw rate. |
| `moveSpeed` | `100` | 25 to 400 percent in steps of 5. Multiplies the original forward acceleration. |
| `liftSpeed` | `100` | 25 to 400 percent in steps of 5. Multiplies longitudinal air resistance. Higher values add drag. |
| `descendSpeed` | `100` | 25 to 400 percent in steps of 5. Multiplies vertical air resistance. Higher values add drag. |

The package itself starts disabled. It reads these keys from its `extended.mod.json`. Upward steering supports pitch-speed and pitch-target-angle controls, with no player-count restriction in the mod. Each player's input controls that entity's pitch. The four percentage controls use the active glider's shared coefficients and affect every player using that glider type.

The [mod guide](https://github.com/bonsaibauer/shroudforge/blob/HEAD/mods/sf-unlimited-flight/README.md) describes technical limits and restoration rules.
