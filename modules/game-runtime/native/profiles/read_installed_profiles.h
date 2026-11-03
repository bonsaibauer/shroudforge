#pragma once
#include <filesystem>
#include <string>
#include <vector>
#include <nlohmann/json.hpp>

namespace KfcRuntimeCompatibility {
struct InstalledProfiles {
    std::filesystem::path directory;
    std::string requested_id;
    std::string generated_profile_id;
    std::vector<nlohmann::json> profiles;
    std::vector<std::string> errors;
    nlohmann::json discovery = {{"enabled", false}, {"state", "disabled"}};
};

// Both native profile classes use the loader's existing runtime path. Reading
// this catalog neither selects a profile nor authorizes engine access.
InstalledProfiles ReadInstalledProfiles(const std::filesystem::path& process_path,
                                       const std::string& target,
                                       const nlohmann::json& image = nlohmann::json{});
}
