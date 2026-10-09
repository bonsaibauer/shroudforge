# Native runtime and game profiles

The native runtime is a built-in part of ShroudForge. It provides a small set of approved operations for mods while Enshrouded is running. Players do not install it separately.

The [registry API](runtime-registry.md) exposes the complete reflected type
registry, owned value codecs, engine component registrations and native procedure
metadata. Component identity can now be resolved directly from the live engine
registry; typed native operations still require a verified calling contract.

This guide brings the runtime, its shared interface, and game profile workflow together. It is for contributors working on the loader.

## How the pieces fit together

The loader is written in Rust, and the native runtime is written in C++. A shared set of rules lets them call each other. Developers call these rules an **ABI** (Application Binary Interface).

~~~mermaid
flowchart LR
    A[Enshrouded build] --> B[Maintainer tools inspect the build]
    B --> C[Review game profile and component map]
    C --> D[Approve profile in the source tree]
    D --> E[Build includes profile in the runtime]
    E --> F[Windows bridge checks ABI version]
    F --> G[Loader reports runtime health]
~~~ 

At startup, the Windows bridge finds **kfc-runtime.dll** and checks its ABI version before calling runtime functions. If the version is not supported, the bridge stops safely. The loader then checks the active profile and reports runtime health.

The runtime is built and shipped with ShroudForge. There is no separate runtime download or release.

### Asset preparation before game startup

The startup proxy installs a temporary gate at the executable entrypoint. The
main thread waits there, outside the Windows loader lock, while the bootstrap
worker prepares the asset files. It restores the original entrypoint instructions
before letting the game read those files. Runtime initialization continues on the
worker thread.

An asset-mod error restores both saved baseline files and allows the game to
start without asset patches. The failed preparation remains visible in mod
status and the log. Startup stops only if preparation fails and the baseline
cannot be restored safely. Runtime mods with export permission do not trigger
another pregame export pass or run their entrypoints during asset preparation.

`src/bootstrap/windows/tests/run.ps1` verifies entrypoint ordering, DLL loading
outside loader lock, instruction restoration, and the unrecoverable-failure path.
The ignored `blueprint_startup_matches_legacy_and_recovers_from_mod_failure` Rust
test uses `SF_BLUEPRINT_GAME_SOURCE` to copy a client or server installation's
executable and asset backups into a temporary directory. It compares the current
and historical blueprint patches and verifies recovery after an intentional Lua
error, without changing the installation.

## The shared interface

The official interface is declared in [runtime.h](../../src/loader/runtime/native/runtime.h). It describes the names of calls and the exact shape of the information passed between the bridge and runtime. There is no second hand-maintained list.

**KfcRuntimeAbi()** returns the interface version. If a change breaks compatibility, the version must be increased and the ShroudForge loader must be updated to match.

The interface includes startup and status calls, access to supported ECS data, profile-backed world operations, and guarded patch operations. **ECS** is how Enshrouded stores game objects and their properties.

Some game values have a layout that depends on the active profile. The bridge treats these values as raw data. Only operations supported by the reviewed profile should be made available to mods.

**KfcRuntimeStatus** returns a short health message. **KfcRuntimeDiagnostics** returns a size-limited JSON report with a schema version. The loader diagnostics module adds mod status, session controls, and measurements shown in the user interface.

## Game profiles

A **profile** is reviewed information for one Enshrouded game version and target, such as the client. The runtime uses it to identify supported game structures and decide which operations are available.

One complete profile per build lives under:

~~~text
src/loader/runtime/profiles/<game>/<target>/<build>.json
~~~

For example, the profile for Enshrouded client build **1076226** is under **src/loader/runtime/profiles/enshrouded/client/**.

Only reviewed information needed by approved operations belongs in a release profile. Scan notes, raw captures, and unapproved values are investigation material; keep them in the maintainer tools area or local **devdata/**.

Before approval, check the game and build identity, where the information came from, the component map, and the required observations from the live game. The launcher installs profile files under `shroudforge/runtime/profiles/`, selects the matching profile automatically, and lets the player choose another profile. A different detected build produces a warning that problems may occur; verified signatures and live layout checks still decide which operations work.

Maintainer tools live under **src/loader/runtime/profile-tools/**. Start with the [profile development guide](../../src/loader/runtime/profile-tools/dev/README.md).

The [runtime discovery tools](../../src/loader/runtime/profile-tools/dev/DISCOVERY.md)
extract all reflected types and inspect client/server code and live tables per
executable SHA-256. Lua resolves reflected types by qualified name and can pass
those names or returned `Type` objects to ECS query/read/write operations. `runtime.ecs.get_catalog()`
reports reflected candidates and their current mapping/access status, including
unresolved components. Discovering a reflected type does not establish a native
function's calling convention or effects.

## Where the source lives

| Path | What it contains |
| --- | --- |
| **src/loader/runtime/native/runtime.h** | The shared interface. |
| **src/loader/runtime/native/** | C++ runtime source and build files. |
| **src/loader/runtime/profiles/** | Reviewed game profiles. |
| **src/loader/runtime/profile-tools/** | Maintainer tools for examining profiles. |
| **src/loader/modules/runtime-diagnostics/** | Status and measurements shown by the loader. |

The root **CMakeLists.txt** builds the runtime. **build.ps1** assembles the complete Windows release.

### World session identity

`runtime.world.session_id()` returns a nonzero, process-local ECS session marker,
or `0` while the active entity table is unavailable or has not been adopted.
It requires the same runtime read access as `runtime.ecs.query`. The optional
native export is `KfcRuntimeWorldSessionId`; old providers return `0` through
the loader. This is a conservative lifetime token, not a persistent world ID:
an ECS layout rebuild also invalidates it. Retain the token with undo journals
and reject work if it is unavailable or changed; do not treat missing handles
from an invalidated session as proof that an old placement was removed.
