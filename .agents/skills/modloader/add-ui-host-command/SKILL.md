---
name: add-ui-host-command
description: Add a concrete Modloader frontend command through the existing IPC parser, native dispatch, owning action and result notification. Use for UI-to-host actions, not for a new Lua API or a second generic command system.
---

# Add a Modloader UI Host Command

Read the shared repository rules at `.agents/skills/repository/repository-working-rules.md`. Paths below are relative to the repository root.

Read the actual frontend caller and `modules/transport/ui/send-host-command.ts`, then `modules/modloader/ui/host/parse_ui_command.rs` and the complete relevant branch in `window.rs`. Put business behavior in its owning module and let the UI host adapt input and report results.

Extend `UiCommand`, the typed `Command` enum, parsing and event-loop dispatch only as needed. Validate required fields and retain request IDs. A new handler belongs under `modules/modloader/ui/host/` only when it actually owns a distinct host adaptation, with a concrete action filename and normal Rust module declaration.

Use `report_command_result.rs` and `notify_command_result.rs` for existing activity logging, request completion, toast severity, restore behavior and saved-mod revision propagation. The frontend receives results in `receive-ui-updates.ts`. Do not create another result protocol or optimistic-save path.

If the same action must be available through persisted control actions, update loader defaults/schema, ownership routing in `modules/loader-config/lib.rs`, `claim_control_actions.rs`, and the appropriate existing dispatcher. Preserve one owner between Modloader and updater. Read-only status and presentation commands need not become persisted actions.

Respect desktop/client/server restrictions, disabled modules and existing process lifecycle. Updater operations use the existing updater request adapter and worker rather than performing downloads in the UI host. Add translations for visible results and verify parsing, error/result delivery and relevant frontend/native checks when requested.
