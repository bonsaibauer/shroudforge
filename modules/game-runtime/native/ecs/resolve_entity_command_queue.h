#pragma once

#include "engine-context/engine_owners.h"
#include <array>
#include <cstddef>
#include <cstdint>

namespace NamedServiceImplementation { struct BindingContract; }

namespace EcsRuntime {
// Original fixed-buffer EntityCommandQueue. This is a data ABI, not a table of
// invocable operations. Its metadata belongs to the original ECS scheduler.
struct EntityCommandQueueHeader {
    std::uintptr_t vtable{}, allocator{}, buffer{};
    std::uint64_t capacity{};
    std::uint32_t byte_cursor{}, command_count{};
    std::uint16_t system_id{}, subtask_id{};
    std::uint8_t execution_mode{};
    std::array<std::uint8_t, 3> reserved{};
    std::uintptr_t change_tracker{}, entity_id_counter{};
    bool operator==(const EntityCommandQueueHeader&) const = default;
};
static_assert(sizeof(EntityCommandQueueHeader) == 64);
static_assert(offsetof(EntityCommandQueueHeader, byte_cursor) == 0x20);
static_assert(offsetof(EntityCommandQueueHeader, system_id) == 0x28);
static_assert(offsetof(EntityCommandQueueHeader, change_tracker) == 0x30);
static_assert(offsetof(EntityCommandQueueHeader, entity_id_counter) == 0x38);

// A bounded inspection limit, not an assumed original worker count.
inline constexpr std::size_t maximum_entity_command_queues = 64;
struct EntityCommandQueueTaskLinks {
    std::uintptr_t manager{}, queue{}, tracker{};
    bool operator==(const EntityCommandQueueTaskLinks&) const = default;
};
struct EntityCommandQueueSelection {
    std::uintptr_t manager{}, tasks_service{}, task_state{}, task_records{}, queue_records{};
    std::uint64_t queue_count{};
    std::uint32_t active_tracking_contexts{};
    std::size_t selected_index{};
    std::uintptr_t queue_address{}, task_record{};
    EntityCommandQueueHeader header;
    std::array<EntityCommandQueueHeader, maximum_entity_command_queues> headers{};
    std::array<EntityCommandQueueTaskLinks, maximum_entity_command_queues> task_links{};
};

// The caller prepares the exact ecs::Tasks binding before taking SessionGuard
// and the original Scene/lifetime guards. They must remain held for these calls
// and every use of the returned borrowed addresses. No allocation, mutex
// acquisition, engine call or memory mutation occurs here. Output is unchanged
// on failure. The existing Scene-exclusive guard provides the system-job phase.
bool ResolveEntityCommandQueue(const NamedServiceImplementation::BindingContract& binding,
    const EngineContext::EcsView& owner, EntityCommandQueueSelection& output);

// Before SessionGuard and the Scene gate: the page queries of the snapshot
// (entity id counter, task and queue records, each queue buffer, the vtable)
// on the fixed regions alone, while the game's frame may run. A spawn or a
// destruction passes the result to the overload below, so that those queries
// (about 2.7 ms each in game) are not made while the Scene gate holds the frame.
bool CheckEntityCommandQueuePages(const EngineContext::EcsView& owner, EntityCommandQueueSelection& checked);

// Under the gates, as above, without querying again the pages that `checked`
// queried at the same address and size. Any other region is queried.
bool ResolveEntityCommandQueue(const NamedServiceImplementation::BindingContract& binding,
    const EngineContext::EcsView& owner, const EntityCommandQueueSelection& checked,
    EntityCommandQueueSelection& output);

// Revalidates the same selection, including all neighbouring original queues.
// The pages of the regions `expected` holds are not queried again.
// Only the selected queue's cursor/count may advance. Its other metadata and
// all owner/backreference/array fields must match expected. The caller compares
// the full header immediately before dispatch, then the precise expected
// command delta afterwards. This function does not establish append success.
bool RereadEntityCommandQueue(const NamedServiceImplementation::BindingContract& binding,
    const EngineContext::EcsView& owner, const EntityCommandQueueSelection& expected,
    EntityCommandQueueSelection& output);
}
