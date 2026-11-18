# Native plugin lifecycle

Existing DLL Manager selections and `register_dll` libraries retain their
process lifetime and optional DLL-to-Lua bridge. A manifest-backed runtime
package can opt into managed callbacks with `native-plugin.ini`:

```ini
[Plugin]
Enabled=true
Dll=bin/plugin.dll
Lifecycle=shroudforge-v1
Services=shroudforge.map,shroudforge.overlay,shroudforge.executable-spans,shroudforge.keybinds,shroudforge.notifications
```

Declare `runtime-register-dll` and `runtime` capabilities in `mod.json`. The
DLL must export `ShroudForgePluginEntry` with the C layout and calling
convention in [native_plugin_api.h](../../modules/mod-runtime/native_plugin_api.h).
The optional executable-span service uses the standalone
[executable_span_api.h](../../modules/game-runtime/native/abi/executable_span_api.h)
contract.
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

The executable-span validation registry retains drained generation tombstones
so an old generation cannot be reused. It keeps at most 4,096 distinct package
generations for the process lifetime. Once that limit is reached, queries for
new validation-service generations return BUSY until the process restarts.

Before starting a managed DLL worker, the host pins its own runtime module for
the process lifetime. Pending workers and retained callback contexts therefore
remain backed by mapped code even after the bootstrap releases its reference.
This also applies to plans containing only managed plugins.

## Services

- `shroudforge.keybinds` v1 reads the package's declared keybind settings. It
  requires an active client runtime package and respects input suppression.
- `shroudforge.notifications` v1 publishes bounded, package-namespaced notices.
  The persisted feed remains authoritative when an optional HUD presenter is
  unavailable.
- `shroudforge.executable-spans` v1 is a client-only, read-only query for a
  bounded executable byte span in the main image. It requires `runtime`,
  `runtime-register-dll` and `runtime-engine-hooks`. The caller supplies both
  the expected original bytes and its current observation; the host checks the
  observation against executable, readable memory before returning
  `UNCHANGED`, `VERIFIED_HOST_OWNED` or `FOREIGN_OR_UNAVAILABLE`. Invalid input
  or a changed observation returns an error without writing the result. The
  service never returns code bytes or addresses and never authorizes a patch or
  proves quiescence.
- shroudforge.executable-span-validation v1 is a client-only, bounded
  callback service for package-owned engine guards. It requires an enabled
  package with runtime, runtime-register-dll and runtime-engine-hooks.
  Registrations are limited to 16 spans per package generation and 64 total;
  callbacks must stay loaded until revoke and drain succeed. Callbacks can
  run synchronously on any thread and must not call host services, retain
  request pointers, or modify executable memory. The host compares the
  registered prefix through a local buffer while preserving every guard
  suffix byte-for-byte.
- `shroudforge.map` v1 and `shroudforge.overlay` v1/v2/v3 are optional native-provider
  services. Query their exact version and handle `UNAVAILABLE`. Host support
  does not establish that a particular game build supplies the service.
  Query, revoke and drain must all be exported before the bridge exposes this
  capability. An incomplete provider cannot hand out a service table.
  Overlay v3 adds bounded fragment programs; its drawing contract is documented
  in [Native map and overlay services](../runtime/native-map-services.en.md).

Lua runtime APIs expose bounded map snapshots through the optional provider.
Gameplay controls and mod-specific compatibility adapters belong to mod packages.
Neither a successful DLL load nor local tests establish game-build or server
compatibility. Provider preparation must complete before the game entrypoint
is released; `native_overlay_startup_demand` reports the selected overlay demand.

## Validation

Use the existing Rust unit-test targets. The native host tests use synthetic
packages and callback tables; they do not load the game or require game files.
The C header also supports ordinary C11 consumers. Test service revocation,
owner identity and failure cleanup when adding a service or lifecycle change.
