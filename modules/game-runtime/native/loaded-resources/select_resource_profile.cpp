#include "loaded-resources/select_resource_profile.h"

namespace ResourceImplementation {
namespace {
bool read_profile(const nlohmann::json& profile, const nlohmann::json& image) {
    try {
        const auto version = profile.value("schemaVersion", 0);
        return (version == 1 || version == 2) && profile.value("target", std::string{}) == image.at("target").get<std::string>() &&
            profile.value("kfcFormat", 0) == 4 && profile.contains("provenance") &&
            profile.at("provenance").value("status", std::string{}) == "read-validated" &&
            profile.at("provenance").value("resourceReadValidated", false);
    } catch (const nlohmann::json::exception&) { return false; }
}
bool exact_image(const nlohmann::json& profile, const nlohmann::json& image) {
    const auto candidate = profile.find("image");
    if (candidate == profile.end() || !candidate->is_object()) return false;
    for (const auto key : {"sha256", "timestamp", "size"}) {
        const auto value = candidate->find(key);
        if (value == candidate->end() || *value != image.at(key)) return false;
    }
    // An unavailable file hash must never become a matching identity.
    return image.at("sha256").is_string() && image.at("sha256").get_ref<const std::string&>().size() == 64;
}
}

ResourceProfileSelection select_resource_profile(const nlohmann::json& image, unsigned kfc_format,
    const std::vector<nlohmann::json>& profiles) {
    ResourceProfileSelection result;
    result.diagnostics = {{"policy", "exact-image"}, {"image", image}, {"kfcFormat", kfc_format},
        {"source", "none"}, {"exactMatches", 0}};
    auto ids = nlohmann::json::array();
    std::size_t eligible{}, normal_matches{}, beta_matches{};
    nlohmann::json beta_profile;
    for (const auto& candidate : profiles) {
        if (!read_profile(candidate, image)) continue;
        ++eligible;
        const auto id = candidate.find("id");
        if (ids.size() < 16 && id != candidate.end() && id->is_string())
            ids.push_back(id->get<std::string>().substr(0, 128));
        if (kfc_format != 4 || !exact_image(candidate, image)) continue;
        const bool beta = id != candidate.end() && id->is_string() &&
            id->get_ref<const std::string&>().starts_with("beta_");
        if (beta) {
            ++beta_matches;
            beta_profile = candidate;
        } else {
            ++normal_matches;
            result.profile = candidate;
        }
    }
    const auto matches = normal_matches ? normal_matches : beta_matches;
    if (!normal_matches) result.profile = std::move(beta_profile);
    result.diagnostics["profiles"] = std::move(ids);
    result.diagnostics["profileCount"] = eligible;
    result.diagnostics["exactMatches"] = matches;
    result.diagnostics["normalExactMatches"] = normal_matches;
    result.diagnostics["betaExactMatches"] = beta_matches;
    result.diagnostics["beta"] = !normal_matches && beta_matches != 0;
    if (matches == 1) result.diagnostics["source"] = "installed";
    else {
        result.profile = nullptr;
        result.error = matches ? "ambiguous-exact-resource-read-profile" : "no-exact-resource-read-profile";
    }
    return result;
}
}
