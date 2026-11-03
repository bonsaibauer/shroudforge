---
name: build-release-package
description: Build and verify a local ShroudForge development or release ZIP with the canonical native, Rust, frontend, test and packaging entrypoints. Use for reviewable local artifacts, not publishing a GitHub release or installing into a game folder.
---

# Build a Release Package

Read `.agents/skills/repository/repository-working-rules.md`, `VERSION`, `dev/build/dev_build.ps1`, `release_build.ps1`, `package_release.ps1`, the invoked build wrappers, and `dev/tests/run_workspace_tests.ps1`. Preserve existing source and user changes. Choose the requested development/release entrypoint and a valid BuildNumber. VERSION must retain MAJOR.MINOR.PATCH format.

Use the existing PowerShell wrapper. It prepares the direct `kfc-parser/` submodule, builds both frontends, native runtime, proxy DLLs, workspace and bootstrap, runs workspace checks unless explicitly skipped, then packages. Do not add another build orchestrator. Use `-UseInstalledDependencies` or `-SkipUiBuild` only when those existing outputs/dependencies are present and the user's requested verification permits reuse. Record every skipped check.

`package_release.ps1` stages under build/package and writes `build/shroudforge-<version>-<build>.zip` plus SHA-256 sidecar. Preserve canonical root proxy paths, `shroudforge/` runtime/launcher/updater paths, licenses, runtime profiles, direct mods and shipped default config. Historical game-build runtime profiles are copied as immutable source references. Never rewrite them during packaging. Mod compatibility adapters are separate inputs.

Preserve version.json updateKind, requiredAction, updaterIncluded and managedPaths. User configuration, generated state/events/locks are not managed release files. Follow updater readers in `modules/updater/system-updates/read_managed_release_files.rs` and `validate_update_paths.rs` before changing package layout. `-WithoutUpdater` is an explicit release-build option and must agree with archive contents/metadata.

Inspect the finished ZIP and checksum using existing archive validation plus a direct sidecar/hash check. Verify required entries, absence of live installation data and truthful test/build completion. Report exact artifact paths, version/build, updater inclusion and executed/skipped checks. Creating a local package does not publish or install it.
