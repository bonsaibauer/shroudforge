#pragma once
#include <cstdint>
#include <cstddef>
#include <string>
namespace MapRuntime {
bool PreflightHudPreparation(std::uintptr_t image,std::size_t image_size);
bool InstallPreparedHudHooks();
bool ValidatePreparedHudActivation();
bool ActivatePreparedHudVisibility();
bool PreflightLegacyHudVisibility(std::uintptr_t image,std::size_t image_size);
bool InstallPreflightedLegacyHudVisibility();
// Preparation and this snapshot run under the Map capture control mutex.
std::string HudPreparationStatus();
void DisableHudVisibility();
bool HudInputAllowed(std::uint64_t now);
bool HudGameplay(std::uint64_t now);
bool HudKnown(std::uint64_t now);
}
