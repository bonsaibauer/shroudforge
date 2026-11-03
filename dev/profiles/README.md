# Game build and runtime profile actions

This directory contains the native C++, Rust and PowerShell development actions and capture data. The Python implementations live together under [`modules/runtime-diagnostics/diagnostics-jobs/game_inspection/`](../../modules/runtime-diagnostics/diagnostics-jobs/game_inspection/). Their subdirectories and shared imports are preserved. The actions and their inputs are listed in the [game build and runtime profile guide](../../docs/development/inspect-game-build-and-create-runtime-profile.de.md).

In the tables below, Python tool paths are relative to that `game_inspection/` source folder. Native files and PowerShell actions remain relative to this directory. Python tests are linked to their module location. Catalogs, schemas and local captures remain here.

## Native developer CLI

| File | Action |
| --- | --- |
| `cli/kfc_runtime_dev.cpp` | Starts the `kfc-runtime-dev` command line and dispatches its subcommands. |
| `cli/profile_cli.h` | Declares the shared command context, option types, and command handlers. |
| `cli/read_cli_option.cpp` | Reads named command-line options and their values. |
| `cli/inspect_executable.cpp` | Writes a report about an executable's PE identity and sections. |
| `cli/select_game_build.cpp` | Selects client or server targets and locates exact build catalogs. |
| `cli/discover_function_candidates.cpp` | Finds heuristic function ranges and candidate addresses in a PE image. |
| `cli/scan_function_signatures.cpp` | Scans an executable for cataloged byte signatures. |
| `cli/export_profile_function_catalog.cpp` | Exports a profile's hooks and operations as a function catalog. |
| `cli/capture_process_modules.cpp` | Records a process image path and its loaded modules. |
| `cli/import_legacy_ecs_capture.cpp` | Converts an older tabular ECS capture to the current catalog format. |
| `cli/inspect_provider_abi.cpp` | Checks a provider DLL's ABI version and exports. |
| `cli/create_runtime_profile_draft.cpp` | Creates an unapproved profile draft for review. |
| `cli/validate_runtime_profile.cpp` | Validates profile structure, offsets, operations, and component data. |
| `cli/approve_runtime_profile.cpp` | Checks recorded evidence and requests confirmation before marking a profile approved. |
| `cli/read_pe_image.cpp` | Reads PE file identity, sections, and executable ranges for the native tools. |
| `cli/read_pe_image.h` | Declares the native PE image reader and its data types. |
| `cli/match_function_signatures.cpp` | Parses byte patterns and searches executable ranges for matches. |
| `cli/match_function_signatures.h` | Declares the native byte-pattern matcher. |
| `cli/write_json_report.cpp` | Writes formatted JSON reports for the native commands. |

## Capture tools

| File | Action |
| --- | --- |
| `capture/capture_runtime.ps1` | Orchestrates the selected client or server capture and optional verification steps. |
| `capture/discover_runtime_build.py` | Extracts executable reflection and optional live process, function, component, and system data. |
| `capture/capture_ecs_registry.py` | Discovers ECS registrations and can export a component catalog. |
| `capture/capture_factory_state.py` | Reads factory and linked inventory observations from a running process. |
| `capture/read_process_memory.py` | Provides read-only process memory access and live memory capture. |
| `capture/discover_component_registry.py` | Discovers and validates component registrations, managers, and storage layouts. |
| `capture/find_entity_managers.cpp` | Searches process memory for entity manager candidates and checks their structure. |
| `capture/capture_legacy_component_registry.cpp` | Finds and prints the older ECS component table. |
| `capture/find_live_type_references.cpp` | Counts live process references to selected reflected types. |
| `capture/export_observed_registry.py` | Exports a unique registry already observed by the production reader. |
| `capture/export_runtime_bindings.py` | Exports a runtime discovery catalog and a compact build index. |

## Executable and resource inspection

| File | Action |
| --- | --- |
| `inspect/read_pe_image.py` | Reads PE sections, RVAs, and executable identity for Python tools. |
| `inspect/extract_executable_reflection.py` | Extracts the reflected type catalog directly from an executable. |
| `inspect/discover_executable_functions.py` | Disassembles PE function ranges and records static code references. |
| `inspect/read_system_descriptors.py` | Reads named ECS descriptors, read/write service names, direct calls and compiler decoding boundaries. Select full original names with repeatable `--system` and reflected field paths with `--layout TYPE FIELD...`. Preserves the shared `discover(pe, functions)` API. Offline only. |
| `inspect/inspect_inventory_transactions.py` | Resolves inventory quantity removal, partial source removal, and usage-cost relationships from original named descriptors, reflected layouts, and decoded instructions. It reports evidence without calling or changing the game. |
| `inspect/inspect_component_metadata.cpp` | Examines component metadata fields against component indices. |
| `inspect/export_kfc_resources.rs` | Exports selected KFC resources through the parser. |

## Build comparison

| File | Action |
| --- | --- |
| `compare/compare_runtime_builds.py` | Compares layouts and function evidence from two game builds. |
| `compare/prepare_profile_rebase.py` | Copies a complete same-target baseline into an external candidate, records every changed leaf, ranks static RVA evidence, and lists unresolved target checks. Optional numeric review plans are bound to both exact images and hashed evidence artifacts. |
| `compare/rank_cross_build_functions.py` | Ranks static function candidates across builds without authorizing calls. |
| `compare/compare_recipe_resources.py` | Compares recipe resources from exported captures. |

## Capture analysis

| File | Action |
| --- | --- |
| `analyze/index_captured_functions.py` | Indexes captured function ranges, calls, descriptors, and type references. |
| `analyze/find_runtime_type.py` | Finds captured types by name or hash and reports all matching locations. |
| `analyze/trace_function_anchors.py` | Traces signature matches to functions and their static callers. |
| `analyze/read_capture_reports.py` | Reads related reports and rejects captures from different executable identities. |
| `analyze/write_inspection_result.py` | Writes analysis results as JSON or to the console. |

## Verification

| File | Action |
| --- | --- |
| `verify/validate_patch_payload.py` | Checks patch payload lengths and embedded references. |
| `verify/verify_runtime_origins.py` | Coordinates profile patch, world operation, and hook origin checks. |
| `verify/verify_runtime_patch_origins.py` | Checks patch origins, argument evidence, and caller paths. |
| `verify/verify_runtime_hook_origins.py` | Checks hook signatures and complete instruction boundaries. |
| `verify/verify_world_operation_origins.py` | Checks world operation addresses and guards. |
| `verify/audit_live_component_registry.ps1` | Runs the native live registry audit and saves its log and candidate mappings. |
| `verify/audit_live_component_registry.cpp` | Audits live registry tables, guards, and candidate mappings. |

## Explicit checks

| File | Action |
| --- | --- |
| `checks/generate_attribute_reference_vectors.py` | Generates isolated emulator vectors for original game attribute interpreters. |
| `checks/run_callback_corpus_tests.ps1` | Runs the existing callback corpus test against a selected registry capture. |
| `checks/sample_entity_manager_registers.cpp` | Samples entity manager registrations from a live process. |
| `checks/verify_building_event_arguments.py` | Verifies original building event argument mappings in an isolated emulator. |
| `checks/verify_building_input_layout.py` | Verifies the client building input layout and hook contract. |
| `checks/verify_component_copy_adapters.py` | Differentially checks component copy adapters on private buffers. |
| `checks/verify_component_registry.cpp` | Verifies a live component registry against production registry rules. |
| `checks/verify_cursor_voxel_context.py` | Checks the cursor-to-voxel context data flow in the original executable. |
| `checks/verify_inventory_decrement.py` | Emulates the original inventory decrement helpers on isolated data, using established image catalogs or the structural inventory inspection report. Engine dependencies remain stubs. |

## Tool tests

| File | Action |
| --- | --- |
| [`tests/python/test_components.py`](../../modules/runtime-diagnostics/tests/python/test_components.py) | Tests component registry parsing and validation with independent fixtures. |
| [`tests/python/test_discovery.py`](../../modules/runtime-diagnostics/tests/python/test_discovery.py) | Tests PE, reflection, function, system, and anchor discovery with synthetic files. |
| [`tests/python/test_mod_origins.py`](../../modules/runtime-diagnostics/tests/python/test_mod_origins.py) | Tests that known invalid runtime patch payloads fail validation. |
| [`tests/python/test_profile_rebase.py`](../../modules/runtime-diagnostics/tests/python/test_profile_rebase.py) | Tests exact identity and target separation, complete baseline preservation, ambiguity, evidence-bound numeric edits, and production-output rejection. |

## Shared data and boundaries

| Location | Contents |
| --- | --- |
| `catalogs/` | Maintained function signatures and captured build indexes. |
| `schemas/` | Contracts for profiles, function catalogs, and component catalogs. |
| `captures/` | Ignored local input files, original executable copies, capture results, and drafts. |
| [`game_inspection/requirements.txt`](../../modules/runtime-diagnostics/diagnostics-jobs/game_inspection/requirements.txt) | Python dependencies beside the diagnostic source package. |
| `../../modules/game-runtime/profiles/` | Approved runtime profiles shipped with the native runtime. |

The native CLI is compiled directly from `modules/game-runtime/native/CMakeLists.txt`. Its build targets and profile-directory definition point to these source folders and the shared catalog root. Python tools use the `game_inspection.*` package under the diagnostic job root. Direct file calls and the existing PowerShell capture action use the same implementations.

Profile approval remains tied to the exact executable identity and its recorded evidence. A function match is not approval to call an engine function.

`capture/capture_runtime.ps1` selects a unique exact profile automatically. For a new image, `-ClientBaselineProfile` and `-ServerBaselineProfile` choose explicit review anchors. Otherwise it uses the latest numeric build ID for the same target and KFC format, with ambiguous latest IDs rejected. `profile-selection.json` records the choice and distinguishes exact identity from a review baseline. No baseline selection authorizes runtime calls.

The shipped JSON catalog under the installation's configured `runtime/profiles` directory is the runtime profile source. Profiles are not embedded into the native provider. Data-only profile updates do not require rebuilding the DLL. The [profile guide](../../docs/development/inspect-game-build-and-create-runtime-profile.de.md) documents complete-copy preparation and its remaining validation requirements.

The optional native [`game-profile-discovery`](../../modules/game-profile-discovery/README.md) module creates `beta_*` profiles only after the complete profile contract has been established from exact old/new executable images. It compares mapped sections or derives original function, data and layout relations independently. Normal profiles have priority. Missing or ambiguous evidence produces no partial profile. The capture/rebase review tools remain available for investigating those unresolved fields.

## World Editor diagnostic jobs

Live World Editor definitions and Lua scripts live together under [`modules/runtime-diagnostics/diagnostics-jobs/world-editor/`](../../modules/runtime-diagnostics/diagnostics-jobs/world-editor/). Runtime collector definitions use `runtime/`, Python definitions and implementations use `game_inspection/`. All jobs share one schema, parser and lifecycle. The six World Editor IDs are `world-editor.api-read-ab`, `world-editor.typed-inputs`, `world-editor.registered-impact-ab`, `world-editor.building-systems`, `world-editor.cursor-type-references` and `world-editor.voxel-write-ab`. The game-inspection jobs also include `inspect.system-dependencies`, which uses the production Rust resolver on exact-matching reports from `capture.runtime`. Select and start jobs through the existing Runtime Diagnostics controls or CLI. Lua and registered mod-action execution retain the active mod's normal permissions and do not load mods. The voxel job changes one cell in a disposable world and is never started just by appearing in the catalog.

Job reports and recovery files belong to the configured Export folder under `diagnostics/<job-id>/`. Job progress appears in the Debug Log. These exports are separate from the ignored developer captures. Existing captures, earlier export reports and the active installation are not moved or rewritten by this source migration. The [diagnostic job guide](../../modules/runtime-diagnostics/diagnostics-jobs/README.md) documents lifecycle, mod calls and export behavior.

The former `inspect_world_editor_operations.py` offline capabilities are consolidated in the existing generic `game_inspection/inspect/read_system_descriptors.py`. For example, from the repository root:

```powershell
python modules/runtime-diagnostics/diagnostics-jobs/game_inspection/inspect/read_system_descriptors.py <executable> --capture <capture-directory> --system player_building_blueprints --system player_building_undo --layout keen::ecs::ClientPlayerInput data cursorInput
```

This action retains exact original system names, service access roles and ordinals, direct call edges, reflected layouts and explicit decoding gaps. It opens no process. The old inspector's raw live code comparisons, owner validation, entity membership, component stride checks and service-binding comparisons are not claimed as equivalent API-job coverage. The unused `watch_entity_command_queues.py` fixed-layout watcher is removed. Its raw queue observations remain historical capture evidence rather than an API measurement.
