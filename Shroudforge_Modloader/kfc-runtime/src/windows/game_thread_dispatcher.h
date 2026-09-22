#pragma once

#include <cstdint>
#include <string>

namespace GameThreadDispatcher {
using Operation = void (*)(void* context);

bool Initialize();
void Shutdown();
bool Ready();
std::uint32_t ThreadId();
std::uintptr_t EntityManager();
std::string Status();

// Execute immediately when already on the captured engine thread. Otherwise
// enqueue and wait until the continuous Keen update consumes the command.
bool Invoke(Operation operation, void* context, std::uint32_t timeout_ms = 3000);
}
