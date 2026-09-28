#pragma once
#include <cstdint>
#include <string>

namespace PatchRuntime {
bool Initialize();
void Shutdown();
bool Available(const char* name);
bool AnyAvailable();
bool SetEnabled(const char* name, bool enabled, std::uint32_t* outcome);
std::string Diagnostics();
}
