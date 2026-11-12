#pragma once
namespace EngineHooks {
bool Install();
// Cached required game-thread + entity-manager hook publication, not a World.
bool StartupReady();
void Uninstall();
}
