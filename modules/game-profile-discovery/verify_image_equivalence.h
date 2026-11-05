#pragma once
#include "discover_game_profile.h"
#include <vector>

namespace GameProfileDiscovery::Detail {
struct Section {
    std::string name;
    std::uint32_t rva{}, virtual_size{}, flags{};
    std::vector<std::uint8_t> bytes;
};
struct Image {
    ImageIdentity identity;
    std::vector<std::uint8_t> file_bytes;
    std::vector<std::uint8_t> normalized_headers;
    std::vector<Section> sections;
};

Image ReadImage(const std::filesystem::path& path, bool read_adjacent_kfc = true);
bool Equivalent(const Image& source, const Image& target, nlohmann::json& evidence);
}
