#pragma once

#include <windows.h>

#include "runtime_bridge.h"
#include "logging_config.h"
#include "startup_gate.h"

#include <atomic>
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


// Private bootstrap state and operations. Public DLL entrypoints stay in startup.cpp.
namespace BootstrapImplementation {

using CreateRuntime = void* (__cdecl*)(const wchar_t*);
using PrepareStartup = bool (__cdecl*)(const wchar_t*);
using UpdateRuntime = bool (__cdecl*)(void*, double);
using DestroyRuntime = void (__cdecl*)(void*);
extern HMODULE self_module;
extern HANDLE stop_event;
extern HANDLE runtime_thread;
extern HANDLE console_stop_event;
extern HANDLE console_process;
extern HANDLE console_show_event;
extern HANDLE modloader_ui_stop_event;
extern HANDLE modloader_ui_process;
extern HANDLE world_editor_ui_stop_event;
extern HANDLE world_editor_ui_process;
extern std::chrono::steady_clock::time_point session_started;
std::string single_line(std::string_view value);
std::string windows_error(DWORD code);
std::filesystem::path module_directory();
bool is_dedicated_server_process();
std::filesystem::path shroudforge_directory(const std::filesystem::path& root);
void begin_log_session(const std::filesystem::path& root);
void log(char level, const std::string& message);
void log(const std::string& message);
void log_shroudforge_banner(bool dedicated_server);
// updated_at: seconds since the epoch of the mods loop's last turn.
void write_runtime_heartbeat(const std::filesystem::path& root, long long updated_at);
void startup_stage(const std::filesystem::path& root, const char* stage);
std::string_view status_value(std::string_view status, std::string_view key);
std::string_view runtime_thread_state(std::string_view value);
std::string runtime_status_state(std::string_view status);
bool run_startup_update_check(const std::filesystem::path& root);
void startup_failed(const std::filesystem::path& root, const char* stage,
        const std::string& detail);
void show_startup_error(const std::filesystem::path& root, const std::string& detail);
void clear_runtime_heartbeat(const std::filesystem::path& root);
void start_debug_console(const std::filesystem::path& root);
void stop_debug_console();
void start_modloader_ui(const std::filesystem::path& root);
void stop_modloader_ui();
void start_world_editor_ui(const std::filesystem::path& root);
void stop_world_editor_ui();
void start_pending_update(const std::filesystem::path& root);
DWORD WINAPI run(void*);
bool is_supported_game_process();
}
