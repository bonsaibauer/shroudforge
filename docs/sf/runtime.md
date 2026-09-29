# Native runtime and game profiles

The native runtime is a built-in part of ShroudForge. It provides a small set of approved operations for mods while Enshrouded is running. Players do not install it separately.

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

## Where the source lives

| Path | What it contains |
| --- | --- |
| **src/loader/runtime/native/runtime.h** | The shared interface. |
| **src/loader/runtime/native/** | C++ runtime source and build files. |
| **src/loader/runtime/profiles/** | Reviewed game profiles. |
| **src/loader/runtime/profile-tools/** | Maintainer tools for examining profiles. |
| **src/loader/modules/runtime-diagnostics/** | Status and measurements shown by the loader. |

The root **CMakeLists.txt** builds the runtime. **build.ps1** assembles the complete Windows release.
