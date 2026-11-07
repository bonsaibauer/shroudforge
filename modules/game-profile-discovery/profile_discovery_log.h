#pragma once

#include <filesystem>
#include <string_view>

namespace GameProfileDiscovery {
// Sends a native discovery message through the loaded Rust runtime logger.
// The optional host export is separate from the stable KFC provider ABI.
void Log(const std::filesystem::path& game_root, char level,
         std::string_view message) noexcept;
}
