#pragma once

#include "camera_layout.h"
#include "stamina_layout.h"

#include <array>
#include <cstddef>
#include <cstdint>
#include <string_view>

namespace FirstPersonControl {

inline bool MatchesSupportedImage(
    const std::uint8_t* image,
    std::size_t imageSize,
    std::uintptr_t loadedBase,
    std::string_view sha256,
    const std::array<std::uint8_t, CameraCallSize>& cameraCall,
    const std::array<std::uint8_t, StaminaCallSize>& staminaCall,
    const std::array<std::uint8_t, CameraSystemEntry.size()>& cameraSystemEntry) {
    return imageSize == ClientImageSize &&
        sha256 == ClientSha256 &&
        MatchesClientPeIdentity(image, imageSize) &&
        MatchesCameraLayout(image, imageSize, loadedBase, cameraCall, cameraSystemEntry) &&
        MatchesStaminaLayout(image, imageSize, loadedBase, staminaCall);
}

} // namespace FirstPersonControl
