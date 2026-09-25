# Mod configuration and application phases

`mod.json` is the single source of activation and setting values, both for the
client and a dedicated server. A missing `enabled` means disabled. A server
administrator edits the installation's own file; no client synchronization or
remote administration is involved.

Setting values belong in `settings`; definitions belong in
`shroudforge.settingsSchema.properties`. Keep the EML manifest base intact.

Optional project links belong in `shroudforge.links` in `mod.json`. They are
shown as one continuous, ordered badge row. `source` is platform-neutral;
provider-specific source badges use `source-github`, `source-gitlab`, or
`source-codeberg`. `issues` stays provider-neutral. Support links are direct
fields such as `support-bmac` and `support-patreon`:

```json
"support-bmac": "https://buymeacoffee.com/yourname",
"support-patreon": "https://patreon.com/yourname"
```

Badge definitions and their order live in the repository under `config/links/`;
SVG icons are stored in `config/links/assets/`. Every link must use HTTPS. Only links
listed in the mod's own manifest are displayed; the modloader does not fetch
project links from ShroudEdit.

- `x-apply: live`: validated values refresh in memory between runtime updates,
  approximately once per second. The mod must call `shroudforge.settings.get`
  when it uses the value. A value cached in a Lua local at initialization does
  not become live automatically.
- `x-apply: restart` (also the default): values take effect at the next process
  start. Changing `enabled`, dependencies, definitions or code requires restart.
- `x-apply: prepare`: prepare assets with the target process stopped, then restart.
  The preparation fingerprint must match the selected installation and mods.

Invalid edits retain the last applied values and produce an error. Pending edits
do not replace active mod scopes. The UI reports desired versus loaded state;
the dedicated server uses the same files without requiring a UI.

For the desktop UI, use `--root <installation> --desktop --target client`
or `--target server`. `--target` selects which game installation the desktop UI
manages; the UI itself runs in desktop mode. The target can be inferred only if
exactly one corresponding executable exists. When attached, the UI verifies the
actual game PID and its installation directory instead of inferring the target
from directory contents.
