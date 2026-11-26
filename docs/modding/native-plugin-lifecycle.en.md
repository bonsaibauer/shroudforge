# Native mods through the existing loader and API

All DLLs use the same ordered process executor in `mod-runtime/native_dlls.rs`.
Choose a package DLL in the DLL Manager, use an existing `[Plugin] Dll=` declaration,
or call `loader.runtime.register_dll` from its package. Existing DLL Manager
approval, activation, target, dependencies, library/chain mode and path rules apply.
`Lifecycle=` and `Services=` INI keys are ignored. No other plugin loader exists.
Ordinary DLLs retain their own startup and stay loaded until process exit.

A package DLL may export `ShroudforgeModNativeInit`, called once after loading
on the existing DLL worker. The [SDK](../../modules/mod-runtime/native_bridge/shroudforge_mod_native.h)
lets it register functions which only its own Lua reaches through
`shroudforge.native.call`. Registration supplies functions. It does not run
gameplay callbacks or grant game access. Failed init withdraws registrations.

## One optional activation contract

Use SDK host ABI >=3 and its `register_lifecycle` field for a V1 factory:

```c
static SFNativeStatusV1 SF_NATIVE_CALL create_activation(
    uint32_t requested_abi, uint32_t out_size, SFNativePluginV1 *out_plugin);

SHROUDFORGE_MOD_NATIVE_EXPORT int32_t SHROUDFORGE_MOD_NATIVE_CALL
ShroudforgeModNativeInit(const ShroudforgeModHost *host)
{
    if (!host || host->abi_version < 3u
        || host->size < offsetof(ShroudforgeModHost, register_lifecycle) + sizeof(host->register_lifecycle)
        || !host->register_lifecycle) return 0;
    return host->register_lifecycle(host, create_activation) == 1u;
}
```

The factory validates ABI 1, output pointer and size before writing the table.
The common runtime calls it later on its control thread. It supplies initialize,
update, stop and shutdown callbacks, without running initialization in the factory.
One DLL registration is accepted in its init window. Results: 1 registered,
2 invalid argument, 3 duplicate, 4 window closed. A package has one callback owner.
Helpers and ordinary DLLs keep their normal loading behavior.

ABI 1/2/3 SDK fields retain their offsets. ABI4's unpublished preentry-registration
tail is reserved and always returns 0. Package Stage/Prepare/entry-restored
callbacks and public executable-span approval services are not supported.
Draft consumers must move game-specific hook requests to reviewed framework
operations backed by profiles, then use the ordinary activation and service API.
`ShroudForgePluginEntry` is not queried. No INI setting can opt into another lifecycle.

## Shared package lifecycle

`mod-runtime/ingame/` owns the dependency-aware package lifecycle for Lua and
native callbacks. A native-only package with registered callbacks needs no dummy
Lua script. A mixed package uses its ordinary Lua lifecycle table.

Activation creates one package generation, calls Lua on_load when present, then
native initialize. Updates use the same traversal, due Lua on_update followed by
native update. A failure uses the common package rollback. Live settings consume
the same canonical package values. Disabled dependencies block both languages.

Cleanup calls Lua on_unload, restores automatic overrides, releases input
reservations, revokes/drains any Lua-used map/overlay owner, then tears down the
native activation. Native teardown revokes, drains, stops, shuts down and drains
again. The package scope stays active for retryable cleanup until all steps
finish. Incomplete cleanup suppresses regular updates and retains native code
and context. It does not count as successful unload.

Lua and DLL service requests from the same activation share package identity and
generation. An unused Lua API creates no service tombstone. A fresh activation
gets a new generation and invokes the existing recorded factory, without loading
or initializing the process DLL again. Old service tables stay revoked.
Pending OS loads cannot be cancelled, but a cancelled activation cannot start
later. Modules remain process-owned and mapped until exit.

The process-lifetime ShroudforgeModHost registers integration once. The
activation-lifetime SFNativeHostV1 supplies bounded settings JSON, package-relative
asset reads and query/revoke/drain services. It expires after teardown. All host
control calls are owner-thread-only. Native drawing callbacks have the stricter
renderer-thread/span contract documented in the map/overlay header. DLL workers
must stop using activation host pointers before shutdown returns.

## Common services and preparation

Declare `runtime` and the operation's capability in the ordinary mod.json.
`runtime-map` requests shared map preparation, `runtime-overlay` requests shared
graphics preparation, for Lua and native packages alike. A Lua call registering
a DLL keeps its existing runtime-register-dll permission. No Services list or
extra execution permission is added for internal preparation.

- shroudforge.map v1 exposes acquire/release on the same store as
  runtime.world.map.get, runtime.world.environment.get and runtime.world.hud.get.
- shroudforge.overlay v1/v2/v3 exposes synchronous native frame callbacks.
  shroudforge.overlay.scene v1 accepts copied JSON scenes, exactly as
  runtime.overlay.submit/clear. Both use one renderer and owner retirement.
  Use either callbacks or a copied scene within one package generation.
- shroudforge.keybinds v1 uses declared keybind settings and the same SDL,
  focus, conflict and capture rules as shroudforge.input. No raw VK reader exists.
- shroudforge.notifications v1 uses the notifications module's validation and
  persisted feed, also used by Lua. In-game presentation is an optional output.

The provider query/revoke/drain exports must all exist before a service table
escapes. Native map/overlay queries enforce their concrete capability before
forwarding. Static capability declarations never prove exact-build preparation,
world presence, fresh capture or actual rendering. Check service and view status.
Late graphics requests require restart when startup creation was not observed.

Only the framework prepares profile-bound hooks through its existing bootstrap
held-entry boundary and shared publisher. Ordinary third-party DLLs are not
globally banned, but foreign changes fail the affected operation's exact guards.
Mod-specific presentation, zoom, composition and behavior stay in mod packages.

See [Map/overlay and Lua API](../runtime/native-map-services.en.md) for data,
limits, generation and drawing contracts. Synthetic tests do not establish live
game, driver, multiplayer or server compatibility.
