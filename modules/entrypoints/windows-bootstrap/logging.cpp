#include "bootstrap_state.h"
#include <atomic>

using namespace BootstrapImplementation;


namespace BootstrapImplementation {
auto session_started = std::chrono::steady_clock::now();
std::atomic_bool runtime_ready{false};

std::string single_line(std::string_view value) {
    std::string result;
    result.reserve(value.size());
    for (const auto character : value) {
        if (character == '\r') result += "\\r";
        else if (character == '\n') result += "\\n";
        else result += character;
    }
    return result;
}

std::string windows_error(DWORD code) {
    wchar_t* message{};
    const auto length = FormatMessageW(FORMAT_MESSAGE_ALLOCATE_BUFFER |
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
        nullptr, code, 0, reinterpret_cast<wchar_t*>(&message), 0, nullptr);
    std::string result = "Windows error " + std::to_string(code);
    if (length && message) {
        const auto bytes = WideCharToMultiByte(CP_UTF8, 0, message,
            static_cast<int>(length), nullptr, 0, nullptr, nullptr);
        if (bytes > 0) {
            std::string utf8(static_cast<size_t>(bytes), '\0');
            WideCharToMultiByte(CP_UTF8, 0, message, static_cast<int>(length),
                utf8.data(), bytes, nullptr, nullptr);
            while (!utf8.empty() && (utf8.back() == '\r' || utf8.back() == '\n' || utf8.back() == ' ')) utf8.pop_back();
            if (!utf8.empty()) result += " (" + utf8 + ")";
        }
        LocalFree(message);
    }
    return result;
}

struct LogGuard {
    HANDLE mutex{CreateMutexW(nullptr, FALSE, L"Local\\ShroudForgeLog")};
    DWORD wait_result{mutex ? WaitForSingleObject(mutex, INFINITE) : WAIT_FAILED};
    bool held{wait_result == WAIT_OBJECT_0 || wait_result == WAIT_ABANDONED};
    ~LogGuard() { if (held) ReleaseMutex(mutex); if (mutex) CloseHandle(mutex); }
};

void begin_log_session(const std::filesystem::path& root) {
    runtime_ready.store(false, std::memory_order_release);
    LogGuard guard;
    if (!guard.held) return;
    try {
        const auto archive = ShroudforgeConfig::Directory(root, "logs", "shroudforge/logs");
        std::filesystem::create_directories(archive);
        const auto stamp = std::chrono::duration_cast<std::chrono::seconds>(
            std::chrono::system_clock::now().time_since_epoch()).count();
        const auto role = is_dedicated_server_process() ? L"server" : L"client";
        const auto current = archive / (std::wstring(L"shroudforge-") + role + L".log");
        if (std::filesystem::is_regular_file(current) && std::filesystem::file_size(current) > 0) {
            auto destination = archive / (std::wstring(L"shroudforge-") + role + L"-" +
                std::to_wstring(stamp) + L".log");
            for (unsigned suffix = 2; std::filesystem::exists(destination); ++suffix) {
                destination = archive / (std::wstring(L"shroudforge-") + role + L"-" +
                    std::to_wstring(stamp) + L"-" +
                    std::to_wstring(suffix) + L".log");
            }
            std::filesystem::rename(current, destination);
        }

        const auto pending = archive / (std::wstring(L"native-proxy-") + role + L".pending");
        const auto legacy_pending = archive / L"native-proxy.pending";
        for (const auto& pending_path : {pending, legacy_pending}) {
          if (std::filesystem::is_regular_file(pending_path)) {
            std::ifstream input(pending_path, std::ios::binary);
            std::ofstream output(current, std::ios::binary | std::ios::app);
            if (input && output) {
                output << input.rdbuf();
                output.flush();
                if (output) std::filesystem::remove(pending_path);
            }
          }
        }
    } catch (...) {
        // Logging remains available in append mode if archival is unavailable.
    }
}

void log(char level, const std::string& message) {
    if (!ShroudforgeConfig::Allows(module_directory(),level)) return;
    const auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now() - session_started).count();
    char prefix[96]{};
    std::snprintf(prefix, sizeof(prefix), "[%c %02lld:%02lld:%02lld,%03lld] [bootstrap] ",
        level, elapsed / 3600000, (elapsed / 60000) % 60, (elapsed / 1000) % 60,
        elapsed % 1000);
    const auto line = std::string(prefix) + single_line(message) + '\n';
    OutputDebugStringA(line.c_str());
    const auto root = module_directory();
    if (root.empty()) return;
    LogGuard guard;
    if (!guard.held) return;
    const auto role = is_dedicated_server_process() ? L"server" : L"client";
    const auto current = ShroudforgeConfig::Directory(root, "logs", "shroudforge/logs") /
        (std::wstring(L"shroudforge-") + role + L".log");
    std::filesystem::create_directories(current.parent_path());
    std::ofstream stream(current, std::ios::app);
    if (stream) {
        stream << line;
        stream.flush();
    }
    if ((level == 'E' || level == 'W') && console_show_event) {
        const auto debug_console = ShroudforgeConfig::Read(root)
            .value("modules", nlohmann::json::object())
            .value("debugConsole", nlohmann::json::object());
        const auto popup_mode = debug_console.value("popupMode", std::string("bootstrap"));
        const bool popup_enabled = debug_console.value("enabled", true) &&
            ((popup_mode == "all" && level == 'E') ||
                (popup_mode == "errorsAndWarnings") ||
                (popup_mode == "bootstrap" && level == 'E' &&
                    !runtime_ready.load(std::memory_order_acquire)));
        if (popup_enabled) SetEvent(console_show_event);
    }
}

void log(const std::string& message) {
    log('I', message);
}

void log_shroudforge_build_identity(const std::filesystem::path& root) {
    std::string version = "unknown";
    std::string build_type = "unknown";
    std::string build_number = "unknown";
    try {
        std::ifstream input(root / L"shroudforge" / L"version.json", std::ios::binary);
        if (input) {
            const auto manifest = nlohmann::json::parse(input);
            if (manifest.is_object()) {
                const auto text_value = [&manifest](const char* key) {
                    const auto value = manifest.find(key);
                    if (value == manifest.end()) return std::string("unknown");
                    if (value->is_string()) return value->get<std::string>();
                    if (value->is_number()) return value->dump();
                    return std::string("unknown");
                };
                version = text_value("version");
                const auto candidate_type = text_value("buildType");
                if (candidate_type == "dev" || candidate_type == "release") {
                    build_type = candidate_type;
                }
                build_number = text_value("build");
            }
        }
    } catch (...) {
        // Keep the session header useful even when an older or damaged manifest is present.
    }
    log("ShroudForge version=" + version + " build=" + build_type +
        " buildNumber=" + build_number);
}

void log_shroudforge_banner(bool dedicated_server) {
    log(R"(   _____ __  ______  ____  __  ______  __________  ____  ____________)");
    log(R"(  / ___// / / / __ \/ __ \/ / / / __ \/ ____/ __ \/ __ \/ ____/ ____/)");
    log(R"(  \__ \/ /_/ / /_/ / / / / / / / / / / /_  / / / / /_/ / / __/ __/   )");
    log(R"( ___/ / __  / _, _/ /_/ / /_/ / /_/ / __/ / /_/ / _, _/ /_/ / /___   )");
    log(R"(/____/_/ /_/_/ |_|\____/\____/_____/_/    \____/_/ |_|\____/_____/   )");
    if (dedicated_server) {
        log(R"(  / ___// ____/ __ \ |  / / ____/ __ \                                )");
        log(R"(  \__ \/ __/ / /_/ / | / / __/ / /_/ /                                )");
        log(R"( ___/ / /___/ _, _/| |/ / /___/ _, _/                                 )");
        log(R"(/____/_____/_/ |_| |___/_____/_/ |_|                                  )");
    }
}

void startup_stage(const std::filesystem::path& root, const char* stage) {
    (void)root;
    if (std::string_view(stage) == "runtime-ready") {
        runtime_ready.store(true, std::memory_order_release);
    }
    log('D', std::string("Startup stage: ") + stage);
}

std::string_view status_value(std::string_view status, std::string_view key) {
    const auto position = status.find(key);
    if (position == std::string_view::npos) return {};
    const auto start = position + key.size();
    const auto end = status.find(' ', start);
    return status.substr(start, end == std::string_view::npos ? end : end - start);
}

std::string_view runtime_thread_state(std::string_view value) {
    if (value.find("stale-game-thread") == 0) return "stale";
    if (value.find("installed-awaiting-world") == 0) return "awaiting-world";
    if (value.find("ready(") == 0) return "ready";
    if (value.find("unavailable") == 0) return "unavailable";
    return "unknown";
}

std::string runtime_status_state(std::string_view status) {
    const auto registry = status_value(status, "registry=");
    const auto layout = status_value(status, "layout=");
    const auto entity_access = status_value(status, "entity_access=");
    const auto entity_realm = status_value(status, "entity_realm=");
    const auto game_thread = runtime_thread_state(status_value(status, "game_thread="));
    const auto voxel_context = status_value(status, "voxel_context=");
    return "registry=" + std::string(registry.empty() ? "unknown" : registry) +
        " layout=" + std::string(layout.empty() ? "unknown" : layout) +
        " entity_access=" + std::string(entity_access.empty() ? "unavailable" : entity_access) +
        " entity_realm=" + std::string(entity_realm.empty() ? "unknown" : entity_realm) +
        " game_thread=" + std::string(game_thread) +
        " voxel_context=" + std::string(voxel_context.empty() ? "unknown" : voxel_context);
}

void show_startup_error(const std::filesystem::path& root, const std::string& detail) {
    if (!ShroudforgeConfig::ModuleEnabled(root, "debugConsole")) return;
    try {
        const auto debug_console = ShroudforgeConfig::Read(root).at("modules").at("debugConsole");
        if (debug_console.value("popupMode", std::string("bootstrap")) == "off") return;
    } catch (...) {}
    auto text = std::wstring(detail.begin(), detail.end());
    text += L"\n\nShroudForge log:\n";
    const auto role = is_dedicated_server_process() ? L"server" : L"client";
    text += (ShroudforgeConfig::Directory(root, "logs", "shroudforge/logs") /
        (std::wstring(L"shroudforge-") + role + L".log")).wstring();
    MessageBoxW(nullptr, text.c_str(), L"ShroudForge startup error",
        MB_OK | MB_ICONERROR | MB_SETFOREGROUND);
}
}
