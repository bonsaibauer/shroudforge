#include "ecs/resolve_entity_command_queue.h"
#include "named-services/named_service_contract.h"
#include <limits>

namespace EcsRuntime {
namespace {
using NamedServiceImplementation::add;
using NamedServiceImplementation::field;
using NamedServiceImplementation::read;

bool extent(std::uintptr_t base, std::uint64_t count, std::size_t stride) {
    return base && count && stride && count <= maximum_entity_command_queues &&
        count <= (UINTPTR_MAX - base) / stride;
}

bool image_vtable(std::uintptr_t address, std::uintptr_t image) {
    MEMORY_BASIC_INFORMATION memory{};
    std::array<std::uintptr_t, 2> entries{};
    if (!address || address % alignof(std::uintptr_t) || !image ||
        VirtualQuery(reinterpret_cast<const void*>(address), &memory, sizeof(memory)) != sizeof(memory) ||
        memory.State != MEM_COMMIT || memory.Type != MEM_IMAGE ||
        reinterpret_cast<std::uintptr_t>(memory.AllocationBase) != image ||
        (memory.Protect != PAGE_READONLY && memory.Protect != PAGE_EXECUTE_READ) ||
        !read(address, entries)) return false;
    return NamedServiceImplementation::executable_callback(entries[0]) &&
        NamedServiceImplementation::executable_callback(entries[1]);
}

bool header_valid(const EntityCommandQueueHeader& header, std::uintptr_t counter, std::uint64_t capacity) {
    return header.vtable && header.allocator && header.buffer && !(header.buffer & 0x3f) &&
        header.capacity == capacity &&
        header.buffer <= UINTPTR_MAX - header.capacity &&
        header.byte_cursor <= header.capacity &&
        ((!header.byte_cursor && !header.command_count) ||
         (header.byte_cursor >= 12 && header.command_count && header.command_count <= header.byte_cursor / 12)) &&
        !header.execution_mode && header.entity_id_counter == counter &&
        NamedServiceImplementation::writable(header.buffer, static_cast<std::size_t>(header.capacity));
}

bool snapshot(const NamedServiceImplementation::BindingContract& binding,
    const EngineContext::EcsView& owner, EntityCommandQueueSelection& result) {
    if (binding.key != "ecs::Tasks" || owner.role != EngineContext::EcsRole::LocalAuthority ||
        !owner.manager || !owner.execution_owner || !owner.registry_owner || !owner.owners.image ||
        !owner.entity_command_queue_layout) return false;
    const auto& layout = *owner.entity_command_queue_layout;
    // The admitted spawn adapter snapshots 160 bytes before and after dispatch.
    if (layout.task_record_stride < sizeof(EntityCommandQueueTaskLinks) || layout.buffer_capacity < 160 ||
        layout.buffer_capacity > std::numeric_limits<std::uint32_t>::max()) return false;
    std::uintptr_t registry{}, expected_state{}, counter{}, expected_tracker{};
    std::uint8_t mode{};
    std::uint64_t task_count{};
    EntityCommandQueueSelection current;
    current.manager = owner.manager;
    if (!field(owner.manager, layout.manager_owner, registry) || registry != owner.registry_owner ||
        !field(owner.execution_owner, layout.execution_mode, mode) || mode != layout.authority_mode ||
        !add(owner.manager, layout.task_state, expected_state) ||
        !add(owner.manager, layout.entity_id_counter, counter) ||
        !add(owner.manager, layout.shared_tracker, expected_tracker) ||
        !NamedServiceImplementation::resolve_binding(binding, owner, current.tasks_service) ||
        !read(current.tasks_service, current.task_state) || current.task_state != expected_state ||
        !field(owner.manager, layout.task_records, current.task_records) ||
        !field(owner.manager, layout.task_count, task_count) ||
        !field(owner.manager, layout.queue_records, current.queue_records) ||
        !field(owner.manager, layout.queue_count, current.queue_count) || task_count != current.queue_count ||
        !extent(current.task_records, task_count, layout.task_record_stride) ||
        !extent(current.queue_records, current.queue_count, sizeof(EntityCommandQueueHeader)) ||
        (current.task_records & 7) || (current.queue_records & 0x3f) ||
        !field(owner.manager, layout.active_tracking_contexts, current.active_tracking_contexts) ||
        current.active_tracking_contexts ||
        !NamedServiceImplementation::writable(counter, sizeof(std::uint32_t)) ||
        !NamedServiceImplementation::writable(current.task_records,
            static_cast<std::size_t>(task_count) * layout.task_record_stride) ||
        !NamedServiceImplementation::writable(current.queue_records,
            static_cast<std::size_t>(current.queue_count) * sizeof(EntityCommandQueueHeader))) return false;
    std::uintptr_t vtable{}, allocator{}, tracker{};
    bool selected{};
    for (std::size_t index = 0; index < current.queue_count; ++index) {
        const auto queue = current.queue_records + index * sizeof(EntityCommandQueueHeader);
        const auto task = current.task_records + index * layout.task_record_stride;
        auto& links = current.task_links[index];
        auto& header = current.headers[index];
        if (!read(task, links) || links.manager != owner.manager || links.queue != queue || links.tracker != expected_tracker ||
            !read(queue, header) || !header_valid(header, counter, layout.buffer_capacity)) return false;
        if (!index) {
            vtable = header.vtable; allocator = header.allocator; tracker = links.tracker;
            if (!image_vtable(vtable, owner.owners.image) || !tracker) return false;
        } else if (header.vtable != vtable || header.allocator != allocator || links.tracker != tracker) return false;
        // Original fixed-capacity buffers are independent allocations. Overlapping them
        // would make a command publication overwrite another worker's commands.
        for (std::size_t previous = 0; previous < index; ++previous) {
            const auto& other = current.headers[previous];
            if (header.buffer < other.buffer + other.capacity && other.buffer < header.buffer + header.capacity)
                return false;
        }
        if (!selected && !header.byte_cursor && !header.command_count && !header.change_tracker) {
            selected = true; current.selected_index = index;
        }
    }
    // The complete second read, not the sampled first header, establishes the
    // stable original bindings that the caller may use while retaining its gate.
    for (std::size_t index = 0; index < current.queue_count; ++index) {
        EntityCommandQueueHeader header;
        EntityCommandQueueTaskLinks links;
        if (!read(current.queue_records + index * sizeof(header), header) || header != current.headers[index] ||
            !read(current.task_records + index * layout.task_record_stride, links) || links != current.task_links[index]) return false;
    }
    std::uintptr_t service{}, state{}, tasks{}, queues{}, registry_again{};
    std::uint64_t tasks_again{}, queues_again{};
    std::uint32_t active{};
    std::uint8_t mode_again{};
    if (!NamedServiceImplementation::resolve_binding(binding, owner, service) || service != current.tasks_service ||
        !read(service, state) || state != current.task_state ||
        !field(owner.manager, layout.manager_owner, registry_again) || registry_again != registry ||
        !field(owner.execution_owner, layout.execution_mode, mode_again) || mode_again != mode ||
        !field(owner.manager, layout.task_records, tasks) || tasks != current.task_records ||
        !field(owner.manager, layout.task_count, tasks_again) || tasks_again != task_count ||
        !field(owner.manager, layout.queue_records, queues) || queues != current.queue_records ||
        !field(owner.manager, layout.queue_count, queues_again) || queues_again != current.queue_count ||
        !field(owner.manager, layout.active_tracking_contexts, active) || active != current.active_tracking_contexts)
        return false;
    current.queue_address = selected ? current.queue_records + current.selected_index * sizeof(current.header) : 0;
    current.task_record = selected ? current.task_records + current.selected_index * layout.task_record_stride : 0;
    if (selected) current.header = current.headers[current.selected_index];
    result = current;
    return true;
}
}

bool ResolveEntityCommandQueue(const NamedServiceImplementation::BindingContract& binding,
    const EngineContext::EcsView& owner, EntityCommandQueueSelection& output) {
    EntityCommandQueueSelection found;
    if (!snapshot(binding, owner, found) || !found.queue_address) return false;
    output = found;
    return true;
}

bool RereadEntityCommandQueue(const NamedServiceImplementation::BindingContract& binding,
    const EngineContext::EcsView& owner, const EntityCommandQueueSelection& expected,
    EntityCommandQueueSelection& output) {
    EntityCommandQueueSelection found;
    if (!expected.queue_address || expected.selected_index >= expected.queue_count ||
        expected.queue_count > maximum_entity_command_queues || !snapshot(binding, owner, found) ||
        found.manager != expected.manager || found.tasks_service != expected.tasks_service ||
        found.task_state != expected.task_state || found.task_records != expected.task_records ||
        found.queue_records != expected.queue_records || found.queue_count != expected.queue_count ||
        found.active_tracking_contexts != expected.active_tracking_contexts ||
        expected.header != expected.headers[expected.selected_index]) return false;
    for (std::size_t index = 0; index < found.queue_count; ++index) {
        if (found.task_links[index] != expected.task_links[index]) return false;
        auto comparable = found.headers[index];
        if (index == expected.selected_index) {
            if (comparable.byte_cursor < expected.header.byte_cursor ||
                comparable.command_count < expected.header.command_count) return false;
            comparable.byte_cursor = expected.header.byte_cursor;
            comparable.command_count = expected.header.command_count;
        }
        if (comparable != expected.headers[index]) return false;
    }
    found.selected_index = expected.selected_index;
    found.queue_address = found.queue_records + found.selected_index * sizeof(found.header);
    found.task_record = found.task_records + found.selected_index * owner.entity_command_queue_layout->task_record_stride;
    if (found.queue_address != expected.queue_address || found.task_record != expected.task_record) return false;
    found.header = found.headers[found.selected_index];
    output = found;
    return true;
}
}
