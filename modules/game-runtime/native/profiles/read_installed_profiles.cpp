#include "profiles/read_installed_profiles.h"
#include "game-profile-discovery/discover_game_profile.h"
#include <algorithm>
#include <fstream>

namespace KfcRuntimeCompatibility {
namespace {
std::filesystem::path configured_path(const nlohmann::json& value) {
    const auto& bytes = value.get_ref<const std::string&>();
    return std::filesystem::path(std::u8string(bytes.begin(), bytes.end()));
}

// Status is polled and copied through bounded provider buffers. Full address
// witnesses belong in the discovery proof, rather than every runtime snapshot.
nlohmann::json discovery_status(nlohmann::json diagnostics) {
    for (const auto key : {"comparison", "rebase"}) {
        const auto detail = diagnostics.find(key);
        if (detail == diagnostics.end() || !detail->is_object()) continue;
        auto summary = nlohmann::json::object();
        for (const auto& [name, value] : detail->items()) {
            if (value.is_primitive()) summary[name] = value;
            else if (value.is_array()) summary[name + "Count"] = value.size();
        }
        const auto unresolved = detail->find("unresolved");
        if (unresolved != detail->end() && unresolved->is_array()) {
            summary["unresolvedSample"] = nlohmann::json::array();
            for (std::size_t i = 0; i < (std::min)(std::size_t{4}, unresolved->size()); ++i) {
                auto sample = nlohmann::json::object();
                if (!(*unresolved)[i].is_object()) continue;
                for (const auto& [name, value] : (*unresolved)[i].items()) {
                    if (value.is_primitive()) sample[name] = value;
                    else if (value.is_array()) sample[name + "Count"] = value.size();
                }
                summary["unresolvedSample"].push_back(std::move(sample));
            }
        }
        *detail = std::move(summary);
    }
    return diagnostics;
}
}

InstalledProfiles ReadInstalledProfiles(const std::filesystem::path& process_path,
                                       const std::string& target, const nlohmann::json& image) {
    InstalledProfiles result;
    const auto game_root = process_path.parent_path();
    auto runtime_directory = game_root / L"shroudforge" / L"runtime";
    auto cache_directory = game_root / L"shroudforge" / L"cache";
    bool discovery_enabled{true};
    const auto config_path = game_root / L"shroudforge" / L"config" / L"modloader-config.json";
    std::ifstream config_file(config_path);
    if (config_file) {
        const auto config = nlohmann::json::parse(config_file, nullptr, false);
        if (config.is_object()) {
            const nlohmann::json::json_pointer enabled_path("/modules/gameProfileDiscovery/enabled");
            if (config.contains(enabled_path))
                discovery_enabled = config.at(enabled_path).is_boolean() && config.at(enabled_path).get<bool>();
            const auto runtime = config.find("runtime");
            if (runtime != config.end() && runtime->is_object()) {
                const auto id = runtime->find("profileId");
                if (id != runtime->end() && id->is_string()) result.requested_id = id->get<std::string>();
            }
            const auto paths = config.find("paths");
            if (paths != config.end() && paths->is_object()) {
                const auto runtime_path = paths->find("runtime");
                if (runtime_path != paths->end() && runtime_path->is_string()) {
                    const auto configured = configured_path(*runtime_path);
                    runtime_directory = configured.is_absolute() ? configured : game_root / configured;
                }
                const auto cache_path = paths->find("cache");
                if (cache_path != paths->end() && cache_path->is_string()) {
                    const auto configured = configured_path(*cache_path);
                    cache_directory = configured.is_absolute() ? configured : game_root / configured;
                }
            }
        } else discovery_enabled = false;
    }
    result.directory = runtime_directory / L"profiles";
    if (std::filesystem::exists(result.directory))
    for (auto iterator = std::filesystem::recursive_directory_iterator(result.directory);
         iterator != std::filesystem::recursive_directory_iterator(); ++iterator) {
        const auto& entry = *iterator;
        if (entry.is_directory() && entry.path().filename() == L"mod-profiles") {
            iterator.disable_recursion_pending();
            continue;
        }
        if (!entry.is_regular_file() || entry.path().extension() != L".json") continue;
        const auto filename_bytes = entry.path().filename().u8string();
        const std::string filename(filename_bytes.begin(), filename_bytes.end());
        std::ifstream file(entry.path());
        const auto candidate = nlohmann::json::parse(file, nullptr, false);
        if (!candidate.is_object()) {
            if (result.errors.size() < 4) result.errors.push_back(filename + ":invalid-json");
            continue;
        }
        const auto candidate_target = candidate.find("target");
        if (candidate_target == candidate.end() || !candidate_target->is_string() || *candidate_target != target) continue;
        const auto candidate_id = candidate.find("id");
        const bool beta_id = candidate_id != candidate.end() && candidate_id->is_string() &&
            candidate_id->get_ref<const std::string&>().starts_with("beta_");
        if (filename.starts_with("beta_") && !beta_id) {
            if (result.errors.size() < 4) result.errors.push_back(filename + ":beta-id-prefix-required");
            continue;
        }
        // Beta files are outputs, never trusted inputs. The discovery module
        // must reproduce their complete contract from the verified normal
        // baseline and current image before they can enter the active catalog.
        if (beta_id) continue;
        result.profiles.push_back(candidate);
    }
    if (discovery_enabled) {
        result.discovery = {{"enabled", true}, {"state", "image-identity-unavailable"}};
        if (image.is_object() && image.contains("sha256") && image.contains("kfcFormat")) {
            try {
                const GameProfileDiscovery::ImageIdentity identity{target, image.at("sha256").get<std::string>(),
                    image.at("timestamp").get<std::uint32_t>(), image.at("size").get<std::uint32_t>(),
                    image.at("kfcFormat").get<std::uint8_t>()};
                auto discovery = GameProfileDiscovery::Prepare(process_path, result.directory,
                    cache_directory, result.profiles, identity);
                result.discovery = discovery_status(std::move(discovery.diagnostics));
                result.discovery["enabled"] = true;
                if (discovery.profile) {
                    const auto& generated = *discovery.profile;
                    const auto id = generated.value("id", std::string{});
                    const bool beta_profile = result.discovery.value("beta", false) &&
                        (result.discovery.value("state", std::string{}) == "beta-generated" ||
                         result.discovery.value("state", std::string{}) == "beta-present") &&
                        id.starts_with("beta_") && generated.value("target", std::string{}) == target &&
                        generated.value("kfcFormat", 0) == identity.kfc_format &&
                        generated.contains("image") && generated.at("image").value("sha256", std::string{}) ==
                            identity.sha256 &&
                        generated.at("image").value("timestamp", std::uint32_t{}) ==
                            identity.timestamp &&
                        generated.at("image").value("size", std::uint32_t{}) ==
                            identity.size;
                    if (beta_profile) {
                        result.generated_profile_id = id;
                        if (std::find(result.profiles.begin(), result.profiles.end(), generated) == result.profiles.end())
                            result.profiles.push_back(generated);
                    } else {
                        result.discovery["state"] = "failed";
                        result.discovery["reason"] = "generated-profile-identity-or-state-invalid";
                    }
                }
            } catch (const std::exception& error) {
                result.discovery = {{"enabled", true}, {"state", "failed"}, {"reason", error.what()}};
            }
        }
    }
    return result;
}
}
