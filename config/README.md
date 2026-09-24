# Configuration and package contract

The player installation contains these configuration and package metadata files:

| Path | Contents |
| --- | --- |
| `config/shroudforge.json` | Loader, logging, and module settings |
| `config/version.json` | Release version and build metadata, generated for the player ZIP |
| `mods/<id>/mod.json` | Activation and settings for each mod |

Reading these files does not modify them. Startup migrations and writes use the
same installation-wide lock under `config/`; mod packages do not contain lock
files.

The loader generates `config/state.json` as needed. It combines update results,
asset preparation, runtime and parser status, diagnostic reports, news and mod
history, catalog records, and the current visibility/request state for runtime
windows. It is not a default configuration and is not included in the player
ZIP. `config/news/news.json` is embedded into the loader binary at build time
and is not shipped as a separate player file.

Module `enabled` values control startup behavior. Window visibility is runtime
state and is changed through the window controls or F9/F10; it is not saved as
a module preference.

Schemas, API contracts, news sources, templates, and documentation stay in the
repository. Runtime profiles are maintained in the
`Shroudforge_Modloader/kfc-runtime` submodule and embedded in `kfc-runtime.dll`.
Compatibility rules are embedded in the loader. Updates preserve
`config/shroudforge.json`, `config/state.json`, and mod manifests with their
saved activation and settings.

Mod setting definitions declare their application timing with `x-apply`:
`live` applies immediately, `restart` applies after a restart, and `prepare`
applies after asset preparation. Unsupported values such as `world-load` are
reported as requiring a restart. Diagnostics are off by default and do not
establish compatibility or approve a mod.
