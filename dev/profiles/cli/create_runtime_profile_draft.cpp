#include "profile_cli.h"

namespace profile_development {
int generate_command(int argc, char** argv) {
    const fs::path base_path = argv[2], image_path = argv[3], scan_path = argv[4];
    const auto out_path = option(argc, argv, "--out");
    const auto components_path = option(argc, argv, "--components");
    const auto catalog_out = option(argc, argv, "--catalog-out");
    const auto kfc_format_option = option(argc, argv, "--kfc-format");
    if (!out_path || !components_path || !kfc_format_option) {
        std::cerr << "generate-profile requires --kfc-format, --components, and --out\n"; return 2;
    }
    const auto kfc_format_text = kfc_format_option->string();
    if (kfc_format_text != "3" && kfc_format_text != "4") {
        std::cerr << "--kfc-format must be 3 or 4\n"; return 2;
    }
    try {
        auto profile = json::parse(std::ifstream(base_path));
        if (kfc_format_text == "4" && profile.contains("ownerLayout")) profile["schemaVersion"] = 2;
        profile["kfcFormat"] = kfc_format_text == "3" ? 3 : 4;
        // A relocated signature does not prove calculation semantics in a new build.
        profile.erase("attributeCalculationModel");
        // Native ownership and write gates are exact-build contracts too.
        // A new image's signature scan cannot transfer their authorization.
        profile.erase("ecsWriteLayout");
        profile.erase("resourceWriteLayout");
        profile.erase("entityCommandQueueLayout");
        profile.erase("buildingPropSpawnLayout");
        const auto image = json::parse(std::ifstream(image_path));
        const auto scan = json::parse(std::ifstream(scan_path));
        const auto observed_components = json::parse(std::ifstream(*components_path));
        if (image.value("sha256", std::string{}).empty() || image.value("sha256", std::string{}) != scan.value("sha256", std::string{})) {
            std::cerr << "image and function scan must refer to the same executable SHA-256\n"; return 2;
        }
        if (observed_components.at("image").value("sha256", std::string{}) != image.at("sha256").get<std::string>() ||
            observed_components.at("image").at("timestamp") != image.at("timestamp") ||
            observed_components.at("image").at("size") != image.at("imageSize")) {
            std::cerr << "component capture and image report do not refer to the same executable identity\n"; return 2;
        }
        profile["image"]["timestamp"] = image.at("timestamp");
        profile["image"]["size"] = image.at("imageSize");
        profile["image"]["sha256"] = image.at("sha256");
        profile["id"] = observed_components.at("id");
        profile["target"] = observed_components.at("target");
        for (const auto& row : scan.value("functions", json::array())) {
            if (row.value("matchCount", 0u) != 1 || row.at("matches").empty()) continue;
            const auto id = row.at("id").get<std::string>();
            const auto rva = row.at("matches").at(0).get<std::uint32_t>();
            if (id.starts_with("hook.")) {
                const auto hook_name = id.substr(5);
                if (profile.at("hooks").contains(hook_name)) {
                    profile["hooks"][hook_name]["signature"] = row.at("signature");
                    if (row.contains("original") && !row.at("original").empty())
                        profile["hooks"][hook_name]["original"] = row.at("original");
                }
            } else if (profile.at("worldOperations").contains(id)) {
                auto& operation = profile["worldOperations"][id];
                if (operation.contains("functionRva") && operation.contains("guardRva")) {
                    const auto old_function = operation.at("functionRva").get<std::int64_t>();
                    const auto old_guard = operation.at("guardRva").get<std::int64_t>();
                    operation["functionRva"] = static_cast<std::uint32_t>(static_cast<std::int64_t>(rva) - (old_guard - old_function));
                    operation["guardRva"] = rva;
                    if (row.contains("original") && !row.at("original").empty()) operation["guardBytes"] = row.at("original");
                }
            } else if (profile.at("runtimePatches").contains(id)) {
                auto& patch = profile["runtimePatches"][id];
                patch["signature"] = row.at("signature");
                const auto containing = std::find_if(scan.at("runtimeFunctions").begin(), scan.at("runtimeFunctions").end(),
                    [&](const json& function) { return function.at("beginRva").get<std::uint32_t>() <= rva &&
                        rva < function.at("endRva").get<std::uint32_t>(); });
                if (containing == scan.at("runtimeFunctions").end()) {
                    std::cerr << "patch signature is not inside a PE runtime function: " << id << '\n'; return 3;
                }
                auto& function = patch["function"];
                function["beginRva"] = containing->at("beginRva");
                function["endRva"] = containing->at("endRva");
                function["targetOffset"] = rva - containing->at("beginRva").get<std::uint32_t>();
            }
        }
        if (profile.value("componentResolution", std::string{}) != "live-registration")
            profile["components"] = observed_components.at("components");
        const json component_catalog = {{"schemaVersion", 1}, {"id", profile.at("id")}, {"target", profile.at("target")},
            {"image", profile.at("image")}, {"components", observed_components.at("components")}};
        profile["allowStructuralRevalidation"] = false;
        json provenance = {{"status", "draft-requires-live-review"}, {"generator", "kfc-runtime-dev"},
            {"executableSha256", image.at("sha256")}, {"imageReport", fs::absolute(image_path).string()},
            {"functionScan", fs::absolute(scan_path).string()}, {"componentCapture", fs::absolute(*components_path).string()},
            {"reflectionCatalog", observed_components.value("source", json::object()).value("reflection", std::string{})},
            {"allFunctionCandidatesUnique", scan.value("allFunctionCandidatesUnique", scan.value("allCandidatesUnique", false))},
            {"runtimeLayoutValidated", false}, {"componentMappingsValidated", false},
            {"functionSemanticsValidated", false},
            {"unresolvedComponentCount", observed_components.value("unresolved", json::array()).size()}};
        profile["provenance"] = std::move(provenance);
        if (catalog_out && !write_json(*catalog_out, component_catalog)) return 2;
        if (!write_json(*out_path, profile)) return 2;
        std::cout << "profile draft written (not installable until live layout, component mappings, and function semantics are reviewed): "
                  << fs::absolute(*out_path).string() << '\n';
        return 0;
    } catch (const std::exception& exception) { std::cerr << "profile generation failed: " << exception.what() << '\n'; return 2; }
}
}
