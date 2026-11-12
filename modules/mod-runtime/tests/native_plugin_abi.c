#include <stddef.h>
#include "../native_plugin_api.h"

/* Match the Rust host layouts from an ordinary x64 C plugin consumer. */
_Static_assert(sizeof(SFNativePluginV1) == 48, "plugin layout");
_Static_assert(offsetof(SFNativePluginV1, initialize) == 16, "initialize offset");
_Static_assert(sizeof(SFNativeHostV1) == 96, "host layout");
_Static_assert(offsetof(SFNativeHostV1, package_generation) == 24, "generation offset");
_Static_assert(offsetof(SFNativeHostV1, read_asset) == 64, "asset callback offset");
_Static_assert(offsetof(SFNativeHostV1, drain_services) == 88, "drain callback offset");
_Static_assert(sizeof(SFKeybindStateV1) == 8, "key state layout");
_Static_assert(sizeof(SFKeybindServiceV1) == 24, "keybind service layout");
_Static_assert(sizeof(SFNotificationServiceV1) == 24, "notification service layout");

SFNativeStatusV1 SF_NATIVE_CALL ShroudForgePluginEntry(
    uint32_t requested_abi, uint32_t out_size, SFNativePluginV1 *out_plugin)
{
    (void)requested_abi;
    (void)out_size;
    (void)out_plugin;
    return SF_NATIVE_STATUS_ABI_MISMATCH;
}

int main(void) { return 0; }
