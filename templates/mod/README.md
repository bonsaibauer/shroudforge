# Mod configuration and application phases

`mod.json` is the single source of activation and setting values, both for the
client and a dedicated server. A missing `enabled` means disabled. A server
administrator edits the installation's own file; no client synchronization or
remote administration is involved.

Setting values belong in `settings`; definitions belong in
`shroudforge.settingsSchema.properties`. Keep the EML manifest base intact.

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

For the standalone UI, use `--root <installation> --standalone --target client`
or `--target server`. The target can be inferred only if exactly one corresponding
executable exists. When attached, the UI verifies the actual game PID and its
installation directory instead of inferring the mode from directory contents.
