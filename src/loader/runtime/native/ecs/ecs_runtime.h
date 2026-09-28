#pragma once

#include "runtime.h"
#include <cstddef>
#include <cstdint>
#include <string>

#define KFC_EXPORT KFC_RUNTIME_API

namespace EcsRuntime {
bool Initialize();
void Tick();
std::string Status();
std::string Diagnostics();
void Shutdown();
}
