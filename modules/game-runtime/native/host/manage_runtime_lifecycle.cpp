#include "abi/runtime_api.h"
#include "graphics/render_overlay.h"
#include "map/capture_game_map.h"
#include "first-person/first_person_control.h"
#include "engine-hooks/install_engine_hooks.h"
#include "ecs/entity_session.h"
#include "loaded-resources/loaded_resource_access.h"
#include "main-menu/native_main_menu.h"
#include "voxels/write_registered_voxel_grid.h"
#include <windows.h>
#include <algorithm>
#include <cstring>
#include <string>
#include <nlohmann/json.hpp>
#include "host/runtime_lifecycle_state.h"

using namespace ProviderImplementation;

namespace ProviderImplementation {
std::atomic<bool> provider_initialized{};
}

extern "C" {
KFC_RUNTIME_API unsigned __cdecl KfcRuntimeAbi() { return KFC_RUNTIME_ABI_VERSION; }
KFC_RUNTIME_API bool __cdecl KfcRuntimeStartupReady() {
    return provider_initialized.load(std::memory_order_acquire) && EngineHooks::StartupReady();
}

KFC_RUNTIME_API bool __cdecl KfcRuntimePrepareOverlay(unsigned demand) {
    try { return demand <= 1 && OverlayRuntime::PrepareGraphicsBoot(demand == 1); }
    catch (...) { return false; }
}
KFC_RUNTIME_API bool __cdecl KfcRuntimePrepareHeldEntry(const EngineHooks::HeldEntryProof* proof, unsigned demand) {
    if (demand == 0) return true;
    if (demand != 1 || !proof) return false;
    return EngineHooks::RunHeldEntryPreparation(*proof,
        +[](const EngineHooks::HeldEntryToken& token, void*) noexcept {
            return FirstPersonControl::Prepare(token);
        }, nullptr);
}

KFC_RUNTIME_API bool __cdecl KfcRuntimeInitialize() {
    try {
        const auto initialized = EcsRuntime::Initialize();
        ResourceRuntime::Initialize();
        NativeMainMenu::Initialize();
        provider_initialized.store(true, std::memory_order_release);
        return initialized;
    } catch (...) {
        KfcRuntimeShutdown();
        return false;
    }
}

KFC_RUNTIME_API void __cdecl KfcRuntimeTick() {
    if (!provider_initialized.load(std::memory_order_acquire)) return;
    try {
        NativeMainMenu::Tick();
        EcsRuntime::Tick();
        OverlayRuntime::TickServices();
        WorldRuntime::RegisteredVoxelGrid::PrepareWriter();
    } catch (...) {
        KfcRuntimeShutdown();
        return;
    }
    static ULONGLONG next_resource_profile_retry{};
    const auto now = GetTickCount64();
    if (!ResourceRuntime::ProfileLoaded() && now >= next_resource_profile_retry) {
        next_resource_profile_retry = now + 2000;
        try { ResourceRuntime::Initialize(); } catch (...) {}
    }
}

KFC_RUNTIME_API void __cdecl KfcRuntimeShutdown() {
    provider_initialized.store(false, std::memory_order_release);
    // A failed owner cleanup must not skip retirement of the remaining owners.
    const auto safely = [](auto stop) noexcept { try { stop(); } catch (...) {} };
    safely([] { WorldRuntime::RegisteredVoxelGrid::StopPreparingWriter(); });
    safely([] { FirstPersonControl::Shutdown(); });
    safely([] { MapRuntime::StopCapture(); });
    safely([] { OverlayRuntime::ShutdownServices(); });
    safely([] { KfcRuntimeNetworkServiceShutdown(); });
    safely([] { NativeMainMenu::Shutdown(); });
    safely([] { ResourceRuntime::Shutdown(); });
    safely([] { EcsRuntime::Shutdown(); });
}
}
