#include "ecs_runtime.h"
#include "dispatcher.h"
#include <nlohmann/json.hpp>
#include "profile.h"
#include "world_runtime.h"
#include "patch_runtime.h"

#include <windows.h>
#include <algorithm>
#include <array>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstddef>
#include <cstring>
#include <mutex>
#include <memory>
#include <sstream>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

static_assert(sizeof(KfcRuntimePropRecord) == 80);
static_assert(sizeof(KfcRuntimePropRecipe) == 28);
static_assert(offsetof(KfcRuntimePropRecord, position) == 8);
static_assert(offsetof(KfcRuntimePropRecord, orientation) == 32);
static_assert(offsetof(KfcRuntimePropRecord, scale) == 48);
static_assert(offsetof(KfcRuntimePropRecord, template_uuid) == 64);

namespace {
constexpr std::size_t max_components = 1024;

struct OperationCounters {
    std::atomic<std::uint64_t> queries{}, query_successes{}, query_incomplete{}, query_failures{};
    std::atomic<std::uint64_t> resolves{}, resolve_successes{}, resolve_failures{};
    std::atomic<std::uint64_t> reads{}, read_successes{}, read_failures{};
    std::atomic<std::uint64_t> writes{}, write_successes{}, write_failures{};
};
OperationCounters operation_counters;
std::atomic<std::uint64_t> active_query_cursor{}, active_query_total{}, active_query_matches{};

struct ComponentType { std::uint16_t index; std::uint32_t size; };
struct ResolvedLayout {
    std::uintptr_t count_address{};
    std::uintptr_t table_address{};
    std::size_t entity_layout{};
    std::size_t entity_storage{};
    std::size_t entity_row{};
    std::size_t entity_id{};
    std::size_t entity_generation{};
    std::size_t component_bits{};
    std::size_t component_offsets{};
    std::size_t component_strides{};
};
struct EntityView {
    std::uintptr_t pointer{};
    std::uintptr_t layout{};
    std::uintptr_t storage{};
    std::uintptr_t definition{};
    std::uint32_t row{};
    std::uint32_t id{};
    std::uint32_t generation{};
};
struct ComponentSlot { std::uint16_t index{}, stride{}, offset{}; };
struct TemplateLayoutSample {
    std::uint64_t uuid[2]{};
    std::uintptr_t layout{};
    std::string name;
    std::uint64_t entity_count{};
    std::vector<ComponentSlot> components;
};
struct HandleRecord {
    std::uint32_t id{};
    std::uint32_t generation{};
    std::uint64_t epoch{};
    std::uintptr_t pointer{};
};

std::mutex state_mutex;
std::unordered_map<std::string, ComponentType> types;
std::unordered_map<std::string, std::uint32_t> configured_types;
std::unordered_map<std::uint32_t, std::unordered_set<std::uint16_t>> observed_indices_by_size;
std::unordered_set<std::uintptr_t> discovered_layouts;
std::unordered_map<std::uintptr_t, std::vector<ComponentSlot>> discovered_layout_components;
std::unordered_map<std::string, TemplateLayoutSample> template_layout_samples;
std::uint64_t template_layout_samples_dropped{};
std::vector<std::uintptr_t> discovery_entities;
std::size_t discovery_cursor{};
std::uintptr_t discovery_table{};
std::uint64_t discovery_last_restart{};
std::uint64_t discovery_scanned_entities{};
std::uint64_t discovery_layout_count{};
std::uint64_t discovery_restarts{};
bool discovery_complete{};
std::mutex discovery_mutex;
ResolvedLayout live_layout{};
bool layout_ready{};
std::uintptr_t live_table{};
std::unordered_map<std::uint32_t, HandleRecord> handles;
std::unordered_map<std::uint64_t, std::uint32_t> reverse_handles;
struct QueryScanState {
    std::uint64_t epoch{};
    std::uint64_t touched_ms{};
    std::vector<std::uintptr_t> pointers;
    std::size_t cursor{};
    std::vector<std::uint32_t> matches;
};
struct QueryCacheEntry {
    std::uint64_t epoch{};
    std::uint64_t completed_ms{};
    std::vector<std::uint32_t> matches;
};
std::unordered_map<std::string, QueryScanState> query_scans;
std::unordered_map<std::string, QueryCacheEntry> query_cache;
struct ResolveScanState {
    std::uint64_t epoch{};
    std::uint64_t touched_ms{};
    std::vector<std::uintptr_t> pointers;
    std::size_t cursor{};
};
struct ResolveCacheEntry {
    std::uint64_t epoch{};
    std::uint64_t completed_ms{};
    std::uint32_t handle{};
};
std::unordered_map<std::uint32_t, ResolveScanState> resolve_scans;
std::unordered_map<std::uint32_t, ResolveCacheEntry> resolve_cache;
std::uint32_t next_handle{1};
std::uint64_t layout_epoch{1};
std::mutex write_mutex;
std::mutex prop_recipe_mutex;
std::unordered_map<std::uint32_t, std::array<float, 6>> prop_recipes;
bool prop_recipe_catalog_ready{};
std::atomic<bool> stop_requested{};
std::uintptr_t image_base{};

bool readable(std::uintptr_t address, std::size_t size) {
    if (!address || !size || address > UINTPTR_MAX - size) return false;
    MEMORY_BASIC_INFORMATION memory{};
    if (!VirtualQuery(reinterpret_cast<const void*>(address), &memory, sizeof(memory))) return false;
    const auto end = reinterpret_cast<std::uintptr_t>(memory.BaseAddress) + memory.RegionSize;
    return memory.State == MEM_COMMIT && !(memory.Protect & (PAGE_GUARD | PAGE_NOACCESS)) &&
        address + size <= end;
}

// Runtime reads originate from the live ECS entity table in this process.
// ReadProcessMemory plus VirtualQuery for every scalar made the incremental
// scan advance only one or two entities per game-thread callback. Keep the
// same fault containment, but copy directly under SEH; callers still validate
// externally supplied ranges where required and invalid/stale pointers fail
// closed on an access violation.
bool guarded_copy(void* destination, const void* source, std::size_t size) {
    __try {
        std::memcpy(destination, source, size);
        return true;
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        return false;
    }
}

bool read_bytes(std::uintptr_t address, void* value, std::size_t size) {
    return address && value && size && address <= UINTPTR_MAX - size &&
        guarded_copy(value, reinterpret_cast<const void*>(address), size);
}
template<class T> bool read(std::uintptr_t address, T& value) {
    return read_bytes(address, &value, sizeof(value));
}


bool layout_snapshot(ResolvedLayout& result) {
    std::scoped_lock lock(state_mutex);
    if (!layout_ready) return false;
    result = live_layout;
    return true;
}
bool entity_pointers(const ResolvedLayout& layout, std::vector<std::uintptr_t>& pointers) {
    std::uint64_t count{};
    std::uintptr_t table{};
    if (!read(layout.count_address, count) || !count || count > (1u << 20) ||
        !read(layout.table_address, table) || !table) {
        std::scoped_lock lock(state_mutex);
        layout_ready = false;
        ++layout_epoch;
        handles.clear();
        reverse_handles.clear();
        return false;
    }
    pointers.resize(static_cast<std::size_t>(count));
    return read_bytes(table, pointers.data(), pointers.size() * sizeof(pointers[0]));
}
bool entity_view(std::uintptr_t pointer, const ResolvedLayout& layout, EntityView& entity) {
    entity.pointer = pointer;
    if (!pointer || !read(pointer + layout.entity_layout, entity.layout) || !entity.layout ||
        !read(pointer + layout.entity_storage, entity.storage) || !entity.storage ||
        !read(pointer + layout.entity_row, entity.row) || entity.row > (1u << 24) ||
        !read(pointer + layout.entity_id, entity.id) || !entity.id ||
        !read(pointer + layout.entity_generation, entity.generation)) return false;
    read(pointer + KfcRuntimeCompatibility::EnshroudedClient::entity_definition, entity.definition);
    return true;
}
bool component_address(const EntityView& entity, const ResolvedLayout& layout,
                       const ComponentType& component, std::uintptr_t& address) {
    std::uint64_t bits{};
    std::uint16_t offset{}, stride{};
    if (component.index >= max_components ||
        !read(entity.layout + layout.component_bits + (component.index / 64) * 8, bits) ||
        !(bits & (std::uint64_t{1} << (component.index % 64))) ||
        !read(entity.layout + KfcRuntimeCompatibility::EnshroudedClient::component_offsets + component.index * 2, offset) ||
        !read(entity.layout + KfcRuntimeCompatibility::EnshroudedClient::component_strides + component.index * 2, stride) || stride != component.size) return false;
    address = entity.storage + offset + static_cast<std::uintptr_t>(entity.row) * stride;
    return readable(address, component.size);
}
bool resolve_component(const char* name, ComponentType& component) {
    if (!name) return false;
    std::scoped_lock lock(state_mutex);
    const auto found = types.find(name);
    if (found == types.end()) return false;
    component = found->second;
    return true;
}
std::uint64_t identity_key(std::uint32_t id, std::uint32_t generation) {
    return static_cast<std::uint64_t>(generation) << 32 | id;
}
std::uint32_t handle_for(const EntityView& entity) {
    std::scoped_lock lock(state_mutex);
    const auto key = identity_key(entity.id, entity.generation);
    if (const auto found = reverse_handles.find(key); found != reverse_handles.end()) {
        handles.at(found->second).pointer = entity.pointer;
        return found->second;
    }
    // Never recycle an opaque handle during this process, including world changes.
    if (!next_handle) return 0;
    const auto handle = next_handle++;
    handles.emplace(handle, HandleRecord{entity.id, entity.generation, layout_epoch, entity.pointer});
    reverse_handles.emplace(key, handle);
    return handle;
}
bool entity_for_handle(std::uint32_t handle, const ResolvedLayout& layout, EntityView& entity) {
    HandleRecord record{};
    {
        std::scoped_lock lock(state_mutex);
        const auto found = handles.find(handle);
        if (found == handles.end() || found->second.epoch != layout_epoch) return false;
        record = found->second;
    }
    // Re-read the live identity and layout; never cache component addresses.
    // A moved/deleted entity is rediscovered by the next query, not a full-world
    // scan for each individual component read and write.
    return entity_view(record.pointer, layout, entity) && entity.id == record.id &&
        entity.generation == record.generation;
}

struct QueryOperation {
    const char* const* names{};
    std::size_t count{};
    std::uint32_t* entities{};
    std::size_t capacity{};
    std::size_t result{SIZE_MAX};
    std::vector<std::string> owned_names;
    std::vector<const char*> name_pointers;
    std::vector<std::uint32_t> output;
};
struct BoundsQueryOperation {
    std::vector<std::string> names;
    double bounds[6]{};
    double padding{};
    std::vector<std::uint32_t> output;
    std::size_t capacity{};
    std::size_t result{SIZE_MAX};
};
struct PropQueryOperation {
    double bounds[6]{};
    double padding{};
    bool exact_recipe_bounds{};
    std::vector<KfcRuntimePropRecord> output;
    std::size_t result{SIZE_MAX};
};
std::string query_key(const QueryOperation& operation) {
    std::string key;
    for (const auto& name : operation.owned_names) {
        key.append(std::to_string(name.size()));
        key.push_back(':');
        key.append(name);
        key.push_back(';');
    }
    return key;
}
std::uint64_t current_epoch() {
    std::scoped_lock lock(state_mutex);
    return layout_epoch;
}
void publish_query_result(QueryOperation& operation, const std::vector<std::uint32_t>& matches) {
    operation.result = matches.size();
    if (operation.entities && operation.capacity)
        std::copy_n(matches.begin(), (std::min)(operation.capacity, matches.size()), operation.entities);
}
void query_on_game_thread(void* opaque) {
    auto& operation = *static_cast<QueryOperation*>(opaque);
    std::vector<ComponentType> components(operation.count);
    for (std::size_t index = 0; index < operation.count; ++index)
        if (!resolve_component(operation.names[index], components[index])) return;
    ResolvedLayout layout{};
    if (!layout_snapshot(layout)) return;
    const auto epoch = current_epoch();
    const auto now_ms = GetTickCount64();
    const auto key = query_key(operation);
    for (auto iterator = query_scans.begin(); iterator != query_scans.end();) {
        if (now_ms - iterator->second.touched_ms > 2000) iterator = query_scans.erase(iterator);
        else ++iterator;
    }
    for (auto iterator = query_cache.begin(); iterator != query_cache.end();) {
        if (now_ms - iterator->second.completed_ms > 2000) iterator = query_cache.erase(iterator);
        else ++iterator;
    }
    if (auto cached = query_cache.find(key); cached != query_cache.end()) {
        if (cached->second.epoch == epoch && now_ms - cached->second.completed_ms <= 50) {
            publish_query_result(operation, cached->second.matches);
            return;
        }
        query_cache.erase(cached);
    }
    auto scan = query_scans.find(key);
    if (scan == query_scans.end() || scan->second.epoch != epoch) {
        QueryScanState fresh{};
        fresh.epoch = epoch;
        fresh.touched_ms = now_ms;
        if (!entity_pointers(layout, fresh.pointers)) return;
        scan = query_scans.insert_or_assign(key, std::move(fresh)).first;
        active_query_cursor.store(0, std::memory_order_relaxed);
        active_query_total.store(scan->second.pointers.size(), std::memory_order_relaxed);
        active_query_matches.store(0, std::memory_order_relaxed);
    }
    auto& progress = scan->second;
    progress.touched_ms = now_ms;
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(1);
    std::size_t visited = 0;
    for (; progress.cursor < progress.pointers.size() && visited < 256; ++progress.cursor, ++visited) {
        const auto pointer = progress.pointers[progress.cursor];
        EntityView entity{};
        if (entity_view(pointer, layout, entity)) {
            bool include = true;
            for (const auto& component : components) {
                std::uintptr_t address{};
                if (!component_address(entity, layout, component, address)) { include = false; break; }
            }
            if (include) progress.matches.push_back(handle_for(entity));
        }
        if (std::chrono::steady_clock::now() >= deadline) {
            ++progress.cursor;
            break;
        }
    }
    if (progress.cursor < progress.pointers.size()) {
        // The caller skips this update and retries on its next tick. This
        // distinct sentinel keeps an incomplete scan from looking like an
        // authoritative empty query result.
        operation.result = SIZE_MAX - 1;
        active_query_cursor.store(progress.cursor, std::memory_order_relaxed);
        active_query_total.store(progress.pointers.size(), std::memory_order_relaxed);
        active_query_matches.store(progress.matches.size(), std::memory_order_relaxed);
        return;
    }
    active_query_cursor.store(progress.cursor, std::memory_order_relaxed);
    active_query_total.store(progress.pointers.size(), std::memory_order_relaxed);
    active_query_matches.store(progress.matches.size(), std::memory_order_relaxed);
    auto completed = std::move(progress.matches);
    query_scans.erase(scan);
    auto [cached, _] = query_cache.insert_or_assign(key, QueryCacheEntry{epoch, now_ms, std::move(completed)});
    publish_query_result(operation, cached->second.matches);
}
void query_bounds_on_game_thread(void* opaque) {
    auto& operation = *static_cast<BoundsQueryOperation*>(opaque);
    std::vector<ComponentType> components;
    components.reserve(operation.names.size());
    for (const auto& name : operation.names) {
        ComponentType component{};
        if (!resolve_component(name.c_str(), component)) return;
        components.push_back(component);
    }
    const auto transform_type = std::find_if(operation.names.begin(), operation.names.end(),
        [](const auto& name) { return name == "keen::ecs::CurrentTransform"; });
    if (transform_type == operation.names.end()) return;
    const auto component_index = static_cast<std::size_t>(transform_type - operation.names.begin());
    ResolvedLayout layout{};
    if (!layout_snapshot(layout)) return;
    std::vector<std::uintptr_t> pointers;
    if (!entity_pointers(layout, pointers) || pointers.size() > (1u << 20)) return;
    operation.output.clear();
    std::unordered_set<std::uint32_t> seen;
    seen.reserve(pointers.size());
    for (const auto pointer : pointers) {
        EntityView entity{};
        if (!entity_view(pointer, layout, entity)) continue;
        if (!seen.insert(entity.id).second) continue;
        bool include = true;
        for (const auto& component : components) {
            std::uintptr_t address{};
            if (!component_address(entity, layout, component, address)) { include = false; break; }
        }
        if (!include) continue;
        std::uintptr_t address{};
        if (!component_address(entity, layout, components[component_index], address)) continue;
        struct NativeTransform { std::int64_t position[3]; float rotation[4]; float scale[3]; std::uint32_t padding; } transform{};
        static_assert(sizeof(NativeTransform) == 0x38);
        if (!read_bytes(address, &transform, sizeof(transform))) continue;
        const auto scale = (std::max)({std::abs(static_cast<double>(transform.scale[0])),
            std::abs(static_cast<double>(transform.scale[1])), std::abs(static_cast<double>(transform.scale[2]))});
        if (!std::isfinite(scale)) continue;
        const auto margin = operation.padding * scale;
        bool inside = true;
        for (int axis = 0; axis < 3; ++axis) {
            const auto world_position = static_cast<double>(transform.position[axis]) / 4294967296.0;
            if (!std::isfinite(world_position) || world_position < operation.bounds[axis] - margin ||
                world_position >= operation.bounds[axis + 3] + margin) { inside = false; break; }
        }
        if (inside) operation.output.push_back(handle_for(entity));
    }
    operation.result = operation.output.size();
}
bool recipe_bounds_intersect(const KfcRuntimePropRecord& prop,
                             const std::array<float, 6>& bounds,
                             const double* query_bounds) {
    for (int axis = 0; axis < 3; ++axis) {
        if (!std::isfinite(bounds[axis]) || !std::isfinite(bounds[axis + 3]) ||
            bounds[axis] > bounds[axis + 3]) return false;
    }
    const double qx = prop.orientation[0], qy = prop.orientation[1];
    const double qz = prop.orientation[2], qw = prop.orientation[3];
    const double norm = qx*qx + qy*qy + qz*qz + qw*qw;
    const double sx = prop.scale[0], sy = prop.scale[1], sz = prop.scale[2];
    if (!std::isfinite(norm) || norm <= 1e-12 || !std::isfinite(sx) ||
        !std::isfinite(sy) || !std::isfinite(sz)) return false;
    const double factor = 2.0 / norm;
    const double rotation[3][3] = {
        {1-factor*(qy*qy+qz*qz), factor*(qx*qy-qz*qw), factor*(qx*qz+qy*qw)},
        {factor*(qx*qy+qz*qw), 1-factor*(qx*qx+qz*qz), factor*(qy*qz-qx*qw)},
        {factor*(qx*qz-qy*qw), factor*(qy*qz+qx*qw), 1-factor*(qx*qx+qy*qy)},
    };
    const double scale[3]{sx, sy, sz};
    double local_center[3]{}, local_half[3]{};
    for (int axis = 0; axis < 3; ++axis) {
        local_center[axis] = (static_cast<double>(bounds[axis]) + bounds[axis + 3]) * 0.5;
        local_half[axis] = std::abs(static_cast<double>(bounds[axis + 3]) - bounds[axis]) * 0.5 * std::abs(scale[axis]);
        local_center[axis] *= scale[axis];
    }
    double world_center[3]{}, world_half[3]{};
    for (int row = 0; row < 3; ++row) {
        world_center[row] = static_cast<double>(prop.position[row]) / 4294967296.0;
        for (int column = 0; column < 3; ++column) {
            world_center[row] += rotation[row][column] * local_center[column];
            world_half[row] += std::abs(rotation[row][column]) * local_half[column];
        }
    }
    if (world_half[0] + world_half[1] + world_half[2] < 1e-6)
        world_half[0] = world_half[1] = world_half[2] = 0.25;
    for (int axis = 0; axis < 3; ++axis) {
        if (world_center[axis] + world_half[axis] < query_bounds[axis] ||
            world_center[axis] - world_half[axis] >= query_bounds[axis + 3]) return false;
    }
    return true;
}
void query_props_native(PropQueryOperation& operation) {
    using namespace KfcRuntimeCompatibility::EnshroudedClient;
    const auto transform_type = std::find_if(runtime_components.begin(), runtime_components.end(),
        [](const auto& component) { return component.qualified_name == "keen::ecs::CurrentTransform"; });
    const auto item_type = std::find_if(runtime_components.begin(), runtime_components.end(),
        [](const auto& component) { return component.qualified_name == "keen::ecs::UsedItem"; });
    if (transform_type == runtime_components.end() || item_type == runtime_components.end()) return;
    ResolvedLayout layout{};
    if (!layout_snapshot(layout)) return;
    std::vector<std::uintptr_t> pointers;
    if (!entity_pointers(layout, pointers) || pointers.size() > (1u << 20)) return;
    std::unordered_map<std::uint32_t, std::array<float, 6>> recipes;
    if (operation.exact_recipe_bounds) {
        std::scoped_lock lock(prop_recipe_mutex);
        if (!prop_recipe_catalog_ready) return;
        recipes = prop_recipes;
    }
    operation.output.clear();
    std::unordered_set<std::uint64_t> seen;
    seen.reserve(pointers.size());
    ComponentType transform_component{transform_type->index, transform_type->size};
    ComponentType item_component{item_type->index, item_type->size};
    for (const auto pointer : pointers) {
        EntityView entity{};
        if (!entity_view(pointer, layout, entity) || !seen.insert(identity_key(entity.id, entity.generation)).second) continue;
        std::uintptr_t transform_address{}, item_address{};
        if (!component_address(entity, layout, transform_component, transform_address) ||
            !component_address(entity, layout, item_component, item_address)) continue;
        struct NativeTransform { std::int64_t position[3]; float rotation[4]; float scale[3]; std::uint32_t padding; } transform{};
        static_assert(sizeof(NativeTransform) == 0x38);
        std::uint32_t item_id{};
        if (!read_bytes(transform_address, &transform, sizeof(transform)) ||
            !read_bytes(item_address, &item_id, sizeof(item_id)) || !item_id) continue;
        KfcRuntimePropRecord prop{};
        prop.item_id = item_id;
        std::copy_n(transform.position, 3, prop.position);
        std::copy_n(transform.rotation, 4, prop.orientation);
        std::copy_n(transform.scale, 3, prop.scale);
        if (!entity.definition || !read_bytes(entity.definition + definition_uuid,
                prop.template_uuid, sizeof(prop.template_uuid)) ||
            !(prop.template_uuid[0] || prop.template_uuid[1])) continue;
        if (operation.exact_recipe_bounds) {
            const auto recipe = recipes.find(item_id);
            if (recipe == recipes.end() || !recipe_bounds_intersect(prop, recipe->second, operation.bounds)) continue;
        } else {
            const double scale = (std::max)({std::abs(static_cast<double>(transform.scale[0])),
                std::abs(static_cast<double>(transform.scale[1])), std::abs(static_cast<double>(transform.scale[2]))});
            if (!std::isfinite(scale)) continue;
            const auto margin = operation.padding * scale;
            bool inside = true;
            for (int axis = 0; axis < 3; ++axis) {
                const auto position = static_cast<double>(transform.position[axis]) / 4294967296.0;
                if (!std::isfinite(position) || position < operation.bounds[axis] - margin ||
                    position >= operation.bounds[axis + 3] + margin) { inside = false; break; }
            }
            if (!inside) continue;
        }
        prop.entity_handle = handle_for(entity);
        if (prop.entity_handle) operation.output.push_back(prop);
    }
    operation.result = operation.output.size();
}

struct ResolveOperation { std::uint32_t entity_id{}, result{}; };
void resolve_on_game_thread(void* opaque) {
    auto& operation = *static_cast<ResolveOperation*>(opaque);
    ResolvedLayout layout{};
    if (!layout_snapshot(layout)) return;
    const auto epoch = current_epoch();
    const auto now_ms = GetTickCount64();
    for (auto iterator = resolve_scans.begin(); iterator != resolve_scans.end();) {
        if (now_ms - iterator->second.touched_ms > 2000) iterator = resolve_scans.erase(iterator);
        else ++iterator;
    }
    for (auto iterator = resolve_cache.begin(); iterator != resolve_cache.end();) {
        if (now_ms - iterator->second.completed_ms > 2000) iterator = resolve_cache.erase(iterator);
        else ++iterator;
    }
    if (auto cached = resolve_cache.find(operation.entity_id); cached != resolve_cache.end()) {
        if (cached->second.epoch == epoch && now_ms - cached->second.completed_ms <= 50) {
            operation.result = cached->second.handle;
            return;
        }
        resolve_cache.erase(cached);
    }
    auto scan = resolve_scans.find(operation.entity_id);
    if (scan == resolve_scans.end() || scan->second.epoch != epoch) {
        ResolveScanState fresh{};
        fresh.epoch = epoch;
        fresh.touched_ms = now_ms;
        if (!entity_pointers(layout, fresh.pointers)) return;
        scan = resolve_scans.insert_or_assign(operation.entity_id, std::move(fresh)).first;
    }
    auto& progress = scan->second;
    progress.touched_ms = now_ms;
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(1);
    std::size_t visited = 0;
    for (; progress.cursor < progress.pointers.size() && visited < 256; ++progress.cursor, ++visited) {
        EntityView candidate{};
        if (entity_view(progress.pointers[progress.cursor], layout, candidate) &&
            candidate.id == operation.entity_id) {
            operation.result = handle_for(candidate);
            resolve_cache.insert_or_assign(operation.entity_id,
                ResolveCacheEntry{epoch, now_ms, operation.result});
            resolve_scans.erase(scan);
            return;
        }
        if (std::chrono::steady_clock::now() >= deadline) {
            ++progress.cursor;
            break;
        }
    }
    if (progress.cursor >= progress.pointers.size()) {
        resolve_cache.insert_or_assign(operation.entity_id, ResolveCacheEntry{epoch, now_ms, 0});
        resolve_scans.erase(scan);
    }
}

struct ReadOperation {
    std::uint32_t handle{};
    const char* name{};
    void* value{};
    std::size_t size{};
    bool result{};
    std::string owned_name;
    std::vector<std::uint8_t> output;
};
void read_on_game_thread(void* opaque) {
    auto& operation = *static_cast<ReadOperation*>(opaque);
    ComponentType component{};
    ResolvedLayout layout{};
    EntityView entity{};
    std::uintptr_t address{};
    operation.result = operation.value && resolve_component(operation.name, component) &&
        component.size == operation.size && layout_snapshot(layout) &&
        entity_for_handle(operation.handle, layout, entity) &&
        component_address(entity, layout, component, address) &&
        read_bytes(address, operation.value, operation.size);
}

struct WriteOperation {
    std::uint32_t handle{};
    const char* name{};
    const void* mask{};
    const void* value{};
    std::size_t size{};
    bool result{};
    std::string owned_name;
    std::vector<std::uint8_t> mask_bytes, destination;
};
void write_on_game_thread(void* opaque) {
    if (stop_requested.load(std::memory_order_acquire)) return;
    auto& operation = *static_cast<WriteOperation*>(opaque);
    ComponentType component{};
    ResolvedLayout layout{};
    EntityView entity{};
    std::uintptr_t address{};
    if (!operation.mask || !operation.value || !resolve_component(operation.name, component) ||
        component.size != operation.size || !layout_snapshot(layout) ||
        !entity_for_handle(operation.handle, layout, entity) ||
        !component_address(entity, layout, component, address)) return;
    std::scoped_lock transaction(write_mutex);
    if (stop_requested.load(std::memory_order_acquire)) return;
    std::vector<std::uint8_t> before(operation.size), verified(operation.size);
    if (!read_bytes(address, before.data(), operation.size)) return;
    auto merged = before;
    const auto* mask = static_cast<const std::uint8_t*>(operation.mask);
    const auto* value = static_cast<const std::uint8_t*>(operation.value);
    for (std::size_t index = 0; index < operation.size; ++index) {
        if (mask[index]) merged[index] = value[index];
    }
    if (merged == before) { operation.result = true; return; }
    SIZE_T written{};
    if (!WriteProcessMemory(GetCurrentProcess(), reinterpret_cast<void*>(address), merged.data(),
            operation.size, &written) || written != operation.size ||
        !read_bytes(address, verified.data(), operation.size) ||
        std::memcmp(verified.data(), merged.data(), operation.size)) {
        SIZE_T restored{};
        WriteProcessMemory(GetCurrentProcess(), reinterpret_cast<void*>(address), before.data(),
            operation.size, &restored);
        return;
    }
    operation.result = true;
}

void reset_component_discovery() {
    std::scoped_lock lock(discovery_mutex);
    observed_indices_by_size.clear();
    discovered_layouts.clear();
    discovered_layout_components.clear();
    template_layout_samples.clear();
    template_layout_samples_dropped = 0;
    discovery_entities.clear();
    discovery_cursor = 0;
    discovery_table = 0;
    discovery_last_restart = 0;
    discovery_scanned_entities = 0;
    discovery_layout_count = 0;
    discovery_restarts = 0;
    discovery_complete = false;
}

bool collect_layout_component_candidates(const EntityView& entity, const ResolvedLayout& layout,
                                         std::vector<ComponentSlot>& components) {
    std::uint64_t component_bits[16]{};
    if (!read_bytes(entity.layout + layout.component_bits, component_bits, sizeof(component_bits))) return false;
    for (std::size_t index = 0; index < max_components; ++index) {
        if (!(component_bits[index / 64] & (std::uint64_t{1} << (index % 64)))) continue;
        std::uint16_t stride{}, offset{};
        if (!read(entity.layout + layout.component_strides + index * sizeof(stride), stride) || !stride ||
            !read(entity.layout + layout.component_offsets + index * sizeof(offset), offset)) continue;
        observed_indices_by_size[stride].insert(static_cast<std::uint16_t>(index));
        components.push_back({static_cast<std::uint16_t>(index), stride, offset});
    }
    ++discovery_layout_count;
    return true;
}

void collect_template_layout_sample(const EntityView& entity,
                                    const std::vector<ComponentSlot>& components) {
    using namespace KfcRuntimeCompatibility::EnshroudedClient;
    if (!entity.definition) return;
    struct DefinitionHeader {
        std::uint64_t uuid[2]{};
        std::uintptr_t name{};
        std::uint64_t name_size{};
    } definition{};
    if (!read_bytes(entity.definition + definition_uuid, definition.uuid, sizeof(definition.uuid)) ||
        !read(entity.definition + definition_name, definition.name) || !definition.name ||
        !read(entity.definition + definition_name_size, definition.name_size) ||
        !definition.name_size || definition.name_size > 128) return;
    std::string name(static_cast<std::size_t>(definition.name_size), '\0');
    if (!read_bytes(definition.name, name.data(), name.size())) return;
    if (const auto end = name.find('\0'); end != std::string::npos) name.resize(end);
    if (name.empty()) return;

    std::string key(sizeof(definition.uuid) + sizeof(entity.layout), '\0');
    std::memcpy(key.data(), definition.uuid, sizeof(definition.uuid));
    std::memcpy(key.data() + sizeof(definition.uuid), &entity.layout, sizeof(entity.layout));
    auto found = template_layout_samples.find(key);
    if (found == template_layout_samples.end()) {
        constexpr std::size_t max_template_layout_samples = 4096;
        if (template_layout_samples.size() >= max_template_layout_samples) {
            ++template_layout_samples_dropped;
            return;
        }
        TemplateLayoutSample sample{};
        std::memcpy(sample.uuid, definition.uuid, sizeof(sample.uuid));
        sample.layout = entity.layout;
        sample.name = std::move(name);
        sample.components = components;
        found = template_layout_samples.emplace(std::move(key), std::move(sample)).first;
    }
    ++found->second.entity_count;
}

void publish_discovered_component_indices() {
    std::unordered_map<std::string, std::uint32_t> contract;
    std::unordered_map<std::string, ComponentType> previously_resolved;
    {
        std::scoped_lock lock(state_mutex);
        contract = configured_types;
        previously_resolved = types;
    }
    std::unordered_map<std::uint32_t, std::vector<std::string>> names_by_size;
    for (const auto& [name, size] : contract) names_by_size[size].push_back(name);
    std::unordered_map<std::string, ComponentType> discovered;
    for (const auto& component : KfcRuntimeCompatibility::EnshroudedClient::runtime_components) {
        const auto known = contract.find(component.qualified_name);
        if (known != contract.end() && known->second == component.size)
            discovered.emplace(known->first, ComponentType{component.index, component.size});
    }
    for (const auto& [name, component] : previously_resolved) {
        const auto configured = contract.find(name);
        if (configured != contract.end() && configured->second == component.size)
            discovered.emplace(name, component);
    }
    for (const auto& [size, names] : names_by_size) {
        // Size can identify a component only when this build's reflected
        // component catalog contains exactly one component of that size.
        if (names.size() != 1) continue;
        const auto candidates = observed_indices_by_size.find(size);
        if (candidates == observed_indices_by_size.end() || candidates->second.size() != 1) continue;
        const auto index = *candidates->second.begin();
        const auto occupied = std::find_if(discovered.begin(), discovered.end(), [index](const auto& entry) {
            return entry.second.index == index;
        });
        if (occupied == discovered.end()) discovered.emplace(names.front(), ComponentType{index, size});
    }
    std::scoped_lock lock(state_mutex);
    if (configured_types != contract) return;
    types = std::move(discovered);
}

void component_discovery_tick(std::uintptr_t manager) {
    std::scoped_lock discovery_lock(discovery_mutex);
    ResolvedLayout layout{};
    if (!layout_snapshot(layout)) return;
    const auto count_address = manager + KfcRuntimeCompatibility::EnshroudedClient::entity_manager_count;
    const auto table_address = manager + KfcRuntimeCompatibility::EnshroudedClient::entity_manager_table;
    std::uint64_t count{};
    std::uintptr_t table{};
    if (!read(count_address, count) || !count || count > (1u << 20) ||
        !read(table_address, table) || !table || !readable(table, count * sizeof(std::uintptr_t))) return;

    const auto now = GetTickCount64();
    if (table != discovery_table || discovery_entities.size() != count ||
        (discovery_complete && now - discovery_last_restart >= 5000)) {
        discovery_entities.resize(static_cast<std::size_t>(count));
        if (!read_bytes(table, discovery_entities.data(), discovery_entities.size() * sizeof(std::uintptr_t))) {
            discovery_entities.clear();
            return;
        }
        discovery_table = table;
        discovery_cursor %= static_cast<std::size_t>(count);
        discovery_last_restart = now;
        discovery_scanned_entities = 0;
        ++discovery_restarts;
        discovery_complete = false;
    }
    if (discovery_complete || discovery_entities.empty()) return;

    constexpr std::size_t entities_per_tick = 128;
    const auto begin = discovery_cursor;
    const auto end = (std::min)(discovery_cursor + entities_per_tick, discovery_entities.size());
    for (; discovery_cursor < end; ++discovery_cursor) {
        EntityView entity{};
        if (!entity_view(discovery_entities[discovery_cursor], layout, entity)) continue;
        auto components = discovered_layout_components.find(entity.layout);
        if (components == discovered_layout_components.end()) {
            std::vector<ComponentSlot> discovered_components;
            if (!collect_layout_component_candidates(entity, layout, discovered_components)) continue;
            discovered_layouts.insert(entity.layout);
            components = discovered_layout_components.emplace(entity.layout, std::move(discovered_components)).first;
        }
        collect_template_layout_sample(entity, components->second);
    }
    discovery_scanned_entities += end - begin;
    if (discovery_cursor >= discovery_entities.size()) {
        discovery_complete = true;
    }
    // Publish every batch so a changing entity table cannot withhold mappings
    // until an entire snapshot has been traversed.
    publish_discovered_component_indices();
}
}

namespace EcsRuntime {
bool Initialize() {
    if (!KfcRuntimeCompatibility::EnshroudedClient::Load()) return false;
    image_base = reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
    if (!image_base) return false;
    const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(image_base);
    if (dos->e_magic != IMAGE_DOS_SIGNATURE) return false;
    const auto nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(image_base + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE) return false;
    stop_requested.store(false, std::memory_order_release);
    return GameThreadDispatcher::Initialize();
}
void Tick() {
    const auto manager = GameThreadDispatcher::EntityManager();
    if (!manager) return;
    const auto count_address = manager + KfcRuntimeCompatibility::EnshroudedClient::entity_manager_count;
    const auto table_address = manager + KfcRuntimeCompatibility::EnshroudedClient::entity_manager_table;
    std::uint64_t count{};
    std::uintptr_t table{};
    if (!read(count_address, count) || !count || count > (1u << 20) ||
        !read(table_address, table) || !readable(table, sizeof(std::uintptr_t))) {
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
        std::scoped_lock lock(state_mutex);
        if (!layout_ready || live_table != table || live_layout.count_address != count_address ||
            live_layout.table_address != table_address) {
            live_layout = {};
            live_table = table;
            live_layout.count_address = count_address;
            live_layout.table_address = table_address;
            live_layout.entity_id = KfcRuntimeCompatibility::EnshroudedClient::entity_id;
            live_layout.entity_generation = KfcRuntimeCompatibility::EnshroudedClient::entity_generation;
            live_layout.entity_layout = KfcRuntimeCompatibility::EnshroudedClient::entity_layout;
            live_layout.entity_storage = KfcRuntimeCompatibility::EnshroudedClient::entity_storage;
            live_layout.entity_row = KfcRuntimeCompatibility::EnshroudedClient::entity_row;
            live_layout.component_bits = KfcRuntimeCompatibility::EnshroudedClient::component_bits;
            live_layout.component_offsets = KfcRuntimeCompatibility::EnshroudedClient::component_offsets;
            live_layout.component_strides = KfcRuntimeCompatibility::EnshroudedClient::component_strides;
            layout_ready = true;
            ++layout_epoch;
            handles.clear();
            reverse_handles.clear();
        }
    }
    component_discovery_tick(manager);
}
std::string Status() {
    std::scoped_lock discovery_lock(discovery_mutex);
    std::scoped_lock lock(state_mutex);
    std::size_t dynamic_candidates{};
    for (const auto& [name, size] : configured_types) {
        (void)size;
        if (name.starts_with("keen::ecs::Dynamic")) ++dynamic_candidates;
    }
    const auto component_candidates = configured_types.size() - dynamic_candidates;
    std::ostringstream text;
    text << "types=" << types.size() << '/' << configured_types.size()
         << "{component=" << component_candidates << ",dynamic=" << dynamic_candidates << '}'
         << " component_index_discovery=" << (discovery_complete ? "complete" : "scanning")
         << '(' << discovery_cursor << '/' << discovery_entities.size()
         << ",layouts=" << discovered_layouts.size()
         << ",templates=" << template_layout_samples.size()
         << ",restarts=" << discovery_restarts << ')'
         << " profile=" << KfcRuntimeCompatibility::EnshroudedClient::status
         << " game_thread=" << GameThreadDispatcher::Status();
    for (const auto& operation : KfcRuntimeCompatibility::EnshroudedClient::runtime_operations)
        text << " world{" << operation.name << '=' << (operation.available ? operation.status : "unavailable") << '}';
    text << " voxel_context=" << (WorldRuntime::ActiveContextAvailable() ? "ready" : "waiting");
    if (types.empty()) return text.str() + " registry=unresolved layout=unavailable";
    if (!layout_ready) return text.str() + " registry=ready layout=discovering";
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
    std::unordered_set<std::string> profile_names;
    std::size_t dynamic_candidates{};
    for (const auto& component : KfcRuntimeCompatibility::EnshroudedClient::runtime_components)
        profile_names.insert(component.qualified_name);
    for (const auto& [name, size] : configured_types) {
        names_by_size[size].push_back(name);
        if (name.starts_with("keen::ecs::Dynamic")) ++dynamic_candidates;
    }
    for (const auto& [name, component] : types) {
        resolved_mappings.push_back({{"name",name},{"index",component.index},{"size",component.size},
            {"source",profile_names.contains(name) ? "exact-build-profile" : "unique-live-stride"}});
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
            const auto reason = names.size() != 1 ? "shared-reflected-size" :
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
        {"candidateTypeBreakdown",nlohmann::json{
            {"total",configured_types.size()},
            {"componentTypes",configured_types.size() - dynamic_candidates},
            {"dynamicRuntimeStructs",dynamic_candidates},
            {"resolved",types.size()},
            {"unresolved",configured_types.size() - types.size()}
        }},
        {"worldOperations",std::move(world_operations)},
        {"voxelContextActive",WorldRuntime::ActiveContextAvailable()},
        {"entityContextReady",WorldRuntime::EntityContextReady()},
        {"configuredTypes",configured_types.size()}, {"resolvedTypes",types.size()},
        {"layoutReady",layout_ready}, {"layoutEpoch",layout_epoch},
        {"componentDiscovery",nlohmann::json{
            {"complete",discovery_complete},
            {"scannedEntities",discovery_scanned_entities},
            {"totalEntities",discovery_entities.size()},
            {"archetypes",discovery_layout_count},
            {"distinctLayoutsSeen",discovered_layouts.size()},
            {"snapshotRestarts",discovery_restarts},
            {"observedStrideBuckets",observed_indices_by_size.size()},
            {"observedIndexCandidates",observed_indices},
            {"templateLayoutSamples",std::move(template_layouts)},
            {"templateLayoutSamplesDropped",template_layout_samples_dropped},
            {"mappingMethod","exact profile entries plus unique reflected size and unique live archetype stride"},
            {"resolved",std::move(resolved_mappings)},
            {"unresolved",std::move(unresolved_types)},
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
            {"writes", operation_counters.writes.load()},
            {"writeSuccesses", operation_counters.write_successes.load()},
            {"writeFailures", operation_counters.write_failures.load()},
            {"runtimePatches", nlohmann::json::parse(PatchRuntime::Diagnostics())}
        }},
        {"dispatcher",nlohmann::json::parse(GameThreadDispatcher::Diagnostics())}
    }).dump();
}
void Shutdown() {
    stop_requested.store(true, std::memory_order_release);
    GameThreadDispatcher::Shutdown();
    reset_component_discovery();
    std::scoped_lock lock(state_mutex);
    types.clear();
    configured_types.clear();
    live_layout = {};
    layout_ready = false;
    handles.clear();
    reverse_handles.clear();
    query_scans.clear();
    query_cache.clear();
    resolve_scans.clear();
    resolve_cache.clear();
}
}

extern "C" bool __cdecl KfcRuntimeEcsConfigure(const char* const* names,
                                                const std::uint32_t* sizes,
                                                std::size_t count) {
    if (!names || !sizes || !count || count > 20'000) return false;
    std::unordered_map<std::string, std::uint32_t> contract;
    for (std::size_t index = 0; index < count; ++index) {
        if (!names[index] || !sizes[index]) return false;
        const std::string name{names[index]};
        if (!name.starts_with("keen::ecs::")) return false;
        contract.emplace(name, sizes[index]);
    }
    {
        std::scoped_lock lock(state_mutex);
        if (configured_types == contract) return true;
        configured_types = std::move(contract);
        types.clear();
        for (const auto& component : KfcRuntimeCompatibility::EnshroudedClient::runtime_components) {
            const auto configured = configured_types.find(std::string(component.qualified_name));
            if (configured != configured_types.end() && configured->second == component.size)
                types.emplace(configured->first, ComponentType{component.index, component.size});
        }
        live_layout = {};
        layout_ready = false;
        handles.clear();
        reverse_handles.clear();
        query_scans.clear();
        query_cache.clear();
        resolve_scans.clear();
        resolve_cache.clear();
    }
    reset_component_discovery();
    return true;
}

extern "C" bool __cdecl KfcRuntimeEcsReady() {
    std::scoped_lock lock(state_mutex);
    return GameThreadDispatcher::Ready() && layout_ready && !types.empty();
}
extern "C" bool __cdecl KfcRuntimeEcsPropQueryReady() {
    ResolvedLayout layout{};
    if (!layout_snapshot(layout)) return false;
    using namespace KfcRuntimeCompatibility::EnshroudedClient;
    const auto transform = std::find_if(runtime_components.begin(), runtime_components.end(),
        [](const auto& component) { return component.qualified_name == "keen::ecs::CurrentTransform"; });
    const auto item = std::find_if(runtime_components.begin(), runtime_components.end(),
        [](const auto& component) { return component.qualified_name == "keen::ecs::UsedItem"; });
    return transform != runtime_components.end() && item != runtime_components.end();
}
extern "C" bool __cdecl KfcRuntimeEcsCanWrite() {
    std::scoped_lock lock(state_mutex);
    return GameThreadDispatcher::Ready() && layout_ready && !types.empty();
}
extern "C" bool __cdecl KfcRuntimeEcsDescribe(const char* name, std::uint32_t* size) {
    ComponentType component{};
    if (!size || !resolve_component(name, component)) return false;
    *size = component.size;
    return true;
}
extern "C" std::size_t __cdecl KfcRuntimeEcsQuery(const char* const* names, std::size_t count,
                                                    std::uint32_t* entities, std::size_t capacity) {
    operation_counters.queries.fetch_add(1, std::memory_order_relaxed);
    if (!names || !count || count > max_components || capacity > (1u << 20) || !KfcRuntimeEcsReady()) {
        operation_counters.query_failures.fetch_add(1, std::memory_order_relaxed);
        return SIZE_MAX;
    }
    auto operation = std::make_shared<QueryOperation>();
    for (std::size_t index = 0; index < count; ++index) {
        if (!names[index]) {
            operation_counters.query_failures.fetch_add(1, std::memory_order_relaxed);
            return SIZE_MAX;
        }
        operation->owned_names.emplace_back(names[index]);
    }
    for (const auto& name : operation->owned_names) operation->name_pointers.push_back(name.c_str());
    operation->names = operation->name_pointers.data();
    operation->count = count;
    operation->output.resize(capacity);
    operation->entities = operation->output.data();
    operation->capacity = capacity;
    if (!GameThreadDispatcher::Invoke(query_on_game_thread, operation)) {
        operation_counters.query_failures.fetch_add(1, std::memory_order_relaxed);
        return SIZE_MAX;
    }
    if (operation->result >= SIZE_MAX - 1) {
        if (operation->result == SIZE_MAX - 1) operation_counters.query_incomplete.fetch_add(1, std::memory_order_relaxed);
        else operation_counters.query_failures.fetch_add(1, std::memory_order_relaxed);
        return operation->result;
    }
    if (entities) std::copy_n(operation->output.begin(), (std::min)(capacity, operation->result), entities);
    operation_counters.query_successes.fetch_add(1, std::memory_order_relaxed);
    return operation->result;
}
extern "C" std::size_t __cdecl KfcRuntimeEcsQueryBounds(const char* const* names, std::size_t count,
                                                           const double* bounds, double padding, std::uint32_t* entities,
                                                           std::size_t capacity) {
    if (!names || !count || count > max_components || !bounds || !std::isfinite(padding) || padding < 0 ||
        capacity > (1u << 20) || !KfcRuntimeEcsReady())
        return SIZE_MAX;
    auto operation = std::make_shared<BoundsQueryOperation>();
    operation->padding = padding;
    for (std::size_t index = 0; index < count; ++index) {
        if (!names[index]) return SIZE_MAX;
        operation->names.emplace_back(names[index]);
    }
    bool has_transform = false;
    for (int axis = 0; axis < 3; ++axis) {
        const auto minimum = bounds[axis], maximum = bounds[axis + 3];
        if (!std::isfinite(minimum) || !std::isfinite(maximum) || minimum >= maximum) return SIZE_MAX;
        operation->bounds[axis] = minimum;
        operation->bounds[axis + 3] = maximum;
    }
    has_transform = std::find(operation->names.begin(), operation->names.end(), "keen::ecs::CurrentTransform") != operation->names.end();
    if (!has_transform) return SIZE_MAX;
    operation->capacity = capacity;
    if (!GameThreadDispatcher::Invoke(query_bounds_on_game_thread, operation, 500)) return SIZE_MAX;
    if (operation->result == SIZE_MAX) return SIZE_MAX;
    if (entities) std::copy_n(operation->output.begin(), (std::min)(capacity, operation->output.size()), entities);
    return operation->result;
}
extern "C" std::size_t __cdecl KfcRuntimeWorldEntityQueryProps(const double* bounds, double padding,
                                                                  KfcRuntimePropRecord* props, std::size_t capacity) {
    if (!bounds || !std::isfinite(padding) || padding < 0 || capacity > (1u << 20) || (!props && capacity)) return SIZE_MAX;
    auto operation = std::make_shared<PropQueryOperation>();
    operation->padding = padding;
    for (int axis = 0; axis < 3; ++axis) {
        if (!std::isfinite(bounds[axis]) || !std::isfinite(bounds[axis + 3]) || bounds[axis] >= bounds[axis + 3]) return SIZE_MAX;
        operation->bounds[axis] = bounds[axis];
        operation->bounds[axis + 3] = bounds[axis + 3];
    }
    query_props_native(*operation);
    if (operation->result == SIZE_MAX) return SIZE_MAX;
    if (props) std::copy_n(operation->output.begin(), (std::min)(capacity, operation->output.size()), props);
    return operation->result;
}
extern "C" std::size_t __cdecl KfcRuntimeWorldEntityQueryPropsInBounds(
    const double* bounds, KfcRuntimePropRecord* props, std::size_t capacity) {
    if (!bounds || capacity > (1u << 20) || (!props && capacity)) return SIZE_MAX;
    auto operation = std::make_shared<PropQueryOperation>();
    operation->exact_recipe_bounds = true;
    for (int axis = 0; axis < 3; ++axis) {
        if (!std::isfinite(bounds[axis]) || !std::isfinite(bounds[axis + 3]) ||
            bounds[axis] >= bounds[axis + 3]) return SIZE_MAX;
        operation->bounds[axis] = bounds[axis];
        operation->bounds[axis + 3] = bounds[axis + 3];
    }
    query_props_native(*operation);
    if (operation->result == SIZE_MAX) return SIZE_MAX;
    if (props) std::copy_n(operation->output.begin(), (std::min)(capacity, operation->output.size()), props);
    return operation->result;
}
extern "C" bool __cdecl KfcRuntimeWorldEntityRegisterPropRecipes(
    const KfcRuntimePropRecipe* recipes, std::size_t count) {
    if ((!recipes && count) || count > 1'000'000) return false;
    std::unordered_map<std::uint32_t, std::array<float, 6>> resolved;
    resolved.reserve(count);
    for (std::size_t index = 0; index < count; ++index) {
        const auto& recipe = recipes[index];
        if (!recipe.item_id || !std::all_of(std::begin(recipe.bounds), std::end(recipe.bounds),
                [](float value) { return std::isfinite(value); }) ||
            recipe.bounds[0] > recipe.bounds[3] || recipe.bounds[1] > recipe.bounds[4] ||
            recipe.bounds[2] > recipe.bounds[5]) return false;
        if (!resolved.emplace(recipe.item_id, std::to_array(recipe.bounds)).second) return false;
    }
    {
        std::scoped_lock lock(prop_recipe_mutex);
        prop_recipes = std::move(resolved);
        prop_recipe_catalog_ready = true;
    }
    return true;
}
extern "C" bool __cdecl KfcRuntimeWorldEntityGetTransform(std::uint32_t handle, KfcRuntimePropRecord* prop) {
    if (!handle || !prop) return false;
    HandleRecord record{};
    {
        std::scoped_lock lock(state_mutex);
        const auto found = handles.find(handle);
        if (found == handles.end() || found->second.epoch != layout_epoch) return false;
        record = found->second;
    }
    ResolvedLayout layout{};
    if (!layout_snapshot(layout)) return false;
    using namespace KfcRuntimeCompatibility::EnshroudedClient;
    const auto transform_type = std::find_if(runtime_components.begin(), runtime_components.end(),
        [](const auto& component) { return component.qualified_name == "keen::ecs::CurrentTransform"; });
    const auto item_type = std::find_if(runtime_components.begin(), runtime_components.end(),
        [](const auto& component) { return component.qualified_name == "keen::ecs::UsedItem"; });
    if (transform_type == runtime_components.end() || item_type == runtime_components.end()) return false;
    std::vector<std::uintptr_t> pointers;
    if (!entity_pointers(layout, pointers) || pointers.size() > (1u << 20)) return false;
    const ComponentType transform_component{transform_type->index, transform_type->size};
    const ComponentType item_component{item_type->index, item_type->size};
    for (const auto pointer : pointers) {
        EntityView entity{};
        if (!entity_view(pointer, layout, entity) || entity.id != record.id || entity.generation != record.generation) continue;
        std::uintptr_t transform_address{}, item_address{};
        struct NativeTransform { std::int64_t position[3]; float rotation[4]; float scale[3]; std::uint32_t padding; } transform{};
        std::uint32_t item_id{};
        if (!component_address(entity, layout, transform_component, transform_address) ||
            !component_address(entity, layout, item_component, item_address) ||
            !read_bytes(transform_address, &transform, sizeof(transform)) ||
            !read_bytes(item_address, &item_id, sizeof(item_id)) || !item_id) return false;
        const auto current_handle = handle_for(entity);
        if (current_handle != handle) return false;
        *prop = {};
        prop->entity_handle = handle;
        prop->item_id = item_id;
        std::copy_n(transform.position, 3, prop->position);
        std::copy_n(transform.rotation, 4, prop->orientation);
        std::copy_n(transform.scale, 3, prop->scale);
        using namespace KfcRuntimeCompatibility::EnshroudedClient;
        if (!entity.definition || !read_bytes(entity.definition + definition_uuid,
                prop->template_uuid, sizeof(prop->template_uuid)) ||
            !(prop->template_uuid[0] || prop->template_uuid[1])) return false;
        return true;
    }
    return false;
}
extern "C" std::uint32_t __cdecl KfcRuntimeEcsResolve(std::uint32_t entity_id) {
    operation_counters.resolves.fetch_add(1, std::memory_order_relaxed);
    if (!entity_id || !KfcRuntimeEcsReady()) {
        operation_counters.resolve_failures.fetch_add(1, std::memory_order_relaxed);
        return 0;
    }
    auto operation = std::make_shared<ResolveOperation>();
    operation->entity_id = entity_id;
    if (!GameThreadDispatcher::Invoke(resolve_on_game_thread, operation)) {
        operation_counters.resolve_failures.fetch_add(1, std::memory_order_relaxed);
        return 0;
    }
    if (operation->result) operation_counters.resolve_successes.fetch_add(1, std::memory_order_relaxed);
    else operation_counters.resolve_failures.fetch_add(1, std::memory_order_relaxed);
    return operation->result;
}
extern "C" bool __cdecl KfcRuntimeEcsRead(std::uint32_t handle, const char* name,
                                           void* value, std::size_t size) {
    operation_counters.reads.fetch_add(1, std::memory_order_relaxed);
    if (!name || !value || !size || size > (1u << 20) || !KfcRuntimeEcsReady()) {
        operation_counters.read_failures.fetch_add(1, std::memory_order_relaxed);
        return false;
    }
    auto operation = std::make_shared<ReadOperation>();
    operation->handle = handle;
    operation->owned_name = name;
    operation->name = operation->owned_name.c_str();
    operation->output.resize(size);
    operation->value = operation->output.data();
    operation->size = size;
    if (!GameThreadDispatcher::Invoke(read_on_game_thread, operation) || !operation->result) {
        operation_counters.read_failures.fetch_add(1, std::memory_order_relaxed);
        return false;
    }
    std::memcpy(value, operation->output.data(), size);
    operation_counters.read_successes.fetch_add(1, std::memory_order_relaxed);
    return true;
}
extern "C" bool __cdecl KfcRuntimeEcsWrite(std::uint32_t handle, const char* name,
                                            const void* mask, const void* value,
                                            std::size_t size) {
    operation_counters.writes.fetch_add(1, std::memory_order_relaxed);
    if (!name || !mask || !value || !size || size > (1u << 20) || !KfcRuntimeEcsCanWrite()) {
        operation_counters.write_failures.fetch_add(1, std::memory_order_relaxed);
        return false;
    }
    auto operation = std::make_shared<WriteOperation>();
    operation->handle = handle;
    operation->owned_name = name;
    operation->name = operation->owned_name.c_str();
    const auto first = static_cast<const std::uint8_t*>(mask);
    const auto last = static_cast<const std::uint8_t*>(value);
    operation->mask_bytes.assign(first, first + size);
    operation->destination.assign(last, last + size);
    operation->mask = operation->mask_bytes.data();
    operation->value = operation->destination.data();
    operation->size = size;
    const bool written = GameThreadDispatcher::Invoke(write_on_game_thread, operation) && operation->result;
    if (written) operation_counters.write_successes.fetch_add(1, std::memory_order_relaxed);
    else operation_counters.write_failures.fetch_add(1, std::memory_order_relaxed);
    return written;
}
extern "C" bool __cdecl KfcRuntimePatchAvailable(const char* name) {
    if (name && std::strcmp(name, "runtime.gameplay.patch") == 0) return PatchRuntime::AnyAvailable();
    return PatchRuntime::Available(name);
}
extern "C" bool __cdecl KfcRuntimePatchSetEnabled(const char* name, bool enabled, std::uint32_t* outcome) {
    return PatchRuntime::SetEnabled(name, enabled, outcome);
}
