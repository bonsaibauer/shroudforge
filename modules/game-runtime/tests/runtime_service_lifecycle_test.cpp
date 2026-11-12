// Exercise the real host lifecycle with inert service owners.
#include "host/manage_runtime_lifecycle.cpp"
#include <cstdio>
#include <cstdlib>
#include <vector>

namespace {
std::vector<unsigned> stopped;
unsigned ticks{}, resource_initializations{};
bool fail_tick{}, fail_stop{}, fail_initialization{}, hooks_ready{};
void check(bool ok) { if (!ok) { std::fputs("Runtime owner retirement failed\n", stderr); std::exit(1); } }
void verify_stopped() { check(stopped == std::vector<unsigned>{1, 2, 3, 4, 5, 6, 7, 8}); }
}
namespace EcsRuntime {
bool Initialize() { return true; }
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
namespace FirstPersonControl {
bool Prepare(const EngineHooks::HeldEntryToken&) noexcept { return false; }
void Shutdown() noexcept { stopped.push_back(2); }
}
namespace MapRuntime { void StopCapture() noexcept { stopped.push_back(3); } }
namespace OverlayRuntime {
bool PrepareGraphicsBoot(bool) { return true; }
void TickServices() { if (fail_tick) throw 1; }
void ShutdownServices() { stopped.push_back(4); }
}
namespace EngineHooks {
bool StartupReady() { return hooks_ready; }
bool RunHeldEntryPreparation(const HeldEntryProof&, HeldEntryCallback, void*) noexcept { return false; }
}
extern "C" void __cdecl KfcRuntimeNetworkServiceShutdown() { stopped.push_back(5); }

int main() {
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
    std::puts("PASS: partial startup, shutdown failure and terminal tick retirement");
}
