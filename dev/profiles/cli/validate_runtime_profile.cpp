#include "profile_cli.h"

namespace profile_development {
bool validate_profile(const json& profile, std::string& error, const json* external_components) {
    try {
        if (profile.at("schemaVersion") != 1) throw std::runtime_error("unsupported schemaVersion");
        if (profile.contains("profileRevision")) {
            const auto& revision = profile.at("profileRevision");
            if (!revision.is_number_integer() ||
                (!revision.is_number_unsigned() && revision.get<std::int64_t>() <= 0) ||
                revision.get<std::uint64_t>() == 0 || revision.get<std::uint64_t>() > UINT32_MAX)
                throw std::runtime_error("invalid profileRevision");
        }
        const auto target = profile.at("target").get<std::string>();
        if (target != "enshrouded.exe" && target != "enshrouded_server.exe") throw std::runtime_error("unsupported target");
        const auto kfc_format = profile.at("kfcFormat").get<unsigned>();
        if (kfc_format != 3 && kfc_format != 4) throw std::runtime_error("unsupported kfcFormat");
        const auto& image = profile.at("image");
        if (image.at("size").get<std::uint64_t>() < 4096) throw std::runtime_error("image size is too small");
        const auto& layout = profile.at("ecsLayout");
        for (const char* key : {"entityManagerCount", "entityManagerTable", "componentOffsets", "componentStrides",
             "entityId", "entityGeneration", "entityLayout", "entityStorage", "entityDefinitionPointer", "entityRow", "componentBits", "lookupManager"})
            if (layout.at(key).get<std::uint64_t>() > 0x10000) throw std::runtime_error(std::string("layout offset out of range: ") + key);
        const auto& definition = profile.at("entityDefinitionLayout");
        for (const char* key : {"uuid", "name", "nameSize"})
            if (definition.at(key).get<std::uint64_t>() > 0x10000) throw std::runtime_error(std::string("entity definition offset out of range: ") + key);
        const auto& hooks = profile.at("hooks");
        for (const char* key : {"game_thread", "entity_manager", "world_prop_update", "world_actor_placement", "world_cursor"}) {
            if (std::string_view(key) == "world_cursor" && !hooks.contains(key)) continue;
            const auto& hook = hooks.at(key);
            if (parse_pattern(hook.at("signature").get<std::string>()).size() < 7) throw std::runtime_error(std::string("invalid hook signature: ") + key);
            const auto bytes = hook.at("original").get<std::vector<unsigned>>();
            if (bytes.size() < 5 || bytes.size() > 32) throw std::runtime_error(std::string("invalid overwritten bytes: ") + key);
            for (const auto byte : bytes) if (byte > 255) throw std::runtime_error(std::string("invalid original byte: ") + key);
        }
        if (hooks.contains("world_building_dispatch")) {
            const auto& hook = hooks.at("world_building_dispatch");
            if (parse_pattern(hook.at("signature").get<std::string>()).size() < 7)
                throw std::runtime_error("invalid hook signature: world_building_dispatch");
            const auto bytes = hook.at("original").get<std::vector<unsigned>>();
            if (bytes.size() < 5 || bytes.size() > 32)
                throw std::runtime_error("invalid overwritten bytes: world_building_dispatch");
            for (const auto byte : bytes) if (byte > 255)
                throw std::runtime_error("invalid original byte: world_building_dispatch");
        }
        if (hooks.contains("world_cursor") && hooks.at("world_cursor").at("captureOffset").get<std::uint64_t>() > 0x10000)
            throw std::runtime_error("native cursor capture offset out of range");
        const auto& placement = profile.at("worldContexts").at("entityPlacement");
        for (const char* key : {"actorFrameServiceViewOffset", "serviceViewWorldOffset", "placementContextOffset",
             "placeQueueOffset", "removeQueueOffset", "publishStateOffset", "publishCommandsOffset", "ownerOffset"})
            if (placement.at(key).get<std::uint64_t>() > 0x10000) throw std::runtime_error(std::string("world context offset out of range: ") + key);
        const auto& grids = profile.at("worldGrids");
        if (!grids.is_object() || grids.empty()) throw std::runtime_error("world grid catalog is missing or empty");
        for (auto item = grids.begin(); item != grids.end(); ++item) {
            const auto origin = item.value().at("origin").get<std::vector<double>>();
            const auto cell_size = item.value().at("cellSize").get<std::vector<double>>();
            const auto maximum = item.value().at("maximum").get<std::vector<std::uint64_t>>();
            if (item.key().empty() || item.key().size() >= 16 || origin.size() != 3 || cell_size.size() != 3 || maximum.size() != 3)
                throw std::runtime_error("invalid world grid specification: " + item.key());
            for (std::size_t axis = 0; axis < 3; ++axis)
                if (!std::isfinite(origin[axis]) || !std::isfinite(cell_size[axis]) || cell_size[axis] <= 0 || !maximum[axis])
                    throw std::runtime_error("invalid world grid specification: " + item.key());
        }
        const auto& operations = profile.at("worldOperations");
        if (operations.empty()) throw std::runtime_error("world operation catalog is empty");
        for (auto item = operations.begin(); item != operations.end(); ++item) {
            const auto& operation = item.value();
            if (!item.key().starts_with("runtime.world.") || operation.at("abi").get<std::string>().empty() ||
                operation.at("thread") != "game" || operation.at("context").get<std::string>().empty() ||
                (operation.value("validated", true) && operation.at("abi") != "actor-world-context" && operation.contains("functionRva") == operation.contains("globalRva")) ||
                (operation.contains("functionRva") && operation.contains("globalRva")))
                throw std::runtime_error("invalid world operation entry: " + item.key());
            for (const auto byte : operation.value("guardBytes", std::vector<unsigned>{}))
                if (byte > 255) throw std::runtime_error("invalid world operation guard bytes: " + item.key());
        }
        const auto& patches = profile.at("runtimePatches");
        if (patches.empty()) throw std::runtime_error("runtime patch catalog is empty");
        for (auto item = patches.begin(); item != patches.end(); ++item) {
            const auto& patch = item.value();
            if (!item.key().starts_with("runtime.patch.") || parse_pattern(patch.at("signature").get<std::string>()).empty() ||
                (patch.at("kind") != "bytes" && patch.at("kind") != "detour") ||
                patch.at("overwriteBytes").get<unsigned>() < 3 || patch.at("overwriteBytes").get<unsigned>() > 32 ||
                patch.at("payload").empty()) throw std::runtime_error("invalid runtime patch entry: " + item.key());
        }
        std::unordered_set<std::string> names;
        std::unordered_set<unsigned> indices;
        const bool live_components = profile.value("componentResolution", std::string{}) == "live-registration";
        if (live_components && profile.contains("components")) throw std::runtime_error("live registration profile must not duplicate component mappings");
        const auto empty_components = json::array();
        const json* components = profile.contains("components") ? &profile.at("components") : external_components;
        if (live_components) components = &empty_components;
        if (!components && profile.contains("componentCatalog")) throw std::runtime_error("external component catalog must be supplied for validation");
        if (!components || !components->is_array()) throw std::runtime_error("component catalog is missing or invalid");
        for (const auto& component : *components) {
            const auto name = component.at("name").get<std::string>();
            const auto index = component.at("index").get<unsigned>();
            const auto size = component.at("size").get<unsigned>();
            if (!name.starts_with("keen::ecs::") || index >= 1024 || !size || size > 65535 ||
                !names.insert(name).second || !indices.insert(index).second) throw std::runtime_error("invalid or duplicate component mapping");
        }
        if (names.empty() && !live_components) throw std::runtime_error("component catalog is empty");
        return true;
    } catch (const std::exception& exception) { error = exception.what(); return false; }
}

int validate_command(const fs::path& path, const std::optional<fs::path>& component_path) {
    try {
        const auto profile = json::parse(std::ifstream(path));
        std::optional<json> component_catalog;
        if (component_path) component_catalog = json::parse(std::ifstream(*component_path)).at("components");
        else if (profile.contains("componentCatalog")) {
            auto catalog_path = path.parent_path() / profile.at("componentCatalog").get<std::string>();
            if (!fs::exists(catalog_path)) throw std::runtime_error("referenced component catalog not found: " + catalog_path.string());
            component_catalog = json::parse(std::ifstream(catalog_path)).at("components");
        }
        std::string error;
        if (!validate_profile(profile, error, component_catalog ? &*component_catalog : nullptr)) { std::cerr << "invalid profile: " << error << '\n'; return 2; }
        const auto is_draft = profile.contains("provenance") && profile.at("provenance").value("status", "") != "approved";
        std::cout << "profile structure valid: " << profile.at("id").get<std::string>() << '\n';
        if (is_draft) {
            std::cout << "runtime readiness: draft only; complete live layout, ECS mapping, and function-semantic review first\n";
            return 3;
        }
        std::cout << "runtime readiness: approved status\n";
        return 0;
    } catch (const std::exception& exception) { std::cerr << "cannot validate profile: " << exception.what() << '\n'; return 2; }
}
}
