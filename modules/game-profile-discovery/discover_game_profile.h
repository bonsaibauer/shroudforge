#pragma once
#include <cstdint>
#include <filesystem>
#include <optional>
#include <string>
#include <vector>
#include <nlohmann/json.hpp>

namespace GameProfileDiscovery {
struct ImageIdentity {
    std::string target;
    std::string sha256;
    std::uint32_t timestamp{};
    std::uint32_t size{};
    std::uint8_t kfc_format{};
};

struct Result {
    std::optional<nlohmann::json> profile;
    nlohmann::json diagnostics;
    std::filesystem::path profile_path;
};

// The caller owns configuration, catalog discovery and normal-before-beta
// selection. Call only when the optional discovery setting is enabled.
// This module never loads or executes a cached executable.
Result Prepare(const std::filesystem::path& executable_path,
               const std::filesystem::path& profile_directory,
               const std::filesystem::path& cache_directory,
               const std::vector<nlohmann::json>& catalog,
               const ImageIdentity& identity);
}
