#include "prove_profile_layout.h"
#include <algorithm>
#include <iterator>
#include <limits>
#include <optional>

namespace GameProfileDiscovery::Detail {
namespace {
using json = nlohmann::json;
using Roots = std::set<std::uint32_t>;

struct Scope {
    Roots roots;
    std::string role;
    bool immediate{};
    bool element_stride{};
    int argument{-1};
    Roots required;
};

bool identical_layout_procedure(const Procedure& source, const Procedure& target) {
    if (source.root != target.root || source.ranges != target.ranges || source.signature != target.signature ||
        source.complete != target.complete || source.references_complete != target.references_complete ||
        source.references.size() != target.references.size() || source.layout_uses.size() != target.layout_uses.size() ||
        source.call_arguments.size() != target.call_arguments.size() || source.constants != target.constants)
        return false;
    for (std::size_t i = 0; i < source.references.size(); ++i) {
        const auto& a = source.references[i];
        const auto& b = target.references[i];
        if (a.range != b.range || a.offset != b.offset || a.target != b.target || a.branch != b.branch ||
            a.indexed_image != b.indexed_image || a.table_entries != b.table_entries ||
            a.table_width != b.table_width || a.read_bytes != b.read_bytes ||
            a.address_only != b.address_only || a.write_bytes != b.write_bytes) return false;
    }
    for (std::size_t i = 0; i < source.layout_uses.size(); ++i) {
        const auto& a = source.layout_uses[i];
        const auto& b = target.layout_uses[i];
        if (a.instruction != b.instruction || a.operand != b.operand || a.value != b.value ||
            a.argument != b.argument || a.kind != b.kind || a.memory_width_bytes != b.memory_width_bytes ||
            a.memory_read != b.memory_read || a.memory_write != b.memory_write) return false;
    }
    for (std::size_t i = 0; i < source.call_arguments.size(); ++i) {
        const auto& a = source.call_arguments[i];
        const auto& b = target.call_arguments[i];
        if (a.instruction != b.instruction || a.target != b.target || a.offsets != b.offsets) return false;
    }
    return true;
}

// These values bound ShroudForge traversal/allocation work. They are not an
// assertion about an engine field or an original fixed buffer capacity.
bool runtime_policy(const std::string& path, std::uint64_t value, json& row) {
    static const std::map<std::string, std::pair<const char*, const char*>> readers{
        {"/ecsReadLayout/maximumSceneEntries", {"engine-context/resolve_engine_owners.cpp", "scene count/capacity <= layout.maximum_scenes"}},
        {"/ecsReadLayout/maximumEntities", {"engine-context/resolve_engine_owners.cpp", "entity table capacity <= layout.entities.maximum_entities"}},
        {"/resourceLayout/maximumEntries", {"loaded-resources/resolve_loaded_resource.cpp", "resource map counts <= layout.maximum_entries"}},
        {"/resourceLayout/maximumChunks", {"loaded-resources/resolve_loaded_resource.cpp", "chunk count <= layout.maximum_chunks"}},
        {"/resourceLayout/maximumResourceBytes", {"loaded-resources/resolve_loaded_resource.cpp", "payload extent <= layout.maximum_resource_bytes"}},
        {"/resourceLayout/itemIdMap/maximumCapacity", {"loaded-resources/resolve_item_resource.cpp", "ItemRegistry capacity <= item.maximum_capacity"}},
        {"/resourceLayout/itemIdMap/maximumProbes", {"loaded-resources/resolve_item_resource.cpp", "bounded probe count < item.maximum_probes"}},
        {"/resourceLayout/itemIdMap/headerSize", {"loaded-resources/resolve_item_resource.cpp", "unchanged bounded ShroudForge header snapshot extent, field extents separately validated"}},
        {"/resourceLayout/map/headerSize", {"loaded-resources/resolve_loaded_resource.cpp", "unchanged ShroudForge header range-overflow extent, field extents separately validated"}},
        {"/ecsWriteLayout/maximumWriteBytes", {"ecs/write_entity_components.cpp", "operation size <= guard.MaximumBytes()"}},
        {"/ecsWriteLayout/maximumTrackingQueueCapacity", {"engine-context/configure_engine_layout.cpp", "nonzero configured validation bound, at most 1<<20"}},
        {"/serviceLayout/maximumRecords", {"named-services/resolve_named_service.cpp", "record count <= contract.maximum_records"}},
        {"/serviceLayout/maximumMapCapacity", {"named-services/resolve_named_service.cpp", "map capacity <= contract.maximum_capacity"}},
        {"/serviceLayout/maximumSystems", {"named-services/resolve_named_service.cpp", "system count <= contract.maximum_systems"}},
    };
    const auto found = readers.find(path);
    if (found == readers.end()) return false;
    row = {{"path", path}, {"before", value}, {"after", value}, {"method", "runtime-policy-preserved"},
        {"reader", std::string("modules/game-runtime/native/") + found->second.first}, {"predicate", found->second.second}};
    return true;
}

bool inactive_server_contract(const json& baseline, const std::string& path, std::uint64_t value, json& row) {
    if (baseline.value("target", std::string{}) != "enshrouded_server.exe") return false;
    static const std::map<std::string, const char*> readers{
        {"/ownerLayout/clientPointerOffset", "engine-context/resolve_engine_owners.cpp::resolve"},
        {"/ownerLayout/controllerPointerOffset", "engine-context/resolve_engine_owners.cpp::resolve"},
        {"/ownerLayout/controllerClientBackPointerOffset", "engine-context/resolve_engine_owners.cpp::resolve"},
        {"/ownerLayout/controllerMainOwnerOffset", "engine-context/resolve_engine_owners.cpp::resolve"},
        {"/ecsReadLayout/clientSceneOwnerOffset", "engine-context/resolve_engine_owners.cpp::select_execution_owner"},
        {"/resourceLayout/mainActiveContextOffset", "loaded-resources/resolve_loaded_resource.cpp"},
    };
    const auto found = readers.find(path);
    if (found == readers.end()) return false;
    row = {{"path", path}, {"before", value}, {"after", value}, {"method", "preserved-inactive-target-contract"},
        {"target", "enshrouded_server.exe"}, {"reader", std::string("modules/game-runtime/native/") + found->second},
        {"branch", "dedicated-server uses registered Scene/world context before the client-owner branch"},
        {"engineOffsetDiscovered", false}};
    return true;
}

class LayoutProver {
    const json& baseline;
    const ProfileRebaseAnalysis& analysis;
    std::map<std::uint32_t, std::pair<std::uint32_t, std::uint32_t>> source_ranges;
    std::map<std::uint32_t, std::pair<std::uint32_t, std::uint32_t>> target_ranges;
    std::map<std::int64_t, std::vector<std::pair<std::uint32_t, const LayoutUse*>>> source_uses_by_value,
        target_uses_by_value;
    std::map<std::uint32_t, std::uint32_t> target_counterparts;
    std::map<std::uint32_t, Roots> source_callers, source_references;
    std::map<std::uint32_t, std::uint32_t> owner_counterparts;
    std::map<std::uint32_t, json> owner_callsites;
    Roots changed_owner_callees;
    Roots owners, tasks, task_callers, queue_initializers, queue_drains, scene_starters,
        scene_constructors, scene_arrays, scene_entries, threads, registries, tracker,
        scene_context_readers, scene_context_owners,
        entity_storage, entity_records, entity_record_owners, entity_definitions, definition_copies,
        resources, resource_callbacks, resource_metadata_getters, resource_key_lookups, resource_header_validators,
        resource_workers, item_registry, services, dependencies, spawn;
    Roots service_system_records, service_system_owners, service_binding_records, service_binding_owners;

    std::optional<std::uint64_t> number(const std::string& path) const {
        const auto pointer = json::json_pointer(path);
        if (!baseline.contains(pointer)) return std::nullopt;
        const auto& value = baseline.at(pointer);
        if (!value.is_number_unsigned() && (!value.is_number_integer() || value.get<std::int64_t>() < 0))
            return std::nullopt;
        return value.get<std::uint64_t>();
    }
    const Procedure* procedure(std::uint32_t rva) const {
        const auto direct = analysis.source_functions.find(rva);
        if (direct != analysis.source_functions.end()) return &direct->second;
        auto range = source_ranges.upper_bound(rva);
        if (range == source_ranges.begin()) return nullptr;
        --range;
        return rva < range->second.first ? &analysis.source_functions.at(range->second.second) : nullptr;
    }
    const Procedure* target_procedure(std::uint32_t rva) const {
        const auto direct = analysis.target_functions.find(rva);
        if (direct != analysis.target_functions.end()) return &direct->second;
        auto range = target_ranges.upper_bound(rva);
        if (range == target_ranges.begin()) return nullptr;
        --range;
        return rva < range->second.first ? &analysis.target_functions.at(range->second.second) : nullptr;
    }
    std::optional<std::uint32_t> counterpart(std::uint32_t root) const {
        if (changed_owner_callees.contains(root)) return std::nullopt;
        const auto exact = analysis.exact_counterparts.find(root);
        if (exact != analysis.exact_counterparts.end()) return exact->second;
        const auto local = owner_counterparts.find(root);
        return local == owner_counterparts.end() ? std::nullopt : std::optional{local->second};
    }
    std::optional<std::uint32_t> layout_counterpart(std::uint32_t root) const {
        const auto proven = analysis.exact_counterparts.find(root);
        if (proven != analysis.exact_counterparts.end()) return proven->second;
        const auto before = analysis.source_functions.find(root), after = analysis.target_functions.find(root);
        if (before == analysis.source_functions.end() || after == analysis.target_functions.end() ||
            !identical_layout_procedure(before->second, after->second)) return std::nullopt;
        return root;
    }
    bool exact_layout_pair(std::uint32_t source_root, std::uint32_t target_root) const {
        const auto mapped = analysis.exact_counterparts.find(source_root);
        const auto source = analysis.source_functions.find(source_root);
        const auto target = analysis.target_functions.find(target_root);
        if (source == analysis.source_functions.end() || target == analysis.target_functions.end()) return false;
        if (mapped != analysis.exact_counterparts.end() && mapped->second == target_root)
            return source->second.complete && target->second.complete &&
                source->second.signature == target->second.signature &&
                source->second.ranges.size() == target->second.ranges.size();
        return source_root == target_root && identical_layout_procedure(source->second, target->second);
    }
    static std::optional<std::uint32_t> target_instruction(const Procedure& source, const Procedure& target, std::uint32_t instruction) {
        if (source.ranges.size() != target.ranges.size()) return std::nullopt;
        for (unsigned i = 0; i < source.ranges.size(); ++i)
            if (instruction >= source.ranges[i].first && instruction < source.ranges[i].second)
                return target.ranges[i].first + instruction - source.ranges[i].first;
        return std::nullopt;
    }
    void bind_owner_children(Roots frontier) {
        Roots seen;
        for (unsigned depth = 0; depth != 3; ++depth) {
            Roots next;
            for (const auto root : frontier) {
                if (!seen.insert(root).second) continue;
                const auto& source = analysis.source_functions.at(root);
                const auto pair = counterpart(root);
                if (!pair) continue;
                const auto& target = analysis.target_functions.at(*pair);
                for (const auto& call : source.call_arguments) {
                    const auto* before = procedure(call.target);
                    if (!before) continue;
                    const auto instruction = target_instruction(source, target, call.instruction);
                    const auto found = std::find_if(target.call_arguments.begin(), target.call_arguments.end(), [&](const auto& candidate) {
                        return instruction && candidate.instruction == *instruction;
                    });
                    const auto* after = found == target.call_arguments.end() ? nullptr : target_procedure(found->target);
                    if (!after || !before->complete || !after->complete || before->signature != after->signature) {
                        changed_owner_callees.insert(before->root);
                        continue;
                    }
                    const auto old = counterpart(before->root);
                    if (old && *old != after->root) { changed_owner_callees.insert(before->root); continue; }
                    owner_counterparts.emplace(before->root, after->root);
                    owner_callsites[before->root].push_back({{"sourceProcedureRva", root}, {"targetProcedureRva", target.root},
                        {"sourceInstructionRva", call.instruction}, {"targetInstructionRva", found->instruction},
                        {"sourceCalleeRva", before->root}, {"targetCalleeRva", after->root}, {"kind", "original-owner-bound-callsite"}});
                    next.insert(before->root);
                }
            }
            frontier = std::move(next);
        }
    }
    Roots at(const std::string& path) const {
        const auto value = number(path);
        if (!value || !*value || *value > UINT32_MAX) return {};
        const auto* function = procedure(static_cast<std::uint32_t>(*value));
        return function ? Roots{function->root} : Roots{};
    }
    static void merge(Roots& into, const Roots& from) { into.insert(from.begin(), from.end()); }
    Roots named(const std::string& name) const {
        const auto found = analysis.named_procedures.find(name);
        return found != analysis.named_procedures.end() && found->second.size() == 1 ? found->second : Roots{};
    }
    bool memory(const Procedure& function, std::uint64_t value, int argument = -1) const {
        return std::any_of(function.layout_uses.begin(), function.layout_uses.end(), [&](const auto& use) {
            return use.value >= 0 && static_cast<std::uint64_t>(use.value) == value &&
                (argument < 0 ? use.kind == "original-memory-displacement" :
                    use.kind == "original-argument-relative-field" && use.argument == argument);
        });
    }
    bool tuple(const Procedure& function, std::initializer_list<const char*> paths, int argument = -1) const {
        for (const auto* path : paths) {
            const auto value = number(path);
            if (!value || !memory(function, *value, argument)) return false;
        }
        return true;
    }
    Roots children(const Roots& roots, unsigned depth = 1) const {
        Roots all = roots, next = roots;
        for (unsigned level = 0; level < depth; ++level) {
            Roots added;
            for (const auto root : next) {
                const auto found = analysis.source_functions.find(root);
                if (found == analysis.source_functions.end()) continue;
                for (const auto& call : found->second.call_arguments) {
                    const auto* target = procedure(call.target);
                    if (target && !all.contains(target->root)) added.insert(target->root);
                }
            }
            merge(all, added);
            next = std::move(added);
        }
        return all;
    }
    Roots callers(const Roots& roots) const {
        Roots result;
        for (const auto root : roots) {
            const auto found = source_callers.find(root);
            if (found != source_callers.end()) merge(result, found->second);
        }
        return result;
    }
    Roots references(std::initializer_list<const char*> paths, bool require_all = false) const {
        Roots result;
        bool first = true;
        for (const auto* path : paths) {
            const auto value = number(path);
            if (!value || !*value || *value > UINT32_MAX) continue;
            const auto found = source_references.find(static_cast<std::uint32_t>(*value));
            const Roots empty;
            const auto& roots = found == source_references.end() ? empty : found->second;
            if (!require_all || first) merge(result, roots);
            else {
                Roots common;
                std::set_intersection(result.begin(), result.end(), roots.begin(), roots.end(), std::inserter(common, common.end()));
                result = std::move(common);
            }
            first = false;
        }
        return result;
    }
    Roots with_tuple(const Roots& roots, std::initializer_list<const char*> paths, int argument = -1) const {
        Roots result;
        for (const auto root : roots) if (tuple(analysis.source_functions.at(root), paths, argument)) result.insert(root);
        return result;
    }
    Scope scope(const std::string& path) const {
        const auto leaf = path.substr(path.find_last_of('/') + 1);
        const bool stride = leaf == "stride" || leaf == "offset" || leaf.ends_with("Stride") ||
            leaf.ends_with("Size") || leaf.ends_with("Capacity") || leaf.ends_with("Value") ||
            leaf.ends_with("State") || leaf.ends_with("Mode") || leaf.ends_with("Mask");
        if (path.starts_with("/ownerLayout/")) return {owners, "original-owner-reference", stride};
        if (path.starts_with("/entityCommandQueueLayout/")) {
            if (leaf == "activeTrackingContextsOffset")
                return {children(queue_drains, 2), "original-queue-drain-manager-counter"};
            return {queue_initializers, "original-ecs::Tasks-queue-initializer", stride};
        }
        if (path.starts_with("/buildingPropSpawnLayout/")) return {spawn, "original-player_building_place_prop-call-contract", true};
        if (path.starts_with("/systemDependencyLayout/")) return {dependencies, "original-system-dependency-builder", true};
        if (path.starts_with("/serviceLayout/")) {
            Roots records;
            if (leaf == "recordsPointerOffset" || leaf == "recordCountOffset")
                records = with_tuple(services, {"/serviceLayout/recordsPointerOffset", "/serviceLayout/recordCountOffset"});
            else if (leaf.starts_with("map")) records = with_tuple(services, {"/serviceLayout/mapBitsOffset",
                "/serviceLayout/mapCapacityOffset", "/serviceLayout/mapKeysOffset", "/serviceLayout/mapValuesOffset"});
            else if (leaf == "systemsOffset" || leaf == "systemCountOffset" || leaf == "systemStride")
                return {service_system_records, "original-registry-system-record-iterator", stride, false, -1, service_system_owners};
            else if (leaf.starts_with("system"))
                return {service_binding_records, "original-descriptor-view-to-record-binding-writer", false, false, 0, service_binding_owners};
            else records = with_tuple(services, {"/serviceLayout/recordHashOffset", "/serviceLayout/recordNormalPointerOffset",
                "/serviceLayout/recordAlternatePointerOffset"});
            return {records, "original-named-service-record-or-header-tuple", stride};
        }
        if (path.starts_with("/resourceWriteLayout/")) return {resource_workers, "original-resource-worker-owner", stride};
        if (path.starts_with("/resourceLayout/itemIdMap/")) {
            const auto records = with_tuple(item_registry, {"/resourceLayout/itemIdMap/bitsPointerOffset",
                "/resourceLayout/itemIdMap/capacityOffset", "/resourceLayout/itemIdMap/keysPointerOffset",
                "/resourceLayout/itemIdMap/valuesPointerOffset"});
            return {records, "original-ItemRegistry-map-header-tuple", stride};
        }
        if (path.starts_with("/resourceLayout/map/")) {
            if (leaf == "keyStride") return {resource_key_lookups, "original-resource-key-index-byte-arithmetic", false, true};
            if (leaf == "bucketStride" || leaf == "valueStride")
                return {resource_key_lookups, "original-resource-array-address-index-scale", false, true};
            if (leaf == "keyCountOffset" || leaf == "valueCountOffset")
                return {resource_header_validators, "original-resource-header-enumeration-validator"};
            return {resource_key_lookups, "original-resource-dictionary-header-reader", false, false, 0};
        }
        if (path.starts_with("/resourceLayout/valueEntry/")) {
            const auto records = with_tuple(children(resource_callbacks), {"/resourceLayout/valueEntry/resourceOffsetOffset",
                "/resourceLayout/valueEntry/packedSizeOffset", "/resourceLayout/valueEntry/chunkIndexOffset"});
            return {records, "original-packed-resource-value-reader", true};
        }
        if (path.starts_with("/resourceLayout/chunkEntry/")) {
            const auto records = with_tuple(resource_callbacks, {"/resourceLayout/resourceServiceChunkTableOffset",
                "/resourceLayout/resourceServiceChunkCountOffset"});
            return {records, "original-resource-chunk-table-reader", true};
        }
        if (path.starts_with("/resourceLayout/")) {
            if (leaf == "resourceServiceIdCallbackOffset" || leaf == "resourceServiceHandleCallbackOffset")
                return {resources, "original-resource-callback-installing-constructor"};
            if (leaf == "resourceServiceReaderOffset" || leaf == "resourceServiceChunkTableOffset" || leaf == "resourceServiceChunkCountOffset")
                return {resource_callbacks, "original-registered-resource-service-argument", false, false, 1};
            if (leaf == "readerMetadataOffset" || leaf == "metadataMapOffset")
                return {resource_metadata_getters, "original-common-resource-reader-metadata-getter", true};
            if (leaf.starts_with("itemRegistry")) return {with_tuple(item_registry,
                {"/resourceLayout/itemRegistryServiceOffset", "/resourceLayout/itemRegistryHandleOffset",
                 "/resourceLayout/itemRegistryResourcePointerOffset"}), "original-ItemRegistry-service-resource-tuple"};
            if (leaf == "mainActiveContextOffset" || leaf == "contextItemRegistryOffset")
                return {task_callers, "original-world-context-service-constructor"};
            return {resource_callbacks, "original-ResourceService-exact-registered-callback", true};
        }
        if (path.starts_with("/ecsWriteLayout/")) {
            if (leaf.starts_with("changeTracker")) {
                if (leaf == "changeTrackerContextOffset") return {queue_drains, "original-task-tracker-links"};
                return {children(tracker, 2), "original-change-tracker-ABI", stride};
            }
            if (leaf == "executionModeOffset" || leaf == "executionAuthorityMode")
                return {tasks, "original-execution-owner-constructor", stride};
            if (leaf.starts_with("scene")) return {children(scene_starters), "original-GameServer-Scene-constructor", stride};
            if (leaf.starts_with("registryEntry") || leaf.starts_with("threadRegistry"))
                return {registries, "original-image-thread-registration", stride};
            return {threads, "original-thread-name-callback-constructor", stride};
        }
        if (path.starts_with("/ecsReadLayout/")) {
            if (leaf == "mainExecutionOwnerOffset") return {task_callers, "original-execution-constructor-return-owner"};
            if (leaf == "executionRegistryOwnerOffset" || leaf == "executionEntityManagerOffset")
                return {tasks, "original-ecs::Tasks-execution-owner"};
            if (leaf == "entityManagerOwnerOffset") return {queue_initializers, "original-queue-manager-input", false, false, 1};
            if (leaf.starts_with("entityManager")) return {entity_storage, "original-entity-map-lookup"};
            if (leaf == "entityId" || leaf == "entityStorage" || leaf == "entityRow")
                return {tracker, "original-change-tracker-entity-argument", false, false, 2};
            if (leaf == "entityLayout" || leaf.starts_with("component"))
                return {entity_storage, "original-entity-component-address-calculation"};
            if (leaf == "entityGeneration") return {entity_records, "original-manager-relative-entity-record-serializer",
                false, false, -1, entity_record_owners};
            if (leaf == "entityDefinitionPointer") return {entity_definitions, "original-entity-map-definition-reader"};
            if (leaf == "entityDefinitionUuidOffset") return {{}, "original-entity-definition-UUID-copy"};
            if (leaf == "clientSceneOwnerOffset") return {owners, "original-client-Scene-owner"};
            if (leaf == "sceneEntryStride") return {scene_arrays, "original-Scene-entry-array", true, true};
            if (leaf.starts_with("sceneEntries")) return {scene_constructors, "original-Scene-array-header"};
            if (leaf == "sceneEntryContextOffset") return {scene_context_readers,
                "original-inline-Scene-context-array-and-world-initializer", false, false, -1, scene_context_owners};
            return {scene_entries, "original-Scene-entry-UUID-state-reader", true};
        }
        return {{}, "profile-field-owner-not-established"};
    }
    std::optional<json> witness(const Procedure& source, const LayoutUse& use) const {
        const auto match = counterpart(source.root);
        if (!match) return std::nullopt;
        const auto found = analysis.target_functions.find(*match);
        if (found == analysis.target_functions.end()) return std::nullopt;
        const auto& target = found->second;
        if (!source.complete || !target.complete || source.signature != target.signature ||
            source.ranges.size() != target.ranges.size()) return std::nullopt;
        for (unsigned i = 0; i < source.ranges.size(); ++i) {
            const auto [begin, end] = source.ranges[i];
            if (use.instruction < begin || use.instruction >= end) continue;
            const auto instruction = target.ranges[i].first + use.instruction - begin;
            const auto same = std::find_if(target.layout_uses.begin(), target.layout_uses.end(), [&](const auto& other) {
                return other.instruction == instruction && other.operand == use.operand && other.kind == use.kind &&
                    other.argument == use.argument && other.value == use.value &&
                    other.memory_width_bytes == use.memory_width_bytes && other.memory_read == use.memory_read && other.memory_write == use.memory_write;
            });
            if (same == target.layout_uses.end()) return std::nullopt;
            json proof{{"sourceProcedureRva", source.root}, {"targetProcedureRva", target.root},
                {"sourceInstructionRva", use.instruction}, {"targetInstructionRva", instruction},
                {"operand", use.operand}, {"kind", use.kind}, {"sourceArgument", use.argument}, {"value", use.value},
                {"memoryWidthBytes", use.memory_width_bytes}, {"memoryRead", use.memory_read}, {"memoryWrite", use.memory_write}};
            const auto binding = owner_callsites.find(source.root);
            if (binding != owner_callsites.end()) proof["ownerCallsites"] = binding->second;
            return proof;
        }
        return std::nullopt;
    }
    std::optional<json> paired_procedure(std::uint32_t root) const {
        const auto match = counterpart(root);
        if (!match) return std::nullopt;
        const auto source = analysis.source_functions.find(root), target = analysis.target_functions.find(*match);
        if (source == analysis.source_functions.end() || target == analysis.target_functions.end() ||
            !source->second.complete || !target->second.complete || source->second.signature != target->second.signature)
            return std::nullopt;
        json proof{{"sourceProcedureRva", root}, {"targetProcedureRva", *match}};
        const auto binding = owner_callsites.find(root);
        if (binding != owner_callsites.end()) proof["ownerCallsites"] = binding->second;
        return proof;
    }
    bool reflected_item_field(const std::string& path, std::uint64_t value, json& row) const {
        const char* field = path == "/resourceLayout/itemIdMap/itemIdOffset" ? "itemId" :
            path == "/resourceLayout/itemIdMap/guidOffset" ? "objectId" : nullptr;
        if (!field || !baseline.contains("resourceLayout") || !baseline.at("resourceLayout").contains("itemIdMap")) return false;
        const auto name = baseline.at("resourceLayout").at("itemIdMap").value("targetType", std::string{});
        const auto before = analysis.source_types.find(name), after = analysis.target_types.find(name);
        if (before == analysis.source_types.end() || after == analysis.target_types.end() ||
            before->second.layout != after->second.layout || before->second.hash != after->second.hash ||
            !before->second.layout.contains("fields") || !before->second.layout.at("fields").contains(field) ||
            before->second.layout.at("fields").at(field).at("offset") != value) return false;
        row = {{"path", path}, {"before", value}, {"after", value}, {"method", "exact-original-qualified-resource-field"},
            {"qualifiedType", name}, {"field", field}, {"sourceMetadataRva", before->second.metadata},
            {"targetMetadataRva", after->second.metadata}};
        return true;
    }
    bool reflected_contract_number(const std::string& path, std::uint64_t value, json& row) const {
        const auto slash = path.find_last_of('/');
        const auto leaf = path.substr(slash + 1);
        std::string name;
        bool size{};
        if (path.starts_with("/serviceLayout/contracts/") && (leaf == "size" || leaf == "qualifiedHash")) {
            const auto& contract = baseline.at(json::json_pointer(path.substr(0, slash)));
            name = contract.value("qualifiedType", std::string{});
            size = leaf == "size";
        } else if (path == "/resourceLayout/itemIdMap/targetSize" || path == "/resourceLayout/itemIdMap/targetHash") {
            name = baseline.at("resourceLayout").at("itemIdMap").value("targetType", std::string{});
            size = leaf == "targetSize";
        } else return false;
        const auto before = analysis.source_types.find(name), after = analysis.target_types.find(name);
        if (before == analysis.source_types.end() || after == analysis.target_types.end() ||
            before->second.layout != after->second.layout || before->second.hash != after->second.hash ||
            (size ? before->second.size : before->second.hash) != value ||
            (size ? after->second.size : after->second.hash) != value) return false;
        row = {{"path", path}, {"before", value}, {"after", value}, {"method", "exact-original-qualified-type-contract"},
            {"qualifiedType", name}, {"property", size ? "size" : "qualifiedHash"},
            {"sourceMetadataRva", before->second.metadata}, {"targetMetadataRva", after->second.metadata}};
        return true;
    }
    std::optional<bool> client_pointer_slot(std::uint64_t value, json& row) const {
        const auto root = number("/ownerLayout/rootObjectRva");
        if (!root || *root > UINT32_MAX || value > UINT32_MAX - *root) return std::nullopt;
        const auto slot = static_cast<std::uint32_t>(*root + value);
        const auto consumers = source_references.find(slot);
        if (consumers == source_references.end()) return std::nullopt;
        row = {{"path", "/ownerLayout/clientPointerOffset"}, {"before", value}, {"after", value},
            {"method", "original-client-slot-RIP-reference-minus-independent-root"}, {"witnesses", json::array()}};
        const auto target_root = analysis.established_rvas.find("/ownerLayout/rootObjectRva");
        if (target_root == analysis.established_rvas.end()) {
            row["reason"] = "independent-target-owner-root-unresolved";
            return false;
        }
        bool observed{};
        for (const auto source_root : consumers->second) {
            const auto paired = paired_procedure(source_root);
            if (!paired) { row["reason"] = "original-client-slot-consumer-unresolved"; return false; }
            const auto& source = analysis.source_functions.at(source_root);
            const auto& target = analysis.target_functions.at(paired->at("targetProcedureRva").get<std::uint32_t>());
            for (const auto& reference : source.references) {
                if (reference.branch || reference.target != slot) continue;
                const auto found = std::find_if(target.references.begin(), target.references.end(), [&](const auto& candidate) {
                    return !candidate.branch && candidate.range == reference.range && candidate.offset == reference.offset;
                });
                if (found == target.references.end() || found->target < target_root->second ||
                    found->target - target_root->second != value) {
                    row["reason"] = "original-client-slot-offset-changed";
                    return false;
                }
                auto proof = *paired;
                proof["sourceInstructionRva"] = source.ranges[reference.range].first + reference.offset;
                proof["targetInstructionRva"] = target.ranges[found->range].first + found->offset;
                proof["sourceSlotRva"] = slot;
                proof["targetSlotRva"] = found->target;
                proof["sourceRootRva"] = *root;
                proof["targetRootRva"] = target_root->second;
                proof["relativeFieldOffset"] = value;
                row["witnesses"].push_back(std::move(proof));
                observed = true;
            }
        }
        return observed;
    }
    bool resource_key_extent(std::uint64_t value, json& row) const {
        if (resource_key_lookups.empty()) return false;
        for (const auto root : resource_key_lookups) {
            std::uint64_t end{};
            for (const auto& use : analysis.source_functions.at(root).layout_uses) {
                if (use.kind != "original-argument-relative-field" || use.argument != 1 ||
                    !use.memory_read || !use.memory_width_bytes || use.value < 0) continue;
                end = std::max(end, static_cast<std::uint64_t>(use.value) + use.memory_width_bytes);
                const auto proof = witness(analysis.source_functions.at(root), use);
                if (!proof) return false;
                row["witnesses"].push_back(*proof);
            }
            if (end != value) return false;
        }
        row["method"] = "original-resource-key-argument-read-extent";
        return true;
    }
    bool definition_uuid(std::uint64_t value, json& row) const {
        if (value != 0 || entity_definitions.empty() || definition_copies.empty()) return false;
        bool observed{};
        for (const auto root : entity_definitions) {
            const auto paired = paired_procedure(root);
            if (!paired) return false;
            row["witnesses"].push_back(*paired);
        }
        for (const auto root : definition_copies) {
            const auto paired = paired_procedure(root);
            if (!paired) return false;
            const auto& function = analysis.source_functions.at(root);
            const auto& target = analysis.target_functions.at(paired->at("targetProcedureRva").get<std::uint32_t>());
            for (const auto& call : function.call_arguments) {
                if (!call.offsets[1] || *call.offsets[1] != 0) continue;
                const auto found = std::find_if(target.call_arguments.begin(), target.call_arguments.end(), [&](const auto& candidate) {
                    return candidate.offsets[1] && *candidate.offsets[1] == 0 &&
                        candidate.instruction - target.root == call.instruction - function.root;
                });
                if (found == target.call_arguments.end()) return false;
                auto proof = *paired;
                proof["sourceInstructionRva"] = call.instruction;
                proof["targetInstructionRva"] = found->instruction;
                proof["kind"] = "original-UUID-copy-source-argument-origin";
                proof["sourceArgument"] = 1;
                proof["copyExtent"] = 2 * sizeof(std::uint64_t);
                row["witnesses"].push_back(std::move(proof));
                observed = true;
            }
        }
        return observed;
    }
    bool exact_global_use_set(const std::string& path, std::uint64_t value, json& row) const {
        if (value > static_cast<std::uint64_t>(INT64_MAX)) return false;
        const auto old_uses = source_uses_by_value.find(static_cast<std::int64_t>(value));
        const auto new_uses = target_uses_by_value.find(static_cast<std::int64_t>(value));
        if (old_uses == source_uses_by_value.end() || new_uses == target_uses_by_value.end() ||
            old_uses->second.empty() || new_uses->second.empty()) return false;
        using UseKey = std::tuple<std::uint32_t, std::uint32_t, unsigned, std::string, int,
            std::int64_t, unsigned, bool, bool>;
        std::multiset<UseKey> expected, actual;
        json witnesses = json::array();
        for (const auto& [source_root, use] : old_uses->second) {
            const auto paired_root = layout_counterpart(source_root);
            if (!paired_root) {
                row["reason"] = "original-global-layout-consumer-unmapped";
                return false;
            }
            const auto source = analysis.source_functions.find(source_root);
            const auto target = analysis.target_functions.find(*paired_root);
            if (source == analysis.source_functions.end() || target == analysis.target_functions.end() ||
                !exact_layout_pair(source_root, *paired_root)) {
                row["reason"] = "original-global-layout-consumer-code-changed";
                return false;
            }
            const auto mapped_instruction = target_instruction(source->second, target->second, use->instruction);
            if (!mapped_instruction) {
                row["reason"] = "original-global-layout-consumer-instruction-unmapped";
                return false;
            }
            const auto match = std::find_if(target->second.layout_uses.begin(), target->second.layout_uses.end(),
                [&](const auto& candidate) {
                    return candidate.instruction == *mapped_instruction && candidate.operand == use->operand &&
                        candidate.value == use->value && candidate.argument == use->argument &&
                        candidate.kind == use->kind && candidate.memory_width_bytes == use->memory_width_bytes &&
                        candidate.memory_read == use->memory_read && candidate.memory_write == use->memory_write;
                });
            if (match == target->second.layout_uses.end()) {
                row["reason"] = "original-global-layout-operand-changed";
                return false;
            }
            expected.emplace(*paired_root, *mapped_instruction, use->operand, use->kind, use->argument,
                use->value, use->memory_width_bytes, use->memory_read, use->memory_write);
            if (witnesses.size() < 8) witnesses.push_back({{"sourceProcedureRva", source_root},
                {"targetProcedureRva", *paired_root}, {"sourceInstructionRva", use->instruction},
                {"targetInstructionRva", *mapped_instruction}, {"kind", use->kind},
                {"operand", use->operand}, {"value", use->value},
                {"memoryWidthBytes", use->memory_width_bytes}, {"memoryRead", use->memory_read},
                {"memoryWrite", use->memory_write}});
        }
        for (const auto& [target_root, use] : new_uses->second) {
            const auto source_root = target_counterparts.find(target_root);
            if (source_root == target_counterparts.end()) {
                row["reason"] = "new-target-global-layout-consumer";
                return false;
            }
            const auto source = analysis.source_functions.find(source_root->second);
            const auto target = analysis.target_functions.find(target_root);
            if (source == analysis.source_functions.end() || target == analysis.target_functions.end() ||
                !exact_layout_pair(source_root->second, target_root)) {
                row["reason"] = "new-target-global-layout-consumer-unresolved";
                return false;
            }
            const auto paired_instruction = target_instruction(target->second, source->second, use->instruction);
            if (!paired_instruction) {
                row["reason"] = "new-target-global-layout-instruction-unmapped";
                return false;
            }
            const auto match = std::find_if(source->second.layout_uses.begin(), source->second.layout_uses.end(),
                [&](const auto& candidate) {
                    return candidate.instruction == *paired_instruction && candidate.operand == use->operand &&
                        candidate.value == use->value && candidate.argument == use->argument &&
                        candidate.kind == use->kind && candidate.memory_width_bytes == use->memory_width_bytes &&
                        candidate.memory_read == use->memory_read && candidate.memory_write == use->memory_write;
                });
            if (match == source->second.layout_uses.end()) {
                row["reason"] = "new-target-global-layout-operand";
                return false;
            }
            actual.emplace(target_root, use->instruction, use->operand, use->kind, use->argument,
                use->value, use->memory_width_bytes, use->memory_read, use->memory_write);
        }
        if (expected != actual) {
            row["reason"] = "original-and-target-global-layout-use-counts-differ";
            return false;
        }
        row = {{"path", path}, {"before", value}, {"after", value},
            {"method", "complete-original-and-target-global-code-use-set"},
            {"sourceUseCount", old_uses->second.size()}, {"targetUseCount", new_uses->second.size()},
            {"witnesses", std::move(witnesses)}};
        return true;
    }
public:
    bool exact_consumer_pair(std::uint32_t source_root, std::uint32_t target_root) const {
        return exact_layout_pair(source_root, target_root);
    }

    LayoutProver(const json& profile, const ProfileRebaseAnalysis& context) : baseline(profile), analysis(context) {
        for (const auto& [root, function] : analysis.source_functions)
            for (const auto& [begin, end] : function.ranges) source_ranges.emplace(begin, std::pair{end, root});
        for (const auto& [root, function] : analysis.source_functions)
            for (const auto& use : function.layout_uses) source_uses_by_value[use.value].emplace_back(root, &use);
        for (const auto& [root, function] : analysis.target_functions)
            for (const auto& [begin, end] : function.ranges) target_ranges.emplace(begin, std::pair{end, root});
        for (const auto& [root, function] : analysis.target_functions)
            for (const auto& use : function.layout_uses) target_uses_by_value[use.value].emplace_back(root, &use);
        for (const auto& [source, target] : analysis.exact_counterparts)
            target_counterparts.emplace(target, source);
        for (const auto& [root, function] : analysis.source_functions) {
            const auto target = analysis.target_functions.find(root);
            if (!target_counterparts.contains(root) && target != analysis.target_functions.end() &&
                identical_layout_procedure(function, target->second))
                target_counterparts.emplace(root, root);
        }
        for (const auto& [root, function] : analysis.source_functions) {
            for (const auto& call : function.call_arguments) {
                const auto* target = procedure(call.target);
                if (target) source_callers[target->root].insert(root);
            }
            for (const auto& ref : function.references) if (!ref.branch) source_references[ref.target].insert(root);
        }
        owners = references({"/ownerLayout/rootObjectRva", "/ownerLayout/rootVtableRva",
            "/ownerLayout/clientVtableRva", "/ownerLayout/controllerVtableRva"});
        const auto image_root = number("/ownerLayout/rootObjectRva"), client_offset = number("/ownerLayout/clientPointerOffset");
        if (image_root && client_offset && *image_root <= UINT32_MAX && *client_offset <= UINT32_MAX - *image_root) {
            const auto slot = source_references.find(static_cast<std::uint32_t>(*image_root + *client_offset));
            if (slot != source_references.end()) merge(owners, slot->second);
        }
        tasks = named("ecs::Tasks");
        task_callers = callers(tasks);
        queue_initializers = with_tuple(children(tasks), {"/entityCommandQueueLayout/taskRecordsOffset",
            "/entityCommandQueueLayout/taskCountOffset", "/entityCommandQueueLayout/queueRecordsOffset",
            "/entityCommandQueueLayout/queueCountOffset", "/entityCommandQueueLayout/taskStateOffset"}, 1);
        Roots all;
        for (const auto& [root, function] : analysis.source_functions) all.insert(root);
        queue_drains = with_tuple(all, {"/entityCommandQueueLayout/taskRecordsOffset",
            "/entityCommandQueueLayout/taskCountOffset", "/entityCommandQueueLayout/queueRecordsOffset",
            "/ecsWriteLayout/changeTrackerContextOffset"});
        tracker = at("/ecsWriteLayout/changeTrackerRva");
        entity_storage = with_tuple(children(tracker), {"/ecsReadLayout/entityLayout",
            "/ecsReadLayout/componentOffsets", "/ecsReadLayout/componentStrides"});
        // The serializer's base differs from the manager only by the concrete
        // arguments passed by their shared owner. Require both table and
        // occupancy relations, not an arbitrary sum of constants.
        const auto table = number("/ecsReadLayout/entityManagerTable"), bits = number("/ecsReadLayout/entityManagerOccupancy");
        if (table && bits) for (const auto owner : callers(queue_drains)) {
            const auto& function = analysis.source_functions.at(owner);
            for (const auto& manager_call : function.call_arguments) {
                const auto* manager = procedure(manager_call.target);
                if (!manager || !queue_drains.contains(manager->root) || !manager_call.offsets[0]) continue;
                for (const auto& call : function.call_arguments) {
                    const auto* serializer = procedure(call.target);
                    if (!serializer || !call.offsets[0]) continue;
                    const auto delta = *manager_call.offsets[0] - *call.offsets[0];
                    if (delta < 0 || !memory(*serializer, static_cast<std::uint64_t>(delta) + *table) ||
                        !memory(*serializer, static_cast<std::uint64_t>(delta) + *bits) ||
                        !tuple(*serializer, {"/ecsReadLayout/entityId", "/ecsReadLayout/entityGeneration",
                            "/ecsReadLayout/entityStorage", "/ecsReadLayout/entityRow", "/ecsReadLayout/entityDefinitionPointer"})) continue;
                    entity_records.insert(serializer->root);
                    entity_record_owners.insert(owner);
                }
            }
        }
        std::set<std::uint32_t> entity_lookup_targets;
        const auto capacity = number("/ecsReadLayout/entityManagerCount");
        const auto header_capacity = number("/resourceLayout/itemIdMap/capacityOffset");
        if (capacity && header_capacity) for (const auto root : entity_storage)
            for (const auto& call : analysis.source_functions.at(root).call_arguments) {
                const auto* lookup = procedure(call.target);
                if (!call.offsets[0] || *call.offsets[0] < 0 ||
                    static_cast<std::uint64_t>(*call.offsets[0]) + *header_capacity != *capacity) continue;
                if (lookup && !tuple(*lookup, {"/resourceLayout/itemIdMap/bitsPointerOffset",
                    "/resourceLayout/itemIdMap/capacityOffset", "/resourceLayout/itemIdMap/keysPointerOffset"}, 0)) continue;
                // A leaf lookup can lack .pdata. Its complete callsite/code
                // binding is still checked by CodeRelations on the witnessed
                // manager consumer, never by guessing a compiler extent here.
                entity_lookup_targets.insert(call.target);
            }
        Roots definition_candidates;
        for (const auto& [root, function] : analysis.source_functions)
            if (std::any_of(function.call_arguments.begin(), function.call_arguments.end(), [&](const auto& call) {
                return entity_lookup_targets.contains(call.target);
            })) definition_candidates.insert(root);
        entity_definitions = with_tuple(definition_candidates, {"/ecsReadLayout/entityManagerCount",
            "/ecsReadLayout/entityManagerTable", "/ecsReadLayout/entityId", "/ecsReadLayout/entityDefinitionPointer"});
        for (const auto root : children(entity_definitions)) {
            const auto& function = analysis.source_functions.at(root);
            const bool uuid_extent = std::any_of(function.layout_uses.begin(), function.layout_uses.end(), [](const auto& use) {
                return use.kind == "original-immediate" && use.value == 2 * sizeof(std::uint64_t);
            });
            if (uuid_extent && std::any_of(function.call_arguments.begin(), function.call_arguments.end(), [](const auto& call) {
                return call.offsets[1] && *call.offsets[1] == 0;
            })) definition_copies.insert(root);
        }
        const auto callback = number("/ecsWriteLayout/threadCallbackRva");
        const auto thread_name = baseline.contains("ecsWriteLayout") ?
            baseline.at("ecsWriteLayout").value("threadName", std::string{}) : std::string{};
        const auto named_threads = analysis.named_procedures.find(thread_name);
        if (callback && named_threads != analysis.named_procedures.end()) for (const auto root : named_threads->second)
            for (const auto& ref : analysis.source_functions.at(root).references)
                if (!ref.branch && ref.target == *callback) scene_starters.insert(root);
        if (scene_starters.size() != 1) scene_starters.clear();
        scene_constructors = with_tuple(children(scene_starters), {"/ecsReadLayout/sceneEntriesPointerOffset",
            "/ecsReadLayout/sceneEntriesCountOffset", "/ecsReadLayout/sceneEntriesCapacityOffset", "/ecsWriteLayout/sceneLockOffset"});
        for (const auto root : scene_starters) for (const auto& call : analysis.source_functions.at(root).call_arguments) {
            const auto pointer = number("/ecsReadLayout/sceneEntriesPointerOffset");
            const auto* target = procedure(call.target);
            if (pointer && target && call.offsets[0] && *call.offsets[0] == static_cast<std::int64_t>(*pointer)) scene_arrays.insert(target->root);
        }
        scene_entries = with_tuple(all, {"/ecsReadLayout/sceneEntriesPointerOffset",
            "/ecsReadLayout/sceneEntriesCountOffset", "/ecsWriteLayout/sceneLockOffset"}, 0);
        const auto context_offset = number("/ecsWriteLayout/sceneContextOffset");
        const auto entry_pointer = number("/ecsReadLayout/sceneEntriesPointerOffset");
        const auto entry_count = number("/ecsReadLayout/sceneEntriesCountOffset");
        const auto thread_offset = number("/ecsWriteLayout/sceneThreadOffset");
        const auto thread_context = number("/ecsWriteLayout/threadContextOffset");
        const auto callbacks = at("/ecsWriteLayout/threadCallbackRva");
        bool context_installed{};
        if (context_offset && thread_offset) for (const auto root : scene_starters)
            for (const auto& call : analysis.source_functions.at(root).call_arguments)
                context_installed |= call.offsets[0] && call.offsets[1] &&
                    *call.offsets[0] == static_cast<std::int64_t>(*thread_offset) &&
                    *call.offsets[1] == static_cast<std::int64_t>(*context_offset);
        if (context_installed && context_offset && entry_pointer && entry_count && thread_context &&
            *entry_pointer >= *context_offset && *entry_count >= *context_offset) {
            const auto initializer_callers = callers(task_callers);
            for (const auto callback_root : callbacks) {
                const auto& callback_body = analysis.source_functions.at(callback_root);
                if (!memory(callback_body, *thread_context, 0)) continue;
                for (const auto& call : callback_body.call_arguments) {
                    const auto* loop = procedure(call.target);
                    if (!loop || !initializer_callers.contains(loop->root) || !call.offsets[0] || *call.offsets[0] != 0 ||
                        !memory(*loop, *entry_pointer - *context_offset) ||
                        !memory(*loop, *entry_count - *context_offset) ||
                        !tuple(*loop, {"/ecsReadLayout/sceneEntryStateOffset"})) continue;
                    scene_context_readers.insert(loop->root);
                    merge(scene_context_owners, scene_starters);
                    scene_context_owners.insert(callback_root);
                    for (const auto& initializer : loop->call_arguments) {
                        const auto* target = procedure(initializer.target);
                        if (target && task_callers.contains(target->root)) scene_context_owners.insert(target->root);
                    }
                }
            }
        }
        threads = children(scene_starters);
        merge(threads, at("/ecsWriteLayout/threadCallbackRva"));
        registries = children(references({"/ecsWriteLayout/threadRegistryEntriesRva",
            "/ecsWriteLayout/threadRegistryLockRva", "/ecsWriteLayout/threadRegistryCountRva"}));
        dependencies = children(at("/systemDependencyLayout/recordBuilderRva"));
        merge(dependencies, at("/systemDependencyLayout/accessSetPackRva"));
        services = children(tasks, 2);
        merge(services, dependencies);
        if (baseline.contains("serviceLayout")) for (const auto& contract : baseline.at("serviceLayout").value("contracts", json::array())) {
            if (contract.contains("mutableSystemFunctionRva")) {
                const auto* function = procedure(contract.at("mutableSystemFunctionRva").get<std::uint32_t>());
                if (function) services.insert(function->root);
            }
        }
        const auto registry_offset = number("/ecsReadLayout/executionRegistryOwnerOffset");
        const auto systems_offset = number("/serviceLayout/systemsOffset");
        const auto system_count = number("/serviceLayout/systemCountOffset");
        const auto system_candidates = with_tuple(all, {"/serviceLayout/systemsOffset", "/serviceLayout/systemCountOffset"});
        if (registry_offset) for (const auto owner : callers(system_candidates))
            for (const auto& call : analysis.source_functions.at(owner).call_arguments) {
                const auto* target = procedure(call.target);
                if (target && system_candidates.contains(target->root) && call.offsets[0] &&
                    *call.offsets[0] == static_cast<std::int64_t>(*registry_offset)) {
                    service_system_records.insert(target->root);
                    service_system_owners.insert(owner);
                }
            }
        const auto binding_candidates = with_tuple(all, {"/serviceLayout/systemNormalBindingsOffset",
            "/serviceLayout/systemAlternateBindingsOffset", "/serviceLayout/systemNormalCountOffset",
            "/serviceLayout/systemAlternateCountOffset"}, 0);
        if (systems_offset && system_count) for (const auto owner : callers(binding_candidates)) {
            const auto& function = analysis.source_functions.at(owner);
            if (!memory(function, *system_count)) continue;
            for (const auto& call : function.call_arguments) {
                const auto* target = procedure(call.target);
                if (target && binding_candidates.contains(target->root) && call.offsets[0] && call.offsets[1] &&
                    *call.offsets[0] == static_cast<std::int64_t>(*systems_offset) && *call.offsets[1] == 0) {
                    service_binding_records.insert(target->root);
                    service_binding_owners.insert(owner);
                }
            }
        }
        resources = references({"/resourceLayout/resourceServiceIdCallbackRva",
            "/resourceLayout/resourceServiceHandleCallbackRva"}, true);
        resource_callbacks = at("/resourceLayout/resourceServiceIdCallbackRva");
        merge(resource_callbacks, at("/resourceLayout/resourceServiceHandleCallbackRva"));
        const auto metadata_offset = number("/resourceLayout/readerMetadataOffset");
        const auto metadata_map = number("/resourceLayout/metadataMapOffset");
        if (metadata_offset && metadata_map) for (const auto root : children(resource_callbacks)) {
            const auto& function = analysis.source_functions.at(root);
            const auto parents = callers({root});
            if (!std::includes(parents.begin(), parents.end(), resource_callbacks.begin(), resource_callbacks.end()) ||
                !memory(function, *metadata_offset, 0)) continue;
            if (std::any_of(function.layout_uses.begin(), function.layout_uses.end(), [&](const auto& use) {
                return use.kind == "original-immediate" && use.value >= 0 && static_cast<std::uint64_t>(use.value) == *metadata_map;
            })) resource_metadata_getters.insert(root);
        }
        resource_key_lookups = with_tuple(children(at("/resourceLayout/resourceServiceIdCallbackRva")),
            {"/resourceLayout/map/bucketRel32Offset", "/resourceLayout/map/bucketCountOffset",
             "/resourceLayout/map/keysRel32Offset", "/resourceLayout/map/valuesRel32Offset"}, 0);
        for (const auto root : callers(resource_key_lookups)) {
            const auto& function = analysis.source_functions.at(root);
            if (metadata_offset && metadata_map && memory(function, *metadata_offset, 0) && memory(function, *metadata_map))
                resource_header_validators.insert(root);
        }
        resource_workers = children(named("KfcResourceContent"));
        merge(resource_workers, at("/resourceWriteLayout/threadCallbackRva"));
        if (baseline.contains("resourceLayout") && baseline.at("resourceLayout").contains("itemIdMap")) {
            const auto name = baseline.at("resourceLayout").at("itemIdMap").value("registry", std::string{});
            item_registry = children(named(name), 2);
        }
        const auto system = analysis.named_systems.find("player_building_place_prop");
        if (system != analysis.named_systems.end() && system->second.size() == 1) spawn = children(system->second, 3);
        Roots binding_owners = owners;
        merge(binding_owners, tasks);
        for (const auto* roots : {&scene_starters, &tracker, &resource_callbacks, &resources, &resource_workers,
            &dependencies, &entity_definitions, &service_binding_owners, &service_system_owners}) merge(binding_owners, *roots);
        if (system != analysis.named_systems.end() && system->second.size() == 1) merge(binding_owners, system->second);
        bind_owner_children(std::move(binding_owners));
    }

    bool prove(const std::string& path, std::uint64_t value, json& row) const {
        if (runtime_policy(path, value, row)) return true;
        if (inactive_server_contract(baseline, path, value, row)) return true;
        if (path == "/ownerLayout/clientPointerOffset") {
            const auto proof = client_pointer_slot(value, row);
            if (proof) return *proof;
        }
        if (reflected_item_field(path, value, row)) return true;
        if (reflected_contract_number(path, value, row)) return true;
        const auto selected = scope(path);
        row = {{"path", path}, {"before", value}, {"after", value},
            {"method", "owner-bound-original-instruction-operands"}, {"owner", selected.role}, {"witnesses", json::array()}};
        if (path == "/resourceLayout/map/resourceKeySize") {
            if (resource_key_extent(value, row)) return true;
            row["reason"] = "original-resource-key-read-extent-not-established";
        }
        if (path == "/ecsReadLayout/entityDefinitionUuidOffset") {
            if (definition_uuid(value, row)) return true;
            row["reason"] = "original-definition-UUID-copy-not-established";
        }
        json global_use_proof;
        if (exact_global_use_set(path, value, global_use_proof)) {
            row = std::move(global_use_proof);
            return true;
        }
        if (global_use_proof.contains("reason")) row["globalUseFallback"] = global_use_proof["reason"];
        for (const auto root : selected.required) {
            const auto paired = paired_procedure(root);
            if (!paired) { row["reason"] = "original-layout-owner-chain-unresolved"; return false; }
            row["ownerChain"].push_back(*paired);
        }
        if (path == "/ecsReadLayout/sceneEntryContextOffset" && !scene_context_readers.empty()) {
            const auto context = *number("/ecsWriteLayout/sceneContextOffset");
            const auto pointer = *number("/ecsReadLayout/sceneEntriesPointerOffset");
            const auto count = *number("/ecsReadLayout/sceneEntriesCountOffset");
            row["ownerComposition"] = {{"inlineContextOffset", context}, {"relativeEntryArrayOffset", pointer - context},
                {"sceneEntryArrayOffset", pointer}, {"relativeCountOffset", count - context}, {"sceneCountOffset", count}};
            for (const auto root : scene_context_readers)
                for (const auto& use : analysis.source_functions.at(root).layout_uses)
                    if (use.kind == "original-memory-displacement" && use.value >= 0 &&
                        (static_cast<std::uint64_t>(use.value) == pointer - context ||
                         static_cast<std::uint64_t>(use.value) == count - context)) {
                        const auto proof = witness(analysis.source_functions.at(root), use);
                        if (proof) row["ownerComposition"]["witnesses"].push_back(*proof);
                    }
        }
        bool observed{}, changed{};
        for (const auto root : selected.roots) {
            const auto& function = analysis.source_functions.at(root);
            unsigned retained{};
            for (const auto& use : function.layout_uses) {
                if (use.value < 0 || static_cast<std::uint64_t>(use.value) != value) continue;
                const bool eligible = selected.argument >= 0 ?
                    use.kind == "original-argument-relative-field" && use.argument == selected.argument :
                    use.kind == "original-memory-displacement" ||
                    (selected.immediate && use.kind == "original-immediate") ||
                    (selected.element_stride && (use.kind == "original-call-argument-element-stride" ||
                        use.kind == "original-index-byte-stride" || use.kind == "original-address-index-scale"));
                if (!eligible) continue;
                observed = true;
                const auto proof = witness(function, use);
                if (!proof) {
                    changed = true;
                    row["failedConsumers"].push_back({{"sourceProcedureRva", root}, {"sourceInstructionRva", use.instruction},
                        {"reason", "owner-consumer-lacks-identical-original-counterpart"}});
                } else if (retained++ < 4) row["witnesses"].push_back(*proof);
            }
        }
        if (observed && !changed) return true;
        row["reason"] = changed ? "original-layout-consumer-changed-or-unresolved" : "original-layout-owner-consumer-not-established";
        return false;
    }
};
}

bool ProveProfileLayouts(const json& baseline, const ProfileRebaseAnalysis& analysis,
    const std::vector<std::pair<std::string, std::uint64_t>>& fields, json& evidence) {
    evidence = {{"fields", json::array()}, {"unresolved", json::array()}};
    const LayoutProver prover(baseline, analysis);
    for (const auto& [path, value] : fields) {
        json row;
        if (value <= static_cast<std::uint64_t>(INT64_MAX) && prover.prove(path, value, row))
            evidence["fields"].push_back(std::move(row));
        else {
            if (row.is_null()) row = {{"path", path}, {"sourceValue", value}, {"reason", "layout-value-out-of-range"}};
            evidence["unresolved"].push_back(std::move(row));
        }
    }
    return evidence["unresolved"].empty();
}

bool ExactLayoutConsumerPair(const ProfileRebaseAnalysis& analysis, std::uint32_t source_root,
                             std::uint32_t target_root) {
    const auto source = analysis.source_functions.find(source_root);
    const auto target = analysis.target_functions.find(target_root);
    if (source == analysis.source_functions.end() || target == analysis.target_functions.end()) return false;
    const auto mapped = analysis.exact_counterparts.find(source_root);
    if (mapped != analysis.exact_counterparts.end() && mapped->second == target_root)
        return source->second.complete && target->second.complete &&
            source->second.signature == target->second.signature &&
            source->second.ranges.size() == target->second.ranges.size();
    return source_root == target_root && identical_layout_procedure(source->second, target->second);
}
}
