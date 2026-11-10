#pragma once

#include <stddef.h>
#include <stdint.h>

#if defined(__cplusplus)
extern "C" {
#endif

#if defined(_WIN32)
#define SHROUDFORGE_MOD_NATIVE_CALL __cdecl
#define SHROUDFORGE_MOD_NATIVE_EXPORT __declspec(dllexport)
#else
#define SHROUDFORGE_MOD_NATIVE_CALL
#define SHROUDFORGE_MOD_NATIVE_EXPORT
#endif

/* Connects a mod's own native DLL to the same mod's Lua code.
 *
 * A DLL loaded through native-plugin.ini, a DLL Manager selection or
 * loader.runtime.register_dll may export ShroudforgeModNativeInit. ShroudForge
 * calls it once per process, right after loading the DLL, on its native DLL
 * worker. A DLL without this export is loaded exactly as before.
 *
 * The bridge is scoped to the owning mod: its Lua code reaches only the
 * functions its own DLLs registered, through shroudforge.native. It does not
 * give the Lua code access to other mods. ABI 2 adds one read-only game read
 * for the DLL, the local camera, under the same conditions as the Lua API. */
#define SHROUDFORGE_MOD_NATIVE_ABI_VERSION 2u

typedef enum ShroudforgeNativeValueKind {
    SHROUDFORGE_NATIVE_NIL = 0,
    SHROUDFORGE_NATIVE_BOOLEAN = 1,
    SHROUDFORGE_NATIVE_INTEGER = 2,
    SHROUDFORGE_NATIVE_NUMBER = 3,
    /* A Lua string or buffer. Arguments point into Lua memory without a copy
     * and are valid only until the function returns. Results are copied. */
    SHROUDFORGE_NATIVE_BYTES = 4
} ShroudforgeNativeValueKind;

typedef struct ShroudforgeNativeBytes {
    const void* data;
    size_t size;
} ShroudforgeNativeBytes;

typedef struct ShroudforgeNativeValue {
    uint32_t kind;     /* ShroudforgeNativeValueKind */
    uint32_t reserved; /* Zero. */
    union {
        int32_t boolean; /* Zero is false. */
        int64_t integer;
        double number;
        ShroudforgeNativeBytes bytes;
    } as;
} ShroudforgeNativeValue;

/* Collects the values a native function returns to Lua. Valid only during
 * the call that received it. */
typedef struct ShroudforgeNativeResults ShroudforgeNativeResults;
struct ShroudforgeNativeResults {
    void* host_state; /* Owned by ShroudForge. */
    /* Appends one result. Bytes are copied before push returns.
     * Result: 1 appended, 2 invalid value, 3 too many results
     * (SHROUDFORGE_MOD_NATIVE_MAX_RESULTS), 4 the call already failed. */
    uint32_t (SHROUDFORGE_MOD_NATIVE_CALL* push)(
        ShroudforgeNativeResults* results, const ShroudforgeNativeValue* value);
    /* Ends the call with an error. Lua receives nil and this UTF-8 reason,
     * and the values pushed before are discarded. The first reason is kept. */
    void (SHROUDFORGE_MOD_NATIVE_CALL* fail)(
        ShroudforgeNativeResults* results, const char* reason);
};

#define SHROUDFORGE_MOD_NATIVE_MAX_RESULTS 32u

/* Runs on ShroudForge's mod runtime thread, never on the game thread, while
 * the mod's Lua update waits for it. Keep it short: long work belongs on the
 * DLL's own threads, with a function Lua polls on a later update.
 * The function must not throw or unwind into ShroudForge. */
typedef void (SHROUDFORGE_MOD_NATIVE_CALL* ShroudforgeNativeFunction)(
    void* context, const ShroudforgeNativeValue* arguments, size_t count,
    ShroudforgeNativeResults* results);

typedef enum ShroudforgeNativeLogLevel {
    SHROUDFORGE_NATIVE_LOG_TRACE = 0,
    SHROUDFORGE_NATIVE_LOG_DEBUG = 1,
    SHROUDFORGE_NATIVE_LOG_INFO = 2,
    SHROUDFORGE_NATIVE_LOG_WARN = 3,
    SHROUDFORGE_NATIVE_LOG_ERROR = 4
} ShroudforgeNativeLogLevel;

/* Valid until the process exits. Read only the fields that fit in size:
 * later ABI versions append fields and never reorder them. */
typedef struct ShroudforgeModHost ShroudforgeModHost;
struct ShroudforgeModHost {
    uint32_t abi_version; /* SHROUDFORGE_MOD_NATIVE_ABI_VERSION of the host. */
    uint32_t size;        /* sizeof(ShroudforgeModHost) of the host. */
    const char* mod_id;        /* UTF-8 id from the owning mod.json. */
    const char* mod_directory; /* UTF-8 path of the owning mod folder. */
    /* Writes one line to the ShroudForge log under the owning mod's id.
     * Any thread, any time. */
    void (SHROUDFORGE_MOD_NATIVE_CALL* log)(
        const ShroudforgeModHost* host, uint32_t level, const char* message);
    /* Makes function callable from the owning mod's Lua code as name.
     * Only during ShroudforgeModNativeInit. name: 1 to 64 ASCII letters,
     * digits, '_' or '.', not starting with a digit or '.'.
     * Result: 1 registered, 2 invalid argument, 3 name already registered
     * by this mod, 4 registration is closed (outside initialization). */
    uint32_t (SHROUDFORGE_MOD_NATIVE_CALL* register_function)(
        const ShroudforgeModHost* host, const char* name,
        ShroudforgeNativeFunction function, void* context);

    /* ABI 2. Reads the local player's camera, for a DLL that draws every
     * frame (an overlay), from the same ClientPlayerInput sample as
     * runtime.world.cursor.get. Any thread, read only, no game call.
     * output receives 9 doubles: camera position x, y, z in meters,
     * orientation x, y, z, w, fovY, aspect ratio. With count >= 17, 8 more:
     * building cursor position x, y, z in meters, orientation x, y, z, w, and
     * its flags (-1 without a cursor).
     * Only while the owning mod is an active ingame runtime mod with the
     * runtime capability, in a game client: the conditions of the Lua API.
     * The field layout comes from the game's reflection; it is known once a
     * runtime.world.cursor.get call decoded the camera.
     * Result: 1 read, 2 invalid argument, 3 layout not known yet, 4 no world
     * session, 5 retry next frame, 6 ambiguous local input, 7 the mod has no
     * runtime access now, 8 this ShroudForge runtime has no camera reader. */
    uint32_t (SHROUDFORGE_MOD_NATIVE_CALL* read_local_camera)(
        const ShroudforgeModHost* host, double* output, size_t count);
};

#define SHROUDFORGE_MOD_NATIVE_CAMERA_VALUES 9u
#define SHROUDFORGE_MOD_NATIVE_VIEW_VALUES 17u

/* Optional export. Return nonzero on success. Zero marks the DLL as failed
 * in the DLL Manager and withdraws the functions it registered. */
typedef int32_t (SHROUDFORGE_MOD_NATIVE_CALL* ShroudforgeModNativeInitFunction)(
    const ShroudforgeModHost* host);
#define SHROUDFORGE_MOD_NATIVE_INIT_EXPORT "ShroudforgeModNativeInit"

#if defined(__cplusplus)
}
#endif
