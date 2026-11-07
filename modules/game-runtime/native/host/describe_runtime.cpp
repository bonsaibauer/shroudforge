#include "abi/runtime_api.h"
#include "ecs/entity_session.h"
#include "loaded-resources/loaded_resource_access.h"
#include "main-menu/native_main_menu.h"
#include <algorithm>
#include <cstring>
#include <string>
#include <nlohmann/json.hpp>
#include "host/runtime_lifecycle_state.h"

using namespace ProviderImplementation;

extern "C" {
KFC_RUNTIME_API void __cdecl KfcRuntimeDiagnostics(char* buffer, std::size_t capacity) {
    if (!buffer || !capacity) return;
    const auto write_error = [buffer, capacity](const char* message, std::size_t required = 0) {
        const auto text = std::string("{\"error\":\"") + message +
            (required ? "\",\"requiredBytes\":" + std::to_string(required) : "\"") + "}";
        const auto size = (std::min)(text.size(), capacity - 1);
        std::memcpy(buffer, text.data(), size); buffer[size] = 0;
    };
    try {
        auto diagnostics = nlohmann::json::parse(EcsRuntime::Diagnostics());
        diagnostics["resourceService"] = nlohmann::json::parse(ResourceRuntime::Status());
        diagnostics["executionProfile"] = diagnostics["profile"];
        const auto active_profile = diagnostics["resourceService"].value("profileId", std::string{});
        if (!active_profile.empty()) diagnostics["profile"] = active_profile;
        diagnostics["nativeMainMenu"] = nlohmann::json::parse(NativeMainMenu::Status());
        const auto text = diagnostics.dump();
        if (text.size() >= capacity) { write_error("report-too-large", text.size() + 1); return; }
        std::memcpy(buffer, text.data(), text.size()); buffer[text.size()] = 0;
    } catch (...) { write_error("diagnostics-exception"); }
}

KFC_RUNTIME_API void __cdecl KfcRuntimeStatus(char* buffer, std::size_t capacity) {
    if (!buffer || !capacity) return;
    try {
        const auto resources = ResourceRuntime::Status();
        const auto active_profile = nlohmann::json::parse(resources).value("profileId", std::string{});
        const auto profile = active_profile.empty() ? std::string{} : "profile=" + active_profile + ' ';
        const auto text = profile + EcsRuntime::Status() + " resource_service=" + resources +
            " native_main_menu=" + NativeMainMenu::Status();
        const auto size = (std::min)(text.size(), capacity - 1);
        std::memcpy(buffer, text.data(), size); buffer[size] = 0;
    } catch (...) { buffer[0] = 0; }
}
}
