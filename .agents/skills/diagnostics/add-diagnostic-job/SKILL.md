---
name: add-diagnostic-job
description: Add a bundled runtime diagnostic job using the existing job schema, discovery, execution adapter, and export lifecycle. Use for collector, Lua, Python, mod-action, or compiled built-in jobs under modules/runtime-diagnostics/diagnostics-jobs/.
---

# Add a Runtime Diagnostic Job

Work from the repository root. Read `.agents/skills/repository/repository-working-rules.md`, `modules/runtime-diagnostics/AGENTS.md`, its `README.md`, `diagnostics-jobs/README.md`, and `catalog/diagnostic-job.schema.json`.

## Select the Existing Execution Route

Read the closest definition, its script, its caller, and its result consumer. Choose a collector-only definition, Lua script, Python script, registered mod action, or schema-allowed compiled Rust action according to the requested effect.

Create a descriptively named `<action>.job.json` and required script in the existing task group under `modules/runtime-diagnostics/diagnostics-jobs/`. Preserve an existing job ID on renames or moves. Use stable IDs for new jobs.

Use the schema's `schemaVersion`, `id`, `name`, `description`, `capabilities`, and optional `execution`. Collector capabilities are `runtime`, `queue`, `mods`, and `operations`. Respect the existing definition and script size limits.

`build.rs` recursively embeds supported files. `catalog/load_diagnostic_jobs.rs` discovers definitions, validates scripts and IDs, and rejects duplicates. Add no second catalog. Every bundled JSON is interpreted as a job, so an unrelated helper JSON does not belong in this bundle.

## Preserve Adapter and Lifecycle Semantics

- Lua jobs require `modId` and `script` and run with that active mod's identity, capabilities, and world/thread restrictions. A job does not activate the mod.
- A `modAction` execution references an action already registered by the active mod.
- Python runs directly without a shell through `execution/run_python_diagnostic_job.rs`. Use its existing interpreter selection. Do not install an interpreter or packages in the runtime flow.
- Python argument placeholders are `{pid}`, `{gameExe}`, `{gameRoot}`, `{jobs}`, `{exports}`, and `{diagnosticExports}`. Use `{exports}` for this job's results. Verify exact image identity before consuming another capture through `{diagnosticExports}`.
- Compiled Rust actions are fixed schema-allowed host operations. Follow the schema and `modules/mod-runtime/ingame/run_diagnostic_job.rs` when adding one. Runtime execution never compiles arbitrary Rust source.

Use the shared `session/` start/update/stop/timeout contract and `execution/run_diagnostic_job.rs`. Preserve cancellation, shutdown, and actual elapsed Lua callback time.

Write results through `execution/write_job_exports.rs` and `JobExports` under the configured exports directory at `diagnostics/<job-id>/`. Keep diagnostics logging and active-mod script identity. Preserve readback, recovery evidence, and repeat-execution protections for mutating jobs.

Installed custom jobs use the configured runtime `diagnostics/jobs/` folder and `user.` IDs. Keep installed jobs, settings, captures, and exports separate from bundled source authoring.

## Verify

Use existing checks in `tests/diagnostic_jobs.rs`, `tests/world_editor_jobs.rs`, and the Python tests when requested. The Cargo package is `shroudforge-runtime-diagnostics`.

For requested live verification use the existing Runtime Diagnostics Start/Stop/Snapshot UI and a disposable world for mutating effects. A listed job does not run automatically.

Report the ID, execution route, required capabilities/mod state, discovery path, exports, lifecycle behavior, and actual checks.
