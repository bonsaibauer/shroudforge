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

/* Connects a mod's own native DLL to its Lua code and optional activation.
 *
 * A DLL loaded through native-plugin.ini, a DLL Manager selection or
 * loader.runtime.register_dll may export ShroudforgeModNativeInit. ShroudForge
 * calls it once per process, right after loading the DLL, on its native DLL
 * worker. A DLL without this export is loaded exactly as before.
 *
 * The bridge is scoped to the owning mod: its Lua code reaches only the
 * functions its own DLLs registered, through shroudforge.native. It does not
 * give the Lua code access to other mods. ABI 2 adds one read-only game read
 * for the DLL, the local camera, under the same conditions as the Lua API.
 * ABI 3 appends activation registration to this same init connection.
 * ABI 4 appends registration for required pre-entry preparation. */
#define SHROUDFORGE_MOD_NATIVE_ABI_VERSION 4u

#define SF_NATIVE_CALL SHROUDFORGE_MOD_NATIVE_CALL

#define SF_NATIVE_PLUGIN_ABI_V1 1u
#define SF_NATIVE_PLUGIN_ABI_V2 2u
#define SF_NATIVE_HOST_ABI_V1 1u
#define SF_NATIVE_MAX_SETTINGS_JSON_BYTES 65536u
#define SF_NATIVE_MAX_ASSET_PATH_BYTES 1024u
#define SF_NATIVE_MAX_ASSET_BYTES 16777216u
#define SF_NATIVE_MAX_LOG_BYTES 4096u
#define SF_NATIVE_DRAIN_TIMEOUT_MS 250u
#define SF_NATIVE_SERVICE_MAP_V1 "shroudforge.map"
#define SF_NATIVE_SERVICE_OVERLAY_V1 "shroudforge.overlay"
#define SF_NATIVE_SERVICE_KEYBINDS_V1 "shroudforge.keybinds"
#define SF_NATIVE_SERVICE_NOTIFICATIONS_V1 "shroudforge.notifications"
#define SF_NOTIFICATION_SERVICE_VERSION 1u
#define SF_KEYBIND_DOWN 1u
#define SF_KEYBIND_PRESSED 2u

typedef int32_t SFNativeStatusV1;
typedef uint32_t SFNativeTargetV1;
typedef uint32_t SFNativeLogLevelV1;

/* Named settings use the existing SDL capture/Hotkey Manager. No VK codes.
 * Only declared control:keybind settings of this package are accessible.
 * read_keybind runs on the owner thread, is revoked with the host, and consumes
 * press edges once per setting. First read after load/rebind establishes a
 * baseline. Empty/disabled settings report zero flags. Focus loss and capture
 * suppress both flags while consuming revisions. Tables expire with the host.
 * Callers that gate actions on their own session state should keep polling
 * while gated so press revisions continue to be consumed.
 * out_size must fit SFKeybindStateV1; no output is written on errors.
 */
typedef struct SFKeybindStateV1 {
    uint32_t size;
    uint32_t flags;
} SFKeybindStateV1;

typedef struct SFKeybindServiceV1 {
    uint32_t size;
    uint32_t version;
    void *context;
    SFNativeStatusV1(SF_NATIVE_CALL *read_keybind)(
        void *context,
        const uint8_t *setting_key_utf8,
        uint32_t setting_key_length,
        uint32_t out_size,
        SFKeybindStateV1 *out_state);
} SFKeybindServiceV1;

#define SF_NATIVE_STATUS_OK ((SFNativeStatusV1)0)
#define SF_NATIVE_STATUS_INVALID_ARGUMENT ((SFNativeStatusV1)1)
#define SF_NATIVE_STATUS_ABI_MISMATCH ((SFNativeStatusV1)2)
#define SF_NATIVE_STATUS_UNAVAILABLE ((SFNativeStatusV1)3)
#define SF_NATIVE_STATUS_UNKNOWN_SERVICE ((SFNativeStatusV1)4)
#define SF_NATIVE_STATUS_BUFFER_TOO_SMALL ((SFNativeStatusV1)5)
#define SF_NATIVE_STATUS_BUSY ((SFNativeStatusV1)6)
#define SF_NATIVE_STATUS_FAILED ((SFNativeStatusV1)7)
#define SF_NATIVE_STATUS_WRONG_THREAD ((SFNativeStatusV1)8)
#define SF_NATIVE_STATUS_NOT_FOUND ((SFNativeStatusV1)9)
#define SF_NATIVE_STATUS_TOO_LARGE ((SFNativeStatusV1)10)

/* Optional client control-thread service, borrowed for this package generation.
 * local_id: 1..80 ASCII letters/digits/dot/dash/underscore; title/message: valid
 * UTF-8, 1..120/2000 bytes, no embedded NUL. level: 0 info, 1 success,
 * 2 warning, 3 error, 4 update. Host supplies the package namespace.
 * OK means the local feed was persisted, not that a frame became visible.
 * In-game mirroring is optional and never changes persistence success.
 * Requires enabled client Runtime and RuntimeRegisterDll authority. Every call
 * checks owner thread/revocation; do not retain/use after host lifetime or call
 * from an overlay callback. Query absence must not disable a mod's behavior.
 */
typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativePublishNoticeFnV1)(
    void*,const uint8_t*,uint32_t,const uint8_t*,uint32_t,const uint8_t*,uint32_t,uint32_t);
typedef struct SFNotificationServiceV1 {
    uint32_t size,version;
    void* context;
    SFNativePublishNoticeFnV1 publish;
} SFNotificationServiceV1;

#define SF_NATIVE_TARGET_CLIENT ((SFNativeTargetV1)1u)
#define SF_NATIVE_TARGET_SERVER ((SFNativeTargetV1)2u)

#define SF_NATIVE_LOG_ERROR ((SFNativeLogLevelV1)1u)
#define SF_NATIVE_LOG_WARN ((SFNativeLogLevelV1)2u)
#define SF_NATIVE_LOG_INFO ((SFNativeLogLevelV1)3u)
#define SF_NATIVE_LOG_DEBUG ((SFNativeLogLevelV1)4u)
#define SF_NATIVE_LOG_TRACE ((SFNativeLogLevelV1)5u)

typedef struct SFNativeHostV1 SFNativeHostV1;

typedef void(SF_NATIVE_CALL *SFNativeLogFnV1)(
    void *package_context,
    SFNativeLogLevelV1 level,
    const uint8_t *message_utf8,
    uint32_t message_length);

typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativeReadSettingsJsonFnV1)(
    void *package_context,
    uint8_t *destination,
    uint32_t capacity,
    uint32_t *out_required_bytes);

/*
 * Returns the current settings object's settingValues JSON as UTF-8, without a
 * trailing NUL. On BUFFER_TOO_SMALL, out_required_bytes is exact and no bytes
 * are written. This callback is restricted to the runtime owner thread.
 */
/*
 * Control-plane package-relative read through the loader's package filesystem.
 * On BUFFER_TOO_SMALL, required_bytes is exact and no bytes are written. On
 * TOO_LARGE, required_bytes is zero because the asset exceeded the fixed cap.
 */
typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativeReadAssetFnV1)(
    void *package_context,
    const uint8_t *relative_path_utf8,
    uint32_t relative_path_length,
    uint8_t *destination,
    uint32_t capacity,
    uint32_t *out_required_bytes);

/* out_capacity bounds every write to out_table for the requested service. */
typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativeQueryServiceFnV1)(
    void *package_context,
    const uint8_t *service_name_utf8,
    uint32_t service_name_length,
    uint32_t requested_version,
    uint32_t out_capacity,
    void *out_table);

/* These owner-bound hooks must return BUSY until all host callbacks have left. */
typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativeRevokeServicesFnV1)(
    void *package_context,
    uint64_t package_generation);

typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativeDrainServicesFnV1)(
    void *package_context,
    uint64_t package_generation,
    uint32_t timeout_ms);

struct SFNativeHostV1 {
    uint32_t struct_size;
    uint32_t abi_version;
    const uint8_t *package_id_utf8;
    uint32_t package_id_length;
    uint64_t package_generation;
    SFNativeTargetV1 process_target;
    uint32_t reserved;
    void *package_context;
    SFNativeLogFnV1 log;
    SFNativeReadSettingsJsonFnV1 read_settings_json;
    SFNativeReadAssetFnV1 read_asset;
    SFNativeQueryServiceFnV1 query_service;
    SFNativeRevokeServicesFnV1 revoke_services;
    SFNativeDrainServicesFnV1 drain_services;
};

typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativeInitializeFnV1)(
    void *plugin_context,
    const SFNativeHostV1 *host);

typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativeUpdateFnV1)(
    void *plugin_context,
    const SFNativeHostV1 *host,
    uint64_t elapsed_microseconds);

typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativeStopFnV1)(
    void *plugin_context,
    const SFNativeHostV1 *host);

typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativeShutdownFnV1)(
    void *plugin_context,
    const SFNativeHostV1 *host);

typedef struct SFNativePluginV1 {
    uint32_t struct_size;
    uint32_t abi_version;
    void *plugin_context;
    SFNativeInitializeFnV1 initialize;
    SFNativeUpdateFnV1 update;
    SFNativeStopFnV1 stop;
    SFNativeShutdownFnV1 shutdown;
} SFNativePluginV1;

typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativePrepareBeforeEntryFnV2)(
    void *plugin_context,
    const SFNativeHostV1 *host);
typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativeStageBeforeEntryFnV2)(
    void *plugin_context,
    const SFNativeHostV1 *host);
typedef SFNativeStatusV1(SF_NATIVE_CALL *SFNativeEntryRestoredFnV2)(
    void *plugin_context);

/* V2 is requested by register_preentry_lifecycle. Its V1 prefix is byte-for-
 * byte the V1 table; preparation is required and the later phase callbacks
 * are optional. */
typedef struct SFNativePluginV2 {
    SFNativePluginV1 base;
    SFNativePrepareBeforeEntryFnV2 prepare_before_entry;
    SFNativeStageBeforeEntryFnV2 stage_before_entry;
    SFNativeEntryRestoredFnV2 entry_restored;
} SFNativePluginV2;

/*
 * ABI and lifetime contract:
 * - The registered factory must reject unsupported requested_abi, NULL out_plugin, or an
 *   out_size smaller than the requested table before writing any byte. V1
 *   callers provide sizeof(SFNativePluginV1); V2 callers provide
 *   sizeof(SFNativePluginV2) through the stable V1 pointer declaration. V2
 *   implementations cast only after validating ABI, size, and pointer.
 * - It negotiates the table only; callbacks are invoked by the host according
 *   to the declared lifecycle, after ordered DLL loading has completed.
 * - V2 stage_before_entry is optional and runs on the same bootstrap/runtime
 *   owner thread after factory negotiation and before framework preparation.
 *   The required prepare_before_entry runs on that thread after framework
 *   startup services are prepared. Both callbacks run inside the trusted
 *   held-entry preparation operation, without host locks held across plugin
 *   code. The host does not suspend peer threads. The
 *   optional entry_restored runs on the original entry thread after its bytes
 *   are restored and before the original entrypoint executes. That callback
 *   receives no host table and must not use owner-thread-bound host callbacks
 *   or retained host pointers.
 *   A service may explicitly issue a native read-only proof lease whose own
 *   contract permits arbitrary-thread use in this callback. That isolated
 *   lease is not permission to retain the host table or use other services.
 * - Optional V2 callbacks must be null when not used. The size-tagged V2 tail
 *   remains the extension point for later optional phases. The V1 prefix and
 *   registered factory signature remain unchanged.
 * - The host table, package ID bytes, and package_context stay valid through
 *   initialize/update/stop/shutdown and the host's final service drain.
 * - The plugin must stop its worker threads and stop using every host pointer
 *   before shutdown returns. Service callbacks may outlive stop, so the host
 *   independently revokes and drains them before releasing the activation.
 *   The common DLL executor keeps module code mapped until process exit.
 * - read_settings_json, read_asset, query_service, revoke_services, and
 *   drain_services run on the runtime control thread. read_asset accepts a
 *   UTF-8 package-relative path of at most SF_NATIVE_MAX_ASSET_PATH_BYTES and reads at most
 *   SF_NATIVE_MAX_ASSET_BYTES. It never resolves against the process CWD.
 * - The host never passes C++ or Rust-owned objects or allocators across this
 *   ABI. Strings and buffers are byte spans with explicit lengths/capacities.
 */
typedef SFNativeStatusV1(SF_NATIVE_CALL *ShroudforgeNativeLifecycleFactory)(
    uint32_t requested_abi, uint32_t out_size, SFNativePluginV1 *out_plugin);


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

    /* ABI 3. Registers this DLL's optional V1 activation callback factory.
     * Only during ShroudforgeModNativeInit. One registration per DLL.
     * No activation or game call runs here. The runtime invokes the factory
     * later on its owner thread, validates its size/version/callback table,
     * and supplies an activation-scoped SFNativeHostV1 to its callbacks.
     * That host and its services expire with the activation. The ModHost
     * passed here remains valid until exit. The registered factory must
     * remain callable until exit, including any code in a helper module.
     * A fresh V1 activation calls the same factory again after quiescent
     * cleanup. Supply a valid context for each activation. The process init
     * export is not called again and old activation services stay revoked.
     * Result: 1 registered, 2 invalid argument, 3 already registered,
     * 4 registration closed. Failed DLL init withdraws this registration.
     * Check abi_version >= 3 and size before accessing this appended field. */
    uint32_t (SHROUDFORGE_MOD_NATIVE_CALL* register_lifecycle)(
        const ShroudforgeModHost* host, ShroudforgeNativeLifecycleFactory factory);

    /* ABI 4. Registers a V2 factory requiring pre-entry preparation. V2 requires
     * Prepare and may supply Stage/entry-restored callbacks. Shares register_lifecycle's window and single
     * slot: registering through either function makes a second call return 3.
     * Check abi_version >= 4 and size before accessing this appended field.
     * Does not run a callback or grant access. Late V2 registration cannot
     * repeat process startup and is rejected by the activation owner. */
    uint32_t (SHROUDFORGE_MOD_NATIVE_CALL* register_preentry_lifecycle)(
        const ShroudforgeModHost* host, ShroudforgeNativeLifecycleFactory factory);
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
