#include "ecs/component_operation.h"
#include "ecs/ecs_session_state.h"

using namespace EcsImplementation;

namespace EcsImplementation {

void read_on_game_thread(void* opaque){
    std::scoped_lock operation_lock(operation_mutex);
    auto& operation = *static_cast<ReadOperation*>(opaque);
    ComponentType component{};
    ResolvedLayout layout{};
    EntityView entity{};
    std::uintptr_t address{};
    // read_bytes copies under SEH, so an unreadable page fails the copy: the
    // page is not queried first. In game, each VirtualQuery here took about
    // 20 ms and made every read cost 40 ms.
    if (!(operation.value && resolve_component(operation.name, component) &&
        component.size == operation.size && layout_snapshot(layout) &&
        entity_for_handle(operation.handle, layout, entity) &&
        component_address(entity, layout, component, address, false) &&
        read_bytes(address, operation.value, operation.size))) return;
    // A non-dispatched read may race an archetype move or an engine update.
    // Publish only equal payload copies belonging to the same current entity.
    std::vector<std::uint8_t> again(operation.size);
    EntityView current{};
    std::uintptr_t current_address{};
    operation.result = read_bytes(address, again.data(), again.size()) &&
        std::memcmp(operation.value, again.data(), operation.size) == 0 &&
        entity_for_handle(operation.handle, layout, current) && current == entity &&
        component_address(current, layout, component, current_address, false) && current_address == address;
}

void read_many_on_game_thread(void* opaque){
    std::scoped_lock operation_lock(operation_mutex);
    auto& operation = *static_cast<ReadManyOperation*>(opaque);
    operation.regions = {};
    ResolvedLayout layout{};
    EntityView entity{};
    if (!layout_snapshot(layout) || !entity_for_handle(operation.handle, layout, entity)) return;
    for (std::size_t index = 0; index < operation.names.size(); ++index) {
        auto& component = operation.components[index];
        if (!resolve_component(operation.names[index].c_str(), component) ||
            component.size != operation.sizes[index]) return;
    }
    std::size_t offset{};
    for (std::size_t index = 0; index < operation.names.size(); ++index) {
        const auto& component = operation.components[index];
        const auto size = operation.sizes[index];
        bool present{};
        // An unreadable presence mask cannot be reported as a missing
        // component: that might conceal another initialized attribute root.
        if (!component_presence(entity, layout, component, present)) return;
        if (present) {
            auto& address = operation.addresses[index];
            auto* output = operation.output.data() + offset;
            EntityView current{};
            std::uintptr_t current_address{};
            if (!component_address(entity, layout, component, address, true, &operation.regions) ||
                !read_bytes(address, output, size) ||
                !read_bytes(address, operation.verification.data(), size) ||
                std::memcmp(output, operation.verification.data(), size) != 0 ||
                !entity_for_handle(operation.handle, layout, current) || current != entity ||
                !component_address(current, layout, component, current_address, true, &operation.regions) || current_address != address) return;
            operation.present[index] = 1;
        }
        offset += size;
    }
    EntityView current{};
    if (!entity_for_handle(operation.handle, layout, current) || current != entity) return;
    // Preserve absence and address identity for every candidate through the
    // end of this owner operation. Individual equal copies do not establish
    // simultaneous values across different engine components.
    for (std::size_t index = 0; index < operation.names.size(); ++index) {
        bool present{};
        if (!component_presence(current, layout, operation.components[index], present) ||
            present != (operation.present[index] == 1)) return;
        if (present) {
            std::uintptr_t address{};
            if (!component_address(current, layout, operation.components[index], address, true, &operation.regions) ||
                address != operation.addresses[index]) return;
        }
    }
    operation.result = operation.regions.revalidate();
}

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
    if (!invoke_read_operation(read_on_game_thread, operation) || !operation->result) {
        operation_counters.read_failures.fetch_add(1, std::memory_order_relaxed);
        return false;
    }
    std::memcpy(value, operation->output.data(), size);
    operation_counters.read_successes.fetch_add(1, std::memory_order_relaxed);
    return true;
}

extern "C" bool __cdecl KfcRuntimeEcsReadMany(std::uint32_t handle, const char* const* names,
    const std::uint32_t* sizes, std::size_t count, std::uint8_t* buffer,
    std::size_t capacity, std::uint8_t* present) {
    operation_counters.read_batches.fetch_add(1, std::memory_order_relaxed);
    const auto fail = [] {
        operation_counters.read_batch_failures.fetch_add(1, std::memory_order_relaxed);
        return false;
    };
    try {
        if (!handle || !names || !sizes || !buffer || !present || !count ||
            count > EcsRuntime::MaximumReadManyComponents || !capacity ||
            capacity > EcsRuntime::MaximumReadManyBytes || !KfcRuntimeEcsReady()) return fail();
        auto operation = std::make_shared<ReadManyOperation>();
        operation->handle = handle;
        operation->names.reserve(count);
        operation->sizes.assign(sizes, sizes + count);
        std::size_t total{}, maximum_size{};
        for (std::size_t index = 0; index < count; ++index) {
            const auto size = operation->sizes[index];
            if (!names[index] || !size || size > EcsRuntime::MaximumReadManyBytes - total) return fail();
            const auto length = strnlen_s(names[index], 512);
            if (!length || length == 512) return fail();
            operation->names.emplace_back(names[index], length);
            total += size;
            maximum_size = (std::max)(maximum_size, static_cast<std::size_t>(size));
        }
        if (total != capacity) return fail();
        // No caller-owned pointers enter the dispatcher. A timed-out queued
        // operation can finish against its own names and buffers safely.
        operation->components.resize(count);
        operation->addresses.resize(count);
        operation->output.resize(total);
        operation->present.resize(count);
        operation->verification.resize(maximum_size);
        if (!invoke_read_operation(read_many_on_game_thread, operation) || !operation->result) return fail();
        std::memcpy(buffer, operation->output.data(), total);
        std::memcpy(present, operation->present.data(), count);
        const auto copied = std::count(operation->present.begin(), operation->present.end(), std::uint8_t{1});
        operation_counters.read_batch_successes.fetch_add(1, std::memory_order_relaxed);
        operation_counters.read_batch_components.fetch_add(static_cast<std::uint64_t>(copied), std::memory_order_relaxed);
        operation_counters.read_batch_absent.fetch_add(count - copied, std::memory_order_relaxed);
        return true;
    } catch (...) { return fail(); }
}
