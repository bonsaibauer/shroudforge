#pragma once
#include "verify_image_equivalence.h"

namespace GameProfileDiscovery::Detail {
// Completes only unresolved, allow-listed private layouts by checking their
// candidate values against the exact executable and its current live process.
bool ResolveLiveProfileLayouts(const std::filesystem::path& executable_path,
    const ImageIdentity& identity, nlohmann::json& profile, const nlohmann::json& rebase_evidence,
    nlohmann::json& evidence);
}
