#pragma once
#include "abi/runtime_api.h"
#include "engine-context/engine_owners.h"
#include <windows.h>
#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

namespace EcsImplementation {
struct ComponentType {
    std::uint16_t index; std::uint32_t size;
    bool operator==(const ComponentType&) const = default;
};

struct ResolvedLayout {
    std::uintptr_t count_address{};
    std::uintptr_t table_address{};
    std::uintptr_t occupancy_address{};
    std::uintptr_t keys_address{};
    std::size_t entity_layout{};
    std::size_t entity_storage{};
    std::size_t entity_row{};
    std::size_t entity_id{};
    std::size_t entity_generation{};
    std::size_t entity_definition{};
    std::size_t entity_definition_uuid{};
    bool entity_definition_uuid_validated{};
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
    bool operator==(const EntityView&) const = default;
};

struct EntityPointer {
    std::uintptr_t pointer{};
    std::size_t table_index{};
};

bool compact_entity_pointers(const std::uintptr_t* slots, std::size_t count,
                            const std::uint64_t* occupancy, std::size_t occupancy_count,
                            std::vector<EntityPointer>& pointers);

struct HandleRecord {
    std::uint32_t id{};
    std::uint32_t generation{};
    std::uint64_t epoch{};
    std::uintptr_t pointer{};
    std::uintptr_t table_slot{};
};

bool readable(std::uintptr_t address, std::size_t size);

bool guarded_copy(void* destination, const void* source, std::size_t size);

bool read_bytes(std::uintptr_t address, void* value, std::size_t size);

template<class T> bool read(std::uintptr_t address, T& value) {
    return read_bytes(address, &value, sizeof(value));
}

bool layout_snapshot(ResolvedLayout& result);

bool entity_pointers(const ResolvedLayout& layout, std::vector<EntityPointer>& pointers);

enum class EntityMembership { Unresolved, Present, Absent };

// Bounded original ID lookup. Outputs are published only for two stable live
// identity/header observations and unchanged manager arrays. Probe exhaustion
// and unreadable occupied buckets are Unresolved, never Absent.
EntityMembership entity_for_id(std::uint32_t id, const ResolvedLayout& layout,
                               EntityView& entity, std::size_t& table_index);

// Absent is published only by two matching original hash-table lookups. A
// missing layout, unreadable bucket or moving allocation is Unresolved.
EntityMembership entity_membership(const EntityView& entity, const ResolvedLayout& layout);

bool entity_registered(const EntityView& entity, const ResolvedLayout& layout);

bool entity_view(std::uintptr_t pointer, const ResolvedLayout& layout, EntityView& entity);

bool component_presence(const EntityView& entity, const ResolvedLayout& layout,
                        const ComponentType& component, bool& present);

bool component_present(const EntityView& entity, const ResolvedLayout& layout, const ComponentType& component);

bool component_address(const EntityView& entity, const ResolvedLayout& layout,
                       const ComponentType& component, std::uintptr_t& address, bool check_page = true);

bool resolve_component(const char* name, ComponentType& component);

std::uint64_t identity_key(std::uint32_t id, std::uint32_t generation);

std::uint32_t handle_for(const EntityView& entity, std::size_t table_index = SIZE_MAX);

bool SnapshotEntityIds(std::vector<std::uint32_t>& ids);

bool ResolvePropEntityId(std::uint32_t entity_id, KfcRuntimePropRecord* prop);

// Shared, stable component snapshot for world-editor queries and handle reads.
// Template UUIDs use the current owner layout on the original-owner path.
bool read_prop_payload(const EntityView& entity, const ResolvedLayout& layout,
                       const ComponentType& transform_component, const ComponentType& item_component,
                       KfcRuntimePropRecord& prop, bool include_template_uuid);

bool read_prop_template_uuid(const EntityView& entity, const ResolvedLayout& layout,
                             KfcRuntimePropRecord& prop);

bool entity_for_handle(std::uint32_t handle, const ResolvedLayout& layout, EntityView& entity);

struct ResolveOperation { std::uint32_t entity_id{}, result{}; };

void resolve_on_game_thread(void* opaque);

struct TemplateUuidOperation {
    std::uint32_t handle{};
    std::array<std::uint8_t, 16> uuid{};
    bool result{};
};

void template_uuid_on_game_thread(void* opaque);

}
