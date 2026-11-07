#include "profiles/game_profile.h"
#include "profiles/identify_game_image.h"
#include "profiles/read_game_profile.h"
#include "profiles/read_installed_profiles.h"
#include <windows.h>
#include <filesystem>
#include <fstream>
#include <vector>
#include <nlohmann/json.hpp>

namespace KfcRuntimeCompatibility::EnshroudedClient {

bool Load() {
    try {
        wchar_t process_path[32768]{};
        if (!GetModuleFileNameW(nullptr, process_path, 32768)) return false;
        const std::uint8_t* base{};
        const IMAGE_NT_HEADERS64* nt{};
        if (!IdentifyGameImage(process_path, base, nt)) return false;
        const auto& process = image_target;
        const auto& process_hash = image_sha256;
        nlohmann::json exact_profile, exact_beta_profile, fallback_profile;
        unsigned exact_count{}, exact_beta_count{}, fallback_count{};
        const auto installed = ReadInstalledProfiles(process_path, process,
            {{"sha256", image_sha256}, {"timestamp", image_timestamp},
             {"size", image_size}, {"kfcFormat", kfc_format}}, "game-profile-selection");
        const auto& requested_profile_id = installed.requested_id;
        auto load_external_profiles = [&](const std::string& id_filter) {
            std::vector<nlohmann::json> loaded_profiles;
            for (const auto& candidate : installed.profiles) {
                if (!candidate.contains("ecsLayout") || !candidate.contains("hooks")) continue;
                const auto component_resolution = candidate.find("componentResolution");
                const bool live_components = component_resolution != candidate.end() &&
                    component_resolution->is_string() && *component_resolution == "live-registration";
                if (!candidate.contains("components") && !live_components) continue;
                if (!id_filter.empty() && candidate.value("id", std::string{}) != id_filter) continue;
                if (candidate.value("target", std::string{}) != process) continue;
                loaded_profiles.push_back(candidate);
            }
            return loaded_profiles;
        };
        std::vector<nlohmann::json> candidates;
        if (!requested_profile_id.empty()) {
            candidates = load_external_profiles(requested_profile_id);
            if (candidates.size() != 1) {
                status = candidates.empty() ? "manual-profile-not-found:" + requested_profile_id
                    : "manual-profile-id-ambiguous:" + requested_profile_id;
                return false;
            }
        } else {
            candidates = load_external_profiles({});
        }
        for (auto& candidate : candidates) {
            if (candidate.at("schemaVersion") != 1 || candidate.at("target") != process) continue;
            const auto candidate_id = candidate.value("id", std::string{});
            const bool generated_beta = !installed.generated_profile_id.empty() &&
                candidate_id == installed.generated_profile_id && candidate_id.starts_with("beta_");
            if (candidate_id.starts_with("beta_") && !generated_beta) continue;
            if (!candidate.contains("kfcFormat") || !candidate.at("kfcFormat").is_number_integer()) continue;
            const auto candidate_kfc_format = candidate.at("kfcFormat").get<int>();
            if (candidate_kfc_format != 3 && candidate_kfc_format != 4) continue;
            // Structural revalidation stays within the actual asset format.
            // KFC3 execution layouts cannot authorize a KFC4 process merely
            // because one of their instruction guards still happens to match.
            if (kfc_format && candidate_kfc_format != kfc_format) continue;
            // Beta files from disk are never trusted. The one generated again
            // by Prepare for this exact image carries an in-memory attestation.
            if (candidate.contains("provenance") &&
                candidate.at("provenance").value("status", std::string{}) != "approved" && !generated_beta) continue;
            const bool matches = candidate.at("image").at("timestamp") == nt->FileHeader.TimeDateStamp &&
                candidate.at("image").at("size") == nt->OptionalHeader.SizeOfImage &&
                (!candidate.at("image").contains("sha256") ||
                 (!process_hash.empty() && candidate.at("image").at("sha256").get<std::string>() == process_hash));
            if (candidate.contains("provenance") && !generated_beta) {
                const auto& provenance = candidate.at("provenance");
                if (provenance.value("status", std::string{}) != "approved" ||
                    !provenance.value("runtimeLayoutValidated", false) ||
                    !provenance.value("componentMappingsValidated", false) ||
                    !provenance.value("functionSemanticsValidated", false) ||
                    !candidate.at("image").contains("sha256") ||
                    provenance.value("executableSha256", std::string{}) != candidate.at("image").at("sha256").get<std::string>() ||
                    provenance.value("unresolvedComponentCount", std::size_t(-1)) != 0) continue;
            }
            if (matches) {
                if (generated_beta) {
                    ++exact_beta_count;
                    exact_beta_profile = std::move(candidate);
                } else {
                    ++exact_count;
                    exact_profile = std::move(candidate);
                }
            } else if (!requested_profile_id.empty() || candidate.value("allowStructuralRevalidation", false)) {
                ++fallback_count;
                fallback_profile = std::move(candidate);
            }
        }
        // Exact image identity always takes precedence over every opt-in
        // structural fallback. Normal exact profiles also precede generated beta.
        const bool exact = exact_count == 1 || (exact_count == 0 && exact_beta_count == 1);
        if (exact_count > 1) {
            status = "ambiguous-exact-profile"; return false;
        }
        if (exact_count == 0 && exact_beta_count > 1) {
            status = "ambiguous-exact-beta-profile"; return false;
        }
        if (exact_count == 0 && exact_beta_count == 0 && fallback_count != 1) {
            status = "missing-or-ambiguous-profile"; return false;
        }
        const auto& selected = exact_count == 1 ? exact_profile :
            exact_beta_count == 1 ? exact_beta_profile : fallback_profile;
        exact_build_match = exact;
        if (!kfc_format) kfc_format = static_cast<std::uint8_t>(selected.at("kfcFormat").get<unsigned>());
        // Identity is a lookup hint. Both hooks still require unique executable
        // matches and exact overwritten instructions before any patch is made.
        ReadGameProfile(selected, exact, base);
        return true;
    } catch (const std::exception& error) {
        status = std::string("profile-error:") + error.what();
        OutputDebugStringA(status.c_str());
        return false;
    }
}
}
