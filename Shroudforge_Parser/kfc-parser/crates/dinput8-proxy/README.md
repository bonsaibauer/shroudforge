# dinput8-proxy

## Purpose

`dinput8-proxy` builds a legacy `dinput8.dll` proxy that starts the imported EML patch and runtime pipeline before forwarding the original Windows exports.

## Current status

This is a legacy EML bootstrap. Current ShroudForge releases use the Windows bootstrap under `Shroudforge_Modloader/bootstrap/windows`.

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

On Steam for Linux, `EML_CONSOLE=1 %command%` enables the console when the selected Proton setup loads this proxy.
