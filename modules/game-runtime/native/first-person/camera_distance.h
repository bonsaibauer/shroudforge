#pragma once

#include <cmath>

namespace FirstPersonControl {

inline constexpr float DefaultCameraDistance = -0.15f;

inline bool ValidCameraDistance(float distance) noexcept {
    return std::isfinite(distance) && distance >= -0.5f && distance <= 0.5f;
}

} // namespace FirstPersonControl
