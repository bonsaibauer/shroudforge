#pragma once
#include <cstdint>
#include <span>
namespace OverlayRuntime {
// Complete Vulkan 1.0 validation followed by the narrower overlay interface.
// Validation consumes only the already bounded, host-owned word copy.
bool ValidateOverlayFragment(std::span<const std::uint32_t> words);
}
