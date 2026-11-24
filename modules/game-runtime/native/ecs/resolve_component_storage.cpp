#include "ecs/ecs_session_state.h"

namespace EcsImplementation {
bool component_presence(const EntityView& entity, const ResolvedLayout& layout,
                        const ComponentType& component, bool& present){
    std::uint64_t bits{};
    if (component.index >= max_components ||
        !read(entity.layout + layout.component_bits + (component.index / 64) * 8, bits)) return false;
    present = (bits & (std::uint64_t{1} << (component.index % 64))) != 0;
    return true;
}

bool component_present(const EntityView& entity, const ResolvedLayout& layout, const ComponentType& component){
    bool present{};
    return component_presence(entity, layout, component, present) && present;
}

bool component_address(const EntityView& entity, const ResolvedLayout& layout,
                       const ComponentType& component, std::uintptr_t& address, bool check_page){
    std::uint16_t offset{}, stride{};
    if (!component_present(entity, layout, component) ||
        !read(entity.layout + layout.component_offsets + component.index * 2, offset) ||
        !read(entity.layout + layout.component_strides + component.index * 2, stride) || stride != component.size) return false;
    const auto delta = offset + static_cast<std::uintptr_t>(entity.row) * stride;
    if (entity.storage > UINTPTR_MAX - delta) return false;
    address = entity.storage + delta;
    // Read-only scan callers immediately copy through guarded_copy (SEH).
    // Re-querying the page for every component doubles kernel transitions for
    // every prop in the world; writes and address-only queries keep the check.
    return !check_page || readable(address, component.size);
}

bool resolve_component(const char* name, ComponentType& component){
    if (!name) return false;
    std::scoped_lock lock(state_mutex);
    const auto found = types.find(name);
    if (found == types.end()) return false;
    component = found->second;
    return true;
}
}

using namespace EcsImplementation;

namespace EcsRuntime {
bool MatchesComponentAddress(std::uint32_t handle, const char* name, const void* expected) {
    ComponentType component{};
    ResolvedLayout layout{};
    EntityView entity{};
    std::uintptr_t address{};
    return resolve_component(name, component) && layout_snapshot(layout) &&
        entity_for_handle(handle, layout, entity) &&
        component_address(entity, layout, component, address) &&
        address == reinterpret_cast<std::uintptr_t>(expected);
}
}
