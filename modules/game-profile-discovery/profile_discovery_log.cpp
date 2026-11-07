#include "profile_discovery_log.h"
#include <windows.h>
#include <cstdint>
#include <string>

namespace GameProfileDiscovery {
namespace {
using RuntimeLog = void(__cdecl*)(std::uint8_t, const wchar_t*, const char*);

RuntimeLog runtime_log() noexcept {
    static const auto sink = []() noexcept -> RuntimeLog {
        const auto runtime = GetModuleHandleW(L"shroudforge-runtime.dll");
        if (!runtime) return nullptr;
        return reinterpret_cast<RuntimeLog>(GetProcAddress(
            runtime, "shroudforge_game_profile_discovery_log"));
    }();
    return sink;
}
}

void Log(const std::filesystem::path& game_root, char level,
         std::string_view message) noexcept {
    try {
        std::string terminated(message);
        if (const auto sink = runtime_log()) {
            sink(static_cast<std::uint8_t>(level), game_root.c_str(), terminated.c_str());
            return;
        }
        const auto line = std::string("[game-profile-discovery][") + level + "] " + terminated + "\n";
        OutputDebugStringA(line.c_str());
    } catch (...) {
        // Logging must never change profile discovery or block runtime startup.
    }
}
}
