#ifndef SHROUDFORGE_NATIVE_PLUGIN_API_H
#define SHROUDFORGE_NATIVE_PLUGIN_API_H

#include <stdint.h>

#if defined(_WIN32)
#define SF_NATIVE_PLUGIN_EXPORT __declspec(dllexport)
#else
#define SF_NATIVE_PLUGIN_EXPORT
#endif

#if defined(_MSC_VER)
#define SF_NATIVE_CALL __cdecl
#else
#define SF_NATIVE_CALL
#endif

#ifdef __cplusplus
extern "C" {
#define SF_NATIVE_EXTERN_C extern "C"
#else
#define SF_NATIVE_EXTERN_C extern
#endif

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

/* V2 is requested only by Lifecycle=shroudforge-v2. Its V1 prefix is byte-for-
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
 * - Entry must reject unsupported requested_abi, NULL out_plugin, or an
 *   out_size smaller than the requested table before writing any byte. V1
 *   callers provide sizeof(SFNativePluginV1); V2 callers provide
 *   sizeof(SFNativePluginV2) through the stable V1 pointer declaration. V2
 *   implementations cast only after validating ABI, size, and pointer.
 * - It negotiates the table only; callbacks are invoked by the host according
 *   to the declared lifecycle, after isolated DLL loading has completed.
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
 * - Optional V2 callbacks must be null when not used. The size-tagged V2 tail
 *   remains the extension point for later optional phases. The V1 prefix and
 *   entry export signature remain unchanged.
 * - The host table, package ID bytes, and package_context stay valid through
 *   initialize/update/stop/shutdown and the host's final service drain.
 * - The plugin must stop its worker threads and stop using every host pointer
 *   before shutdown returns. Service callbacks may outlive stop, so the host
 *   independently revokes and drains them before unloading plugin code.
 * - read_settings_json, read_asset, query_service, revoke_services, and
 *   drain_services run on the runtime control thread. read_asset accepts a
 *   UTF-8 package-relative path of at most SF_NATIVE_MAX_ASSET_PATH_BYTES and reads at most
 *   SF_NATIVE_MAX_ASSET_BYTES. It never resolves against the process CWD.
 * - The host never passes C++ or Rust-owned objects or allocators across this
 *   ABI. Strings and buffers are byte spans with explicit lengths/capacities.
 */
SF_NATIVE_EXTERN_C SF_NATIVE_PLUGIN_EXPORT SFNativeStatusV1 SF_NATIVE_CALL
ShroudForgePluginEntry(
    uint32_t requested_abi,
    uint32_t out_size,
    SFNativePluginV1 *out_plugin);

#ifdef __cplusplus
}
#endif

#endif
