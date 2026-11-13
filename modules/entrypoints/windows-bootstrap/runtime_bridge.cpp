#include "runtime_bridge.h"
#include "../../game-runtime/native/abi/runtime_api.h"
#include <windows.h>
#include <filesystem>

namespace {
HMODULE provider{};
using AbiFunction = decltype(&KfcRuntimeAbi);
using InitializeFunction = decltype(&KfcRuntimeInitialize);
using VoidFunction = decltype(&KfcRuntimeTick);
using StatusFunction = decltype(&KfcRuntimeStatus);
VoidFunction tick{}, shutdown{};
StatusFunction status{};
std::string error{"provider-not-loaded"};
}
namespace EcsRuntime {
bool LoadProvider() {
    if(provider)return true;
    HMODULE self{};
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        reinterpret_cast<LPCWSTR>(&Initialize), &self)) return false;
    wchar_t path[32768]{};
    if (!GetModuleFileNameW(self, path, 32768)) return false;
    const auto dll = std::filesystem::path(path).parent_path() / L"shroudforge" / L"kfc-runtime.dll";
    provider = LoadLibraryExW(dll.c_str(), nullptr, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
    if (!provider) { error = "kfc-runtime.dll-load-failed"; return false; }
    const auto abi = reinterpret_cast<AbiFunction>(GetProcAddress(provider, "KfcRuntimeAbi"));
    const auto initialize = reinterpret_cast<InitializeFunction>(GetProcAddress(provider, "KfcRuntimeInitialize"));
    tick = reinterpret_cast<VoidFunction>(GetProcAddress(provider, "KfcRuntimeTick"));
    shutdown = reinterpret_cast<VoidFunction>(GetProcAddress(provider, "KfcRuntimeShutdown"));
    status = reinterpret_cast<StatusFunction>(GetProcAddress(provider, "KfcRuntimeStatus"));
    const auto actual_abi = abi ? abi() : 0;
    if (!abi || actual_abi != KFC_RUNTIME_ABI_VERSION || !initialize || !tick || !shutdown || !status) {
        error = abi
            ? "incompatible-runtime-provider-ABI:expected=" + std::to_string(KFC_RUNTIME_ABI_VERSION) +
                ",actual=" + std::to_string(actual_abi)
            : "missing-runtime-provider-ABI-export";
        tick = nullptr; shutdown = nullptr; status = nullptr;
        FreeLibrary(provider); provider = nullptr; return false;
    }
    return true;
}
bool Initialize() {
    if(!LoadProvider())return false;
    return reinterpret_cast<InitializeFunction>(GetProcAddress(provider,"KfcRuntimeInitialize"))();
}
bool PrepareSelected(const SelectedStartup::PlanV2& plan,const EngineHooks::HeldEntryProof& proof,
        SelectedStartup::StageNativePlugins stage_native_plugins,
        SelectedStartup::PrepareNativePlugins native_plugins) {
    if(!SelectedStartup::Valid(plan) || !LoadProvider())return false;
    // Resolve the complete selected lifecycle before publishing any callbacks.
    const auto activate=GetProcAddress(provider,"KfcRuntimeActivateSelectedV1");
    if(!activate) {
        error="selected-runtime-exports-unavailable";return false;
    }
    unsigned completed{};
    if(plan.services&SelectedStartup::NativePluginPreentry) {
        const auto prepare=reinterpret_cast<decltype(&KfcRuntimePrepareSelectedV2)>(
            GetProcAddress(provider,"KfcRuntimePrepareSelectedV2"));
        if(!prepare || !stage_native_plugins || !native_plugins) {
            error="selected-v2-native-plugin-exports-unavailable";return false;
        }
        return prepare(&plan,&proof,stage_native_plugins,native_plugins,&completed) &&
            completed==SelectedStartup::PreparedServices(plan);
    }
    const auto prepare=reinterpret_cast<decltype(&KfcRuntimePrepareSelectedV1)>(
        GetProcAddress(provider,"KfcRuntimePrepareSelectedV1"));
    return prepare && prepare(&plan,&proof,nullptr,0,&completed) &&
        completed==SelectedStartup::PreparedServices(plan);
}
bool ActivateSelected(const SelectedStartup::PlanV2& plan) {
    if(!provider)return false;
    const auto activate=reinterpret_cast<decltype(&KfcRuntimeActivateSelectedV1)>(GetProcAddress(provider,"KfcRuntimeActivateSelectedV1"));
    unsigned completed{};return activate && activate(&plan,&completed) && completed==plan.services;
}


void Tick() { if (tick) tick(); }
void Shutdown() { if (shutdown) shutdown(); }
std::string Status() {
    if (!status) return error;
    char text[2048]{}; status(text, sizeof(text)); return text;
}
}
