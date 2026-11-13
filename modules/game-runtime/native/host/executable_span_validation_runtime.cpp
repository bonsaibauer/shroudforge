#include "host/executable_span_validation.h"
#include "abi/runtime_api.h"
#include "engine-hooks/held_entry_preparation.h"
#include <windows.h>

namespace {
bool ReadMainImageCode(std::uint32_t rva, std::uint8_t* output,
        std::size_t size) noexcept {
    return output && size && size <= 512 &&
        KfcRuntimeFunctionCode(rva, output, size) == size;
}

bool HeldEntryActive() noexcept {
    return EngineHooks::HeldEntryPreparationActiveOnCurrentThread();
}

bool ExecutableCallback(SFExecutableSpanValidationCallbackV1 callback) noexcept {
    if (!callback) return false;
    const auto address = reinterpret_cast<std::uintptr_t>(callback);
    MEMORY_BASIC_INFORMATION region{};
    if (VirtualQuery(reinterpret_cast<const void*>(address), &region, sizeof(region)) != sizeof(region) ||
        region.State != MEM_COMMIT || (region.Protect & (PAGE_GUARD | PAGE_NOACCESS))) return false;
    const auto protection = region.Protect & 0xff;
    if (protection != PAGE_EXECUTE && protection != PAGE_EXECUTE_READ &&
        protection != PAGE_EXECUTE_READWRITE && protection != PAGE_EXECUTE_WRITECOPY) return false;
    const auto start = reinterpret_cast<std::uintptr_t>(region.BaseAddress);
    return address >= start && address - start < region.RegionSize;
}
}

namespace ExecutableSpanValidation {
Registry& Services() noexcept {
    static auto* registry = new Registry(ReadMainImageCode, HeldEntryActive, ExecutableCallback);
    return *registry;
}

bool RevalidateAllRegistered() noexcept {
    return Services().RevalidateAllRegistered();
}
}
