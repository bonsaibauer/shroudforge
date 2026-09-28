#pragma once

#include <cstdint>
#include <cstddef>
#include <string>
#include <memory>

namespace GameThreadDispatcher {
using Operation = void (*)(void* context);

bool Initialize();
void Shutdown();
bool Ready();
bool EntityContextReady();
std::uint32_t ThreadId();
std::uintptr_t EntityManager();
std::string Status();
std::string Diagnostics();
bool WriteCode(std::uintptr_t address, const void* expected, const void* replacement, std::size_t size);

// Execute immediately when already on the captured engine thread. Otherwise
// enqueue and wait until the continuous Keen update consumes the command.
// Jobs own all input/output storage, including after a caller times out.
bool Invoke(Operation operation, std::shared_ptr<void> context, std::uint32_t timeout_ms = 75);
}
