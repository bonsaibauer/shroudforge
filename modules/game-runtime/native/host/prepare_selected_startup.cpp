#include "abi/runtime_api.h"
#include "abi/map_overlay_api.h"
#include "host/selected_startup_state.h"
#include "host/runtime_lifecycle_state.h"
#include "engine-hooks/held_entry_preparation.h"
#include "engine-hooks/install_engine_hooks.h"
#include "graphics/render_overlay.h"
#include "map/capture_game_map.h"
#include <windows.h>
#include <filesystem>

namespace {
SelectedStartup::State selected;
constexpr unsigned public_services = SelectedStartup::Metadata | SelectedStartup::GenericHooks |
    SelectedStartup::MapCapture | SelectedStartup::Graphics;
struct Context { const EngineHooks::HeldEntryToken* token{}; };
unsigned actual_role() {
    wchar_t path[32768]{};
    const auto size=GetModuleFileNameW(nullptr,path,32768);
    if(!size || size>=32768)return 0;
    const auto name=std::filesystem::path(path).filename().wstring();
    if(!_wcsicmp(name.c_str(),L"enshrouded.exe"))return SelectedStartup::Client;
    if(!_wcsicmp(name.c_str(),L"enshrouded_server.exe"))return SelectedStartup::DedicatedServer;
    return 0;
}
SelectedStartup::Operations operations(Context& context) {
    return {&context,
        +[](void*,bool execution){return ProviderImplementation::initialize_provider(execution,true);},
        +[](void*){return ProviderImplementation::provider_initialized.load() && EngineHooks::StartupReady();},
        +[](void* value){const auto token=static_cast<Context*>(value)->token;return token && MapRuntime::PrepareCapture(*token)==SF_MAP_READY;},
        +[](void*){return OverlayRuntime::PrepareGraphicsBoot(true);},
        +[](void*){ProviderImplementation::shutdown_provider_components();}};
}
}
namespace ProviderImplementation {
bool selected_startup_attempted() noexcept {return selected.Attempted();}
void close_selected_startup() noexcept {if(selected.Attempted())selected.Close();}
}
extern "C" {
// PlanV2 retains its versioned layout. Unsupported provider bits/inputs must
// refuse before initialization; they cannot be silently removed from intent.
KFC_RUNTIME_API bool __cdecl KfcRuntimePrepareSelectedV1(const SelectedStartup::PlanV2* plan,
        const EngineHooks::HeldEntryProof* proof,const wchar_t* ini,unsigned units,unsigned* completed) noexcept {
    if(!completed)return false;
    *completed=0;
    if(!plan || !proof || !SelectedStartup::Valid(*plan) || (plan->services&~public_services) ||
        plan->opt_ins || ini || units || ProviderImplementation::provider_initialized.load())return false;
    try {
        const auto original=*plan;
        if(original.process_role!=actual_role())return false;
        struct Call {SelectedStartup::PlanV2 plan;unsigned* completed;} call{original,completed};
        return EngineHooks::RunHeldEntryPreparation(*proof,
            +[](const EngineHooks::HeldEntryToken& token,void* value) noexcept {
                auto& call=*static_cast<Call*>(value);Context context{&token};
                return selected.Prepare(call.plan,operations(context),*call.completed);
            },&call);
    } catch(...){return false;}
}
KFC_RUNTIME_API bool __cdecl KfcRuntimeActivateSelectedV1(const SelectedStartup::PlanV2* plan,unsigned* completed) noexcept {
    if(!completed)return false;
    *completed=0;if(!plan)return false;
    Context context;return selected.Activate(*plan,operations(context),*completed);
}


}
