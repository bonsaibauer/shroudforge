---
name: change-bootstrap-entrypoint
description: Change Windows proxy bootstrap, startup gating, runtime lifecycle, or loader process/CLI routing while preserving pre-entrypoint preparation, owned child processes, and failure behavior.
---

# Change Bootstrap or Loader Entrypoints

Read `.agents/skills/repository/repository-working-rules.md`. Trace the specific external entrypoint to its canonical feature owner. CLI/UI/process-mode routing supplies inputs and presents results. It must not duplicate preparation, mod planning, update, or gameplay logic.

## Read the Actual Startup Chain

For native bootstrap work, read the affected `modules/entrypoints/windows-bootstrap/` files: `startup.cpp`, `startup_gate.h`, `runtime_lifecycle.cpp`, `runtime_bridge.cpp`, `module_processes.cpp`, `logging.cpp`, `logging_config.h`, `bootstrap_state.h`, and `winmm_proxy.cpp`. Include `ShroudForge.Bootstrap.vcxproj` and the actual packaging caller for output-path changes.

For Rust routes, read `modules/entrypoints/loader/main.rs`, `process_modes.rs`, `command_line.rs`, `embedded.rs`, the affected command, and the module it calls. Follow embedded startup asset preparation, native DLL preparation, updater controller, UI child modes, and stop events where they participate.

## Preserve Startup and Process Ownership

`DllMain` admits the supported game executable, installs the entrypoint gate, and starts a worker. Preparation waits at the executable entrypoint outside the loader lock. Do not move parser work, DLL loading, blocking preparation, or UI initialization into `DllMain` or block the loader lock while waiting for another module.

Preserve the established order: gate entered, logging/update/required preparation, Rust startup preparation and native provider preparation, entrypoint release, then the runtime lifecycle. Native hooks that depend on pre-entrypoint setup must be ready at their existing boundary. Startup preparation failure must prevent entrypoint release through the existing failure result.

Keep native and Rust lifecycle ABI declarations synchronized. Preserve the proxy's system DLL forwarding names and ordinals. The native bridge loads its canonical runtime path. Avoid a second provider loader or a new installation location.

Retain dedicated-server headless behavior and the distinct interactive client child modes. Start owned helpers hidden where the existing contract requires it. Pass root, parent PID, stop event, and mode arguments through the existing route. Preserve bounded shutdown and terminate only children owned by this lifecycle. Retain required-update exit meanings, timeout behavior, and pending installation after game exit.

For asset preparation changes, read `loader/asset_preparation/` and its tests. Preserve the exclusive startup lease, transaction recovery, immutable original backups versus working backups, current application fingerprint, and existing updater action for recycling stale baselines. Do not manually remove local backups or reapply already prepared assets from a runtime/export path.

## Verify

Build the native bootstrap with `pwsh -NoProfile -File dev/build/build_windows_bootstrap.ps1` and the affected Rust package through its existing build entrypoint. `cargo test --release -p shroudforge-modloader` covers loader/asset-preparation behavior.

Run `pwsh -NoProfile -File modules/entrypoints/windows-bootstrap/tests/run.ps1` for gate success and injected preparation failure. It checks failure exit code 1114 and a deadlock deadline. `dev/tests/run_workspace_tests.ps1` combines that gate test and the release workspace tests when the broader check is appropriate.

When live startup is requested, exercise the actual installed client/server entrypoint, inspect the role-specific bootstrap/runtime/debug logs, verify the expected provider and session readiness, and confirm owned helper shutdown. A successful process start is not proof that all runtime calls work.

Report routing/ordering changes, owning modules, output/package paths, failure and cleanup behavior checked, and live checks actually performed.
