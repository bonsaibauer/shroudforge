#pragma once
#include <cstdint>
// Exact dynamic-attach validation only. No private RVA or publisher function
// is invoked, and no executable bytes or attach parameters are changed.
namespace GreeneyeDynamic {
bool Begin(unsigned char* entry,const unsigned char* original) noexcept;
bool Witness(std::uintptr_t loaded_module) noexcept;
bool Finalize() noexcept;
const char* Failure() noexcept;
}
