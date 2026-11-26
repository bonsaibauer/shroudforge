#include "profiles/read_hud_capture_profile.h"
#include <nlohmann/json.hpp>
#include <cstring>
#include <stdexcept>

namespace KfcRuntimeCompatibility {
HudCaptureSelection ReadHudCaptureProfile(const nlohmann::json& profile) {
    if (!profile.contains("hudCapture")) return {};
    try {
        using json = nlohmann::json;
        const auto number = [](const json& object, const char* key, std::uint64_t min, std::uint64_t max) {
            const auto& value = object.at(key);
            if (!value.is_number_integer() ||
                (!value.is_number_unsigned() && value.get<std::int64_t>() < 0) ||
                value.get<std::uint64_t>() < min || value.get<std::uint64_t>() > max)
                throw std::runtime_error(std::string("invalid HUD value: ") + key);
            return static_cast<std::uint32_t>(value.get<std::uint64_t>());
        };
        const auto& model = profile.at("hudCapture");
        if (profile.at("target") != "enshrouded.exe" || !model.is_object() || model.size() != 13 ||
            model.at("id") != "hud-observation-v1")
            throw std::runtime_error("unsupported HUD capture contract");
        const auto image_size = number(profile.at("image"), "size", 4096, UINT32_MAX);
        auto result = std::make_shared<HudCaptureProfile>();
        result->image_size = image_size;
        result->profile_id = profile.at("id").get<std::string>();
        if (profile.contains("profileRevision"))
            result->revision = number(profile, "profileRevision", 1, UINT32_MAX);
        result->call_rva = number(model, "callRva", 1, image_size - 5);
        result->renderer_rva = number(model, "rendererRva", 1, image_size - 1);
        result->context_rva = number(model, "contextGlobalRva", 1, image_size - sizeof(std::uintptr_t));
        result->holder_offset = number(model, "holderOffset", 0, 65536);
        result->play_state_offset = number(model, "playStateOffset", 0, 65536);
        result->screen_offset = number(model, "screenOffset", 0, 65536);
        result->maximum_age_ms = number(model, "maximumAgeMs", 1, 5000);
        result->minimum_screen = static_cast<std::uint8_t>(number(model, "minimumScreen", 1, 255));
        result->maximum_screen = static_cast<std::uint8_t>(number(model, "maximumScreen", result->minimum_screen, 255));
        result->gameplay_screen = static_cast<std::uint8_t>(number(model, "gameplayScreen", result->minimum_screen, result->maximum_screen));
        const auto policy = model.at("guardPolicy").get<std::string>();
        if (policy != "exact" && policy != "runtime-owned-prefix")
            throw std::runtime_error("unsupported HUD guard policy");
        result->runtime_owned_prefix = policy == "runtime-owned-prefix";
        const auto& guards = model.at("guards");
        if (!guards.is_array() || guards.size() < 2 || guards.size() > 32)
            throw std::runtime_error("invalid HUD guard count");
        const auto nibble = [](char value) -> unsigned {
            if (value >= '0' && value <= '9') return value - '0';
            if (value >= 'a' && value <= 'f') return value - 'a' + 10;
            throw std::runtime_error("invalid HUD guard hex");
        };
        for (const auto& value : guards) {
            if (!value.is_object() || value.size() != 2) throw std::runtime_error("invalid HUD guard shape");
            HudCaptureProfile::Guard guard;
            guard.rva = number(value, "guardRva", 1, image_size - 1);
            const auto hex = value.at("bytes").get<std::string>();
            if (hex.empty() || hex.size() > 128 || hex.size() % 2)
                throw std::runtime_error("invalid HUD guard length");
            for (std::size_t i = 0; i < hex.size(); i += 2)
                guard.bytes.push_back(static_cast<std::uint8_t>((nibble(hex[i]) << 4) | nibble(hex[i + 1])));
            if (guard.bytes.size() > image_size - guard.rva)
                throw std::runtime_error("HUD guard exceeds image");
            for (const auto& previous : result->guards)
                if (guard.rva < previous.rva + previous.bytes.size() && previous.rva < guard.rva + guard.bytes.size())
                    throw std::runtime_error("overlapping HUD guards");
            result->guards.push_back(std::move(guard));
        }
        bool call{}, renderer{};
        for (const auto& guard : result->guards) {
            renderer |= guard.rva == result->renderer_rva;
            if (guard.rva != result->call_rva) continue;
            if (guard.bytes.size() < 5 || guard.bytes[0] != 0xe8)
                throw std::runtime_error("HUD guard has no original relative CALL");
            std::int32_t displacement{};
            std::memcpy(&displacement, guard.bytes.data() + 1, sizeof(displacement));
            if (std::int64_t(result->call_rva) + 5 + displacement != result->renderer_rva)
                throw std::runtime_error("HUD CALL target differs from renderer");
            call = true;
        }
        if (!call || !renderer) throw std::runtime_error("HUD entry or CALL guard missing");
        return {std::move(result), "ready"};
    } catch (const nlohmann::json::exception& error) {
        return {{}, std::string("invalid-hud-capture-profile: ") + error.what()};
    } catch (const std::runtime_error& error) {
        return {{}, std::string("invalid-hud-capture-profile: ") + error.what()};
    }
}
}
