#include "profile.h"
#include "embedded_profile_ids.h"
#include <windows.h>
#include <bcrypt.h>
#include <array>
#include <filesystem>
#include <fstream>
#include <cmath>
#include <unordered_set>
#include <vector>
#include <nlohmann/json.hpp>

namespace KfcRuntimeCompatibility::EnshroudedClient {
namespace {
std::string sha256_file(const std::filesystem::path& path) {
    BCRYPT_ALG_HANDLE algorithm{};
    BCRYPT_HASH_HANDLE hash{};
    DWORD object_size{}, digest_size{}, received{};
    if (BCryptOpenAlgorithmProvider(&algorithm, BCRYPT_SHA256_ALGORITHM, nullptr, 0) < 0) return {};
    if (BCryptGetProperty(algorithm, BCRYPT_OBJECT_LENGTH, reinterpret_cast<PUCHAR>(&object_size), sizeof(object_size), &received, 0) < 0 ||
        BCryptGetProperty(algorithm, BCRYPT_HASH_LENGTH, reinterpret_cast<PUCHAR>(&digest_size), sizeof(digest_size), &received, 0) < 0) {
        BCryptCloseAlgorithmProvider(algorithm, 0); return {};
    }
    std::vector<UCHAR> object(object_size), digest(digest_size), chunk(1u << 20);
    if (BCryptCreateHash(algorithm, &hash, object.data(), object_size, nullptr, 0, 0) < 0) {
        BCryptCloseAlgorithmProvider(algorithm, 0); return {};
    }
    std::ifstream input(path, std::ios::binary);
    bool ok = static_cast<bool>(input);
    while (ok && input) {
        input.read(reinterpret_cast<char*>(chunk.data()), static_cast<std::streamsize>(chunk.size()));
        const auto count = input.gcount();
        if (count > 0 && BCryptHashData(hash, chunk.data(), static_cast<ULONG>(count), 0) < 0) ok = false;
    }
    ok = ok && input.eof() && BCryptFinishHash(hash, digest.data(), digest_size, 0) >= 0;
    BCryptDestroyHash(hash);
    BCryptCloseAlgorithmProvider(algorithm, 0);
    if (!ok) return {};
    constexpr char digits[] = "0123456789abcdef";
    std::string result;
    result.reserve(digest.size() * 2);
    for (const auto byte : digest) { result.push_back(digits[byte >> 4]); result.push_back(digits[byte & 15]); }
    return result;
}
}

bool Load() {
    try {
        HMODULE self{};
        if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            reinterpret_cast<LPCWSTR>(&Load), &self)) return false;
        wchar_t process_path[32768]{};
        if (!GetModuleFileNameW(nullptr, process_path, 32768)) return false;
        const auto process = std::filesystem::path(process_path).filename().string();
        const auto process_hash = sha256_file(process_path);
        const auto base = reinterpret_cast<const std::uint8_t*>(GetModuleHandleW(nullptr));
        const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base);
        if (dos->e_magic != IMAGE_DOS_SIGNATURE) return false;
        const auto nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(base + dos->e_lfanew);
        if (nt->Signature != IMAGE_NT_SIGNATURE || nt->OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR64_MAGIC) return false;
        nlohmann::json exact_profile, fallback_profile;
        unsigned exact_count{}, fallback_count{};
        std::string requested_profile_id;
        const auto game_root = std::filesystem::path(process_path).parent_path();
        const auto loader_config_path = game_root / L"shroudforge" / L"config" / L"modloader-config.json";
        auto runtime_directory = game_root / L"shroudforge" / L"runtime";
        if (std::filesystem::exists(loader_config_path)) {
            std::ifstream config_file(loader_config_path);
            if (config_file) {
                const auto config = nlohmann::json::parse(config_file, nullptr, false);
                if (config.is_object() && config.contains("paths") && config["paths"].is_object() &&
                    config["paths"].contains("runtime") && config["paths"]["runtime"].is_string()) {
                    const auto configured = std::filesystem::path(config["paths"]["runtime"].get<std::string>());
                    runtime_directory = configured.is_absolute() ? configured : game_root / configured;
                }
                if (config.is_object() && config.contains("runtime") && config["runtime"].is_object() &&
                    config["runtime"].contains("profileId") && config["runtime"]["profileId"].is_string())
                    requested_profile_id = config["runtime"]["profileId"].get<std::string>();
            }
        }
        const auto profile_root = runtime_directory / L"profiles";
        auto load_external_profiles = [&](const std::string& id_filter) {
            std::vector<nlohmann::json> loaded_profiles;
            if (!std::filesystem::exists(profile_root)) return loaded_profiles;
            for (const auto& entry : std::filesystem::recursive_directory_iterator(profile_root)) {
                if (!entry.is_regular_file() || entry.path().extension() != L".json") continue;
                std::ifstream profile_file(entry.path());
                if (!profile_file) continue;
                auto candidate = nlohmann::json::parse(profile_file, nullptr, false);
                if (!candidate.is_object() || !candidate.contains("ecsLayout") ||
                    !candidate.contains("hooks") || !candidate.contains("components")) continue;
                if (!id_filter.empty() && candidate.value("id", std::string{}) != id_filter) continue;
                if (candidate.value("target", std::string{}) != process) continue;
                loaded_profiles.push_back(std::move(candidate));
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
            if (candidates.empty()) {
                for (const auto resource_id : KfcRuntimeEmbedded::profile_resource_ids) {
                    const auto resource = FindResourceW(self, MAKEINTRESOURCEW(resource_id), MAKEINTRESOURCEW(10));
                    if (!resource) throw std::runtime_error("embedded compatibility profile is missing");
                    const auto loaded = LoadResource(self, resource);
                    if (!loaded) throw std::runtime_error("embedded compatibility profile could not be loaded");
                    const auto size = SizeofResource(self, resource);
                    const auto* data = static_cast<const char*>(LockResource(loaded));
                    if (!data || !size) throw std::runtime_error("embedded compatibility profile is empty");
                    auto candidate = nlohmann::json::parse(data, data + size);
                    candidates.push_back(std::move(candidate));
                }
            }
        }
        for (auto& candidate : candidates) {
            if (candidate.at("schemaVersion") != 1 || candidate.at("target") != process) continue;
            // Generated profile drafts are developer artifacts. They must never
            // become active merely because the EXE identity happens to match.
            if (candidate.contains("provenance") &&
                candidate.at("provenance").value("status", std::string{}) != "approved") continue;
            const bool matches = candidate.at("image").at("timestamp") == nt->FileHeader.TimeDateStamp &&
                candidate.at("image").at("size") == nt->OptionalHeader.SizeOfImage &&
                (!candidate.at("image").contains("sha256") ||
                 (!process_hash.empty() && candidate.at("image").at("sha256").get<std::string>() == process_hash));
            if (candidate.contains("provenance")) {
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
                ++exact_count;
                exact_profile = std::move(candidate);
            } else if (!requested_profile_id.empty() || candidate.value("allowStructuralRevalidation", false)) {
                ++fallback_count;
                fallback_profile = std::move(candidate);
            }
        }
        // Exact image identity always takes precedence over every opt-in
        // structural fallback, regardless of file enumeration order.
        const bool exact = exact_count == 1;
        if (exact_count > 1) {
            status = "ambiguous-exact-profile"; return false;
        }
        if (exact_count == 0 && fallback_count != 1) {
            status = "missing-or-ambiguous-profile"; return false;
        }
        const auto& selected = exact ? exact_profile : fallback_profile;
        exact_build_match = exact;
        // Identity is a lookup hint. Both hooks still require unique executable
        // matches and exact overwritten instructions before any patch is made.
        image_timestamp = nt->FileHeader.TimeDateStamp;
        image_size = nt->OptionalHeader.SizeOfImage;
        const auto& layout = selected.at("ecsLayout");
        auto offset = [&](const char* key) {
            const auto value = layout.at(key).get<std::size_t>();
            if (value > 0x10000) throw std::runtime_error("layout offset out of range");
            return value;
        };
        entity_manager_count = offset("entityManagerCount");
        entity_manager_table = offset("entityManagerTable");
        component_offsets = offset("componentOffsets"); component_strides = offset("componentStrides");
        entity_id = offset("entityId"); entity_generation = offset("entityGeneration");
        entity_layout = offset("entityLayout"); entity_storage = offset("entityStorage");
        entity_definition = offset("entityDefinitionPointer"); entity_row = offset("entityRow");
        component_bits = offset("componentBits"); lookup_manager = offset("lookupManager");
        const auto& definition_layout = selected.at("entityDefinitionLayout");
        auto definition_offset = [&](const char* key) {
            const auto value = definition_layout.at(key).get<std::size_t>();
            if (value > 0x10000) throw std::runtime_error(std::string("entity definition offset out of range: ") + key);
            return value;
        };
        definition_uuid = definition_offset("uuid");
        definition_name = definition_offset("name");
        definition_name_size = definition_offset("nameSize");
        const auto& hooks = selected.at("hooks");
        game_thread_signature = hooks.at("game_thread").at("signature").get<std::string>();
        entity_manager_signature = hooks.at("entity_manager").at("signature").get<std::string>();
        game_thread_original = hooks.at("game_thread").at("original").get<std::vector<std::uint8_t>>();
        entity_manager_original = hooks.at("entity_manager").at("original").get<std::vector<std::uint8_t>>();
        if (game_thread_original.size() < 5 || game_thread_original.size() > 32 ||
            entity_manager_original.size() < 5 || entity_manager_original.size() > 32) throw std::runtime_error("invalid hook length");
        world_prop_update_signature = hooks.at("world_prop_update").at("signature").get<std::string>();
        world_prop_update_original = hooks.at("world_prop_update").at("original").get<std::vector<std::uint8_t>>();
        world_actor_placement_signature = hooks.at("world_actor_placement").at("signature").get<std::string>();
        world_actor_placement_original = hooks.at("world_actor_placement").at("original").get<std::vector<std::uint8_t>>();
        if (hooks.contains("world_building_dispatch")) {
            world_building_dispatch_signature = hooks.at("world_building_dispatch").at("signature").get<std::string>();
            world_building_dispatch_original = hooks.at("world_building_dispatch").at("original").get<std::vector<std::uint8_t>>();
        } else {
            world_building_dispatch_signature.clear();
            world_building_dispatch_original.clear();
        }
        if (world_prop_update_original.size() < 5 || world_prop_update_original.size() > 32 ||
            world_actor_placement_original.size() < 5 || world_actor_placement_original.size() > 32)
            throw std::runtime_error("invalid world context hook length");
        const auto& cursor_hook = hooks.at("world_cursor");
        world_cursor_signature = cursor_hook.at("signature").get<std::string>();
        world_cursor_original = cursor_hook.at("original").get<std::vector<std::uint8_t>>();
        world_cursor_capture_offset = cursor_hook.at("captureOffset").get<std::size_t>();
        if (world_cursor_signature.empty() || world_cursor_signature.size() > 256 ||
            world_cursor_original.size() != 7 || world_cursor_capture_offset > 0x10000)
            throw std::runtime_error("invalid native world cursor hook profile");
        const auto& entity_context = selected.at("worldContexts").at("entityPlacement");
        auto context_offset = [&](const char* key) {
            const auto value = entity_context.at(key).get<std::size_t>();
            if (value > 0x10000) throw std::runtime_error(std::string("world context offset out of range: ") + key);
            return value;
        };
        world_context_layout.actor_frame_service_view = context_offset("actorFrameServiceViewOffset");
        world_context_layout.service_view_world = context_offset("serviceViewWorldOffset");
        world_context_layout.placement_context = context_offset("placementContextOffset");
        world_context_layout.place_queue = context_offset("placeQueueOffset");
        world_context_layout.remove_queue = context_offset("removeQueueOffset");
        world_context_layout.publish_state = context_offset("publishStateOffset");
        world_context_layout.publish_commands = context_offset("publishCommandsOffset");
        world_context_layout.owner = context_offset("ownerOffset");
        runtime_world_grids.clear();
        const auto& grids = selected.at("worldGrids");
        if (!grids.is_object() || grids.empty()) throw std::runtime_error("world grid catalog is missing or empty");
        for (auto item = grids.begin(); item != grids.end(); ++item) {
            RuntimeWorldGrid grid{};
            grid.id = item.key();
            const auto origin = item.value().at("origin").get<std::array<double, 3>>();
            const auto cell_size = item.value().at("cellSize").get<std::array<double, 3>>();
            const auto maximum = item.value().at("maximum").get<std::array<std::uint64_t, 3>>();
            for (int axis = 0; axis < 3; ++axis) {
                if (!std::isfinite(origin[axis]) || !std::isfinite(cell_size[axis]) || cell_size[axis] <= 0 || !maximum[axis])
                    throw std::runtime_error("invalid world grid specification: " + grid.id);
                grid.origin[axis] = origin[axis];
                grid.cell_size[axis] = cell_size[axis];
                grid.maximum[axis] = maximum[axis];
            }
            runtime_world_grids.push_back(std::move(grid));
        }
        runtime_components.clear();
        std::unordered_set<std::string> names;
        std::unordered_set<unsigned> indices;
        const nlohmann::json* component_entries{};
        if (selected.contains("components")) {
            component_entries = &selected.at("components");
        } else {
            throw std::runtime_error("selected build profile has no inline ECS components");
        }
        if (!component_entries || !component_entries->is_array())
            throw std::runtime_error("profile component catalog is not an array");
        for (const auto& component : *component_entries) {
            const auto name = component.at("name").get<std::string>();
            const auto index = component.at("index").get<unsigned>();
            const auto size = component.at("size").get<unsigned>();
            if (!name.starts_with("keen::ecs::") || index >= 1024 || !size || size > 65535 ||
                !names.insert(name).second || !indices.insert(index).second) throw std::runtime_error("invalid component mapping");
            if (exact) runtime_components.push_back({name, static_cast<std::uint16_t>(index), size});
        }
        if (exact && runtime_components.empty()) throw std::runtime_error("empty component profile");
        runtime_operations.clear();
        runtime_patches.clear();
        world_finish_event_id_rva = 0;
        const auto operations = selected.find("worldOperations");
        if (operations != selected.end()) {
            std::unordered_set<std::string> operation_names;
            for (auto item = operations->begin(); item != operations->end(); ++item) {
                const auto& value = item.value();
                RuntimeOperation operation{};
                operation.name = item.key();
                operation.function_rva = value.value("functionRva", std::uintptr_t{});
                operation.global_rva = value.value("globalRva", std::uintptr_t{});
                operation.guard_rva = value.value("guardRva", std::uintptr_t{});
                operation.validation_offset = value.value("validationOffset", std::uintptr_t{});
                operation.mode = value.value("mode", std::uint32_t{});
                operation.context_pointer_offset = value.value("contextPointerOffset", std::ptrdiff_t{});
                operation.world_offset = value.value("worldOffset", std::ptrdiff_t{});
                if (operation.name == "runtime.world.entity.finish_building")
                    world_finish_event_id_rva = value.value("eventIdRva", std::uintptr_t{});
                operation.guard_bytes = value.value("guardBytes", std::vector<std::uint8_t>{});
                operation.abi = value.at("abi").get<std::string>();
                operation.thread = value.at("thread").get<std::string>();
                operation.context = value.at("context").get<std::string>();
                if (!operation.name.starts_with("runtime.world.") ||
                    !operation_names.insert(operation.name).second ||
                    operation.abi.empty() || operation.abi.size() > 256 ||
                    operation.thread != "game" || operation.context.empty() || operation.context.size() > 256 ||
                    (operation.function_rva == 0) == (operation.global_rva == 0) ||
                    operation.guard_bytes.size() > 64)
                    throw std::runtime_error("invalid world operation profile entry: " + operation.name);
                if (operation.name == "runtime.world.context.active" &&
                    (!operation.global_rva || !operation.context_pointer_offset || !operation.world_offset))
                    throw std::runtime_error("active world context requires a validated pointer chain");

                const auto selected_image_size = static_cast<std::size_t>(nt->OptionalHeader.SizeOfImage);
                const bool target_in_image = operation.function_rva
                    ? operation.function_rva < selected_image_size
                    : operation.global_rva < selected_image_size;
                const bool guard_in_image = operation.guard_bytes.empty() ||
                    (operation.guard_rva < selected_image_size &&
                     operation.guard_bytes.size() <= selected_image_size - operation.guard_rva);
                const bool guard_matches = guard_in_image &&
                    (operation.guard_bytes.empty() ||
                     std::memcmp(base + operation.guard_rva, operation.guard_bytes.data(),
                                 operation.guard_bytes.size()) == 0);
                const bool event_id_valid = operation.name != "runtime.world.entity.finish_building" ||
                    (world_finish_event_id_rva && world_finish_event_id_rva < selected_image_size &&
                     sizeof(std::uint32_t) <= selected_image_size - world_finish_event_id_rva);
                operation.available = target_in_image && guard_matches && event_id_valid &&
                    (exact || !operation.guard_bytes.empty());
                operation.status = !target_in_image ? "target-outside-image" :
                    !guard_in_image ? "guard-outside-image" :
                    !guard_matches ? "instruction-guard-mismatch" :
                    !event_id_valid ? "finish-event-id-outside-image" :
                    !exact ? "build-diff-guard-verified" :
                    operation.global_rva ? "runtime-pointer-validation-required" : "verified";
                runtime_operations.push_back(std::move(operation));
            }
        }
        const auto patches = selected.find("runtimePatches");
        if (patches != selected.end()) {
            std::unordered_set<std::string> patch_names;
            for (auto item = patches->begin(); item != patches->end(); ++item) {
                const auto& value = item.value();
                RuntimePatch patch{};
                patch.name = item.key();
                patch.signature = value.at("signature").get<std::string>();
                patch.kind = value.at("kind").get<std::string>();
                const auto& function = value.at("function");
                patch.function_id = function.at("id").get<std::string>();
                patch.function_begin_rva = function.at("beginRva").get<std::uintptr_t>();
                patch.function_end_rva = function.at("endRva").get<std::uintptr_t>();
                patch.target_offset = function.at("targetOffset").get<std::size_t>();
                patch.overwrite = value.at("overwriteBytes").get<std::size_t>();
                patch.payload = value.at("payload").get<std::vector<std::uint8_t>>();
                patch.return_rel32_offset = value.value("returnRel32Offset", std::size_t{});
                if (!patch.name.starts_with("runtime.patch.") || !patch_names.insert(patch.name).second ||
                    patch.function_id.empty() || patch.function_id.size() > 128 ||
                    patch.function_begin_rva >= patch.function_end_rva ||
                    patch.target_offset >= patch.function_end_rva - patch.function_begin_rva ||
                    patch.signature.empty() || patch.signature.size() > 256 || patch.overwrite < 3 || patch.overwrite > 32 ||
                    patch.payload.empty() || patch.payload.size() > 256 ||
                    (patch.kind != "bytes" && patch.kind != "detour") ||
                    (patch.kind == "bytes" && patch.payload.size() != patch.overwrite) ||
                    (patch.kind == "detour" && (patch.return_rel32_offset < 1 || patch.return_rel32_offset + 4 > patch.payload.size() ||
                        patch.payload[patch.return_rel32_offset - 1] != 0xe9)))
                    throw std::runtime_error("invalid runtime patch profile entry: " + patch.name);
                if (exact && (patch.function_end_rva > nt->OptionalHeader.SizeOfImage ||
                    patch.function_begin_rva >= nt->OptionalHeader.SizeOfImage))
                    throw std::runtime_error("runtime patch function range outside image: " + patch.name);
                runtime_patches.push_back(std::move(patch));
            }
        }
        status = selected.at("id").get<std::string>() + (exact ? ":exact-image" : ":build-diff-warning");
        return true;
    } catch (const std::exception& error) {
        status = std::string("profile-error:") + error.what();
        OutputDebugStringA(status.c_str());
        return false;
    }
}
}
