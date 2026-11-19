#include "bootstrap_state.h"

using namespace BootstrapImplementation;


namespace BootstrapImplementation {
HANDLE console_stop_event{};

HANDLE console_process{};

HANDLE console_show_event{};

HANDLE modloader_ui_stop_event{};

HANDLE modloader_ui_process{};

HANDLE world_editor_ui_stop_event{};

HANDLE world_editor_ui_process{};

bool run_startup_update_check(const std::filesystem::path& root) {
    const auto executable = shroudforge_directory(root) / L"shroudforge-updater.exe";
    if (!std::filesystem::is_regular_file(executable)) {
        log('W', "Updater executable is missing. Skipping the startup update check");
        return true;
    }
    if (!ShroudforgeConfig::ModuleEnabled(root, "updates")) {
        log('W', "Startup update service is unavailable because the Updates module is disabled in Settings");
        return true;
    }

    std::wstring command = L"\"" + executable.wstring() + L"\" --startup-check --root \"" +
        root.wstring() + L"\"";
    STARTUPINFOW startup{sizeof(startup)};
    PROCESS_INFORMATION process{};
    if (!CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, FALSE,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT, nullptr, root.c_str(),
            &startup, &process)) {
        const auto detail = "Startup update check could not start. " + windows_error(GetLastError());
        log('E', detail);
        if (!is_dedicated_server_process()) show_startup_error(root, detail);
        return false;
    }
    CloseHandle(process.hThread);
    const auto result = WaitForSingleObject(process.hProcess, 45000);
    if (result != WAIT_OBJECT_0) {
        if (result == WAIT_TIMEOUT) {
            log('W', "Startup update check timed out after 45 seconds. Game startup continues");
            TerminateProcess(process.hProcess, ERROR_TIMEOUT);
            WaitForSingleObject(process.hProcess, 1000);
        } else {
            log('W', "Could not wait for startup update check. " + windows_error(GetLastError()));
        }
        CloseHandle(process.hProcess);
        return true;
    }

    DWORD exit_code = 0;
    const bool read_exit_code = GetExitCodeProcess(process.hProcess, &exit_code) != FALSE;
    CloseHandle(process.hProcess);
    if (!read_exit_code) {
        log('W', "Could not read startup update check result. " + windows_error(GetLastError()));
        return true;
    }
    if (exit_code == 2) {
        log('E', "A required ShroudForge hotfix is available. Game startup is blocked until it is installed");
        return false;
    }
    if (exit_code != 0) {
        const auto detail = "Startup update check failed with exit code " + std::to_string(exit_code) +
            ". Game startup stopped. Inspect the updater log for the repair error";
        log('E', detail);
        if (!is_dedicated_server_process()) show_startup_error(root, detail);
        return false;
    }
    return true;
}

void start_debug_console(const std::filesystem::path& root) {
    if (is_dedicated_server_process()) {
        log('D', "Dedicated server uses file logging. Interactive Debug Console is skipped");
        return;
    }
    if (!ShroudforgeConfig::ModuleEnabled(root, "debugConsole")) {
        log('W', "Debug Console is unavailable because the module is disabled in Settings");
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
    log('D', "Debug Console module started. Use its configured shortcut to toggle.");
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

void start_modloader_ui(const std::filesystem::path& root, bool vanilla) {
    if (is_dedicated_server_process()) {
        log('D', "Modloader UI window is skipped in the Dedicated Server process");
        return;
    }
    if (!vanilla && !ShroudforgeConfig::ModuleEnabled(root, "modloaderUi")) {
        log('W', "Modloader UI service is unavailable because the module is disabled in Settings");
        return;
    }
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
        L" --stop-event \"" + event_name + L"\"" + (vanilla ? L" --vanilla-session" : L"");
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
    log('D', "Modloader UI module started. Use its configured shortcut to toggle.");
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
    if (!ShroudforgeConfig::ModuleEnabled(root, "worldEditor")) {
        log('W', "World Editor service is unavailable because the module is disabled in Settings");
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
    log('D', "World Editor window host started. Use its configured shortcut to toggle.");
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
    if (!ShroudforgeConfig::ModuleEnabled(root, "updates")) {
        if (std::filesystem::is_regular_file(marker)) {
            log('W', "Pending update was not installed because the Updates module is disabled in Settings");
        }
        return;
    }
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
}
