#pragma once
#include <cstdint>

namespace BuildingInput {
bool Available();
// Called only by the verified client_cursor hook in the ordinary game input path.
void Observe(void* cursor, void* execution_view);
void Reset();
}
