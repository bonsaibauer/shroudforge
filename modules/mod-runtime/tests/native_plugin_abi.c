#include <stddef.h>
#include "../../game-runtime/native/abi/executable_span_api.h"
#include "../native_bridge/shroudforge_mod_native.h"
#include "../../game-runtime/native/abi/map_overlay_api.h"
#include "../../game-runtime/native/abi/executable_span_validation_api.h"
#include "../../game-runtime/native/abi/runtime_api.h"

/* Match the Rust host layouts from an ordinary x64 C plugin consumer. */
_Static_assert(KFC_RUNTIME_ABI_VERSION == 12u, "stable runtime ABI");
_Static_assert(SHROUDFORGE_MOD_NATIVE_ABI_VERSION == 4u, "extended mod bridge ABI");
_Static_assert(sizeof(ShroudforgeModHost) == 64, "mod host layout");
_Static_assert(offsetof(ShroudforgeModHost, read_local_camera) == 40, "stable ABI 2 tail");
_Static_assert(offsetof(ShroudforgeModHost, register_lifecycle) == 48, "appended ABI 3 tail");
_Static_assert(offsetof(ShroudforgeModHost, reserved_preentry_registration) == 56, "reserved ABI 4 tail");
_Static_assert(sizeof(SFNativePluginV1) == 48, "plugin layout");
_Static_assert(offsetof(SFNativePluginV1, initialize) == 16, "initialize offset");
_Static_assert(sizeof(SFNativeHostV1) == 96, "host layout");
_Static_assert(offsetof(SFNativeHostV1, package_generation) == 24, "generation offset");
_Static_assert(offsetof(SFNativeHostV1, read_asset) == 64, "asset callback offset");
_Static_assert(offsetof(SFNativeHostV1, drain_services) == 88, "drain callback offset");
_Static_assert(sizeof(SFKeybindStateV1) == 8, "key state layout");
_Static_assert(sizeof(SFKeybindServiceV1) == 24, "keybind service layout");
_Static_assert(sizeof(SFNotificationServiceV1) == 24, "notification service layout");
_Static_assert(SF_EXECUTABLE_SPAN_UNSET == 0u, "span unset value");
_Static_assert(SF_EXECUTABLE_SPAN_UNCHANGED == 1u, "span unchanged value");
_Static_assert(SF_EXECUTABLE_SPAN_VERIFIED_HOST_OWNED == 2u, "span owned value");
_Static_assert(SF_EXECUTABLE_SPAN_FOREIGN_OR_UNAVAILABLE == 3u, "span foreign value");
_Static_assert(sizeof(SFExecutableSpanQueryV1) == 80, "executable span query layout");
_Static_assert(sizeof(SFExecutableSpanResultV1) == 8, "executable span result layout");
_Static_assert(sizeof(SFExecutableSpanServiceV1) == 24, "executable span service layout");
_Static_assert(sizeof(SFExecutableSpanValidationRequestV1) == 84, "span request layout");
_Static_assert(SF_EXECUTABLE_SPAN_VALIDATION_UNCHANGED == 0u, "validation unchanged value");
_Static_assert(SF_EXECUTABLE_SPAN_VALIDATION_UNCHANGED != SF_EXECUTABLE_SPAN_UNCHANGED, "separate span result domains");
_Static_assert(SF_EXECUTABLE_SPAN_VALIDATION_VERIFIED_OWNER == 1u, "validation owner value");
_Static_assert(SF_EXECUTABLE_SPAN_VALIDATION_REJECT == 2u, "validation reject value");
_Static_assert(offsetof(SFExecutableSpanValidationRequestV1, expected) == 20, "span expected offset");
_Static_assert(offsetof(SFExecutableSpanValidationRequestV1, observed) == 52, "span observed offset");
_Static_assert(sizeof(SFExecutableSpanValidationResponseV1) == 48, "span response layout");
_Static_assert(offsetof(SFExecutableSpanValidationResponseV1, owner_identity) == 16, "span identity offset");
_Static_assert(sizeof(SFExecutableSpanRegistrationV1) == 64, "span registration layout");
_Static_assert(offsetof(SFExecutableSpanRegistrationV1, callback) == 48, "span callback offset");
_Static_assert(sizeof(SFExecutableSpanValidationServiceV1) == 24, "span service layout");

_Static_assert(sizeof(SFMapFrameV1) == 640, "map frame layout");
_Static_assert(sizeof(SFMapMarkerV1) == 88, "map marker layout");
_Static_assert(offsetof(SFMapMarkerV1, icon_candidates) == 48, "marker candidates offset");
_Static_assert(sizeof(SFMapServiceV1) == 32, "map service layout");
_Static_assert(sizeof(SFOverlayServiceV1) == 40, "overlay service layout");
_Static_assert(sizeof(SFOverlaySceneServiceV1) == 32, "copied scene uses same owner");
_Static_assert(offsetof(SFOverlaySceneServiceV1, read_target) == 24, "scene target reader");
_Static_assert(sizeof(SFOverlayEncoderV1) == 48, "overlay encoder layout");
_Static_assert(sizeof(SFOverlayEncoderV2) == 56, "extended encoder layout");
_Static_assert(sizeof(SFOverlayStatusV1) == 48, "overlay status layout");

static SFNativeStatusV1 SF_NATIVE_CALL lifecycle_factory(
    uint32_t requested_abi, uint32_t out_size, SFNativePluginV1 *out_plugin)
{
    (void)requested_abi;
    (void)out_size;
    (void)out_plugin;
    return SF_NATIVE_STATUS_ABI_MISMATCH;
}

SHROUDFORGE_MOD_NATIVE_EXPORT int32_t SHROUDFORGE_MOD_NATIVE_CALL
ShroudforgeModNativeInit(const ShroudforgeModHost *host)
{
    if (!host || host->abi_version < 3u || host->size < sizeof(*host)
        || !host->register_lifecycle) return 0;
    return host->register_lifecycle(host, lifecycle_factory) == 1u;
}

static ShroudforgeNativeLifecycleFactory registered_factory;
static uint32_t SHROUDFORGE_MOD_NATIVE_CALL register_lifecycle(
    const ShroudforgeModHost *host, ShroudforgeNativeLifecycleFactory factory)
{
    (void)host;
    registered_factory = factory;
    return 1u;
}

static uint32_t SHROUDFORGE_MOD_NATIVE_CALL reserved_registration(
    const ShroudforgeModHost *host, ShroudforgeNativeLifecycleFactory factory)
{
    (void)host;
    (void)factory;
    return 0u;
}

int main(void)
{
    ShroudforgeModHost host = {0};
    host.abi_version = 2u;
    host.size = 48u;
    if (ShroudforgeModNativeInit(&host) != 0 || registered_factory) return 1;
    host.abi_version = SHROUDFORGE_MOD_NATIVE_ABI_VERSION;
    host.size = sizeof(host);
    host.register_lifecycle = register_lifecycle;
    host.reserved_preentry_registration = reserved_registration;
    if (ShroudforgeModNativeInit(&host) != 1 || !registered_factory) return 2;
    if (registered_factory(99u, 0u, NULL) != SF_NATIVE_STATUS_ABI_MISMATCH) return 3;
    if (host.reserved_preentry_registration(&host, lifecycle_factory) != 0u) return 4;
    return 0;
}
