# The `settings.<key>` key

The placeholder `<key>` is replaced by a name chosen by the mod author. For example, `settings.flightSpeed` names a setting `flightSpeed`.

The key accepts 1 to 80 letters, numbers, dots, hyphens, and underscores. Letter case matters. Lua uses the exact same name: `shroudforge.settings.get("flightSpeed", 1.0)`.

This is not a global ShroudForge setting. Each mod has its own setting values.
