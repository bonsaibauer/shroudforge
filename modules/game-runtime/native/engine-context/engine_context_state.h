#pragma once

#include "engine-context/engine_owners.h"
#include "engine-context/hold_resource_lifetime.h"
#include "engine-context/lock_scene_access.h"
#include "profiles/game_profile.h"
#include <windows.h>
#include <algorithm>
#include <cstring>
#include <limits>
#include <mutex>
#include <stdexcept>
#include <nlohmann/json.hpp>

// Private implementation contract. State is defined once, in the owning unit.

namespace EngineContextImplementation {
struct RegisteredScene {
    std::uintptr_t scene{}, thread{}, handle{};
    bool operator==(const RegisteredScene&) const = default;
};

struct WriteLayout {
    bool enabled{};
    std::uintptr_t registry_lock{}, registry_entries{}, registry_count{}, callback{};
    std::size_t capacity{}, stride{}, row_handle{}, row_name{}, row_length{};
    std::size_t inline_name{}, scene_thread{}, name_pointer{}, name_length{}, name_capacity{}, name_capacity_value{};
    std::size_t thread_handle{}, thread_context{}, thread_stop{}, thread_callback{};
    std::size_t scene_context{}, scene_lock{}, scene_state{}, execution_mode{}, maximum_bytes{};
    std::uint8_t running_state{}, authority_mode{};
    std::array<char, 64> name{};
    std::size_t name_size{};
    std::uintptr_t tracker{};
    std::size_t tracking_context{}, tracking_manager{}, tracking_queue_b{}, maximum_tracking_capacity{};
    std::size_t tracking_registry_a{}, tracking_registry_b{};
    std::array<std::uint8_t, 33> tracker_bytes{};
};

struct ResourceWriteLayout {
    bool enabled{};
    std::size_t service_thread{}, initialized{}, reader{}, payload_pointer{}, payload_size{};
    std::uint32_t initialized_value{};
    std::uintptr_t callback{};
    std::array<char, 64> name{};
    std::size_t name_size{};
};

struct Layout {
    bool dedicated_server{};
    std::size_t image_size{};
    std::uintptr_t root_rva{}, root_vtable{}, client_vtable{}, controller_vtable{};
    std::size_t client_offset{}, controller_offset{}, controller_back_pointer{}, main_owner_offset{};
    bool ecs{};
    std::size_t execution_owner_offset{}, registry_owner_offset{}, entity_manager_offset{}, manager_owner_offset{};
    bool authority{};
    std::size_t client_scene_offset{}, scene_entries_offset{}, scene_count_offset{}, scene_capacity_offset{};
    std::size_t entry_stride{}, entry_context_offset{}, entry_uuid_offset{}, entry_state_offset{}, maximum_scenes{};
    std::uint8_t entry_ready_state{};
    EngineContext::EcsLayout entities;
    WriteLayout write;
    std::optional<EngineContext::EntityCommandQueueLayout> entity_command_queue_layout;
    std::optional<EngineContext::BuildingPropSpawnLayout> building_prop_spawn_layout;
    ResourceWriteLayout resource_write;
};

extern std::mutex context_mutex;

extern Layout active_layout;

extern bool configured;

extern nlohmann::json system_dependency_layout;

extern nlohmann::json service_layout;

template<class T> bool read(std::uintptr_t address, T& value) {
    if (!address || address > UINTPTR_MAX - sizeof(value)) return false;
    __try { std::memcpy(&value, reinterpret_cast<const void*>(address), sizeof(value)); return true; }
    __except (EXCEPTION_EXECUTE_HANDLER) { return false; }
}

bool read_span(std::uintptr_t address, void* output, std::size_t size);

bool add(std::uintptr_t base, std::size_t offset, std::uintptr_t& address);

template<class T> bool field(std::uintptr_t base, std::size_t offset, T& value) {
    std::uintptr_t address{};
    return add(base, offset, address) && read(address, value);
}

bool untracked_component(std::uintptr_t owner, std::uint16_t index,
                         std::size_t mask_a, std::size_t mask_b);

bool try_shared(std::uintptr_t address);

bool try_exclusive(std::uintptr_t address);

void release_shared(std::uintptr_t address) noexcept;

void release_exclusive(std::uintptr_t address) noexcept;

bool tracking_call(std::uintptr_t function, const std::array<std::uint8_t, 33>& expected,
                   std::uintptr_t manager, std::uintptr_t context, std::uintptr_t entity, std::uint16_t index);

bool vtable(std::uintptr_t object, std::uintptr_t rva, std::uintptr_t image, std::size_t size);

// Caller owns the original thread-registry Shared gate. This borrows the one
// matching live Scene without allocation, a host mutex, or a Scene lock.
bool resolve_registered_scene(const Layout& layout, std::uintptr_t image,
    RegisteredScene& output, const char** reason = nullptr);

bool resolve(const Layout& layout, EngineContext::Owners& output, std::string* reason, std::uintptr_t known_image = 0);

bool select_execution_owner(const Layout& layout, EngineContext::EcsView& view, std::string* reason, std::uintptr_t image = 0);

bool resolve_ecs(const Layout& layout, EngineContext::EcsView& output, std::string* reason, std::uintptr_t image = 0);
}
