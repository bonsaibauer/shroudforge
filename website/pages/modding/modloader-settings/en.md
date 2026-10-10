# Modloader settings

These settings configure the Modloader interface, built-in modules, and their data locations. They live in `shroudforge/config/modloader-config.json`. They are separate from mod settings in `extended.mod.json`.

The field pages are generated from `loader.schema.json` and the defaults file. Each setting has its own Markdown page. Internal action and session values such as `requestId` and window-position state are not presented as user preferences.

## Areas

- **General**: language, username, and display behavior.
- **Logging**: minimum level for saved log entries.
- **Debug Console**: source, filter, shortcut, and read limit.
- **Modloader and World Editor UI**: enablement and refresh behavior.
- **Network**: Dedicated Server fallback and optional SteamID64 allowlist.
- **Runtime diagnostics**: areas, interval, duration, and slow-callback threshold.
- **Updates**: release channel and check interval.
- **Storage locations**: mods, state, logs, cache, exports, and runtime data.

Changing `minimumLevel` controls what gets saved. A Debug Console filter does not change the log file contents.
