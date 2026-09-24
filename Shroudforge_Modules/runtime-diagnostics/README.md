# Runtime diagnostics

The module's library runs inside the loader and records the running world's
ECS context and Lua lifecycle timings per mod.
It reads the native provider's diagnostic export for observed ECS layout state,
game-thread heartbeat, queue depth, timeouts, rejections, operation timings, and
entity-manager changes. No additional memory scans or game writes are performed.

Entity-manager changes and layout epochs are measured ECS-context transitions,
not invented world names or guaranteed world-join events. A missing provider/export
is reported as a concrete failure. Measurements do not grant compatibility.

## Configuration and controls

`config/shroudforge.json`, `modules.runtimeDiagnostics`, is the only
configuration. The loader polls controls every 500 ms; samples use the configured
interval. Sessions stop at `maximumDurationSeconds` and persist `enabled: false`.
`continuous: false` captures one snapshot. Start increments `requestId` so an
already-running session can be restarted intentionally. Invalid settings are
rejected by the loader schema.

Use Settings / Modules / Runtime Diagnostics, or:

```powershell
shroudforge.exe --runtime-diagnostics --root "C:\Games\Enshrouded" --start
shroudforge.exe --runtime-diagnostics --root "C:\Games\Enshrouded" --snapshot
shroudforge.exe --runtime-diagnostics --root "C:\Games\Enshrouded" --stop
shroudforge.exe --runtime-diagnostics --root "C:\Games\Enshrouded" --status
```

Commands are requests, not fabricated success reports. A running loader consumes
them; otherwise they take effect at its next start. Status is reported through
the generated `config/state.json` and validated against the repository schema.
The UI displays remaining duration and last sample time. Previously elapsed work
is not reconstructed. Parser/API startup checks, build/profile validation and
compatibility decisions are not part of this module.

English output uses the existing `shroudforge.log` format and the `diagnostics`
source. Diagnostic entries appear in the ShroudForge Debug Log tab alongside
other loader messages. `logging.enabled` and `minimumLevel` still apply. INFO
summaries may be filtered at WARN/ERROR; slow callbacks and measurement failures
use WARN.

## Explicit native tools

The five native inspection tools remain available in this module and release.
They never run automatically:

```powershell
shroudforge.exe --runtime-diagnostics --root "C:\Games\Enshrouded" --pid 1234 --tool live-entity-managers --timeout-seconds 10
```

`--list` lists tools. `inspect-component-metadata` requires `--address` in hex.
Each child has a 1–60 second timeout and captures at most 2 MiB per stream. Explicit
`--continuous` scans also obey the configured session duration and enable flags.
These exploratory tools have game-specific assumptions; they are not compatibility
verdicts. A timeout kills only the inspection child, never the game.
