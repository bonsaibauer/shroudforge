#pragma once
#include <nlohmann/json.hpp>
#include <vector>
#include <string>

namespace ResourceImplementation {
struct ResourceProfileSelection {
    nlohmann::json profile;
    nlohmann::json diagnostics;
    std::string error;
};

ResourceProfileSelection select_resource_profile(const nlohmann::json& image, unsigned kfc_format,
    const std::vector<nlohmann::json>& profiles);
}
