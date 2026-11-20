#pragma once
#include <atomic>
namespace ProviderImplementation {
extern std::atomic<bool> provider_initialized;
bool initialize_provider(bool enable_execution_hooks, bool selected);
void shutdown_provider_components() noexcept;
bool selected_startup_attempted() noexcept;
void close_selected_startup() noexcept;
}
