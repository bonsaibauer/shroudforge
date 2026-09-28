#pragma once

#include <string>

namespace EcsRuntime {
bool Initialize();
void Tick();
std::string Status();
void Shutdown();
}
