#include "engine-context/engine_context_state.h"

using namespace EngineContextImplementation;

namespace EngineContextImplementation {
std::mutex context_mutex;

Layout active_layout;

bool configured{};

nlohmann::json system_dependency_layout;

nlohmann::json service_layout;

bool read_span(std::uintptr_t address, void* output, std::size_t size){
    if (!address || address > UINTPTR_MAX - size) return false;
    __try { std::memcpy(output, reinterpret_cast<const void*>(address), size); return true; }
    __except (EXCEPTION_EXECUTE_HANDLER) { return false; }
}

bool add(std::uintptr_t base, std::size_t offset, std::uintptr_t& address){
    if (!base || offset > UINTPTR_MAX - base) return false;
    address = base + offset;
    return true;
}

bool vtable(std::uintptr_t object, std::uintptr_t rva, std::uintptr_t image, std::size_t size){
    std::uintptr_t actual{};
    return rva < size && read(object, actual) && actual == image + rva;
}

// Read the canonical ready-entry list for both hosted client worlds and the
// dedicated server. No host locks or allocations are allowed in this helper.
static bool select_scene_context(const Layout& layout, EngineContext::EcsView& view, const char** reason){
    const auto fail = [&](const char* message) { if (reason) *reason = message; return false; };
    std::uint64_t count{}, capacity{};
    if (!field(view.scene_owner, layout.scene_entries_offset, view.scene_entries) ||
        !field(view.scene_owner, layout.scene_count_offset, count) || count > layout.maximum_scenes ||
        !field(view.scene_owner, layout.scene_capacity_offset, capacity) || capacity < count || capacity > layout.maximum_scenes)
        return fail("ecs-active-scene-list-unavailable");
    if (count && (!view.scene_entries || view.scene_entries > UINTPTR_MAX - count * layout.entry_stride))
        return fail("ecs-active-scene-list-range-invalid");
    std::size_t ready{};
    for (std::size_t index = 0; index < count; ++index) {
        const auto entry = view.scene_entries + index * layout.entry_stride;
        std::uint8_t state{};
        if (!field(entry, layout.entry_state_offset, state)) return fail("ecs-scene-state-unreadable");
        if (state != layout.entry_ready_state) continue;
        if (++ready > 1) return fail("ecs-local-authority-is-ambiguous");
        view.scene_entry = entry;
        if (!field(entry, layout.entry_context_offset, view.scene_context) || !view.scene_context ||
            !field(entry, layout.entry_uuid_offset, view.scene_uuid) || !(view.scene_uuid[0] || view.scene_uuid[1]))
            return fail("ecs-ready-scene-context-invalid");
    }
    if ((count || layout.dedicated_server) && !ready) return fail("ecs-active-scene-is-transitioning");
    if (ready) {
        std::uint8_t state{};
        std::uintptr_t context{};
        std::array<std::uint64_t, 2> uuid{};
        if (!field(view.scene_entry, layout.entry_state_offset, state) || state != layout.entry_ready_state ||
            !field(view.scene_entry, layout.entry_context_offset, context) || context != view.scene_context ||
            !field(view.scene_entry, layout.entry_uuid_offset, uuid) || uuid != view.scene_uuid)
            return fail("ecs-ready-scene-context-changed");
        view.role = EngineContext::EcsRole::LocalAuthority;
    }
    std::uintptr_t entries_again{};
    std::uint64_t count_again{}, capacity_again{};
    if (!field(view.scene_owner, layout.scene_entries_offset, entries_again) || entries_again != view.scene_entries ||
        !field(view.scene_owner, layout.scene_count_offset, count_again) || count_again != count ||
        !field(view.scene_owner, layout.scene_capacity_offset, capacity_again) || capacity_again != capacity)
        return fail("ecs-active-scene-list-changed");
    return true;
}

bool resolve(const Layout& layout, EngineContext::Owners& output, std::string* reason, std::uintptr_t known_image){
    const auto fail = [&](const char* message) { if (reason) *reason = message; return false; };
    EngineContext::Owners view;
    view.image = known_image ? known_image : reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
    if (layout.dedicated_server) {
        std::uintptr_t registry_lock{};
        if (!view.image || !add(view.image, layout.write.registry_lock, registry_lock) || !try_shared(registry_lock))
            return fail("ecs-thread-registry-busy");
        RegisteredScene scene;
        const char* failure{};
        EngineContext::EcsView world;
        bool valid = resolve_registered_scene(layout, view.image, scene, &failure);
        if (valid) {
            world.scene_owner = scene.scene;
            valid = select_scene_context(layout, world, &failure);
        }
        // Registry ownership protects the Scene allocation during traversal.
        // Publish diagnostics only after releasing the original engine lock.
        release_shared(registry_lock);
        if (!valid) return fail(failure ? failure : "ecs-server-world-unavailable");
        view.dedicated_server = true;
        view.main_owner = scene.scene;
        view.world_context = world.scene_context;
        view.world_uuid = world.scene_uuid;
        output = view;
        if (reason) reason->clear();
        return true;
    }
    if (!view.image || !add(view.image, layout.root_rva, view.root) ||
        !vtable(view.root, layout.root_vtable, view.image, layout.image_size)) return fail("root-vtable-mismatch");
    std::uintptr_t field{}, back{};
    if (!add(view.root, layout.client_offset, field) || !read(field, view.client) ||
        !vtable(view.client, layout.client_vtable, view.image, layout.image_size)) return fail("client-owner-unavailable-or-vtable-mismatch");
    if (!add(view.client, layout.controller_offset, field) || !read(field, view.controller) ||
        !vtable(view.controller, layout.controller_vtable, view.image, layout.image_size)) return fail("client-controller-unavailable-or-vtable-mismatch");
    if (!add(view.controller, layout.controller_back_pointer, field) || !read(field, back) || back != view.client)
        return fail("client-controller-owner-mismatch");
    if (!add(view.controller, layout.main_owner_offset, field) || !read(field, view.main_owner) || !view.main_owner)
        return fail("main-owner-unavailable");
    output = view;
    if (reason) reason->clear();
    return true;
}

bool select_execution_owner(const Layout& layout, EngineContext::EcsView& view, std::string* reason, std::uintptr_t image){
    const auto fail = [&](const char* message) { if (reason) *reason = message; return false; };
    if (!resolve(layout, view.owners, reason, image)) return false;
    auto context = view.owners.main_owner;
    std::uintptr_t address{};
    if (layout.authority) {
        if (layout.dedicated_server) view.scene_owner = view.owners.main_owner;
        else if (!field(view.owners.client, layout.client_scene_offset, view.scene_owner) || !view.scene_owner)
            return fail("ecs-active-scene-list-unavailable");
        const char* failure{};
        if (!select_scene_context(layout, view, &failure)) return fail(failure);
        if (layout.dedicated_server && (view.scene_context != view.owners.world_context || view.scene_uuid != view.owners.world_uuid))
            return fail("ecs-server-world-owner-changed");
        if (view.role == EngineContext::EcsRole::LocalAuthority) context = view.scene_context;
    }
    if (!add(context, layout.execution_owner_offset, address) || !read(address, view.execution_owner) || !view.execution_owner)
        return fail("ecs-execution-owner-unavailable");
    return true;
}

bool resolve_ecs(const Layout& layout, EngineContext::EcsView& output, std::string* reason, std::uintptr_t image){
    const auto fail = [&](const char* message) { if (reason) *reason = message; return false; };
    EngineContext::EcsView view;
    std::uintptr_t manager_owner{}, table{};
    std::uint64_t count{};
    if (!select_execution_owner(layout, view, reason, image)) return false;
    if (!add(view.execution_owner, layout.registry_owner_offset, view.registry_owner) ||
        !add(view.execution_owner, layout.entity_manager_offset, view.manager) ||
        !field(view.manager, layout.manager_owner_offset, manager_owner) || manager_owner != view.registry_owner)
        return fail("ecs-manager-registry-owner-mismatch");
    if (!field(view.manager, layout.entities.manager_count, count) || !count || count > layout.entities.maximum_entities ||
        !field(view.manager, layout.entities.manager_table, table) || !table)
        return fail("ecs-entity-table-unavailable");
    if (table > UINTPTR_MAX - count * sizeof(std::uintptr_t)) return fail("ecs-entity-table-range-overflow");
    view.entity_table = table;
    EngineContext::EcsView again;
    std::uintptr_t owner_again{}, table_again{};
    std::uint64_t count_again{};
    if (!select_execution_owner(layout, again, reason, image) || again.owners != view.owners || again.role != view.role ||
        again.scene_owner != view.scene_owner || again.scene_entries != view.scene_entries || again.scene_entry != view.scene_entry ||
        again.scene_context != view.scene_context || again.scene_uuid != view.scene_uuid || again.execution_owner != view.execution_owner ||
        !field(view.manager, layout.manager_owner_offset, owner_again) || owner_again != view.registry_owner ||
        !field(view.manager, layout.entities.manager_count, count_again) || count_again != count ||
        !field(view.manager, layout.entities.manager_table, table_again) || table_again != table)
        return fail("ecs-engine-owner-changed-during-read");
    view.layout = layout.entities;
    view.entity_command_queue_layout = layout.entity_command_queue_layout;
    output = view;
    if (reason) reason->clear();
    return true;
}
}

namespace EngineContext {

void Shutdown() {
    std::scoped_lock lock(context_mutex);
    configured = false;
    active_layout = {};
    system_dependency_layout = nullptr;
    service_layout = nullptr;
}

nlohmann::json SystemDependencyLayout() {
    std::scoped_lock lock(context_mutex);
    return configured ? system_dependency_layout : nlohmann::json{};
}

nlohmann::json ServiceLayout() {
    std::scoped_lock lock(context_mutex);
    return configured ? service_layout : nlohmann::json{};
}

std::optional<BuildingPropSpawnLayout> ConfiguredBuildingPropSpawnLayout() {
    std::scoped_lock lock(context_mutex);
    return configured ? active_layout.building_prop_spawn_layout : std::nullopt;
}

const char* RoleName(EcsRole role) { return role == EcsRole::LocalAuthority ? "local-authority" : "client-replica"; }

bool ResolveOwners(Owners& owners, std::string* reason) {
    Layout layout;
    { std::scoped_lock lock(context_mutex); if (!configured) { if (reason) *reason = "engine-owner-profile-unavailable"; return false; } layout = active_layout; }
    return resolve(layout, owners, reason);
}

bool ResolveEcs(EcsView& output, std::string* reason) {
    Layout layout;
    { std::scoped_lock lock(context_mutex); if (!configured || !active_layout.ecs) { if (reason) *reason = "ecs-read-owner-profile-unavailable"; return false; } layout = active_layout; }
    return resolve_ecs(layout, output, reason);
}

bool ResolveClientEcs(EcsView& output, std::string* reason) {
    Layout layout;
    { std::scoped_lock lock(context_mutex); if (!configured || !active_layout.ecs) { if (reason) *reason = "ecs-read-owner-profile-unavailable"; return false; } layout = active_layout; }
    if (layout.dedicated_server) {
        if (reason) *reason = "dedicated-server-has-no-local-client-input";
        return false;
    }
    layout.authority = false;
    return resolve_ecs(layout, output, reason);
}

bool EcsWriteConfigured() {
    std::scoped_lock lock(context_mutex);
    return configured && active_layout.write.enabled;
}

bool ResourceWriteConfigured() {
    std::scoped_lock lock(context_mutex);
    return configured && active_layout.resource_write.enabled;
}
}
