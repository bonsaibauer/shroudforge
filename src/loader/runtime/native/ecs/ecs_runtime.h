#pragma once

#include "runtime.h"
#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#define KFC_EXPORT KFC_RUNTIME_API

namespace EcsRuntime {
bool Initialize();
void Tick();
std::string Status();
std::string Diagnostics();
bool SnapshotEntityIds(std::vector<std::uint32_t>& ids);
bool ResolvePropEntityId(std::uint32_t entity_id, KfcRuntimePropRecord* prop);
bool EntityIdentity(std::uint32_t handle, std::uint32_t* entity_id);
// Only from a live engine callback. Checks generation and layout epoch.
bool MatchesComponentAddress(std::uint32_t handle, const char* name, const void* expected);
void Shutdown();
}
