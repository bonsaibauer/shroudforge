#pragma once
#include "profile_rebase_analysis.h"

namespace GameProfileDiscovery::Detail {
bool ProveProfileLayouts(const nlohmann::json& baseline, const ProfileRebaseAnalysis& analysis,
    const std::vector<std::pair<std::string, std::uint64_t>>& fields, nlohmann::json& evidence,
    std::map<std::uint32_t, std::uint32_t>& verified_pairs);
bool ExactLayoutConsumerPair(const ProfileRebaseAnalysis& analysis, std::uint32_t source_root,
    std::uint32_t target_root);
}
