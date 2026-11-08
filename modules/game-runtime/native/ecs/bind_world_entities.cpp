#include "ecs/component_operation.h"
#include "ecs/building_prop_query_state.h"
#include "ecs/registry/component_discovery_state.h"
#include "engine-context/observe_entity_manager.h"
#include "engine-hooks/install_engine_hooks.h"
#include "ecs/ecs_session_state.h"

using namespace EcsImplementation;

static_assert(sizeof(KfcRuntimePropRecord) == 88);

static_assert(sizeof(KfcRuntimePropRecipe) == 32);

static_assert(offsetof(KfcRuntimePropRecord, position) == 8);

static_assert(offsetof(KfcRuntimePropRecord, orientation) == 32);

static_assert(offsetof(KfcRuntimePropRecord, scale) == 48);

static_assert(offsetof(KfcRuntimePropRecord, template_uuid) == 64);

static_assert(offsetof(KfcRuntimePropRecord, entity_id) == 80);

namespace EcsImplementation {
OperationCounters operation_counters;

std::atomic<std::uint64_t> active_query_cursor{}, active_query_total{}, active_query_matches{};

std::mutex state_mutex;

// Entity operations and lifecycle invalidation share this outer lock. Discovery
// scans run outside it. Lock order is operation, discovery, then state.
std::recursive_mutex operation_mutex;

std::unordered_map<std::string, ComponentType> types;

std::unordered_map<std::string, std::uint32_t> configured_types;

ResolvedLayout live_layout{};

bool layout_ready{};

bool read_only_binding{};

EngineContext::EcsView read_only_view;

std::uintptr_t live_table{};

std::unordered_map<std::uint32_t, HandleRecord> handles;

std::unordered_map<std::uint64_t, std::uint32_t> reverse_handles;

std::uint32_t next_handle{1};

std::uint64_t layout_epoch{1};

// Protected by state_mutex. A confirmed new native binding epoch is the only
// reset. Busy guards and ordinary expected-value conflicts never set this.
std::uint64_t write_fault_epoch{};

const char* write_fault_reason{};

bool write_fault_active_locked(){ return write_fault_epoch == layout_epoch && write_fault_reason; }

void mark_write_uncertain_locked(std::uint64_t epoch, const char* reason){
    write_fault_epoch = epoch;
    write_fault_reason = reason;
    last_write_reason.store(reason, std::memory_order_relaxed);
}

std::mutex write_mutex;

std::atomic<bool> stop_requested{};

std::uintptr_t image_base{};

std::size_t image_size{};

std::uint64_t current_epoch(){
    std::scoped_lock lock(state_mutex);
    return layout_epoch;
}

bool invoke_read_operation(GameThreadDispatcher::Operation operation, const std::shared_ptr<void>& context,
                           std::uint32_t timeout_ms){
    bool direct{};
    { std::scoped_lock lock(state_mutex); direct = read_only_binding; }
    if (!direct) return GameThreadDispatcher::Invoke(operation, context, timeout_ms);
    std::scoped_lock operation_lock(operation_mutex);
    EngineContext::EcsView expected, before, after;
    std::uint64_t epoch{};
    {
        std::scoped_lock lock(state_mutex);
        if (!read_only_binding || !layout_ready || !component_registry_ready || !component_registry_world_bound) return false;
        expected = read_only_view;
        epoch = layout_epoch;
    }
    if (!EngineContext::ResolveEcs(before) || before != expected) return false;
    operation(context.get());
    return EngineContext::ResolveEcs(after) && after == before && current_epoch() == epoch;
}

}

namespace EcsRuntime {
struct SessionGuard::State {
    std::unique_lock<std::mutex> lock{state_mutex};
    EngineContext::EcsView view;
    std::uint64_t epoch{};
};

SessionGuard::SessionGuard() = default;

SessionGuard::~SessionGuard() = default;

bool SessionGuard::TryAcquire(std::uint64_t expected_session) {
    if (state_ || !expected_session) return false;
    auto state = std::make_unique<State>();
    if (stop_requested.load(std::memory_order_acquire) || !read_only_binding || !layout_ready ||
        !component_registry_ready || !component_registry_world_bound || expected_session != layout_epoch + 1 ||
        read_only_view.role != EngineContext::EcsRole::LocalAuthority ||
        component_registry.owner != read_only_view.registry_owner ||
        component_registry_manager != read_only_view.manager) return false;
    EngineContext::EcsView current;
    std::uint64_t count{};
    std::uintptr_t table{};
    if (!EngineContext::ResolveEcs(current) || current != read_only_view ||
        live_layout.count_address != current.manager + current.layout.manager_count ||
        live_layout.table_address != current.manager + current.layout.manager_table ||
        !read(live_layout.count_address, count) || !count || count > (1u << 20) ||
        !read(live_layout.table_address, table) || !table || table != live_table ||
        table != current.entity_table) return false;
    state->view = current;
    state->epoch = layout_epoch;
    state_ = std::move(state);
    return true;
}

const EngineContext::EcsView& SessionGuard::View() const { return state_->view; }

bool SessionGuard::WritesAllowed() const { return state_ && !write_fault_active_locked(); }

bool SessionGuard::ResolveEntityIdentity(std::uint32_t handle,
    std::uint32_t& id, std::uint32_t& generation) const {
    if (!state_ || !handle || state_->epoch != layout_epoch) return false;
    const auto found = handles.find(handle);
    if (found == handles.end() || found->second.epoch != state_->epoch) return false;
    const auto& expected = found->second;
    EntityView entity;
    if (!entity_view(expected.pointer, live_layout, entity) || !entity.id || entity.id != expected.id ||
        entity.generation != expected.generation) return false;
    id = entity.id;
    generation = entity.generation;
    return true;
}

void SessionGuard::MarkWriteUncertain(const char* reason) {
    if (!state_ || !reason) return;
    mark_write_uncertain_locked(state_->epoch, reason);
}

bool MarkSessionWriteUncertain(std::uint64_t expected_session, const char* reason) {
    if (!expected_session || !reason) return false;
    std::scoped_lock lock(state_mutex);
    if (expected_session != layout_epoch + 1) return false;
    mark_write_uncertain_locked(layout_epoch, reason);
    return true;
}

bool Initialize() {
    const bool execution_profile = KfcRuntimeCompatibility::EnshroudedClient::Load();
    image_base = reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
    if (!image_base) return false;
    const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(image_base);
    if (dos->e_magic != IMAGE_DOS_SIGNATURE) return false;
    const auto nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(image_base + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE) return false;
    image_size = nt->OptionalHeader.SizeOfImage;
    stop_requested.store(false, std::memory_order_release);
    if (execution_profile) EngineHooks::Install();
    // Metadata discovery remains available when a build lacks proven native
    // execution adapters. Original-owner entity reads do not authorize writes.
    return true;
}

void Tick() {
    if (stop_requested.load(std::memory_order_acquire)) return;
    auto manager = GameThreadDispatcher::EntityManager();
    EngineContext::EcsView owners;
    const bool direct_read = (!manager || !GameThreadDispatcher::Ready()) && EngineContext::ResolveEcs(owners);
    if (direct_read) manager = owners.manager;
    read_component_registry(manager);
    if (!manager) {
        std::scoped_lock operation_lock(operation_mutex);
        std::scoped_lock lock(state_mutex);
        if (layout_ready) {
            layout_ready = false;
            ++layout_epoch;
            handles.clear();
            reverse_handles.clear();
        }
        return;
    }
    const auto count_address = manager + (direct_read ? owners.layout.manager_count : KfcRuntimeCompatibility::EnshroudedClient::entity_manager_count);
    const auto table_address = manager + (direct_read ? owners.layout.manager_table : KfcRuntimeCompatibility::EnshroudedClient::entity_manager_table);
    std::uint64_t count{};
    std::uintptr_t table{};
    // The table's first entry read under SEH tells it is readable, without
    // VirtualQuery: queried at every tick, it took 3 to 14 ms in game and
    // slowed the mods' loop as the session went on (26/10).
    std::uintptr_t first_entry{};
    if (!read(count_address, count) || !count || count > (1u << 20) ||
        !read(table_address, table) || !table || !read(table, first_entry)) {
        std::scoped_lock operation_lock(operation_mutex);
        std::scoped_lock lock(state_mutex);
        if (layout_ready) {
            layout_ready = false;
            ++layout_epoch;
            handles.clear();
            reverse_handles.clear();
        }
        return;
    }
    {
        std::scoped_lock operation_lock(operation_mutex);
        std::scoped_lock lock(state_mutex);
        if (!layout_ready || live_table != table || live_layout.count_address != count_address ||
            live_layout.table_address != table_address || read_only_binding != direct_read ||
            (direct_read && read_only_view != owners)) {
            live_layout = {};
            live_table = table;
            read_only_binding = direct_read;
            read_only_view = direct_read ? owners : EngineContext::EcsView{};
            live_layout.count_address = count_address;
            live_layout.table_address = table_address;
            live_layout.occupancy_address = direct_read && owners.layout.manager_occupancy ? manager + owners.layout.manager_occupancy : 0;
            live_layout.keys_address = direct_read && owners.layout.manager_keys ? manager + owners.layout.manager_keys : 0;
            live_layout.entity_id = direct_read ? owners.layout.entity_id : KfcRuntimeCompatibility::EnshroudedClient::entity_id;
            live_layout.entity_generation = direct_read ? owners.layout.entity_generation : KfcRuntimeCompatibility::EnshroudedClient::entity_generation;
            live_layout.entity_layout = direct_read ? owners.layout.entity_layout : KfcRuntimeCompatibility::EnshroudedClient::entity_layout;
            live_layout.entity_storage = direct_read ? owners.layout.entity_storage : KfcRuntimeCompatibility::EnshroudedClient::entity_storage;
            live_layout.entity_definition = direct_read ? owners.layout.entity_definition : KfcRuntimeCompatibility::EnshroudedClient::entity_definition;
            live_layout.entity_definition_uuid = direct_read ? owners.layout.entity_definition_uuid : 0;
            live_layout.entity_definition_uuid_validated = direct_read && owners.layout.entity_definition_uuid_validated;
            live_layout.entity_row = direct_read ? owners.layout.entity_row : KfcRuntimeCompatibility::EnshroudedClient::entity_row;
            live_layout.component_bits = direct_read ? owners.layout.component_bits : KfcRuntimeCompatibility::EnshroudedClient::component_bits;
            live_layout.component_offsets = direct_read ? owners.layout.component_offsets : KfcRuntimeCompatibility::EnshroudedClient::component_offsets;
            live_layout.component_strides = direct_read ? owners.layout.component_strides : KfcRuntimeCompatibility::EnshroudedClient::component_strides;
            layout_ready = true;
            ++layout_epoch;
            handles.clear();
            reverse_handles.clear();
        }
    }
    if (!direct_read) component_discovery_tick(manager);
}

void Shutdown() {
    stop_requested.store(true, std::memory_order_release);
    EngineHooks::Uninstall();
    std::shared_ptr<PropQueryOperation> retired_prop_query;
    PropQueryResultCache retired_prop_results;
    // Exact queries acquire their call lock before operation_mutex too. Keep
    // both until the binding is invalidated, then retire snapshots outside them.
    std::scoped_lock query_lock(prop_query_call_mutex);
    std::scoped_lock operation_lock(operation_mutex);
    pending_prop_query.swap(retired_prop_query);
    std::swap(prop_query_result_cache, retired_prop_results);
    reset_component_discovery();
    std::scoped_lock lock(state_mutex);
    types.clear();
    configured_types.clear();
    component_registry = {};
    component_registry_ready = false;
    component_registry_world_bound = false;
    component_registry_seen = false;
    component_registry_json.clear();
    component_registry_error = "runtime stopped";
    live_layout = {};
    layout_ready = false;
    read_only_binding = false;
    read_only_view = {};
    handles.clear();
    reverse_handles.clear();
    query_scans.clear();
    query_cache.clear();
    resolve_scans.clear();
    resolve_cache.clear();
}
}

extern "C" std::uint64_t __cdecl KfcRuntimeWorldSessionId() {
    std::scoped_lock lock(state_mutex);
    if (read_only_binding) {
        EngineContext::EcsView current;
        if (!EngineContext::ResolveEcs(current) || current != read_only_view) return 0;
    }
    const auto manager = read_only_binding ? component_registry_manager : GameThreadDispatcher::EntityManager();
    std::uint64_t count{};
    std::uintptr_t table{};
    if (!manager || !layout_ready ||
        live_layout.count_address != manager + (read_only_binding ? read_only_view.layout.manager_count : KfcRuntimeCompatibility::EnshroudedClient::entity_manager_count) ||
        !read(live_layout.count_address, count) || !count || count > (1u << 20) ||
        !read(live_layout.table_address, table) || !table || table != live_table) return 0;
    // Conservative identity: rebuilding ECS layouts also invalidates old journals.
    return layout_epoch + 1;
}

extern "C" bool __cdecl KfcRuntimeEcsReady() {
    std::scoped_lock lock(state_mutex);
    if (!layout_ready || types.empty()) return false;
    if (!read_only_binding) return GameThreadDispatcher::Ready() &&
        (!component_registry_ready || component_registry_world_bound);
    EngineContext::EcsView current;
    return component_registry_ready && component_registry_world_bound &&
        EngineContext::ResolveEcs(current) && current == read_only_view;
}
