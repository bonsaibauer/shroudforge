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

Declare `runtime-register-dll` and `runtime` capabilities in `mod.json`. Include
the existing [mod native SDK](../../modules/mod-runtime/native_bridge/shroudforge_mod_native.h)
and export `ShroudforgeModNativeInit`. Mod bridge ABI 3 appends
`register_lifecycle` to `ShroudforgeModHost`. During that one-time DLL
initialization, pass it the DLL's lifecycle factory. No second export is queried.

```c
static SFNativeStatusV1 SF_NATIVE_CALL create_activation(
    uint32_t requested_abi, uint32_t out_size, SFNativePluginV1 *out_plugin);

SHROUDFORGE_MOD_NATIVE_EXPORT int32_t SHROUDFORGE_MOD_NATIVE_CALL
ShroudforgeModNativeInit(const ShroudforgeModHost *host)
{
    if (!host || host->abi_version < 3u || host->size < sizeof(*host)
        || !host->register_lifecycle) return 0;
    return host->register_lifecycle(host, create_activation) == 1u;
}
```

`create_activation` validates the requested version, pointer and capacity before
writing the callback table described in the SDK. It only supplies callbacks.
The runtime calls it later on the activation owner thread. Returning an error
or an invalid table prevents activation. Registering it does not start the mod
or grant access to the game. Duplicate registration returns 3, registration
outside DLL initialization returns 4, and invalid arguments return 2. The bridge
withdraws both the factory and that DLL's Lua functions if DLL initialization
fails. The factory is stored in the same process-owned DLL record as its load
result. Reloading Lua does not rerun DLL initialization or factory registration.

The process-lifetime `ShroudforgeModHost` and activation-lifetime `SFNativeHostV1`
have different lifetimes. The first registers the DLL's integration once. The
second supplies settings, package reads and services to that activation's
callbacks, with its package identity and generation. Its services expire when
the activation is revoked and drained. Neither table is a separate loader.

Existing mod bridge ABI 1/2 fields retain their offsets and behavior. DLLs
that register only Lua functions or do not use the optional init export keep
their existing path. Consumers of the unpublished `native_plugin_api.h` draft
must replace that include with this SDK and register their former
`ShroudForgePluginEntry` implementation as a local factory through the existing
init export. That old export is no longer queried. The additional `Lifecycle`
and `Services` declarations shown above still
select preparation in this intermediate PR state. Their removal and integration
with the common package lifecycle are tracked in [issue #149](https://github.com/bonsaibauer/shroudforge/issues/149).

The optional executable-span service uses the standalone
[executable_span_api.h](../../modules/game-runtime/native/abi/executable_span_api.h)
contract.
The registered factory supplies a table. Initialization work belongs in its `initialize`
callback. A DLL with an invalid explicit lifecycle declaration is rejected,
including through a profile or Lua registration. It cannot fall back to an
ordinary load. Other DLLs in the same package retain their selected load mode.

All DLL loads and optional `ShroudforgeModNativeInit` calls run on the existing
ordered process worker in `native_dlls.rs`. Selected DLLs, Lua registrations and
managed declarations share canonical-path deduplication and the same load
result. Initialization and updates run on the runtime owner thread after that
asynchronous load. Settings and package asset reads use bounded UTF-8 byte spans.
Short output buffers report the required size without partial writes. Paths
are relative to the owning package, including for archived packages.
DLL binaries use the existing resolver for installed, unpacked mod folders.
The runtime does not extract another native package copy.

Each activation has a new owner generation. Shutdown revokes services, drains
in-flight callbacks, stops the plugin, calls shutdown and drains again before
releasing the activation. DLL code stays owned by the process executor until
exit, including after an initialization error. A failed drain or callback also
retains its owner context. The runtime reports that a restart is required.
Plugins must stop their workers and release host pointers before returning from
shutdown. A pending activation cancelled during disable cannot initialize later. Cancellation
does not cancel an OS load already queued or release the process-owned module.
When reactivation is supported, it reuses the recorded load and bridge result.
A failed bridge initialization is never retried in the same process.

A V1 activation that completed shutdown and both service drains releases its
activation record. A later activation invokes the same registered factory with
a fresh host context and generation, without loading or initializing the DLL
again. Its previous service tables remain revoked. The factory must provide a
valid context for each activation. Failed cleanup retains the activation and
rejects reactivation with a restart-required error. V2 preparation still belongs
to process startup and cannot be repeated in place.

The executable-span validation registry retains drained generation tombstones
so an old generation cannot be reused. It keeps at most 4,096 distinct package
generations for the process lifetime. Once that limit is reached, queries for
new validation-service generations return BUSY until the process restarts.

Before starting the common DLL worker, the host pins its own runtime module for
the process lifetime. Pending loads and retained callback contexts therefore
remain backed by mapped code even after the bootstrap releases its reference.
This also applies to plans containing only managed plugins.

## Services

- `shroudforge.keybinds` v1 reads the package's declared keybind settings. It
  uses the same SDL input, focus, extra-modifier and capture-suppression rules
  as `shroudforge.input`. The map service has no separate raw-key reader.
- `shroudforge.notifications` v1 publishes bounded, package-namespaced notices.
  Lua and native notices share the notifications module's validation and
  persistence. The map/overlay service does not publish or store notices.
  The persisted feed remains authoritative when an optional HUD presenter is
  unavailable.
- `shroudforge.executable-spans` v1 is a client-only, read-only query for a
  bounded executable byte span in the main image. An enabled package needs
  `runtime`, `runtime-register-dll` and `runtime-executable-spans`. The caller supplies both
  the expected original bytes and its current observation; the host checks the
  observation against executable, readable memory before returning
  `UNCHANGED`, `VERIFIED_HOST_OWNED` or `FOREIGN_OR_UNAVAILABLE`. Invalid input
  or a changed observation returns an error without writing the result. The
  service never returns code bytes or addresses and never authorizes a patch or
  proves quiescence.
- shroudforge.executable-span-validation v1 is a client-only, bounded
  callback service for package-owned engine guards. It requires an enabled
  package with `runtime`, `runtime-register-dll` and
  `runtime-executable-span-validation`. The broader `runtime-engine-hooks`
  permission remains valid for both span services.
  Registrations are limited to 16 spans per package generation and 64 total;
  callbacks must stay loaded until revoke and drain succeed. Callbacks can
  run synchronously on any thread and must not call host services, retain
  request pointers, or modify executable memory. The host compares the
  registered prefix through a local buffer while preserving every guard
  suffix byte-for-byte.
  Version 2 adds a read-only lease for a prefix of an existing registration.
  Acquire it during Stage. Callbacks may use the copied lease across threads.
  Verification checks the observed span and its suffix against the live
  owner generation. Revoking the package or retiring its generation invalidates
  the lease. The host table retains its owner-thread restrictions.
- `shroudforge.map` v1 and `shroudforge.overlay` v1/v2/v3 are optional native-provider
  services. Query their exact version and handle `UNAVAILABLE`. Host support
  does not establish that a particular game build supplies the service.
  Query, revoke and drain must all be exported before the bridge exposes this
  capability. An incomplete provider cannot hand out a service table.
  Overlay v3 adds bounded fragment programs; its drawing contract is documented
  in [Native map and overlay services](../runtime/native-map-services.en.md).

Gameplay controls and mod-specific compatibility adapters belong to mod packages.
Neither a successful DLL load nor local tests establish game-build or server
compatibility. Provider preparation must complete before the game entrypoint
is released; `native_overlay_startup_demand` reports the selected overlay demand.

## Validation

Use the existing Rust unit-test targets. The native host tests use synthetic
packages and callback tables; they do not load the game or require game files.
The C header also supports ordinary C11 consumers. Test service revocation,
owner identity and failure cleanup when adding a service or lifecycle change.
