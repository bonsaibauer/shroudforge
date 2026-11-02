---
{
  "id": "api-native",
  "locale": "en",
  "title": "Call your mod's own DLL from Lua",
  "summary": "A mod DLL can register functions that the same mod's Lua code calls through shroudforge.native.",
  "navigation": {
    "path": [
      "Lua API guides",
      "Feature guides"
    ]
  },
  "apiPrefixes": [
    "shroudforge.native."
  ]
}
---

# Call your mod's own DLL from Lua

A mod can ship a DLL in its folder, declared in `native-plugin.ini` or registered with `loader.runtime.register_dll`. ShroudForge loads it once per game process. When the DLL exports `ShroudforgeModNativeInit`, ShroudForge calls it right after the load. The DLL registers named functions there, and the same mod's Lua code calls them through `shroudforge.native`.

The bridge connects two parts of the same mod. A mod reaches only the functions its own DLLs registered, never another mod's. The only game data the host gives the DLL is the local camera, read only, under the same conditions as the Lua API (see below). A DLL without the export is loaded exactly as before.

## The DLL side

Copy `modules/mod-runtime/native_bridge/shroudforge_mod_native.h` from the ShroudForge repository into your DLL project. It is a plain C header.

```c
#include "shroudforge_mod_native.h"

static void SHROUDFORGE_MOD_NATIVE_CALL add(void* context,
    const ShroudforgeNativeValue* arguments, size_t count,
    ShroudforgeNativeResults* results)
{
    if (count != 2 || arguments[0].kind != SHROUDFORGE_NATIVE_INTEGER
        || arguments[1].kind != SHROUDFORGE_NATIVE_INTEGER) {
        results->fail(results, "add expects two integers");
        return;
    }
    ShroudforgeNativeValue sum = { SHROUDFORGE_NATIVE_INTEGER };
    sum.as.integer = arguments[0].as.integer + arguments[1].as.integer;
    results->push(results, &sum);
}

SHROUDFORGE_MOD_NATIVE_EXPORT int32_t SHROUDFORGE_MOD_NATIVE_CALL
ShroudforgeModNativeInit(const ShroudforgeModHost* host)
{
    if (host->abi_version < 1) return 0;
    host->log(host, SHROUDFORGE_NATIVE_LOG_INFO, "native part ready");
    return host->register_function(host, "add", add, NULL) == 1;
}
```

`host` stays valid until the process exits. It gives the owning mod's id and folder, a `log` function that writes to the ShroudForge log under that id from any thread, and `register_function`, which works only during `ShroudforgeModNativeInit`. When the export returns zero, the DLL Manager reports the DLL as failed and its functions are withdrawn.

## The Lua side

```lua
local sum, reason = shroudforge.native.call("add", 2, 40)
if sum == nil then
  shroudforge.log.warn("native add failed: " .. reason)
end
```

Arguments may be nil, booleans, integers, numbers, strings and buffers. Strings and buffers reach the native function without a copy and stay valid only during the call. The function returns up to 32 values. Bytes it returns arrive as Lua strings. A function that returns nothing gives `true`. A failure, or a name the DLL did not register, gives `nil` and a reason. `shroudforge.native.functions()` lists the registered names.

## Threads and timing

A call is synchronous. It runs on the mod runtime thread, never on the game thread, and the mod's update waits for it. Keep native functions short. Run long work on threads the DLL starts itself, and let Lua ask for the result on a later update, as with other ShroudForge operations that report a pending state.

Functions belong to the process like the DLL itself. They survive a reload of the mods' Lua code, so a reloaded mod finds them again.

A native function must not throw an exception or unwind into ShroudForge. A crash in the DLL ends the game process like any other native crash.

## The local camera

A DLL that draws every frame, for example as a Vulkan layer, cannot wait for the mod's Lua update to learn where the player looks. Since ABI 2, `host->read_local_camera` reads the local player's camera from the same input sample as `runtime.world.cursor.get`, on any thread:

```c
double view[SHROUDFORGE_MOD_NATIVE_VIEW_VALUES];
if (host->abi_version >= 2
    && host->read_local_camera(host, view, SHROUDFORGE_MOD_NATIVE_VIEW_VALUES) == 1) {
    /* view[0..2] position in meters, view[3..6] orientation,
       view[7] fovY, view[8] aspect ratio, then the building cursor. */
}
```

It answers only while the mod is an active ingame runtime mod with the `runtime` capability, in a game client, which are the conditions of the Lua runtime API. Result 7 means the mod has no runtime access at that moment. The field layout comes from the game's reflection and becomes known once a `runtime.world.cursor.get` call decoded the camera, so result 3 is expected until then. The header lists every result code.
