# Loader diagnostics

This ShroudForge module records live loader health and Lua callback timings. It
reads the KFC Runtime provider's diagnostic snapshot and adds loader-owned
measurements such as callback duration, queue state, and mod errors. It does not
inspect game memory independently and does not create or edit KFC Runtime profiles.

Entity-manager changes and layout epochs are measured ECS-context transitions,
not invented world names or guaranteed world-join events. A missing provider/export
is reported as a concrete failure. Measurements do not grant compatibility.

## Configuration and controls

`shroudforge/config/shroudforge.json`, `modules.runtimeDiagnostics`, is the only
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
the generated `shroudforge/config/state.json` and validated against the repository schema.
The UI displays remaining duration and last sample time. Previously elapsed work
is not reconstructed. Parser/API startup checks, build/profile validation and
compatibility decisions are not part of this module.

English output uses the existing `shroudforge/shroudforge.log` format and the `diagnostics`
source. Diagnostic entries appear in the ShroudForge Debug Log tab alongside
other loader messages. The shared `logging.minimumLevel` setting controls which entries are written and displayed. INFO
summaries may be filtered at WARN/ERROR; slow callbacks and measurement failures
use WARN.

Profile investigation tools, live ECS capture, and component-registry audits
are developer tools owned and shipped by the standalone KFC Runtime project.
See that repository's `dev/README.md` for commands and the profile approval
workflow. This module remains responsible for ShroudForge's runtime session,
status file, logging, and UI controls.
