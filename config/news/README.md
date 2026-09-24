# News

`news.json` is loader content, not player configuration. It is embedded into
the loader binary at build time; edits take effect in the next build. `messages`
contains the bundled notices; `templates` contains copy for mod and system
update events that actually occur. Placeholders are `{name}`, `{version}`, and
`{previousVersion}`. The UI reads changes in its next snapshot and validates
them against `news-schema.json`.

Notices explain configuration, changes, and optional project support. They are
adapted for ShroudForge and do not claim that an installation succeeded or an
update is available unless that result is known.

The loader stores generated state together in `config/state.json`:

- `events` contains notices from mods and mod installation events.
- `news` contains read notice IDs and Unix timestamps.
- `mods` contains the last observed mod versions used to detect changes.

Migration retains old files as recovery copies and does not overwrite existing
destination files. After migration, the UI reads only the new paths. System
update notices use the current updater status and copy from `templates`.

The system updater installs the binary that contains the bundled news. It does
not manage a separate `news.json` file. Player ZIPs do not contain news schemas.

With `repeatEveryDays: 90`, a notice becomes unread again 90 days after it was
last marked as read. Without this field, a notice appears only once. Legacy read
IDs without timestamps receive the current time during migration, so their
repeat interval starts then. Events are validated against `event-schema.json`
when written and read. Invalid configuration and state are reported as errors
instead of being replaced with empty files.
