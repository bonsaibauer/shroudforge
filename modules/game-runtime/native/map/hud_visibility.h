#pragma once
#include <cstdint>
#include <cstddef>
#include <string>
namespace MapRuntime {
bool PrepareHudVisibility(std::uintptr_t image,std::size_t image_size);
// Preparation and this snapshot run under the Map capture control mutex.
std::string HudPreparationStatus();
bool FinalizeHudOriginalOwners();
bool EnableHudVisibility(std::uintptr_t image,std::size_t image_size);
void DisableHudVisibility();
bool HudInputAllowed(std::uint64_t now);
bool HudGameplay(std::uint64_t now);
bool HudKnown(std::uint64_t now);
}
