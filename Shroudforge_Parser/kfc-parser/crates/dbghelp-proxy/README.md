# dbghelp-proxy

## Purpose

`dbghelp-proxy` builds a legacy `dbghelp.dll` proxy that starts the imported EML patch and runtime pipeline before forwarding the original Windows exports.

## Current status

This is a legacy EML bootstrap. Current ShroudForge releases use the Windows bootstrap under `Shroudforge_Modloader/bootstrap/windows`.

## Wine and Proton

The proxy must be preferred over the built-in DLL.

```bash
WINEDLLOVERRIDES="dbghelp=native,builtin" %command%
```

For standalone Wine or Proton execution, apply the same environment variable to the game command.

## Environment variables

| Variable | Purpose | Default |
| --- | --- | --- |
| `EML_CONSOLE` | Opens the legacy debug console | `false` |
| `EML_LOG_FILE_ENABLED` | Enables file logging | `true` |
| `EML_LOG_FILE_FILTER` | Sets the tracing filter for file logs | Application default |
| `EML_LOG_FILE_PATH` | Selects the log directory | `./logs` |
| `EML_LOG_FILE_MAX` | Limits retained log files | `128` |
| `EML_LOG_STDOUT_ENABLED` | Enables standard-output logging | `true` |
| `EML_LOG_STDOUT_FILTER` | Sets the tracing filter for standard output | Application default |
