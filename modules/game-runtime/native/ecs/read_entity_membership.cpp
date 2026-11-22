#include "ecs/ecs_session_state.h"

namespace EcsImplementation {
bool compact_entity_pointers(const std::uintptr_t* slots, std::size_t count,
                            const std::uint64_t* occupancy, std::size_t occupancy_count,
                            std::vector<EntityPointer>& pointers){
    if ((!slots && count) || (occupancy && occupancy_count != (count + 63) / 64) ||
        (!occupancy && occupancy_count)) return false;
    pointers.clear();
    if (occupancy) {
        std::size_t occupied{};
        for (std::size_t word = 0; word < occupancy_count; ++word)
            occupied += std::popcount(occupancy[word]);
        pointers.reserve(occupied);
    } else {
        pointers.reserve(count);
    }
    if (occupancy) {
        // Original entity tables are sparse. Visit occupied bits in slot order
        // instead of testing every empty bucket on each query refresh.
        for (std::size_t word = 0; word < occupancy_count; ++word) {
            auto bits = occupancy[word];
            while (bits) {
                const auto index = word * 64 + std::countr_zero(bits);
                if (index < count && slots[index]) pointers.push_back({slots[index], index});
                bits &= bits - 1;
            }
        }
    } else {
        for (std::size_t index = 0; index < count; ++index)
            if (slots[index]) pointers.push_back({slots[index], index});
    }
    return true;
}

bool readable(std::uintptr_t address, std::size_t size){
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
bool guarded_copy(void* destination, const void* source, std::size_t size){
    __try {
        std::memcpy(destination, source, size);
        return true;
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        return false;
    }
}

bool read_bytes(std::uintptr_t address, void* value, std::size_t size){
    return address && value && size && address <= UINTPTR_MAX - size &&
        guarded_copy(value, reinterpret_cast<const void*>(address), size);
}

bool layout_snapshot(ResolvedLayout& result){
    std::scoped_lock lock(state_mutex);
    if (!layout_ready) return false;
    result = live_layout;
    return true;
}

bool entity_pointers(const ResolvedLayout& layout, std::vector<EntityPointer>& pointers){
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
    std::vector<std::uintptr_t> slots(static_cast<std::size_t>(count));
    if (!read_bytes(table, slots.data(), slots.size() * sizeof(slots[0]))) return false;
    pointers.clear();
    if (layout.occupancy_address) {
        std::uintptr_t occupancy{};
        if (!read(layout.occupancy_address, occupancy) || !occupancy) return false;
        std::vector<std::uint64_t> bits((slots.size() + 63) / 64);
        if (!read_bytes(occupancy, bits.data(), bits.size() * sizeof(bits[0]))) return false;
        if (!compact_entity_pointers(slots.data(), slots.size(), bits.data(), bits.size(), pointers)) return false;
        std::uintptr_t occupancy_again{}, table_again{};
        std::uint64_t count_again{};
        if (!read(layout.occupancy_address, occupancy_again) || occupancy_again != occupancy ||
            !read(layout.table_address, table_again) || table_again != table ||
            !read(layout.count_address, count_again) || count_again != count) return false;
    } else {
        if (!compact_entity_pointers(slots.data(), slots.size(), nullptr, 0, pointers)) return false;
    }
    return true;
}

namespace {
struct OriginalEntityTable {
    std::uint64_t count{};
    std::uintptr_t table{}, occupancy{}, keys{};
    bool operator==(const OriginalEntityTable&) const = default;
};
bool original_entity_table(const ResolvedLayout& layout, OriginalEntityTable& output) {
    OriginalEntityTable table;
    if (!layout.occupancy_address || !layout.keys_address ||
        !read(layout.count_address, table.count) || !table.count || table.count > (1u << 20) ||
        (table.count & (table.count - 1)) || !read(layout.table_address, table.table) || !table.table ||
        !read(layout.occupancy_address, table.occupancy) || !table.occupancy ||
        !read(layout.keys_address, table.keys) || !table.keys ||
        table.table > UINTPTR_MAX - table.count * sizeof(std::uintptr_t) ||
        table.keys > UINTPTR_MAX - table.count * sizeof(std::uint32_t) ||
        table.occupancy > UINTPTR_MAX - ((table.count + 63) / 64) * 8) return false;
    output = table;
    return true;
}
struct IdObservation {
    EntityMembership membership{EntityMembership::Unresolved};
    std::size_t slot{};
    std::uintptr_t pointer{};
    std::uint32_t generation{};
    bool operator==(const IdObservation&) const = default;
};
IdObservation observe_original_id(std::uint32_t id, const ResolvedLayout& layout, const OriginalEntityTable& table) {
    auto hash = id;
    hash = ((hash >> 16) ^ hash) * 0x45d9f3bu;
    hash = ((hash >> 16) ^ hash) * 0x45d9f3bu;
    hash = (hash >> 16) ^ hash;
    auto slot = static_cast<std::size_t>(hash & (table.count - 1));
    for (std::size_t visited = 0; visited < (std::min)(table.count, std::uint64_t{8192});
         ++visited, slot = (slot + 1) & (table.count - 1)) {
        std::uint64_t bits{};
        std::uint32_t key{};
        if (!read(table.occupancy + (slot / 64) * 8, bits)) return {};
        if (!(bits & (std::uint64_t{1} << (slot % 64)))) return {EntityMembership::Absent, slot};
        if (!read(table.keys + slot * sizeof(key), key)) return {};
        if (key != id) continue;
        std::uintptr_t current{};
        std::uint32_t observed_id{}, generation{};
        if (!read(table.table + slot * sizeof(current), current) || !current ||
            !read(current + layout.entity_id, observed_id) || observed_id != id ||
            !read(current + layout.entity_generation, generation)) return {};
        return {EntityMembership::Present, slot, current, generation};
    }
    return {};
}
IdObservation stable_original_id(std::uint32_t id, const ResolvedLayout& layout) {
    OriginalEntityTable table, again;
    if (!id || !original_entity_table(layout, table)) return {};
    const auto before = observe_original_id(id, layout, table);
    const auto after = observe_original_id(id, layout, table);
    return before == after && original_entity_table(layout, again) && again == table ? before : IdObservation{};
}
}

// Erased buckets may retain plausible objects. Both handle membership and
// native queued-ID verification share the same original occupied ID lookup.
EntityMembership entity_membership(const EntityView& entity, const ResolvedLayout& layout){
    const auto current = stable_original_id(entity.id, layout);
    if (current.membership != EntityMembership::Present) return current.membership;
    if (current.generation != entity.generation) return EntityMembership::Absent;
    return current.pointer == entity.pointer ? EntityMembership::Present : EntityMembership::Unresolved;
}

EntityMembership entity_for_id(std::uint32_t id, const ResolvedLayout& layout,
                               EntityView& output, std::size_t& table_index) {
    const auto before = stable_original_id(id, layout);
    if (before.membership != EntityMembership::Present) return before.membership;
    EntityView entity, again;
    if (!entity_view(before.pointer, layout, entity) || entity.id != id || entity.generation != before.generation ||
        !entity_view(before.pointer, layout, again) || again != entity) return EntityMembership::Unresolved;
    const auto after = stable_original_id(id, layout);
    if (after != before) return EntityMembership::Unresolved;
    output = entity;
    table_index = before.slot;
    return EntityMembership::Present;
}

bool entity_registered(const EntityView& entity, const ResolvedLayout& layout){
    if (!layout.occupancy_address && !layout.keys_address) return true;
    return entity_membership(entity, layout) == EntityMembership::Present;
}

bool entity_view(std::uintptr_t pointer, const ResolvedLayout& layout, EntityView& entity){
    entity.pointer = pointer;
    if (!pointer || !read(pointer + layout.entity_layout, entity.layout) || !entity.layout ||
        !read(pointer + layout.entity_storage, entity.storage) || !entity.storage ||
        !read(pointer + layout.entity_row, entity.row) || entity.row > (1u << 24) ||
        !read(pointer + layout.entity_id, entity.id) || !entity.id ||
        !read(pointer + layout.entity_generation, entity.generation)) return false;
    read(pointer + layout.entity_definition, entity.definition);
    return entity_registered(entity, layout);
}
}
