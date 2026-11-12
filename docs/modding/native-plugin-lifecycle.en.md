# Native plugin lifecycle

Existing DLL Manager selections and `register_dll` libraries retain their
process lifetime and optional DLL-to-Lua bridge. A manifest-backed runtime
package can opt into managed callbacks with `native-plugin.ini`:

```ini
[Plugin]
Enabled=true
Dll=bin/plugin.dll
Lifecycle=shroudforge-v1
Services=shroudforge.map,shroudforge.overlay,shroudforge.keybinds,shroudforge.notifications
```

Declare `runtime-register-dll` and `runtime` capabilities in `mod.json`. The
DLL must export `ShroudForgePluginEntry` with the C layout and calling
convention in [native_plugin_api.h](../../modules/mod-runtime/native_plugin_api.h).
The entrypoint negotiates a table; initialize work belongs in its `initialize`
callback. A DLL with an invalid explicit lifecycle declaration is rejected,
including through a profile or Lua registration. It cannot fall back to an
ordinary load. Other DLLs in the same package retain their selected load mode.

Initialization and updates run on the runtime owner thread after asynchronous
library loading. Settings and package asset reads use bounded UTF-8 byte spans.
Short output buffers report the required size without partial writes. Paths
are relative to the owning package, including for archived packages.

Each activation has a new owner generation. Shutdown revokes services, drains
in-flight callbacks, stops the plugin, calls shutdown and drains again before
unloading code. A failed drain or callback retains the library and owner
context; the runtime reports that a restart is required. Plugins must stop
their workers and release host pointers before returning from shutdown. A
pending load cancelled during disable cannot initialize later.

## Services

- `shroudforge.keybinds` v1 reads the package's declared keybind settings. It
  requires an active client runtime package and respects input suppression.
- `shroudforge.notifications` v1 publishes bounded, package-namespaced notices.
  The persisted feed remains authoritative when an optional HUD presenter is
  unavailable.
- `shroudforge.map` v1 and `shroudforge.overlay` v1/v2 are optional native-provider
  services. Query their exact version and handle `UNAVAILABLE`. Host support
  does not establish that a particular game build supplies the service.

Lua runtime APIs expose map snapshots, First Person and Flight control through
the corresponding optional provider exports. Flight control retains its
bundled-package authority and requires the complete control/retirement ABI.
Neither a successful DLL load nor local tests establish game-build or server
compatibility. Provider preparation must complete before the game entrypoint
is released; `native_overlay_startup_demand` reports the selected overlay demand.

## Validation

Use the existing Rust unit-test targets. The native host tests use synthetic
packages and callback tables; they do not load the game or require game files.
The C header also supports ordinary C11 consumers. Test service revocation,
owner identity and failure cleanup when adding a service or lifecycle change.
