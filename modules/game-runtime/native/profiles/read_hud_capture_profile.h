#pragma once
#include <cstdint>
#include <memory>
#include <string>
#include <vector>
#include <nlohmann/json_fwd.hpp>

namespace KfcRuntimeCompatibility {
// Immutable selected-profile data. No engine access or hook ownership is
// granted by parsing this description.
struct HudCaptureProfile {
    struct Guard { std::uint32_t rva{}; std::vector<std::uint8_t> bytes; };
    std::string profile_id;
    std::uint32_t revision{}, image_size{}, call_rva{}, renderer_rva{}, context_rva{};
    std::uint32_t holder_offset{}, play_state_offset{}, screen_offset{}, maximum_age_ms{};
    std::uint8_t minimum_screen{}, maximum_screen{}, gameplay_screen{};
    bool runtime_owned_prefix{};
    std::vector<Guard> guards;
};
struct HudCaptureSelection {
    std::shared_ptr<const HudCaptureProfile> profile;
    std::string status{"hud-capture-profile-absent"};
};
HudCaptureSelection ReadHudCaptureProfile(const nlohmann::json& exact_profile);
}
