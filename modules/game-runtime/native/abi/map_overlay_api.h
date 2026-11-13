#pragma once
#include <stdint.h>

/* Native service ABI, separate from the built-in KfcRuntime ABI and package SDK.
 * Coordinates are baseline world units converted from signed Q32.32. X/Z map
 * to atlas u=x/10240, v=1-z/10240; headings are radians. No physical unit claim.
 * A lease owns every borrowed pointer in its frame until release. Generation
 * changes invalidate its use in a new session, but never free its storage. */
#define SF_MAP_SERVICE_VERSION 1u
#define SF_MAP_CLOCK_DOMAIN_WINDOWS_TICK64 UINT64_C(0x57494e5449434b31)

typedef enum SFMapStatusV1 {
    SF_MAP_READY = 0, SF_MAP_DISABLED = 1, SF_MAP_UNSUPPORTED_BUILD = 2,
    SF_MAP_NO_SESSION = 3, SF_MAP_NOT_CAPTURED = 4, SF_MAP_STALE = 5,
    SF_MAP_WRONG_GENERATION = 6, SF_MAP_INVALID_ARGUMENT = 7,
    SF_MAP_BUSY = 8, SF_MAP_FAILED = 9, SF_MAP_CAPABILITY_DENIED = 10
} SFMapStatusV1;
typedef enum SFMapFeatureV1 {
    SF_MAP_FEATURE_PLAYER = 1u, SF_MAP_FEATURE_MARKERS = 2u,
    SF_MAP_FEATURE_FOG = 4u, SF_MAP_FEATURE_ENVIRONMENT = 8u,
    SF_MAP_FEATURE_WORLD = 16u, SF_MAP_FEATURE_DISCOVERY = 32u,
    SF_MAP_FEATURE_HUD = 64u,
    SF_MAP_FEATURE_CLOCK = 128u, SF_MAP_FEATURE_WEATHER = 256u,
    SF_MAP_FEATURE_DEATHS = 512u
} SFMapFeatureV1;
/* ENVIRONMENT summarizes any admitted clock/weather facility. It never grants
 * either leaf facility. WORLD admits world/custom markers only, never DEATHS.
 * Leaf bits govern publication and each view's unsupported/not-captured status. */
typedef enum SFMapFlagsV1 {
    SF_MAP_POSE_VALID = 1u, SF_MAP_POSE_HEADING = 2u,
    SF_MAP_MARKER_VISIBLE = 1u, SF_MAP_MARKER_CUSTOM = 2u,
    SF_MAP_MARKER_HEADING = 4u, SF_MAP_MARKER_DISCOVERED = 8u,
    SF_MAP_STATE_ACTIVE = 1u, SF_MAP_STATE_GAMEPLAY = 2u,
    SF_MAP_STATE_MENU = 4u, SF_MAP_STATE_FOCUSED = 8u
} SFMapFlagsV1;

typedef struct SFMapSnapshotMetaV1 {
    uint32_t status, freshness_limit_ms;
    uint64_t session_generation, revision, published_at_ms, clock_domain;
} SFMapSnapshotMetaV1;
typedef struct SFMapPoseV1 {
    float position[3], heading_radians;
    uint32_t flags, reserved;
} SFMapPoseV1;
typedef struct SFMapPlayerViewV1 {
    SFMapSnapshotMetaV1 meta;
    SFMapPoseV1 pose, camera;
} SFMapPlayerViewV1;
typedef struct SFMapMarkerV1 {
    uint64_t id;
    float position[3], heading_radians;
    uint32_t kind, flags;
    uint64_t expires_at_ms;
    uint32_t icon_candidate_count, reserved;
    uint32_t icon_candidates[10];
} SFMapMarkerV1;
/* icon_candidates are bounded numeric atlas-identity candidates, in the low,
 * high order of each of the five captured identity words. They are opaque
 * integers, never addresses or dereferenceable engine pointers. Count 0 means
 * absent; counts greater than 10 are invalid. `id` retains full 64-bit identity.
 * Consumers select their own matching assets and presentation. */
typedef struct SFMapMarkerViewV1 {
    SFMapSnapshotMetaV1 meta;
    const SFMapMarkerV1* data;
    uint32_t count, reserved;
} SFMapMarkerViewV1;
typedef struct SFMapFogViewV1 {
    SFMapSnapshotMetaV1 meta;
    float world_width, world_height;
    uint32_t columns, rows;
    const uint8_t* tiles;
    uint64_t byte_count;
} SFMapFogViewV1;
typedef struct SFMapEnvironmentV1 {
    SFMapSnapshotMetaV1 clock_meta, weather_meta;
    int32_t minute_of_day;
    uint32_t daytime, weather_kind, reserved;
} SFMapEnvironmentV1;
typedef struct SFMapFrameV1 {
    uint32_t size, version, status, state_flags;
    uint64_t session_generation, clock_domain, now_ms;
    uint32_t supported_features, reserved;
    SFMapPlayerViewV1 player;
    SFMapMarkerViewV1 world_markers, remote_players, deaths, waypoints, pings, discovery;
    SFMapFogViewV1 fog;
    SFMapEnvironmentV1 environment;
} SFMapFrameV1;
typedef uint64_t SFMapFrameLeaseV1;
typedef struct SFInputKeyStateV1 {
    uint32_t virtual_key, flags; /* DOWN=1, PRESSED=2, RELEASED=4, FOCUSED=8. */
    uint64_t sequence, sampled_at_ms;
} SFInputKeyStateV1;
typedef struct SFMapServiceV1 {
    uint32_t size, version;
    void* context;
    /* out_size is checked before writing. expected_generation=0 accepts the
     * current session. Release exactly once, including non-ready frames. */
    int32_t (*acquire)(void* context, uint64_t expected_generation, uint32_t out_size,
        SFMapFrameV1* out_frame, SFMapFrameLeaseV1* out_lease);
    int32_t (*release)(void* context, SFMapFrameLeaseV1 lease);
    /* At most 32 configured Windows virtual keys (1..255). Shared high-bit
     * sampling never consumes GetAsyncKeyState's process-global pressed bit. */
    int32_t (*read_keys)(void* context, const uint32_t* virtual_keys, uint32_t count,
        uint32_t out_capacity, SFInputKeyStateV1* out_keys);
} SFMapServiceV1;

/* All control/encoder function results use the package SDK SF_NATIVE_STATUS_*
 * numbers. Snapshot availability is separately expressed by SFMapStatusV1.
 * Tables are COPIED by query_service. Structs and spans below are synchronous;
 * no pointer, command, or encoder may be retained after the callback returns. */
#define SF_OVERLAY_SERVICE_VERSION 1u
#define SF_OVERLAY_EXTENDED_SERVICE_VERSION 2u
#define SF_OVERLAY_PROGRAMMABLE_SERVICE_VERSION 3u
#define SF_OVERLAY_SHADER_VERSION 1u
#define SF_OVERLAY_MAX_SHADER_BYTES 262144u
#define SF_OVERLAY_MAX_PROGRAM_TEXTURES 4u
#define SF_OVERLAY_MAX_PROGRAM_CONSTANT_BYTES 128u
#define SF_OVERLAY_MAX_PROGRAMS_PER_OWNER 32u
#define SF_OVERLAY_MAX_PROGRAMS_PER_IMAGE 128u
#define SF_OVERLAY_MAX_PROGRAM_DRAWS 4096u
#define SF_OVERLAY_MAX_TEXTURE_BYTES UINT64_C(67108864)
#define SF_OVERLAY_MAX_RECTANGLES 262144u
#define SF_OVERLAY_MAX_TEXTURE_COMMANDS 4096u
typedef struct SFOverlayTextureV1 {
    uint64_t resource_id, revision;
    uint32_t width, height;
    const uint8_t* rgba;
    uint64_t rgba_bytes;
    const uint8_t* coverage_mask;
    uint64_t coverage_mask_bytes;
} SFOverlayTextureV1;
/* RGBA8 row-major, straight alpha. Optional coverage is one byte/texel, 0/1,
 * provided by the caller. Same owner/resource_id/revision
 * identifies immutable content. The host copies/uploads only a new revision. */
typedef struct SFOverlayRectangleV1 { int32_t x,y,width,height; uint32_t rgba; } SFOverlayRectangleV1;
typedef struct SFOverlayTextureCommandV1 {
    SFOverlayTextureV1 texture;
    uint32_t source_x,source_y,source_width,source_height;
    int32_t x,y,width,height,clip_center_x,clip_center_y,clip_radius;
    float rotation_radians;
    uint32_t tint_rgba;
} SFOverlayTextureCommandV1;
/* Packed colors are RRGGBBAA, not native-endian byte arrays. Tint multiplies
 * straight RGB/alpha independently. Source is texels (zero extent=full image),
 * destination is pixels, positive rotation clockwise around destination center,
 * clip_radius>0 applies a screen-space circle. */
typedef struct SFOverlayEncoderV1 {
    uint32_t size,version;
    void* context;
    uint64_t owner_id,device_generation;
    int32_t (*draw_rectangles)(void*,uint64_t,const SFOverlayRectangleV1*,uint32_t);
    int32_t (*draw_textures)(void*,uint64_t,const SFOverlayTextureCommandV1*,uint32_t);
} SFOverlayEncoderV1;
/* Version 2 is requested through shroudforge.overlay before registration.
 * The V1 structures and callbacks retain their original layouts/semantics. */
typedef struct SFOverlayTextureCommandV2 {
    SFOverlayTextureCommandV1 base;
    uint32_t square_clip;
} SFOverlayTextureCommandV2;
typedef struct SFOverlayEncoderV2 {
    SFOverlayEncoderV1 base;
    int32_t (*draw_textures_v2)(void*,uint64_t,const SFOverlayTextureCommandV2*,uint32_t);
} SFOverlayEncoderV2;
/* V3 is append-only. Program identity/revision denotes immutable fragment
 * SPIR-V, independent of texture identities and revisions. Words are copied
 * synchronously on first use; a cached revision does not reread that span.
 * Use Vulkan 1.0 / SPIR-V 1.0, one Fragment entry named main, optional smooth
 * float32 vec2 UV input at location 0, optional float32 vec4 FragCoord input,
 * location 0 float32 vec4 straight-alpha output, set 0 bindings 0..3 combined
 * samplers. The fixed vertex supplies UV even when the fragment omits it.
 * Bindings are sampler2D. An optional push-constant block at offset 0 is at most 128
 * bytes. No other resources, capabilities or shader stages are supported. */
typedef struct SFOverlayShaderV1 {
    uint32_t size, version;
    uint64_t program_id, revision;
    const uint32_t* spirv;
    uint32_t spirv_bytes, reserved;
} SFOverlayShaderV1;
typedef struct SFOverlayProgramCommandV1 {
    SFOverlayShaderV1 shader;
    /* Positive rectangle wholly inside the target, each side <=16384 pixels.
     * Fixed quad UV is (0,0) at top-left and (1,1) at bottom-right. */
    int32_t x,y,width,height;
    uint32_t texture_count, constant_bytes;
    /* Index i supplies set 0 binding i. Missing bindings use transparent black
     * RGBA (0,0,0,0), 1x1. Texture dimensions/revision rules match V1. */
    const SFOverlayTextureV1* textures;
    /* Opaque bytes, count divisible by 4, at most 128. Remaining bytes are zero.
     * Fragment constants alone own clipping, rotation and rendering behavior. */
    const void* constants;
} SFOverlayProgramCommandV1;
typedef struct SFOverlayEncoderV3 {
    SFOverlayEncoderV2 base;
    int32_t (*draw_programs)(void*,uint64_t,const SFOverlayProgramCommandV1*,uint32_t);
} SFOverlayEncoderV3;
typedef struct SFOverlayFrameV1 {
    uint32_t size,version,width,height;
    uint64_t device_generation;
    /* Optional synchronous snapshot, never a map-readiness promise. May be
     * NULL; without map capture the current host provides SF_MAP_DISABLED.
     * The pointer and all its spans expire when the callback returns. */
    const SFMapFrameV1* map_frame;
} SFOverlayFrameV1;
/* These are output encodings, not VkColorSpaceKHR values. V3 does no implicit
 * color conversion. Shaders encode RGB for the observed target before output;
 * alpha is straight coverage, unchanged by color encoding. */
typedef enum SFOverlayTargetEncodingV3 {
    SF_OVERLAY_TARGET_SRGB_VALUES=0,
    SF_OVERLAY_TARGET_SCRGB_LINEAR=1,
    SF_OVERLAY_TARGET_HDR10_PQ=2
} SFOverlayTargetEncodingV3;
typedef struct SFOverlayFrameV3 {
    SFOverlayFrameV1 base;
    uint32_t target_encoding, reserved[3];
    float diffuse_white_nits, linear_unit_nits, pq_peak_nits, reserved_float;
} SFOverlayFrameV3;
typedef int32_t (*SFOverlayFrameCallbackV1)(void*,const SFOverlayFrameV1*,const SFOverlayEncoderV1*);
typedef uint64_t SFOverlayRegistrationV1;
typedef enum SFOverlayReasonV1 {
    SF_OVERLAY_REASON_NONE=0, SF_OVERLAY_REASON_AWAIT_OBSERVED_CREATE=1,
    SF_OVERLAY_REASON_DEVICE_LOST=2,
    SF_OVERLAY_REASON_BOOT_NOT_ARMED=3, SF_OVERLAY_REASON_BOOT_MISSED=4,
    SF_OVERLAY_REASON_MAINTENANCE_UNSUPPORTED=5, SF_OVERLAY_REASON_FEATURE_CONFLICT=6,
    SF_OVERLAY_REASON_PRESENT_CONFLICT=7, SF_OVERLAY_REASON_PRESENT_FATAL=8,
    SF_OVERLAY_REASON_PRESENT_UNCERTAIN=9
} SFOverlayReasonV1;
/* status uses SF_NATIVE_STATUS_* (0 ready, 3 unavailable, 7 failed).
 * reason is the independent SFOverlayReasonV1 domain. Boot-not-armed/missed
 * requires restart with an enabled V1 overlay service declaration; unsupported
 * maintenance/features remain unavailable. Present-fatal means a retryable OOM
 * became logical DEVICE_LOST after overlay submission consumed caller waits.
 * Present-uncertain is an unsupported driver/validation result with unresolved
 * final ownership; it is not proof of actual driver device loss. Await-observed-create
 * waits for actual creation through the armed boot path. No extent/format/
 * colorspace is inferred for an existing unobserved swapchain. */
typedef struct SFOverlayStatusV1 {
    uint32_t size,version,status,reason;
    uint64_t device_generation,rendered_frames,busy_frames,failed_frames;
} SFOverlayStatusV1;
typedef struct SFOverlayServiceV1 {
    uint32_t size,version;
    void* context;
    int32_t (*register_frame)(void*,SFOverlayFrameCallbackV1,void*,SFOverlayRegistrationV1*);
    /* Prevents new entries immediately; BUSY means an existing callback remains.
     * The package SDK independently revokes/drains before freeing mod code. */
    int32_t (*unregister_frame)(void*,SFOverlayRegistrationV1);
    int32_t (*read_status)(void*,uint32_t,SFOverlayStatusV1*);
} SFOverlayServiceV1;
