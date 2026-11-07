#include "profiles/read_installed_profiles.h"
#include "game-profile-discovery/discover_game_profile.h"
#include "game-profile-discovery/profile_discovery_log.h"
#include <algorithm>
#include <fstream>

namespace KfcRuntimeCompatibility {
namespace {
std::string path_utf8(const std::filesystem::path& path) {
    const auto encoded = path.generic_u8string();
    return {encoded.begin(), encoded.end()};
}

std::string profile_id(const nlohmann::json& profile) {
    if (!profile.is_object()) return "<missing-or-invalid>";
    const auto id = profile.find("id");
    return id != profile.end() && id->is_string() ? id->get<std::string>() : "<missing-or-invalid>";
}

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
                                       const std::string& target, const nlohmann::json& image,
                                       const char* caller) {
    InstalledProfiles result;
    const auto game_root = process_path.parent_path();
    GameProfileDiscovery::Log(game_root, 'I', "Installed profile catalog scan started. caller=" +
        std::string(caller ? caller : "<unknown>") + " target=" + target + " process=" +
        path_utf8(process_path));
    auto runtime_directory = game_root / L"shroudforge" / L"runtime";
    auto cache_directory = game_root / L"shroudforge" / L"cache";
    bool discovery_enabled{true};
    const auto config_path = game_root / L"shroudforge" / L"config" / L"modloader-config.json";
    std::ifstream config_file(config_path);
    if (config_file) {
        const auto config = nlohmann::json::parse(config_file, nullptr, false);
        if (config.is_object()) {
            const nlohmann::json::json_pointer enabled_path("/modules/gameProfileDiscovery/enabled");
            if (config.contains(enabled_path)) {
                const auto& configured_enabled = config.at(enabled_path);
                if (configured_enabled.is_boolean()) {
                    discovery_enabled = configured_enabled.get<bool>();
                } else {
                    discovery_enabled = false;
                    GameProfileDiscovery::Log(game_root, 'W', "The game-profile discovery enabled setting is not a boolean. Discovery is disabled for this catalog read. config=" +
                        path_utf8(config_path));
                }
            }
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
        } else {
            discovery_enabled = false;
            GameProfileDiscovery::Log(game_root, 'W', "Loader configuration could not be parsed as a JSON object. " +
                path_utf8(config_path) + ". Game-profile discovery is disabled for this catalog read");
        }
    } else if (std::filesystem::exists(config_path)) {
        GameProfileDiscovery::Log(game_root, 'W', "Loader configuration exists but could not be opened. " +
            path_utf8(config_path) + "; defaults are being used");
    } else {
        GameProfileDiscovery::Log(game_root, 'D', "Loader configuration is absent. Discovery defaults to enabled. path=" +
            path_utf8(config_path));
    }
    result.directory = runtime_directory / L"profiles";
    GameProfileDiscovery::Log(game_root, 'D', "Profile catalog paths resolved. enabled=" +
        std::string(discovery_enabled ? "true" : "false") + " profileDirectory=" + path_utf8(result.directory) +
        " cacheDirectory=" + path_utf8(cache_directory) + " requestedProfile=" +
        (result.requested_id.empty() ? std::string("<automatic>") : result.requested_id) +
        " config=" + path_utf8(config_path));
    std::size_t candidate_files{};
    std::size_t accepted_profiles{};
    std::size_t invalid_profiles{};
    std::size_t target_mismatches{};
    std::size_t beta_profiles_excluded{};
    if (std::filesystem::exists(result.directory))
    for (auto iterator = std::filesystem::recursive_directory_iterator(result.directory);
         iterator != std::filesystem::recursive_directory_iterator(); ++iterator) {
        const auto& entry = *iterator;
        if (entry.is_directory() && entry.path().filename() == L"mod-profiles") {
            iterator.disable_recursion_pending();
            continue;
        }
        if (!entry.is_regular_file() || entry.path().extension() != L".json") continue;
        ++candidate_files;
        const auto filename_bytes = entry.path().filename().u8string();
        const std::string filename(filename_bytes.begin(), filename_bytes.end());
        std::ifstream file(entry.path());
        const auto candidate = nlohmann::json::parse(file, nullptr, false);
        if (!candidate.is_object()) {
            ++invalid_profiles;
            if (result.errors.size() < 4) result.errors.push_back(filename + ":invalid-json");
            GameProfileDiscovery::Log(game_root, 'T', "Profile candidate rejected because it is not a JSON object. file=" +
                path_utf8(entry.path()));
            continue;
        }
        const auto candidate_target = candidate.find("target");
        if (candidate_target == candidate.end() || !candidate_target->is_string() || *candidate_target != target) {
            ++target_mismatches;
            GameProfileDiscovery::Log(game_root, 'T', "Profile candidate skipped because its target does not match. file=" +
                path_utf8(entry.path()) + " expectedTarget=" + target);
            continue;
        }
        const auto candidate_id = candidate.find("id");
        const bool beta_id = candidate_id != candidate.end() && candidate_id->is_string() &&
            candidate_id->get_ref<const std::string&>().starts_with("beta_");
        if (filename.starts_with("beta_") && !beta_id) {
            ++invalid_profiles;
            if (result.errors.size() < 4) result.errors.push_back(filename + ":beta-id-prefix-required");
            GameProfileDiscovery::Log(game_root, 'W', "Beta profile filename has no beta_ profile ID and is excluded. file=" +
                path_utf8(entry.path()));
            continue;
        }
        // Beta files are outputs, never trusted inputs. The discovery module
        // must reproduce their complete contract from the verified normal
        // baseline and current image before they can enter the active catalog.
        if (beta_id) {
            ++beta_profiles_excluded;
            GameProfileDiscovery::Log(game_root, 'T', "Existing beta profile excluded from discovery baselines. file=" +
                path_utf8(entry.path()));
            continue;
        }
        result.profiles.push_back(candidate);
        ++accepted_profiles;
        GameProfileDiscovery::Log(game_root, 'T', "Normal profile candidate accepted. id=" +
            profile_id(candidate) + " file=" + path_utf8(entry.path()));
    }
    GameProfileDiscovery::Log(game_root, 'D', "Profile catalog scan completed. jsonFiles=" +
        std::to_string(candidate_files) + " acceptedNormalProfiles=" + std::to_string(accepted_profiles) +
        " invalidProfiles=" + std::to_string(invalid_profiles) + " targetMismatches=" +
        std::to_string(target_mismatches) + " excludedBetaProfiles=" + std::to_string(beta_profiles_excluded));
    if (discovery_enabled) {
        result.discovery = {{"enabled", true}, {"state", "image-identity-unavailable"}};
        if (image.is_object() && image.contains("sha256") && image.contains("kfcFormat")) {
            try {
                const GameProfileDiscovery::ImageIdentity identity{target, image.at("sha256").get<std::string>(),
                    image.at("timestamp").get<std::uint32_t>(), image.at("size").get<std::uint32_t>(),
                    image.at("kfcFormat").get<std::uint8_t>()};
                GameProfileDiscovery::Log(game_root, 'I', "Game-profile discovery analysis is starting. imageSha256=" +
                    identity.sha256 + " caller=" + (caller ? caller : "<unknown>") +
                    " timestamp=" + std::to_string(identity.timestamp) + " size=" +
                    std::to_string(identity.size) + " kfcFormat=" + std::to_string(identity.kfc_format));
                auto discovery = GameProfileDiscovery::Prepare(process_path, result.directory,
                    cache_directory, result.profiles, identity);
                result.discovery = discovery_status(std::move(discovery.diagnostics));
                result.discovery["enabled"] = true;
                const auto state = result.discovery.value("state", std::string("unknown"));
                const auto reason = result.discovery.value("reason", std::string("<none>"));
                const auto evidence_path = result.discovery.value("evidencePath", std::string{});
                const auto proof_path = result.discovery.value("proofPath", std::string{});
                const auto profile_path = result.discovery.value("profilePath", std::string{});
                GameProfileDiscovery::Log(game_root,
                    state == "error" ? 'E' : state == "unresolved" || state == "failed" ? 'W' : 'D',
                    "Discovery analysis returned. state=" + state + " reason=" + reason +
                    " baselineId=" + result.discovery.value("baselineId", std::string("<none>")) +
                    " evidencePath=" + (evidence_path.empty() ? "<not-written>" : evidence_path) +
                    " proofPath=" + (proof_path.empty() ? "<not-created>" : proof_path) +
                    " profilePath=" + (profile_path.empty() ? "<not-created>" : profile_path));
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
                        GameProfileDiscovery::Log(game_root, 'I', "Generated beta profile passed the caller's exact image identity guard. id=" +
                            id + " savedPath=" + profile_path + " proofPath=" + proof_path);
                        if (std::find(result.profiles.begin(), result.profiles.end(), generated) == result.profiles.end())
                            result.profiles.push_back(generated);
                    } else {
                        result.discovery["state"] = "failed";
                        result.discovery["reason"] = "generated-profile-identity-or-state-invalid";
                        GameProfileDiscovery::Log(game_root, 'E', "Discovery output was rejected by the caller's beta profile identity guard. id=" +
                            id + " claimedPath=" + (profile_path.empty() ? "<unknown>" : profile_path) +
                            " proofPath=" + (proof_path.empty() ? "<unknown>" : proof_path));
                    }
                }
            } catch (const std::exception& error) {
                result.discovery = {{"enabled", true}, {"state", "failed"}, {"reason", error.what()}};
                GameProfileDiscovery::Log(game_root, 'E', "Exception while preparing a beta profile. caller=" +
                    std::string(caller ? caller : "<unknown>") + " reason=" + std::string(error.what()) +
                    " profileDirectory=" + path_utf8(result.directory) +
                    " cacheDirectory=" + path_utf8(cache_directory));
            }
        } else {
            GameProfileDiscovery::Log(game_root, 'W', "Discovery is enabled, but the current executable image identity is incomplete. No profile analysis or report was produced");
        }
    } else {
        GameProfileDiscovery::Log(game_root, 'I', "Game-profile discovery is disabled by loader configuration. config=" +
            path_utf8(config_path));
    }
    return result;
}
}
