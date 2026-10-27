#include "abi/runtime_api.h"
#include "ecs/entity_session.h"
#include "loaded-resources/loaded_resource_access.h"
#include "main-menu/native_main_menu.h"
#include "voxels/write_registered_voxel_grid.h"
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

KFC_RUNTIME_API bool __cdecl KfcRuntimeInitialize() {
    try {
        const auto initialized = EcsRuntime::Initialize();
        ResourceRuntime::Initialize();
        NativeMainMenu::Initialize();
        provider_initialized.store(true, std::memory_order_release);
        return initialized;
    } catch (...) {
        provider_initialized.store(false, std::memory_order_release);
        NativeMainMenu::Shutdown();
        ResourceRuntime::Shutdown();
        EcsRuntime::Shutdown();
        return false;
    }
}

KFC_RUNTIME_API void __cdecl KfcRuntimeTick() {
    NativeMainMenu::Tick();
    try { EcsRuntime::Tick(); } catch (...) { EcsRuntime::Shutdown(); }
    WorldRuntime::RegisteredVoxelGrid::PrepareWriter();
}

KFC_RUNTIME_API void __cdecl KfcRuntimeShutdown() {
    provider_initialized.store(false, std::memory_order_release);
    WorldRuntime::RegisteredVoxelGrid::StopPreparingWriter();
    KfcRuntimeNetworkServiceShutdown();
    NativeMainMenu::Shutdown();
    ResourceRuntime::Shutdown();
    EcsRuntime::Shutdown();
}
}
