#include "bootstrap_state.h"

using namespace BootstrapImplementation;


namespace BootstrapImplementation {
long long seconds_now() {
    return std::chrono::duration_cast<std::chrono::seconds>(
        std::chrono::system_clock::now().time_since_epoch()).count();
}

void write_runtime_heartbeat(const std::filesystem::path& root, long long updated_at) {
    const auto directory = ShroudforgeConfig::Directory(root, "runtime", "shroudforge/runtime");
    std::error_code error;
    std::filesystem::create_directories(directory, error);
    if (error) return;
    const auto path = directory / L"heartbeat.json";
    const auto temporary = directory / L"heartbeat.json.tmp";
    std::ofstream stream(temporary, std::ios::trunc);
    if (!stream) return;
    stream << "{\"schemaVersion\":1,\"pid\":" << GetCurrentProcessId()
        << ",\"mode\":\"" << (std::filesystem::is_regular_file(root / L"enshrouded.exe") ? "CLIENT" : "SERVER")
        << "\",\"updatedAt\":" << updated_at << "}";
    stream.close();
    if (stream) MoveFileExW(temporary.c_str(), path.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH);
}

void clear_runtime_heartbeat(const std::filesystem::path& root) {
    std::error_code ignored;
    std::filesystem::remove(ShroudforgeConfig::Directory(root, "runtime", "shroudforge/runtime") / L"heartbeat.json", ignored);
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
    // Check the release manifest before starting any UI or loading runtime DLLs.
    startup_stage(root, "startup-update-check");
    if (!run_startup_update_check(root)) return 1;
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
    startup_stage(root, "startup-preparation");
    if (!prepare_startup(root.c_str())) {
        startup_failed(root, "startup-preparation", "Startup preparation failed. See the preceding asset or DLL Manager error. Game startup was stopped before releasing the entrypoint");
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
    // The heartbeat is written by a thread of its own: its flush to the disk
    // held every mod up to half a second while the game loads. updatedAt
    // stays the loop's last turn, so a stalled loop still reads as inactive.
    std::atomic<long long> last_turn{seconds_now()};
    const HANDLE heartbeat_stop = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    std::thread heartbeat;
    if (heartbeat_stop) {
        try {
            heartbeat = std::thread([&] {
                do write_runtime_heartbeat(root, last_turn.load(std::memory_order_relaxed));
                while (WaitForSingleObject(heartbeat_stop, 2000) == WAIT_TIMEOUT);
            });
        } catch (...) {
            log('W', "Could not start the heartbeat thread: the mods loop writes it");
        }
    }
    const bool heartbeat_thread = heartbeat.joinable();
    while (WaitForSingleObject(stop_event, 16) == WAIT_TIMEOUT) {
        const auto now = std::chrono::steady_clock::now();
        const auto delta = std::chrono::duration<double>(now - previous).count();
        previous = now;
        last_turn.store(seconds_now(), std::memory_order_relaxed);
        EcsRuntime::Tick();
        if (now >= next_runtime_status) {
            if (!heartbeat_thread) write_runtime_heartbeat(root, last_turn.load(std::memory_order_relaxed));
            const auto status = EcsRuntime::Status();
            const auto state = runtime_status_state(status);
            if (state != previous_runtime_status) {
                log('D', "KFC Runtime details " + status);
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
    if (heartbeat_stop) SetEvent(heartbeat_stop);
    if (heartbeat.joinable()) heartbeat.join();
    if (heartbeat_stop) CloseHandle(heartbeat_stop);
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
}
