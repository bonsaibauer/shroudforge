# Runtime profile maintainer workflow

For complete reflected-registry extraction, live client/server table discovery,
hash lookups and cross-build code comparison, start with
[Runtime discovery](DISCOVERY.md). This complements the reviewed operation
catalogs below and records unresolved coverage explicitly.

This guide covers support for new Enshrouded executable builds. The runtime is
an internal ShroudForge loader component. The player build includes the native
provider DLL; capture tools and draft data are maintainer-only.

## Ownership and locations

| Data | Repository location | Release behavior |
| --- | --- | --- |
| C ABI and native implementation | `src/loader/runtime/native/` | ABI header stays in source; provider is compiled into `kfc-runtime.dll` |
| Approved game profiles and component maps | `src/loader/runtime/profiles/` | Embedded into the provider DLL |
| Profile schemas, catalogs and tools | `src/loader/runtime/profile-tools/dev/` | Not shipped to players |
| Local captures and unapproved drafts | `src/loader/runtime/profile-tools/devdata/` | Maintainer workspace only |

The ABI header [`runtime.h`](../../native/runtime.h) is canonical. `KfcRuntimeAbi()`
must match the value expected by the loader bridge before any provider function
is called. There is no separate ABI inventory or runtime package.

## Build the integrated runtime and optional tools

Use Windows x64 with CMake 3.24+ and MSVC from the repository root:

```powershell
cmake -S . -B build/native-runtime -A x64
cmake --build build/native-runtime --config Release --target kfc-runtime
cmake --build build/native-runtime --config Release --target kfc-runtime-dev
```

`build.ps1` invokes this root CMake project as part of the complete ShroudForge
build and stages only `kfc-runtime.dll` with the loader. Optional inspection
executables are excluded from player releases.
## Runtime profiles

Profiles are grouped by game, target, and game build. A complete per-build profile lives in one file:

```text
src/loader/runtime/profiles/enshrouded/client/1076226.json
```

The profile contains hook signatures, structural offsets, named world
operations, guarded patch definitions, and the ECS component map. CMake embeds
the complete profile in the DLL and the release also copies profiles to
`shroudforge/runtime/profiles/` for selection in the launcher. Automatic mode
uses executable identity. A manually selected profile with a different build is
still tried; the launcher warns that problems may occur, and each operation
must resolve against the running executable before use.

PE timestamp and image size are recorded for identity. Newly generated profiles
also record executable SHA-256 and require an exact hash match. The shipped
1076226 profile predates SHA-256 provenance and uses timestamp plus image size.

## Developer workflow for a new build

The developer tools run only while adding support for a new game build. They do
not run at game launch:

```powershell
$exe = 'D:\Games\Enshrouded\enshrouded.exe'
$capture = 'src\loader\runtime\profile-tools\devdata\enshrouded-client-new-build'
build\native-runtime\tools\kfc-runtime-dev.exe select-build $exe --out-dir $capture
build\native-runtime\tools\kfc-runtime-dev.exe extract-profile-functions `
  src\loader\runtime\profiles\enshrouded\client\1076226.json --out "$capture\functions.json"
```

`select-build` records PE identity, compiler-described x64 function ranges, and
heuristic leads. It scans an existing function catalog only when the selected
executable matches that catalog's PE timestamp and image size. For a new build,
review/update the extracted development catalog's signatures, expected RVAs,
original bytes, calling conventions, and side effects, then scan it:

```powershell
build\native-runtime\tools\kfc-runtime-dev.exe scan-functions $exe `
  "$capture\functions.json" --out "$capture\function-scan.json"
```

Function ranges and byte matches do not infer source names or prove call
semantics. A developer must review the actual functions and calling conventions.

Start the same game executable, load into a world, then capture the live ECS
registry and join it against `kfc-parser`'s `reflection_data.json`:

```powershell
$game = Get-Process enshrouded | Select-Object -First 1
$reflection = '<path to kfc-parser reflection_data.json>'
build\native-runtime\tools\kfc-runtime-capture-ecs.exe $game.Id 2>&1 | Tee-Object "$capture\ecs-capture.log"
build\native-runtime\tools\kfc-runtime-dev.exe import-ecs-capture "$capture\ecs-capture.log" `
  --image-report "$capture\image-report.json" --reflection $reflection `
  --id enshrouded-client-new-build --out "$capture\components-live.json"
```

The importer reads the parser's native `version`/`types` format (`qualifiedName`
and `size`) and also accepts the normalized `entries` format. Parser `version`
identifies KFC data, not the game executable build; pass `--id` to name the game
build. Any unresolved name/size joins block profile approval.

Generate and validate a draft entirely under `src/loader/runtime/profile-tools/devdata/`:

```powershell
$profileDraft = "$capture\new-build.json"
$componentsDraft = "$capture\enshrouded-client-new-build.components.json"
build\native-runtime\tools\kfc-runtime-dev.exe generate-profile `
  src\loader\runtime\profiles\enshrouded\client\1076226.json `
  "$capture\image-report.json" "$capture\function-scan.json" `
  --components "$capture\components-live.json" `
  --catalog-out $componentsDraft --out $profileDraft
build\native-runtime\tools\kfc-runtime-dev.exe validate-profile $profileDraft
build\native-runtime\tools\kfc-runtime-dev.exe approve-profile $profileDraft `
  --function-scan "$capture\function-scan.json" --components $componentsDraft
```

Generation applies unique scanned hook signatures, shifted world-function RVAs,
and patch target/function ranges to the draft. It keeps structural offsets and
global data RVAs as review items because a binary signature scan cannot infer
their meaning. Approval checks exact executable identity, unique byte matches,
zero unresolved ECS joins, structural validity, and explicit developer review
of live layouts and function behavior. An unapproved draft is rejected by the
runtime.

After approval, place the profile under the build's production profile path and
build/package the DLL:

```powershell
$profileDir = 'src\loader\runtime\profiles\enshrouded\client'
Copy-Item $profileDraft "$profileDir\new-build.json"
cmake --build build/native-runtime --config Release --target kfc-runtime
(No separate runtime package is produced.)
```

The approved profile is the only per-build runtime data. The profile uses
camelCase field names consistently; its sections are ordered as executable
identity, memory layout, hooks, world access, ECS components, and runtime code
patches. See `src/loader/runtime/profiles/README.md` for the field definitions.
Separate capture data
may remain under `profile-tools/devdata/` for review.
The function catalog, capture logs, reflection source, and draft stay in the
development area.

## Runtime behavior and limits

Automatic selection prefers an exact executable identity. A profile selected
manually is still tried when the running build differs, even if it did not opt
in to automatic structural fallback. The launcher reports that the build
differs and problems may occur. Every hook, byte signature, component layout,
and live access is checked before use; operations whose checks fail remain
unavailable. These checks do not prove that every engine semantic stayed the
same; new function semantics or calling conventions require profile review and
sometimes native implementation changes.

The provider retains hook trampolines and pins its DLL until process exit.
Shutdown stops new work and attempts to restore original code without freeing
addresses that may still be on engine thread stacks. Runtime binary updates take
effect after the game exits, not by hot-unloading. Timed-out writes may finish
after the caller times out, so hosts must reconcile state before retrying
non-idempotent operations.

## Enshrouded-specific inspection tools

The native inspection tools are kept in this repository because they gather
evidence for Enshrouded runtime profiles. They are developer-only tools, not
part of the public modloader API. Build them with the Development package or
individually with CMake:

```powershell
cmake --build build/native-runtime --config Release --target kfc-runtime-inspect-component-metadata
cmake --build build/native-runtime --config Release --target kfc-runtime-live-entity-manager-sample
cmake --build build/native-runtime --config Release --target kfc-runtime-live-entity-managers
cmake --build build/native-runtime --config Release --target kfc-runtime-live-type-references
cmake --build build/native-runtime --config Release --target kfc-runtime-audit-live-component-registry
```

These exploratory tools are read-only and their output is evidence to review,
not a compatibility verdict. For example, run the component registry audit
against a live world and an explicit type catalog:

```powershell
.\src\loader\runtime\profile-tools\dev\tools\enshrouded\run-live-component-registry-audit.ps1 `
  -TypeCatalog '<path to runtime-ecs-types.json>'
```

The audit saves its raw log and candidate mappings under `src/loader/runtime/profile-tools/devdata/`. It does not
edit a profile. Review candidate table matches and conflicts before changing a
development catalog or proposing a production profile entry.
