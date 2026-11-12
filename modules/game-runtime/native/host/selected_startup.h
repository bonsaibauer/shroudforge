#pragma once
#include <cstdint>

// Optional bootstrap/provider contract, separate from the stable ABI12 layout.
namespace SelectedStartup {
enum Service : std::uint32_t {
    Metadata = 1, GenericHooks = 2, MapCapture = 4, Graphics = 16,
};
constexpr std::uint32_t AllServices = Metadata | GenericHooks | MapCapture | Graphics;
enum Role : std::uint32_t { Client = 1, DedicatedServer = 2 };
struct PlanV2 {
    std::uint32_t struct_bytes = sizeof(PlanV2), version = 2;
    std::uint32_t services = Metadata, process_role = Client, opt_ins = 0, reserved = 0;
};
static_assert(sizeof(PlanV2) == 24);
constexpr bool Same(const PlanV2& a, const PlanV2& b) noexcept {
    return a.struct_bytes == b.struct_bytes && a.version == b.version &&
        a.services == b.services && a.process_role == b.process_role &&
        a.opt_ins == b.opt_ins && a.reserved == b.reserved;
}
constexpr bool Valid(const PlanV2& p) noexcept {
    if (p.struct_bytes != sizeof(PlanV2) || p.version != 2 || p.reserved ||
        (p.services & ~AllServices) || !(p.services & Metadata) || p.opt_ins ||
        (p.process_role != Client && p.process_role != DedicatedServer)) return false;
    if (p.process_role != Client && (p.services & (MapCapture | Graphics))) return false;
    return true;
}
constexpr std::uint32_t PreparedServices(const PlanV2& p) noexcept { return p.services; }
}

#include "../abi/runtime_api.h"
#include "../engine-hooks/held_entry_preparation.h"
extern "C" {
KFC_RUNTIME_API bool __cdecl KfcRuntimePrepareSelectedV1(const SelectedStartup::PlanV2*,
    const EngineHooks::HeldEntryProof*,const wchar_t*,unsigned,unsigned*) noexcept;
KFC_RUNTIME_API bool __cdecl KfcRuntimeActivateSelectedV1(const SelectedStartup::PlanV2*,unsigned*) noexcept;
}
