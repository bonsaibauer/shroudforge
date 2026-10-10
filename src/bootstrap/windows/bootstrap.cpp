#include <windows.h>

#include "runtime_bridge.h"
#include "logging_config.h"
#include "startup_gate.h"

#include <chrono>
#include <condition_variable>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <mutex>
#include <string>
#include <string_view>
#include <ctime>
#include <thread>

namespace {
using CreateRuntime = void* (__cdecl*)(const wchar_t*);
using PrepareStartup = bool (__cdecl*)(const wchar_t*);
using UpdateRuntime = bool (__cdecl*)(void*, double);
using DestroyRuntime = void (__cdecl*)(void*);

HMODULE self_module{};
HANDLE stop_event{};
HANDLE runtime_thread{};
HANDLE console_stop_event{};
HANDLE console_process{};
HANDLE console_show_event{};
HANDLE modloader_ui_stop_event{};
HANDLE modloader_ui_process{};
HANDLE world_editor_ui_stop_event{};
HANDLE world_editor_ui_process{};
auto session_started = std::chrono::steady_clock::now();

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

std::filesystem::path module_directory() {
    std::wstring path(32768, L'\0');
    const auto length = GetModuleFileNameW(self_module, path.data(), static_cast<DWORD>(path.size()));
    if (length == 0 || length == path.size()) return {};
    path.resize(length);
    return std::filesystem::path(path).parent_path();
}

bool is_dedicated_server_process() {
    wchar_t path[32768]{};
    const auto length = GetModuleFileNameW(nullptr, path, static_cast<DWORD>(std::size(path)));
    if (length == 0 || length >= std::size(path)) return false;
    const std::wstring_view executable(path, length);
    const auto separator = executable.find_last_of(L"\\/");
    const auto name = separator == std::wstring_view::npos
        ? executable
        : executable.substr(separator + 1);
    return CompareStringOrdinal(name.data(), static_cast<int>(name.size()),
        L"enshrouded_server.exe", -1, TRUE) == CSTR_EQUAL;
}

std::filesystem::path shroudforge_directory(const std::filesystem::path& root) {
    return root / L"shroudforge";
}

void begin_log_session(const std::filesystem::path& root) {
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
    if (level == 'E' && console_show_event) SetEvent(console_show_event);
}

void log(const std::string& message) {
    log('I', message);
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

void write_runtime_heartbeat(const std::filesystem::path& root) {
    const auto directory = ShroudforgeConfig::Directory(root, "runtime", "shroudforge/runtime");
    std::error_code error;
    std::filesystem::create_directories(directory, error);
    if (error) return;
    const auto path = directory / L"heartbeat.json";
    const auto temporary = directory / L"heartbeat.json.tmp";
    const auto now = std::chrono::duration_cast<std::chrono::seconds>(
        std::chrono::system_clock::now().time_since_epoch()).count();
    std::ofstream stream(temporary, std::ios::trunc);
    if (!stream) return;
    stream << "{\"schemaVersion\":1,\"pid\":" << GetCurrentProcessId()
        << ",\"mode\":\"" << (std::filesystem::is_regular_file(root / L"enshrouded.exe") ? "CLIENT" : "SERVER")
        << "\",\"updatedAt\":" << now << "}";
    stream.close();
    if (stream) MoveFileExW(temporary.c_str(), path.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH);
}

void startup_stage(const std::filesystem::path& root, const char* stage) {
    (void)root;
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
    const auto game_thread = runtime_thread_state(status_value(status, "game_thread="));
    const auto voxel_context = status_value(status, "voxel_context=");
    return "registry=" + std::string(registry.empty() ? "unknown" : registry) +
        " layout=" + std::string(layout.empty() ? "unknown" : layout) +
        " game_thread=" + std::string(game_thread) +
        " voxel_context=" + std::string(voxel_context.empty() ? "unknown" : voxel_context);
}

void startup_failed(const std::filesystem::path& root, const char* stage,
        const std::string& detail) {
    (void)root;
    log('E', std::string("Startup failed at ") + stage + ": " + detail);
}

void show_startup_error(const std::filesystem::path& root, const std::string& detail) {
    auto text = std::wstring(detail.begin(), detail.end());
    text += L"\n\nShroudForge log:\n";
    const auto role = is_dedicated_server_process() ? L"server" : L"client";
    text += (ShroudforgeConfig::Directory(root, "logs", "shroudforge/logs") /
        (std::wstring(L"shroudforge-") + role + L".log")).wstring();
    MessageBoxW(nullptr, text.c_str(), L"ShroudForge startup error",
        MB_OK | MB_ICONERROR | MB_SETFOREGROUND);
}

void clear_runtime_heartbeat(const std::filesystem::path& root) {
    std::error_code ignored;
    std::filesystem::remove(ShroudforgeConfig::Directory(root, "runtime", "shroudforge/runtime") / L"heartbeat.json", ignored);
}

void start_debug_console(const std::filesystem::path& root) {
    if (is_dedicated_server_process()) {
        log('D', "Dedicated server uses file logging. Interactive Debug Console is skipped");
        return;
    }
    if (!std::filesystem::is_regular_file(root / L"enshrouded.exe") &&
        !std::filesystem::is_regular_file(root / L"enshrouded_server.exe")) return;
    const auto executable = shroudforge_directory(root) / L"shroudforge.exe";
    if (!std::filesystem::is_regular_file(executable)) {
        const std::string detail = "Debug Console executable is missing: " + executable.string();
        log('E', detail);
        show_startup_error(root, detail);
        return;
    }
    const auto pid = GetCurrentProcessId();
    const auto event_name = L"Local\\ShroudForge.DebugConsole." + std::to_wstring(pid);
    const auto show_name = L"Local\\ShroudForge.DebugConsole.Show." + std::to_wstring(pid);
    console_stop_event = CreateEventW(nullptr, TRUE, FALSE, event_name.c_str());
    if (!console_stop_event) {
        const auto detail = "Debug Console stop event could not be created. " + windows_error(GetLastError());
        log('E', detail);
        show_startup_error(root, detail);
        return;
    }
    console_show_event = CreateEventW(nullptr, TRUE, FALSE, show_name.c_str());
    if (!console_show_event) {
        const auto detail = "Debug Console error event could not be created. " + windows_error(GetLastError());
        log('E', detail);
        show_startup_error(root, detail);
        CloseHandle(console_stop_event);
        console_stop_event = nullptr;
        return;
    }
    std::error_code log_size_error;
    const auto log_start_offset = std::filesystem::file_size(
        ShroudforgeConfig::Directory(root, "logs", "shroudforge/logs") /
            (std::wstring(L"shroudforge-") + (is_dedicated_server_process() ? L"server" : L"client") + L".log"), log_size_error);
    const auto start_offset = log_size_error ? 0 : log_start_offset;
    std::wstring command = L"\"" + executable.wstring() + L"\" --debug-console --root \"" +
        root.wstring() + L"\" --game-pid " + std::to_wstring(pid) +
        L" --stop-event \"" + event_name + L"\" --show-event \"" + show_name +
        L"\" --log-start-offset " + std::to_wstring(start_offset) + L" --startup-watch";
    STARTUPINFOW startup{sizeof(startup)};
    PROCESS_INFORMATION process{};
    if (!CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, FALSE,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT, nullptr, root.c_str(), &startup, &process)) {
        const auto error = GetLastError();
        const auto detail = "Debug Console process could not be started. " + windows_error(error);
        log('E', detail);
        show_startup_error(root, detail);
        CloseHandle(console_show_event);
        console_show_event = nullptr;
        CloseHandle(console_stop_event);
        console_stop_event = nullptr;
        return;
    }
    CloseHandle(process.hThread);
    console_process = process.hProcess;
    log('D', "Debug Console module started. Press F10 to toggle.");
}

void stop_debug_console() {
    if (console_stop_event) SetEvent(console_stop_event);
    if (console_process) {
        if (WaitForSingleObject(console_process, 3000) != WAIT_OBJECT_0) {
            log('W', "Debug Console did not stop after its stop event. Terminating the owned process");
            if (TerminateProcess(console_process, 1)) WaitForSingleObject(console_process, 1000);
            else log('E', "Could not terminate the owned Debug Console process");
        }
        CloseHandle(console_process);
        console_process = nullptr;
    }
    if (console_stop_event) {
        CloseHandle(console_stop_event);
        console_stop_event = nullptr;
    }
    if (console_show_event) {
        CloseHandle(console_show_event);
        console_show_event = nullptr;
    }
}

void start_modloader_ui(const std::filesystem::path& root) {
    if (is_dedicated_server_process()) {
        log('D', "Modloader UI window is skipped in the Dedicated Server process");
        return;
    }
    if (!ShroudforgeConfig::ModuleEnabled(root,"modloaderUi")) return;
    const auto executable = shroudforge_directory(root) / L"shroudforge.exe";
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
    std::wstring command = L"\"" + executable.wstring() + L"\" --module-ui --root \"" +
        root.wstring() + L"\" --game-pid " + std::to_wstring(pid) +
        L" --stop-event \"" + event_name + L"\"";
    STARTUPINFOW startup{sizeof(startup)};
    PROCESS_INFORMATION process{};
    if (!CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, FALSE,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT, nullptr, root.c_str(), &startup, &process)) {
        log('E', "Modloader UI process could not be started");
        CloseHandle(modloader_ui_stop_event);
        modloader_ui_stop_event = nullptr;
        return;
    }
    CloseHandle(process.hThread);
    modloader_ui_process = process.hProcess;
    log('D', "Modloader UI module started. Press F9 to toggle.");
}

void stop_modloader_ui() {
    if (modloader_ui_stop_event) SetEvent(modloader_ui_stop_event);
    if (modloader_ui_process) {
        if (WaitForSingleObject(modloader_ui_process, 3000) != WAIT_OBJECT_0) {
            log('W', "Modloader UI did not stop after its stop event. Terminating the owned process");
            if (TerminateProcess(modloader_ui_process, 1)) WaitForSingleObject(modloader_ui_process, 1000);
            else log('E', "Could not terminate the owned Modloader UI process");
        }
        CloseHandle(modloader_ui_process);
        modloader_ui_process = nullptr;
    }
    if (modloader_ui_stop_event) {
        CloseHandle(modloader_ui_stop_event);
        modloader_ui_stop_event = nullptr;
    }
}

void start_world_editor_ui(const std::filesystem::path& root) {
    if (is_dedicated_server_process()) {
        log('D', "World Editor window is skipped in the Dedicated Server process");
        return;
    }
    const auto executable = shroudforge_directory(root) / L"shroudforge.exe";
    if (!std::filesystem::is_regular_file(executable)) {
        log('W', "World Editor window host is not installed");
        return;
    }
    const auto pid = GetCurrentProcessId();
    const auto event_name = L"Local\\ShroudForge.WorldEditorUI." + std::to_wstring(pid);
    world_editor_ui_stop_event = CreateEventW(nullptr, TRUE, FALSE, event_name.c_str());
    if (!world_editor_ui_stop_event) {
        log('E', "World Editor window host stop event could not be created");
        return;
    }
    std::wstring command = L"\"" + executable.wstring() + L"\" --world-editor-ui --root \"" +
        root.wstring() + L"\" --game-pid " + std::to_wstring(pid) +
        L" --stop-event \"" + event_name + L"\"";
    STARTUPINFOW startup{sizeof(startup)};
    PROCESS_INFORMATION process{};
    if (!CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, FALSE,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT, nullptr, root.c_str(), &startup, &process)) {
        log('E', "World Editor window host process could not be started");
        CloseHandle(world_editor_ui_stop_event);
        world_editor_ui_stop_event = nullptr;
        return;
    }
    CloseHandle(process.hThread);
    world_editor_ui_process = process.hProcess;
    log('D', "World Editor window host started. F1-F8 open its strip and F2 toggles it.");
}

void stop_world_editor_ui() {
    if (world_editor_ui_stop_event) SetEvent(world_editor_ui_stop_event);
    if (world_editor_ui_process) {
        if (WaitForSingleObject(world_editor_ui_process, 3000) != WAIT_OBJECT_0) {
            log('W', "World Editor window host did not stop after its stop event. Terminating the owned process");
            if (TerminateProcess(world_editor_ui_process, 1)) WaitForSingleObject(world_editor_ui_process, 1000);
        }
        CloseHandle(world_editor_ui_process);
        world_editor_ui_process = nullptr;
    }
    if (world_editor_ui_stop_event) {
        CloseHandle(world_editor_ui_stop_event);
        world_editor_ui_stop_event = nullptr;
    }
}

void start_pending_update(const std::filesystem::path& root) {
    const auto updates = ShroudforgeConfig::Directory(root, "updates", "shroudforge/updates");
    const auto staged = updates / L"pending";
    const auto marker = updates / L"pending.ready";
    const auto executable = shroudforge_directory(root) / L"shroudforge-updater.exe";
    if (!std::filesystem::is_regular_file(marker) ||
        !std::filesystem::is_regular_file(staged / L"shroudforge" / L"shroudforge.exe") ||
        !std::filesystem::is_regular_file(executable)) return;

    std::wstring command = L"\"" + executable.wstring() + L"\" --queue-install --root \"" +
        root.wstring() + L"\" --wait-pid " + std::to_wstring(GetCurrentProcessId());
    STARTUPINFOW startup{sizeof(startup)};
    PROCESS_INFORMATION process{};
    if (!CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, FALSE,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT, nullptr, root.c_str(),
            &startup, &process)) {
        log('E', "Independent updater request could not be started");
        return;
    }
    CloseHandle(process.hThread);
    const auto result = WaitForSingleObject(process.hProcess, 5000);
    if (result == WAIT_OBJECT_0) {
        DWORD exit_code = 1;
        GetExitCodeProcess(process.hProcess, &exit_code);
        if (exit_code == 0) log("Pending update queued for independent installation after game exit");
        else log('E', "Independent updater rejected pending update request");
    } else {
        log('W', "Independent updater request is still starting. Check the updater log");
    }
    CloseHandle(process.hProcess);
}

DWORD WINAPI run(void*) {
    // Never touch KFC files until the main thread is parked outside loader lock.
    // A late-loaded proxy cannot safely prepare assets for an already running game.
    if (WaitForSingleObject(StartupGate::entered, 30000) != WAIT_OBJECT_0) {
        StartupGate::Complete(false);
        return 1;
    }
    struct CompleteStartup {
        bool released = false;
        ~CompleteStartup() { if (!released) StartupGate::Complete(false); }
        void release() { released = true; StartupGate::Complete(true); }
    } startup;
    const auto root = module_directory();
    if (root.empty()) {
        log('E', "Bootstrap failed: loader directory is unavailable");
        return 1;
    }
    begin_log_session(root);
    session_started = std::chrono::steady_clock::now();
    clear_runtime_heartbeat(root);
    startup_stage(root, "bootstrap-thread-started");
    log("Log session started for game PID " + std::to_string(GetCurrentProcessId()) +
        ". Archived log files contain earlier sessions");
    log_shroudforge_banner(is_dedicated_server_process());
    // Start the independent log viewer before any ShroudForge DLL or mod code
    // can fail during process startup.
    start_debug_console(root);
    const auto runtime_path = shroudforge_directory(root) / L"shroudforge-runtime.dll";
    startup_stage(root, "runtime-dll-load");
    const auto runtime = LoadLibraryW(runtime_path.c_str());
    if (!runtime) {
        const auto error = GetLastError();
        startup_failed(root, "runtime-dll-load", "shroudforge-runtime.dll could not be loaded. " + windows_error(error));
        EcsRuntime::Shutdown();
        return 1;
    }
    const auto prepare_startup = reinterpret_cast<PrepareStartup>(
        GetProcAddress(runtime, "shroudforge_prepare_startup"));
    const auto create = reinterpret_cast<CreateRuntime>(GetProcAddress(runtime, "shroudforge_create"));
    const auto update = reinterpret_cast<UpdateRuntime>(GetProcAddress(runtime, "shroudforge_update"));
    const auto destroy = reinterpret_cast<DestroyRuntime>(GetProcAddress(runtime, "shroudforge_destroy"));
    if (!prepare_startup || !create || !update || !destroy) {
        startup_failed(root, "runtime-exports", "runtime exports are incomplete");
        FreeLibrary(runtime);
        return 1;
    }
    startup_stage(root, "runtime-exports-resolved");
    start_modloader_ui(root);
    start_world_editor_ui(root);
    startup_stage(root, "startup-assets");
    if (!prepare_startup(root.c_str())) {
        startup_failed(root, "startup-assets", "Asset preparation and baseline recovery failed. Game startup stopped before loading inconsistent assets");
        FreeLibrary(runtime);
        return 1;
    }
    // Install native hooks while the executable entrypoint is still parked.
    // Releasing it first races hook writes against short-lived startup threads.
    startup_stage(root, "native-provider-init");
    if (!EcsRuntime::Initialize()) {
        log('W', "KFC Runtime initialization failed: " + EcsRuntime::Status());
    }
    log("Startup assets and native-provider setup complete. Releasing the game entrypoint");
    startup.release();
    startup_stage(root, "runtime-create");
    std::mutex create_watchdog_mutex;
    std::condition_variable create_watchdog_condition;
    bool create_finished = false;
    std::thread create_watchdog;
    try {
        create_watchdog = std::thread([&] {
            std::unique_lock lock(create_watchdog_mutex);
            if (!create_watchdog_condition.wait_for(lock, std::chrono::seconds(15),
                    [&] { return create_finished; })) {
                lock.unlock();
                log('E', "Runtime initialization has not returned after 15 seconds at runtime-create. Inspect the last [shroudforge::runtime] startup stage above");
            }
        });
    } catch (...) {
        log('W', "Could not start runtime-create watchdog");
    }
    void* handle = create(root.c_str());
    {
        std::lock_guard lock(create_watchdog_mutex);
        create_finished = true;
    }
    create_watchdog_condition.notify_one();
    if (create_watchdog.joinable()) create_watchdog.join();
    if (!handle) {
        startup_failed(root, "runtime-create", "Runtime initialization returned no handle. See preceding runtime log entries");
        EcsRuntime::Shutdown();
        FreeLibrary(runtime);
        return 1;
    }
    startup_stage(root, "runtime-ready");
    log("Runtime initialized");
    auto previous = std::chrono::steady_clock::now();
    auto next_runtime_status = previous;
    std::string previous_runtime_status;
    while (WaitForSingleObject(stop_event, 16) == WAIT_TIMEOUT) {
        const auto now = std::chrono::steady_clock::now();
        const auto delta = std::chrono::duration<double>(now - previous).count();
        previous = now;
        EcsRuntime::Tick();
        if (now >= next_runtime_status) {
            write_runtime_heartbeat(root);
            const auto status = EcsRuntime::Status();
            log('D', "KFC Runtime details " + status);
            const auto state = runtime_status_state(status);
            if (state != previous_runtime_status) {
                log("KFC Runtime state: " + state);
                previous_runtime_status = state;
            }
            next_runtime_status = now + std::chrono::seconds(2);
        }
        if (!update(handle, delta)) {
            startup_failed(root, "runtime-update", "Runtime update failed. Stopping the Lua runtime");
            break;
        }
    }
    destroy(handle);
    EcsRuntime::Shutdown();
    stop_world_editor_ui();
    stop_modloader_ui();
    stop_debug_console();
    clear_runtime_heartbeat(root);
    FreeLibrary(runtime);
    log("Runtime stopped");
    start_pending_update(root);
    return 0;
}

bool is_supported_game_process() {
    wchar_t executable_path[32768]{};
    const auto length = GetModuleFileNameW(nullptr, executable_path,
        static_cast<DWORD>(sizeof(executable_path) / sizeof(executable_path[0])));
    if (length == 0 || length >= sizeof(executable_path) / sizeof(executable_path[0])) return false;
    const std::wstring_view path(executable_path, length);
    const auto separator = path.find_last_of(L"\\/");
    const auto name = separator == std::wstring_view::npos ? path : path.substr(separator + 1);
    return CompareStringOrdinal(name.data(), static_cast<int>(name.size()),
        L"enshrouded.exe", -1, TRUE) == CSTR_EQUAL ||
        CompareStringOrdinal(name.data(), static_cast<int>(name.size()),
        L"enshrouded_server.exe", -1, TRUE) == CSTR_EQUAL;
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
        // winmm.dll is found beside every executable in the game directory.
        // The bundled ShroudForge UI also runs there, but must never bootstrap
        // another runtime or race Enshrouded's asset startup.
        if (!is_supported_game_process()) return TRUE;
        stop_event = CreateEventW(nullptr, TRUE, FALSE, nullptr);
        if (!stop_event) return FALSE;
        if (!StartupGate::Install()) return FALSE;
        runtime_thread = CreateThread(nullptr, 0, run, nullptr, 0, nullptr);
        return runtime_thread != nullptr;
    }
    if (reason == DLL_PROCESS_DETACH && reserved == nullptr && stop_event) {
        SetEvent(stop_event);
    }
    return TRUE;
}
