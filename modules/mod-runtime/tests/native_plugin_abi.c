#include <stddef.h>
#include "../../game-runtime/native/abi/executable_span_api.h"
#include "../native_plugin_api.h"
#include "../../game-runtime/native/abi/map_overlay_api.h"
#include "../../game-runtime/native/abi/executable_span_validation_api.h"
#include "../../game-runtime/native/abi/runtime_api.h"

/* Match the Rust host layouts from an ordinary x64 C plugin consumer. */
_Static_assert(KFC_RUNTIME_ABI_VERSION == 12u, "stable runtime ABI");
_Static_assert(sizeof(SFNativePluginV1) == 48, "plugin layout");
_Static_assert(offsetof(SFNativePluginV1, initialize) == 16, "initialize offset");
_Static_assert(sizeof(SFNativePluginV2) == 72, "version 2 plugin layout");
_Static_assert(offsetof(SFNativePluginV2, prepare_before_entry) == 48, "version 2 prepare offset");
_Static_assert(offsetof(SFNativePluginV2, stage_before_entry) == 56, "version 2 stage offset");
_Static_assert(offsetof(SFNativePluginV2, entry_restored) == 64, "version 2 restored offset");
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
_Static_assert(sizeof(SFMapServiceV1) == 40, "map service layout");
_Static_assert(sizeof(SFOverlayServiceV1) == 40, "overlay service layout");
_Static_assert(sizeof(SFOverlayEncoderV1) == 48, "overlay encoder layout");
_Static_assert(sizeof(SFOverlayEncoderV2) == 56, "extended encoder layout");
_Static_assert(sizeof(SFOverlayStatusV1) == 48, "overlay status layout");

SFNativeStatusV1 SF_NATIVE_CALL ShroudForgePluginEntry(
    uint32_t requested_abi, uint32_t out_size, SFNativePluginV1 *out_plugin)
{
    (void)requested_abi;
    (void)out_size;
    (void)out_plugin;
    return SF_NATIVE_STATUS_ABI_MISMATCH;
}

int main(void) { return 0; }
