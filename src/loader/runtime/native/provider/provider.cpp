#include "ecs_runtime.h"
#include "world_runtime.h"
#include <algorithm>
#include <cstring>
#include <string>
extern "C" {
KFC_EXPORT unsigned __cdecl KfcRuntimeAbi() { return 7; }
KFC_EXPORT bool __cdecl KfcRuntimeInitialize() {
    try { return EcsRuntime::Initialize(); } catch (...) { EcsRuntime::Shutdown(); return false; }
}
KFC_EXPORT void __cdecl KfcRuntimeTick() {
    try { EcsRuntime::Tick(); } catch (...) { EcsRuntime::Shutdown(); }
}
KFC_EXPORT void __cdecl KfcRuntimeShutdown() { EcsRuntime::Shutdown(); }
KFC_EXPORT void __cdecl KfcRuntimeDiagnostics(char* buffer, std::size_t capacity) {
    if (!buffer || !capacity) return;
    const auto write_error = [buffer, capacity](const char* message, std::size_t required = 0) {
        const auto text = std::string("{\"error\":\"") + message +
            (required ? "\",\"requiredBytes\":" + std::to_string(required) : "\"") + "}";
        const auto size = (std::min)(text.size(), capacity - 1);
        std::memcpy(buffer, text.data(), size); buffer[size] = 0;
    };
    try {
        const auto text = EcsRuntime::Diagnostics();
        if (text.size() >= capacity) { write_error("report-too-large", text.size() + 1); return; }
        std::memcpy(buffer, text.data(), text.size()); buffer[text.size()] = 0;
    } catch (...) { write_error("diagnostics-exception"); }
}
KFC_EXPORT void __cdecl KfcRuntimeStatus(char* buffer, std::size_t capacity) {
    if (!buffer || !capacity) return;
    try {
        const auto text = EcsRuntime::Status();
        const auto size = (std::min)(text.size(), capacity - 1);
        std::memcpy(buffer, text.data(), size); buffer[size] = 0;
    } catch (...) { buffer[0] = 0; }
}
}
