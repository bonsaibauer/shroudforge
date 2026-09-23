#include <windows.h>

#include "../../kfc-runtime/src/windows/ecs_runtime.h"
#include "../../kfc-runtime/src/windows/logging_config.h"

#include <chrono>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <string>

namespace {
using CreateRuntime = void* (__cdecl*)(const wchar_t*, const wchar_t*);
using UpdateRuntime = bool (__cdecl*)(void*, double);
using DestroyRuntime = void (__cdecl*)(void*);

HMODULE self_module{};
HANDLE stop_event{};
HANDLE runtime_thread{};
HANDLE console_stop_event{};
HANDLE console_process{};
HANDLE modloader_ui_stop_event{};
HANDLE modloader_ui_process{};
auto session_started = std::chrono::steady_clock::now();

struct LogGuard {
    HANDLE mutex{CreateMutexW(nullptr, FALSE, L"Local\\ShroudForgeLog")};
    DWORD wait_result{mutex ? WaitForSingleObject(mutex, INFINITE) : WAIT_FAILED};
    bool held{wait_result == WAIT_OBJECT_0 || wait_result == WAIT_ABANDONED};
    ~LogGuard() { if (held) ReleaseMutex(mutex); if (mutex) CloseHandle(mutex); }
};

std::filesystem::path module_directory() {
    std::wstring path(32768, L'\0');
    const auto length = GetModuleFileNameW(self_module, path.data(), static_cast<DWORD>(path.size()));
    if (length == 0 || length == path.size()) return {};
    path.resize(length);
    return std::filesystem::path(path).parent_path();
}

void begin_log_session(const std::filesystem::path& root) {
    LogGuard guard;
    if (!guard.held) return;
    try {
        const auto current = root / L"shroudforge.log";
        if (!std::filesystem::is_regular_file(current) || std::filesystem::file_size(current) == 0) return;
        const auto archive = root / L"logs";
        std::filesystem::create_directories(archive);
        const auto stamp = std::chrono::duration_cast<std::chrono::seconds>(
            std::chrono::system_clock::now().time_since_epoch()).count();
        auto destination = archive / (L"shroudforge-" + std::to_wstring(stamp) + L".log");
        for (unsigned suffix = 2; std::filesystem::exists(destination); ++suffix) {
            destination = archive / (L"shroudforge-" + std::to_wstring(stamp) + L"-" +
                std::to_wstring(suffix) + L".log");
        }
        std::filesystem::rename(current, destination);
    } catch (...) {
        // Logging remains available in append mode if archival is unavailable.
    }
}

void log(char level, const std::string& message) {
    if (!KfcRuntimeConfig::Allows(module_directory(),level)) return;
    const auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now() - session_started).count();
    char prefix[96]{};
    std::snprintf(prefix, sizeof(prefix), "[%c %02lld:%02lld:%02lld,%03lld] [bootstrap] ",
        level, elapsed / 3600000, (elapsed / 60000) % 60, (elapsed / 1000) % 60,
        elapsed % 1000);
    const auto line = std::string(prefix) + message + '\n';
    OutputDebugStringA(line.c_str());
    const auto root = module_directory();
    if (root.empty()) return;
    LogGuard guard;
    if (!guard.held) return;
    std::ofstream stream(root / "shroudforge.log", std::ios::app);
    if (stream) stream << line;
}

void log(const std::string& message) {
    log('I', message);
}

void start_debug_console(const std::filesystem::path& root) {
    if (!KfcRuntimeConfig::ModuleEnabled(root,"debugConsole")) return;
    if (!std::filesystem::is_regular_file(root / L"enshrouded.exe")) return;
    const auto executable = root / L"Shroudforge_Modules" / L"debug-console" /
        L"shroudforge-debug-console.exe";
    if (!std::filesystem::is_regular_file(executable)) {
        log('W', "Debug Console module is not installed");
        return;
    }
    const auto pid = GetCurrentProcessId();
    const auto event_name = L"Local\\ShroudForge.DebugConsole." + std::to_wstring(pid);
    console_stop_event = CreateEventW(nullptr, TRUE, FALSE, event_name.c_str());
    if (!console_stop_event) {
        log('E', "Debug Console stop event could not be created");
        return;
    }
    std::wstring command = L"\"" + executable.wstring() + L"\" --root \"" +
        root.wstring() + L"\" --game-pid " + std::to_wstring(pid) +
        L" --stop-event \"" + event_name + L"\"";
    STARTUPINFOW startup{sizeof(startup)};
    PROCESS_INFORMATION process{};
    if (!CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, FALSE,
            CREATE_UNICODE_ENVIRONMENT, nullptr, root.c_str(), &startup, &process)) {
        log('E', "Debug Console process could not be started");
        CloseHandle(console_stop_event);
        console_stop_event = nullptr;
        return;
    }
    CloseHandle(process.hThread);
    console_process = process.hProcess;
    log("Debug Console module started; press F10 to toggle");
}

void stop_debug_console() {
    if (console_stop_event) SetEvent(console_stop_event);
    if (console_process) {
        WaitForSingleObject(console_process, 3000);
        CloseHandle(console_process);
        console_process = nullptr;
    }
    if (console_stop_event) {
        CloseHandle(console_stop_event);
        console_stop_event = nullptr;
    }
}

void start_modloader_ui(const std::filesystem::path& root) {
    if (!KfcRuntimeConfig::ModuleEnabled(root,"modloaderUi")) return;
    const auto executable = root / L"Shroudforge_Modules" / L"modloader-ui" /
        L"shroudforge-modloader-ui.exe";
    if (!std::filesystem::is_regular_file(executable)) {
        log('W', "Modloader UI module is not installed");
        return;
    }
    const auto pid = GetCurrentProcessId();
    const auto event_name = L"Local\\ShroudForge.ModloaderUI." + std::to_wstring(pid);
    modloader_ui_stop_event = CreateEventW(nullptr, TRUE, FALSE, event_name.c_str());
    if (!modloader_ui_stop_event) {
        log('E', "Modloader UI stop event could not be created");
        return;
    }
    std::wstring command = L"\"" + executable.wstring() + L"\" --root \"" +
        root.wstring() + L"\" --game-pid " + std::to_wstring(pid) +
        L" --stop-event \"" + event_name + L"\"";
    STARTUPINFOW startup{sizeof(startup)};
    PROCESS_INFORMATION process{};
    if (!CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, FALSE,
            CREATE_UNICODE_ENVIRONMENT, nullptr, root.c_str(), &startup, &process)) {
        log('E', "Modloader UI process could not be started");
        CloseHandle(modloader_ui_stop_event);
        modloader_ui_stop_event = nullptr;
        return;
    }
    CloseHandle(process.hThread);
    modloader_ui_process = process.hProcess;
    log("Modloader UI module started; press F9 to toggle");
}

void stop_modloader_ui() {
    if (modloader_ui_stop_event) SetEvent(modloader_ui_stop_event);
    if (modloader_ui_process) {
        WaitForSingleObject(modloader_ui_process, 3000);
        CloseHandle(modloader_ui_process);
        modloader_ui_process = nullptr;
    }
    if (modloader_ui_stop_event) {
        CloseHandle(modloader_ui_stop_event);
        modloader_ui_stop_event = nullptr;
    }
}

void start_pending_update(const std::filesystem::path& root) {
    const auto staged = root / L"Shroudforge_Updates" / L"pending";
    const auto marker = root / L"Shroudforge_Updates" / L"pending.ready";
    const auto package = std::filesystem::is_regular_file(staged / L"version.json") ? staged : staged / L"game";
    const auto executable = package / L"Shroudforge_Updater" /
        L"shroudforge-updater.exe";
    if (!std::filesystem::is_regular_file(marker) ||
        !std::filesystem::is_regular_file(executable)) return;

    std::wstring command = L"\"" + executable.wstring() + L"\" --root \"" +
        root.wstring() + L"\" --staged \"" + staged.wstring() +
        L"\" --wait-pid " + std::to_wstring(GetCurrentProcessId());
    STARTUPINFOW startup{sizeof(startup)};
    PROCESS_INFORMATION process{};
    if (!CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, FALSE,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT, nullptr, root.c_str(),
            &startup, &process)) {
        log('E', "Pending updater could not be started");
        return;
    }
    CloseHandle(process.hThread);
    CloseHandle(process.hProcess);
    log("Pending update will be installed after the game exits");
}

DWORD WINAPI run(void*) {
    const auto root = module_directory();
    if (root.empty()) {
        log('E', "Bootstrap failed: loader directory is unavailable");
        return 1;
    }
    begin_log_session(root);
    session_started = std::chrono::steady_clock::now();
    if (!EcsRuntime::Initialize()) {
        log('W', "KFC Runtime unavailable for this game build; non-runtime mods remain available");
    }
    const auto runtime_path = root / L"shroudforge-runtime.dll";
    const auto runtime = LoadLibraryW(runtime_path.c_str());
    if (!runtime) {
        log('E', "Bootstrap failed: shroudforge-runtime.dll could not be loaded");
        EcsRuntime::Shutdown();
        return 1;
    }
    const auto create = reinterpret_cast<CreateRuntime>(GetProcAddress(runtime, "shroudforge_create"));
    const auto update = reinterpret_cast<UpdateRuntime>(GetProcAddress(runtime, "shroudforge_update"));
    const auto destroy = reinterpret_cast<DestroyRuntime>(GetProcAddress(runtime, "shroudforge_destroy"));
    if (!create || !update || !destroy) {
        log('E', "Bootstrap failed: runtime exports are incomplete");
        EcsRuntime::Shutdown();
        FreeLibrary(runtime);
        return 1;
    }
    void* handle = create(root.c_str(), (root / L"mods").c_str());
    if (!handle) {
        log('E', "Runtime initialization failed; no mod was activated");
        EcsRuntime::Shutdown();
        FreeLibrary(runtime);
        return 1;
    }
    log("Runtime initialized");
    start_debug_console(root);
    start_modloader_ui(root);
    auto previous = std::chrono::steady_clock::now();
    auto next_runtime_status = previous;
    std::string previous_runtime_status;
    while (WaitForSingleObject(stop_event, 16) == WAIT_TIMEOUT) {
        const auto now = std::chrono::steady_clock::now();
        const auto delta = std::chrono::duration<double>(now - previous).count();
        previous = now;
        EcsRuntime::Tick();
        if (now >= next_runtime_status) {
            const auto status = EcsRuntime::Status();
            if (status != previous_runtime_status) {
                log("KFC Runtime " + status);
                previous_runtime_status = status;
            }
            next_runtime_status = now + std::chrono::seconds(2);
        }
        if (!update(handle, delta)) {
            log('E', "Runtime update failed; stopping Lua runtime");
            break;
        }
    }
    destroy(handle);
    EcsRuntime::Shutdown();
    stop_modloader_ui();
    stop_debug_console();
    FreeLibrary(runtime);
    log("Runtime stopped");
    start_pending_update(root);
    return 0;
}
}

extern "C" __declspec(dllexport) BOOL __cdecl ShroudforgeStop(DWORD timeout_milliseconds) {
    if (stop_event) SetEvent(stop_event);
    if (!runtime_thread) return TRUE;
    return WaitForSingleObject(runtime_thread, timeout_milliseconds) == WAIT_OBJECT_0;
}

BOOL APIENTRY DllMain(HMODULE module, DWORD reason, LPVOID reserved) {
    if (reason == DLL_PROCESS_ATTACH) {
        self_module = module;
        DisableThreadLibraryCalls(module);
        stop_event = CreateEventW(nullptr, TRUE, FALSE, nullptr);
        if (!stop_event) return FALSE;
        runtime_thread = CreateThread(nullptr, 0, run, nullptr, 0, nullptr);
        return runtime_thread != nullptr;
    }
    if (reason == DLL_PROCESS_DETACH && reserved == nullptr && stop_event) {
        SetEvent(stop_event);
    }
    return TRUE;
}
