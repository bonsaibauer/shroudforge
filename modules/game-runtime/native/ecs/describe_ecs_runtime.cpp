#include "ecs/registry/component_discovery_state.h"
#include "ecs/component_operation.h"
#include "ecs/ecs_session_state.h"

using namespace EcsImplementation;

namespace EcsRuntime {
std::string Status() {
    std::scoped_lock discovery_lock(discovery_mutex);
    std::scoped_lock lock(state_mutex);
    std::ostringstream text;
    text << "types=" << types.size() << '/' << configured_types.size()
         << "{reflected_ecs_types=" << configured_types.size()
         << ",registered_storage_types=" << types.size()
         << ",registrations=" << (component_registry_ready ? component_registry.entries.size() : 0) << '}'
         << " component_index_discovery=" << (discovery_complete ? "complete" : "scanning")
         << '(' << discovery_cursor << '/' << discovery_entities.size()
         << ",layouts=" << discovered_layout_components.size()
         << ",templates=" << template_layout_samples.size()
         << ",restarts=" << discovery_restarts << ')'
         << " execution_profile=" << KfcRuntimeCompatibility::EnshroudedClient::status
         << " game_thread=" << GameThreadDispatcher::Status()
         << " entity_hooks{" << WorldRuntime::EntityHookStatus() << '}';
    for (const auto& operation : KfcRuntimeCompatibility::EnshroudedClient::runtime_operations)
        text << " world{" << operation.name << '=' << (operation.available ? operation.status : "unavailable") << '}';
    text << " voxel_context=" << (WorldRuntime::ActiveContextAvailable() ? "ready" : "waiting");
    text << " context_probe{" << WorldRuntime::ContextStatus() << '}';
    if (types.empty()) return text.str() + " registry=unresolved layout=unavailable";
    text << " registry=ready";
    if (!layout_ready || (component_registry_ready && !component_registry_world_bound))
        return text.str() + " layout=unavailable entity_access=unavailable(dispatcher-or-world-layout)";
    if (read_only_binding) {
        EngineContext::EcsView current;
        if (!EngineContext::ResolveEcs(current) || current != read_only_view)
            return text.str() + " layout=unavailable entity_access=unavailable(engine-owner-changed)";
        text << " entity_access=" << (EngineContext::EcsWriteConfigured() &&
            read_only_view.role == EngineContext::EcsRole::LocalAuthority ? "guarded-read-write" : "read-only")
            << " entity_realm=" << EngineContext::RoleName(read_only_view.role);
    } else text << " entity_access=" << (GameThreadDispatcher::Ready() ? "read-write" : "unavailable")
        << " entity_realm=engine-observed";
    text << " layout=ready"
         << " entity{id=0x" << std::hex << live_layout.entity_id
         << ",generation=0x" << live_layout.entity_generation
         << ",layout=0x" << live_layout.entity_layout
         << ",storage=0x" << live_layout.entity_storage
         << ",row=0x" << live_layout.entity_row << std::dec << '}'
         << " components{bits=0x" << std::hex << live_layout.component_bits
         << ",offsets=0x" << live_layout.component_offsets
         << ",strides=0x" << live_layout.component_strides << std::dec << '}'
         << " epoch=" << layout_epoch;
    return text.str();
}

std::string Diagnostics() {
    std::scoped_lock discovery_lock(discovery_mutex);
    std::scoped_lock lock(state_mutex);
    EngineContext::EcsView current_read_view;
    const bool current_read_binding = !read_only_binding ||
        (EngineContext::ResolveEcs(current_read_view) && current_read_view == read_only_view);
    std::size_t observed_indices{};
    for (const auto& entry : observed_indices_by_size) observed_indices += entry.second.size();
    nlohmann::json resolved_mappings = nlohmann::json::array();
    nlohmann::json unresolved_types = nlohmann::json::array();
    nlohmann::json stride_candidates = nlohmann::json::array();
    nlohmann::json template_layouts = nlohmann::json::array();
    nlohmann::json world_operations = nlohmann::json::object();
    for (const auto& operation : KfcRuntimeCompatibility::EnshroudedClient::runtime_operations)
        world_operations[operation.name] = {{"available", operation.available}, {"status", operation.status},
            {"abi", operation.abi}, {"thread", operation.thread}, {"context", operation.context}};
    std::unordered_map<std::uint32_t, std::vector<std::string>> names_by_size;
    std::size_t dynamic_candidates{};
    std::size_t template_types{}, template_only{};
    if (component_registry_ready) for (const auto& entry : component_registry.entries) {
        template_types += !entry.configuration.name.empty();
        template_only += entry.storage.name.empty();
    }
    for (const auto& [name, size] : configured_types) {
        names_by_size[size].push_back(name);
        if (name.starts_with("keen::ecs::Dynamic")) ++dynamic_candidates;
    }
    for (const auto& [name, component] : types) {
        resolved_mappings.push_back({{"name",name},{"index",component.index},{"size",component.size},
            {"source",component_registry_ready ? "live-engine-registration" : "exact-build-profile"}});
    }
    for (const auto& [size, names] : names_by_size) {
        const auto candidate_set = observed_indices_by_size.find(size);
        nlohmann::json indices = nlohmann::json::array();
        if (candidate_set != observed_indices_by_size.end())
            for (const auto index : candidate_set->second) indices.push_back(index);
        stride_candidates.push_back({{"size",size},{"indices",std::move(indices)},
            {"reflectedTypeCount",names.size()}});
        for (const auto& name : names) {
            if (types.contains(name)) continue;
            const auto candidates = candidate_set == observed_indices_by_size.end() ? 0 : candidate_set->second.size();
            const auto reason = component_registry_ready ? "reflected-type-without-entity-storage" :
                names.size() != 1 ? "shared-reflected-size" :
                candidates == 0 ? "no-live-archetype-observed" :
                candidates != 1 ? "multiple-live-indices-for-size" : "index-conflict";
            unresolved_types.push_back({{"name",name},{"size",size},{"reason",reason}});
        }
    }
    for (const auto& [key, sample] : template_layout_samples) {
        (void)key;
        nlohmann::json component_slots = nlohmann::json::array();
        for (const auto& component : sample.components)
            component_slots.push_back({{"index", component.index}, {"stride", component.stride},
                {"offset", component.offset}});
        template_layouts.push_back({{"templateUuidQwords", {sample.uuid[0], sample.uuid[1]}},
            {"templateName", sample.name},
            {"entitiesSeen", sample.entity_count}, {"componentSlots", std::move(component_slots)}});
    }
    return nlohmann::json({
        {"schemaVersion",1}, {"providerAbi",KFC_RUNTIME_ABI_VERSION},
        {"profile",KfcRuntimeCompatibility::EnshroudedClient::status},
        {"kfcFormat",KfcRuntimeCompatibility::EnshroudedClient::kfc_format},
        {"candidateTypeBreakdown",nlohmann::json{
            {"total",configured_types.size()},
            {"otherEcsTypes",configured_types.size() - dynamic_candidates},
            {"dynamicNameTypes",dynamic_candidates},
            {"engineRegistrations",component_registry_ready ? component_registry.entries.size() : 0},
            {"registeredStorageTypes",component_registry_ready ? types.size() : 0},
            {"registeredTemplateTypes",template_types}, {"templateOnlyRegistrations",template_only},
            {"resolved",types.size()},
            {"unresolved",component_registry_ready ? 0 : configured_types.size() - types.size()},
            {"reflectedWithoutEntityStorage",configured_types.size() - types.size()}
        }},
        {"worldOperations",std::move(world_operations)},
        {"voxelContextActive",WorldRuntime::ActiveContextAvailable()},
        {"worldContext",nlohmann::json::parse(WorldRuntime::ContextDiagnostics())},
        {"entityContextReady",WorldRuntime::EntityContextReady()},
        {"entityHookStatus",WorldRuntime::EntityHookStatus()},
        {"configuredTypes",configured_types.size()}, {"resolvedTypes",types.size()},
        {"layoutReady",layout_ready}, {"layoutEpoch",layout_epoch},
        {"registryAvailable",component_registry_ready}, {"registryManagerBound",component_registry_world_bound},
        {"entityReadSource",read_only_binding ? "original-engine-owner-chain" : "game-thread-dispatcher"},
        {"entityReadRole",read_only_binding ? EngineContext::RoleName(read_only_view.role) : "engine-observed"},
        {"originalWriteGateConfigured",EngineContext::EcsWriteConfigured()},
        {"originalWriteGateLastReason",last_write_reason.load(std::memory_order_relaxed)},
        {"writeFault",write_fault_active_locked()},
        {"writeFaultReason",write_fault_active_locked() ? write_fault_reason : "none"},
        {"registryDiscovery",nlohmann::json{
            {"source","resident-private-memory"},
            {"scanning",registry_discovery.Status().scanning},
            {"available",registry_discovery.Status().available},
            {"reason",registry_discovery.Status().reason},
            {"scannedBytes",registry_discovery.Status().scanned_bytes},
            {"skippedNonresidentBytes",registry_discovery.Status().skipped_nonresident_bytes},
            {"validationBytes",registry_discovery.Status().validation_bytes},
            {"budgetLimitedSteps",registry_discovery.Status().budget_limited_steps},
            {"completedPasses",registry_discovery.Status().completed_passes},
            {"distinctRegistries",registry_discovery.Status().distinct_registries},
            {"distinctCatalogs",registry_discovery.Status().distinct_catalogs},
            {"entityAccessAuthorized",false}
        }},
        {"entityAccessReady",layout_ready && !types.empty() &&
            (read_only_binding ? current_read_binding && component_registry_ready && component_registry_world_bound :
                GameThreadDispatcher::Ready() && (!component_registry_ready || component_registry_world_bound))},
        {"componentDiscovery",nlohmann::json{
            {"complete",discovery_complete},
            {"scannedEntities",discovery_scanned_entities},
            {"totalEntities",discovery_entities.size()},
            {"archetypes",discovery_layout_count},
            {"distinctLayoutsSeen",discovered_layout_components.size()},
            {"snapshotRestarts",discovery_restarts},
            {"observedStrideBuckets",observed_indices_by_size.size()},
            {"observedIndexCandidates",observed_indices},
            {"templateLayoutSamples",std::move(template_layouts)},
            {"templateLayoutSamplesDropped",template_layout_samples_dropped},
            {"mappingMethod",component_registry_ready ? "live engine registration, reflection and parallel arrays" :
                KfcRuntimeCompatibility::EnshroudedClient::runtime_components.empty() ? "waiting for live engine registration" : "exact build profile"},
            {"registrationReady",component_registry_ready}, {"registrationReason",component_registry_error},
            {"registrationCount",component_registry.entries.size()},
            {"resolved",std::move(resolved_mappings)},
            {"unresolved",component_registry_ready ? nlohmann::json::array() : unresolved_types},
            {"reflectedWithoutEntityStorage",component_registry_ready ? std::move(unresolved_types) : nlohmann::json::array()},
            {"strideCandidates",std::move(stride_candidates)}
        }},
        {"activeQueryScan",nlohmann::json{
            {"cursor",active_query_cursor.load(std::memory_order_relaxed)},
            {"total",active_query_total.load(std::memory_order_relaxed)},
            {"matches",active_query_matches.load(std::memory_order_relaxed)}
        }},
        {"operations", nlohmann::json{
            {"queries", operation_counters.queries.load()},
            {"querySuccesses", operation_counters.query_successes.load()},
            {"queryIncomplete", operation_counters.query_incomplete.load()},
            {"queryFailures", operation_counters.query_failures.load()},
            {"resolves", operation_counters.resolves.load()},
            {"resolveSuccesses", operation_counters.resolve_successes.load()},
            {"resolveFailures", operation_counters.resolve_failures.load()},
            {"reads", operation_counters.reads.load()},
            {"readSuccesses", operation_counters.read_successes.load()},
            {"readFailures", operation_counters.read_failures.load()},
            {"readBatches", operation_counters.read_batches.load()},
            {"readBatchSuccesses", operation_counters.read_batch_successes.load()},
            {"readBatchFailures", operation_counters.read_batch_failures.load()},
            {"readBatchComponents", operation_counters.read_batch_components.load()},
            {"readBatchAbsent", operation_counters.read_batch_absent.load()},
            {"writes", operation_counters.writes.load()},
            {"writeSuccesses", operation_counters.write_successes.load()},
            {"writeFailures", operation_counters.write_failures.load()},
            {"runtimePatches", nlohmann::json::parse(PatchRuntime::Diagnostics())}
        }},
        {"dispatcher",nlohmann::json::parse(GameThreadDispatcher::Diagnostics())}
    }).dump();
}
}
