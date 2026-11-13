// Exercise the real host lifecycle with inert service owners.
#include "host/manage_runtime_lifecycle.cpp"
DWORD WINAPI SelectedFixtureModuleName(HMODULE,wchar_t* path,DWORD count) {
    const wchar_t name[]=L"enshrouded.exe";
    if(count<std::size(name))return 0;
    std::memcpy(path,name,sizeof(name));return static_cast<DWORD>(std::size(name)-1);
}
#define GetModuleFileNameW SelectedFixtureModuleName
#include "host/prepare_selected_startup.cpp"
#undef GetModuleFileNameW
#include <cstring>
#include <cstdio>
#include <cstdlib>
#include <thread>
#include <vector>

namespace {
std::vector<unsigned> stopped;
unsigned ticks{}, resource_initializations{};
bool fail_tick{}, fail_stop{}, fail_initialization{}, hooks_ready{}, selected_execution{};
bool held_entry_callback{};
unsigned native_plugin_preparations{};
std::vector<unsigned> prepared;
struct ActivationProbe { bool revalidation_succeeds{}; unsigned revalidations{}, stops{}; };
bool activation_probe_metadata(void*, bool) { return true; }
bool activation_probe_revalidate(void* context) {
    auto& probe = *static_cast<ActivationProbe*>(context);
    ++probe.revalidations;
    return probe.revalidation_succeeds;
}
void activation_probe_stop(void* context) {
    ++static_cast<ActivationProbe*>(context)->stops;
}
void check(bool ok) { if (!ok) { std::fputs("Runtime owner retirement failed\n", stderr); std::exit(1); } }
void verify_stopped() { check(stopped == std::vector<unsigned>{1, 3, 4, 5, 6, 7, 8}); }
bool __cdecl stage_native_plugins_fixture() {
    check(held_entry_callback);
    prepared.push_back(0);
    return true;
}
bool __cdecl prepare_native_plugins_fixture() {
    check(held_entry_callback);
    ++native_plugin_preparations;
    prepared.push_back(9);
    return true;
}
bool entry_restored_fixture() {
    check(!held_entry_callback);
    prepared.push_back(10);
    return true;
}
}
namespace EcsRuntime {
bool Initialize() { return true; }
bool Initialize(bool execution) {selected_execution=execution;prepared.push_back(1);return true;}
void Tick() { ++ticks; }
void Shutdown() { stopped.push_back(8); }
}
namespace ResourceRuntime {
bool Initialize() { ++resource_initializations; if (fail_initialization) throw 1; return true; }
bool ProfileLoaded() { return false; }
void Shutdown() { stopped.push_back(7); }
}
namespace NativeMainMenu {
void Initialize() noexcept {}
void Tick() noexcept {}
void Shutdown() noexcept { stopped.push_back(6); }
}
namespace WorldRuntime::RegisteredVoxelGrid {
void PrepareWriter() {}
void StopPreparingWriter() { stopped.push_back(1); if (fail_stop) throw 1; }
}

namespace MapRuntime {
void StopCapture() noexcept { stopped.push_back(3); }
std::int32_t PrepareCapture(const EngineHooks::HeldEntryToken& token) noexcept {prepared.push_back(2);return token.ValidForCurrentThread()?0:1;}
}
namespace OverlayRuntime {
bool PrepareGraphicsBoot(bool) {prepared.push_back(4);return true;}
void TickServices() { if (fail_tick) throw 1; }
void ShutdownServices() { stopped.push_back(4); }
}
namespace EngineHooks {
bool StartupReady() { return hooks_ready; }
bool HeldEntryToken::ValidForCurrentThread() const noexcept {return proof_.is_held && proof_.is_held(proof_.parked_thread_id);}
bool RunHeldEntryPreparation(const HeldEntryProof& proof, HeldEntryCallback callback, void* context) noexcept {
    if(!proof.is_held || !proof.is_held(proof.parked_thread_id))return false;
    HeldEntryToken token(proof,GetCurrentThreadId());
    held_entry_callback=true;
    const bool result=callback(token,context);
    held_entry_callback=false;
    return result;
}
}
extern "C" void __cdecl KfcRuntimeNetworkServiceShutdown() { stopped.push_back(5); }

int main(int argc,char**) {
    {
        SelectedStartup::PlanV2 plan;
        plan.services = SelectedStartup::Metadata;
        ActivationProbe pass{true}, reject{false};
        const auto probe_operations = [](ActivationProbe& probe) {
            return SelectedStartup::Operations{&probe, activation_probe_metadata, nullptr,
                nullptr, nullptr, nullptr, nullptr, activation_probe_revalidate,
                activation_probe_stop};
        };
        SelectedStartup::State accepted;
        auto completed = 0u;
        check(accepted.Prepare(plan, probe_operations(pass), completed) &&
            completed == SelectedStartup::Metadata);
        check(accepted.Activate(plan, probe_operations(pass), completed) &&
            completed == SelectedStartup::Metadata && pass.revalidations == 1 &&
            accepted.ActiveServices() == SelectedStartup::Metadata);

        SelectedStartup::State denied;
        check(denied.Prepare(plan, probe_operations(reject), completed));
        check(!denied.Activate(plan, probe_operations(reject), completed) && completed == 0 &&
            reject.revalidations == 1 && reject.stops == 1 && denied.ActiveServices() == 0 &&
            std::strcmp(denied.FailureStage(), "executable-span-revalidation") == 0);
    }
    if(argc>1) {
        fail_initialization=true;
        SelectedStartup::PlanV2 plan;plan.services=SelectedStartup::Metadata|SelectedStartup::MapCapture;
        EngineHooks::HeldEntryProof proof;proof.parked_thread_id=123;
        proof.is_held=+[](std::uint32_t thread) noexcept{return thread==123;};
        unsigned completed=999;
        check(!KfcRuntimePrepareSelectedV1(&plan,&proof,nullptr,0,&completed) && completed==0);
        verify_stopped();
        check(prepared==std::vector<unsigned>{1});
        check(!KfcRuntimeInitialize() && !KfcRuntimeActivateSelectedV1(&plan,&completed));
        check(!KfcRuntimePrepareSelectedV1(&plan,&proof,nullptr,0,&completed));
        verify_stopped();
        std::puts("PASS: failed selected preparation retires once and cannot restart");
        return 0;
    }
    check(!KfcRuntimeStartupReady());
    check(KfcRuntimeInitialize());
    check(!KfcRuntimeStartupReady());
    hooks_ready = true;
    check(KfcRuntimeStartupReady());
    fail_stop = true;
    KfcRuntimeShutdown();
    verify_stopped();
    check(!KfcRuntimeStartupReady());
    const auto before = resource_initializations;
    KfcRuntimeTick();
    check(ticks == 0 && resource_initializations == before);

    stopped.clear();
    check(KfcRuntimeInitialize());
    fail_tick = true;
    KfcRuntimeTick();
    verify_stopped();
    check(ticks == 1 && !ProviderImplementation::provider_initialized.load());
    KfcRuntimeTick();
    check(ticks == 1);

    stopped.clear();
    fail_initialization = true;
    check(!KfcRuntimeInitialize());
    verify_stopped();
    fail_initialization=fail_stop=fail_tick=false;stopped.clear();prepared.clear();
    SelectedStartup::PlanV2 plan;plan.services=SelectedStartup::Metadata|SelectedStartup::MapCapture|
        SelectedStartup::NativePluginPreentry|SelectedStartup::Graphics;
    EngineHooks::HeldEntryProof proof;unsigned completed=999;
    check(!KfcRuntimePrepareSelectedV1(&plan,&proof,nullptr,0,&completed) && completed==0 && prepared.empty());
    proof.parked_thread_id=123;proof.is_held=+[](std::uint32_t thread) noexcept{return thread==123;};
    for (const auto removed : {32u,64u,128u}) {
        auto unsupported=plan;unsupported.services|=removed;
        check(!KfcRuntimePrepareSelectedV1(&unsupported,&proof,nullptr,0,&completed) && completed==0 && prepared.empty());
    }
    for (const auto opt_in : {1u,2u,4u}) {
        auto unsupported=plan;unsupported.opt_ins=opt_in;
        check(!KfcRuntimePrepareSelectedV1(&unsupported,&proof,nullptr,0,&completed) && completed==0 && prepared.empty());
    }
    auto invalid=plan;invalid.services|=32u|64u;invalid.opt_ins=5;
    check(!KfcRuntimePrepareSelectedV1(&invalid,&proof,nullptr,0,&completed) && prepared.empty());
    invalid=plan;invalid.process_role=SelectedStartup::DedicatedServer;
    check(!KfcRuntimePrepareSelectedV1(&invalid,&proof,nullptr,0,&completed));
    check(!KfcRuntimePrepareSelectedV1(&plan,&proof,nullptr,0,&completed) && completed==0 && prepared.empty());
    check(KfcRuntimePrepareSelectedV2(&plan,&proof,stage_native_plugins_fixture,
        prepare_native_plugins_fixture,&completed) &&
        completed==plan.services && !selected_execution);
    check(prepared==std::vector<unsigned>({0,1,2,4,9}) && native_plugin_preparations==1 && !KfcRuntimeInitialize());
    bool continuation_succeeded{};
    std::thread original_entry([&] {
        continuation_succeeded = entry_restored_fixture() &&
            KfcRuntimeActivateSelectedV1(&plan,&completed) && completed==plan.services;
    });
    original_entry.join();
    check(continuation_succeeded);
    check(prepared==std::vector<unsigned>({0,1,2,4,9,10}));
    const auto selected_resources=resource_initializations;KfcRuntimeTick();check(resource_initializations==selected_resources);
    check(!KfcRuntimeActivateSelectedV1(&plan,&completed));
    stopped.clear();KfcRuntimeShutdown();verify_stopped();
    check(!KfcRuntimeInitialize() && !KfcRuntimePrepareSelectedV1(&plan,&proof,nullptr,0,&completed));
    std::puts("PASS: runtime lifecycle and actual selected export order, masks, held-entry and terminal retirement");
}
