---
name: capture-runtime-evidence
description: Capture exact-executable reflection, compiler code ranges, live ECS registrations, and production-reader evidence for client/server investigations. Use to collect or compare evidence without treating it as a production profile or native-call approval.
---

# Capture Runtime Evidence

Read `.agents/skills/repository/repository-working-rules.md`. Select client, server, or both deliberately. Keep each executable/build capture in a new output directory with its full identity. Use the existing profile skills when the deliverable is a new production profile.

## Use the Canonical Capture Action

Read `dev/profiles/capture/capture_runtime.ps1` and its actual Python/native callers. Its default target is `both`, so pass `-Target client` or `-Target server` when only one target is intended. Supply the correct installation directory and a new `-OutputDirectory`.

The Python implementation lives under `modules/runtime-diagnostics/diagnostics-jobs/game_inspection/`. Its pinned development dependencies are in `requirements.txt`. Reuse the configured interpreter and existing environment. Do not add a runtime interpreter/package installer or change installed job/configuration files to perform a source investigation.

For offline investigation, use for example:

```powershell
pwsh -NoProfile -File dev/profiles/capture/capture_runtime.ps1 -Target server -ServerDirectory '<server-directory>' -OutputDirectory '<new-output-directory>' -Offline
```

For live capture, build the production verifier explicitly with `cmake --build build/native-runtime --config Release --target kfc-runtime-verify-component-registry`, start the selected executable through its normal authorized startup route, and invoke the same capture action without `-Offline`. The wrapper requires exactly one process whose executable path equals the selected file. Its read-only process adapter checks path, SHA-256, timestamp, image size, and module base. A process restart invalidates captured heap addresses.

## Preserve Evidence Boundaries

Read `discover_runtime_build.py`, `read_process_memory.py`, `discover_component_registry.py`, and `export_runtime_bindings.py` for the reports being consumed. Static reflection is freshly extracted. Live tables require repeated identity/storage evidence and parallel-array agreement. Runtime storage and template-only registrations are distinct. Callback addresses do not establish callback ABIs.

Use `compare_runtime_builds.py` for exact-image layout/code comparisons and `trace_function_anchors.py` for fresh signature-to-procedure/caller traces. Preserve ambiguous candidates, undecoded ranges, and incomplete coverage. A matched procedure entry does not transfer an old hook/patch site or capture offset. Read each action's CLI contract and run it through the existing `game_inspection` Python package root.

The production verifier adds the actual production registry reader's observations. Keep `provider-verification.json`, function/adapter evidence, and source identity together. `runtime-bindings.json` and `runtime-index.json` remain discovery artifacts with `productionProfile:false` and incomplete-engine-API limitations.

Optional `-VerifyRuntimeOrigins`, `-VerifyBufferAdapters`, and `-RunCallbackCorpusTests` perform different extra actions. Inspect their implementations and dependencies first. Copy-adapter verification executes accepted straight-line original bodies on private buffers. Isolated emulation and static byte proofs do not execute a live gameplay transaction.

Historical profiles are immutable. Read them as evidence anchors without editing them. Capture does not authorize new profile fields, remove old operations, or satisfy live semantic review by itself.

## Deliver the Evidence

Retain successful and partial reports, including failure reasons. Report exact paths, executable identity, PID/session where relevant, static versus live scope, registered/runtime/template counts, ambiguities, and which optional checks ran. Do not call an offline capture a working client/server profile or claim a complete engine API from compiler-range coverage.
