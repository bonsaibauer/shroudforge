#pragma once
#include <cstdint>
#include <string>

namespace NativeMainMenu {
// Process-owned native UI binding. Menu discovery does not depend on an ECS profile.
void Initialize() noexcept;
void Tick() noexcept;
void Shutdown() noexcept;
std::uint32_t MainMenuState() noexcept;
std::string Status();
}
