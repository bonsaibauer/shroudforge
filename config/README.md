# Configuration and package contract

Players edit these files:

| Path | Contents |
| --- | --- |
| `config/shroudforge.json` | Loader, logging, and module settings |
| `mods/<id>/mod.json` | Activation and settings for each mod |

Reading these files does not modify them. Startup migrations and writes use the
same installation-wide lock under `config/`; mod packages do not contain lock
files.

The loader generates `config/state.json` as needed. It combines update results,
asset preparation, runtime and parser status, diagnostic reports, news and mod
history, and catalog records. It is not a default configuration and is not
included in the player ZIP. `config/news/news.json` is shipped loader content,
not a player setting.

Schemas, API contracts, news sources, templates, and documentation stay in the
repository. Runtime profiles come from `Shroudforge_Modloader/kfc-runtime` and
are shipped at `runtime/compatibility/profiles/` for the runtime reader.
Compatibility rules are embedded in the loader. Updates preserve
`config/shroudforge.json`, `config/state.json`, and mod manifests with their
saved activation and settings.

Mod setting definitions declare their application timing with `x-apply`:
`live` applies immediately, `restart` applies after a restart, and `prepare`
applies after asset preparation. Unsupported values such as `world-load` are
reported as requiring a restart. Diagnostics are off by default and do not
establish compatibility or approve a mod.
