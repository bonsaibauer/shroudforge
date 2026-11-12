#pragma once
#include <cstdint>

// Internal presentation acknowledgements. No interception or graphics demand.
namespace NotificationOverlay {
std::uint64_t BeginFrame() noexcept;
void Presented(std::uint64_t frame, std::uint64_t now) noexcept;
void Stop() noexcept;
}
