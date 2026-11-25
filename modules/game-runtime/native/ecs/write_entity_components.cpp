#include "ecs/component_operation.h"
#include "ecs/ecs_session_state.h"

namespace EcsImplementation {
static std::shared_ptr<WriteOperation> prepare_component_write_buffers(
    std::uint32_t handle, const char* name, const void* mask, const void* value,
    std::size_t size, const void* expected = nullptr) {
    auto operation = std::make_shared<WriteOperation>();
    operation->handle = handle;
    operation->owned_name = name;
    operation->name = operation->owned_name.c_str();
    const auto first = static_cast<const std::uint8_t*>(mask);
    const auto last = static_cast<const std::uint8_t*>(value);
    operation->mask_bytes.assign(first, first + size);
    operation->destination.assign(last, last + size);
    if (expected) {
        const auto* bytes = static_cast<const std::uint8_t*>(expected);
        operation->expected.assign(bytes, bytes + size);
    }
    operation->mask = operation->mask_bytes.data();
    operation->value = operation->destination.data();
    operation->size = size;
    operation->before.resize(size);
    operation->merged.resize(size);
    operation->verified.resize(size);
    return operation;
}

// Called with operation_mutex held, before any original engine gate. All maps,
// buffers and host mutexes are prepared here, outside the short memory scope.
bool prepare_write(WriteOperation& operation){
    if (stop_requested.load(std::memory_order_acquire) || !operation.mask || !operation.value ||
        operation.before.size() != operation.size || operation.merged.size() != operation.size ||
        operation.verified.size() != operation.size || !resolve_component(operation.name, operation.component) ||
        operation.component.size != operation.size || !layout_snapshot(operation.layout)) return false;
    std::scoped_lock lock(state_mutex);
    if (write_fault_active_locked()) {
        operation.blocked_by_write_fault = true;
        return false;
    }
    const auto found = handles.find(operation.handle);
    if (found == handles.end() || found->second.epoch != layout_epoch) return false;
    operation.record = found->second;
    operation.epoch = layout_epoch;
    operation.direct = read_only_binding;
    if (operation.direct) {
        if (!component_registry_ready || !component_registry_world_bound ||
            component_registry.owner != read_only_view.registry_owner || !operation.record.table_slot) return false;
        operation.owner = read_only_view;
    }
    return true;
}

// No allocation, mutex acquisition or task dispatch in this body. The guarded
// path records changes through the validated native tracker before mutation.
// Direct callers own both original gates. Legacy callers own a dispatcher phase.
void write_payload(WriteOperation& operation, const EngineContext::EcsWriteGuard* guard){
    if (stop_requested.load(std::memory_order_acquire) || (guard && !guard->Validate())) return;
    const auto& layout = operation.layout;
    const auto& record = operation.record;
    if (operation.direct) {
        std::uint64_t count{};
        std::uintptr_t table{}, pointer{};
        if (!read(layout.count_address, count) || !count || count > (1u << 20) ||
            !read(layout.table_address, table) || table != operation.owner.entity_table ||
            table > UINTPTR_MAX - count * sizeof(pointer) || record.table_slot < table ||
            record.table_slot >= table + count * sizeof(pointer) || (record.table_slot - table) % sizeof(pointer) ||
            !read(record.table_slot, pointer) || pointer != record.pointer) return;
    }
    EntityView entity{};
    std::uintptr_t address{};
    if (!entity_view(record.pointer, layout, entity) || entity.id != record.id || entity.generation != record.generation ||
        !component_address(entity, layout, operation.component, address, false) ||
        !read_bytes(address, operation.before.data(), operation.size)) return;
    if (!operation.expected.empty() && operation.before != operation.expected) {
        operation.conflict = true;
        return;
    }
    std::copy(operation.before.begin(), operation.before.end(), operation.merged.begin());
    const auto* mask = static_cast<const std::uint8_t*>(operation.mask);
    const auto* value = static_cast<const std::uint8_t*>(operation.value);
    for (std::size_t index = 0; index < operation.size; ++index)
        if (mask[index]) operation.merged[index] = value[index];
    if (guard && !guard->ComponentSupported(operation.component.index)) return;
    if (operation.merged == operation.before) { operation.result = true; return; }
    if (guard && !guard->TrackChange(entity.pointer, operation.component.index)) return;
    operation.mutation_attempted = true;
    if (!guarded_copy(reinterpret_cast<void*>(address), operation.merged.data(), operation.size) ||
        !read_bytes(address, operation.verified.data(), operation.size) || operation.verified != operation.merged) {
        guarded_copy(reinterpret_cast<void*>(address), operation.before.data(), operation.size);
        // A failed copy can leave a partial mutation. Only a complete equal
        // readback establishes rollback, even when the restore copy faulted on
        // a read-only page that already contains the original payload.
        operation.uncertain = !read_bytes(address, operation.verified.data(), operation.size) ||
            operation.verified != operation.before;
        return;
    }
    operation.result = true;
}

void write_on_game_thread(void* opaque){
    std::scoped_lock operation_lock(operation_mutex);
    auto& operation = *static_cast<WriteOperation*>(opaque);
    if (!prepare_write(operation) || operation.direct) return;
    std::scoped_lock transaction(write_mutex);
    write_payload(operation);
    if (operation.uncertain) {
        std::scoped_lock lock(state_mutex);
        if (layout_epoch == operation.epoch) {
            write_fault_epoch = layout_epoch;
            write_fault_reason = "ecs-write-outcome-uncertain-rollback-unverified";
        }
    }
}

bool invoke_write_operation(const std::shared_ptr<WriteOperation>& operation){
    bool direct{};
    { std::scoped_lock lock(state_mutex); direct = read_only_binding; }
    if (!direct) {
        const bool completed = GameThreadDispatcher::Invoke(write_on_game_thread, operation);
        if (completed && operation->uncertain)
            last_write_reason.store("ecs-write-outcome-uncertain-rollback-unverified", std::memory_order_relaxed);
        return completed;
    }
    std::scoped_lock operation_lock(operation_mutex);
    if (!prepare_write(*operation) || !operation->direct) {
        last_write_reason.store(operation->blocked_by_write_fault ?
            "ecs-writes-disabled-after-uncertain-rollback" : "ecs-write-handle-or-layout-invalid", std::memory_order_relaxed);
        return false;
    }
    std::scoped_lock transaction(write_mutex);
    EcsRuntime::SessionGuard session;
    if (!session.TryAcquire(operation->epoch + 1) || session.View() != operation->owner || !session.WritesAllowed()) {
        last_write_reason.store("ecs-write-session-changed-or-disabled", std::memory_order_relaxed);
        return false;
    }
    EngineContext::EcsWriteGuard guard;
    const char* reason{};
    if (!guard.TryAcquire(operation->owner, &reason)) {
        last_write_reason.store(reason, std::memory_order_relaxed);
        return false;
    }
    if (operation->size > guard.MaximumBytes()) {
        guard.Release();
        last_write_reason.store("ecs-write-size-exceeds-gate-budget", std::memory_order_relaxed);
        return false;
    }
    write_payload(*operation, &guard);
    if (!guard.Validate()) {
        operation->result = false;
        operation->uncertain = operation->uncertain || operation->mutation_attempted;
    }
    if (operation->uncertain) {
        session.MarkWriteUncertain("ecs-write-outcome-uncertain-rollback-unverified");
    }
    guard.Release();
    last_write_reason.store(operation->uncertain ? "ecs-write-outcome-uncertain-rollback-unverified" :
        operation->result ? "success" : operation->conflict ? "compare-conflict" :
        "ecs-write-identity-or-memory-check-failed", std::memory_order_relaxed);
    return true;
}
}

using namespace EcsImplementation;

extern "C" bool __cdecl KfcRuntimeEcsCanWrite() {
    std::scoped_lock lock(state_mutex);
    if (!layout_ready || types.empty() || write_fault_active_locked()) return false;
    if (!read_only_binding) return GameThreadDispatcher::Ready() &&
        (!component_registry_ready || component_registry_world_bound);
    EngineContext::EcsView current;
    return component_registry_ready && component_registry_world_bound &&
        read_only_view.role == EngineContext::EcsRole::LocalAuthority && EngineContext::EcsWriteConfigured() &&
        EngineContext::ResolveEcs(current) && current == read_only_view;
}

extern "C" bool __cdecl KfcRuntimeEcsCanWriteComponent(const char* name) {
    std::scoped_lock operation_lock(operation_mutex);
    ComponentType component{};
    if (!KfcRuntimeEcsCanWrite() || !resolve_component(name, component)) return false;
    EngineContext::EcsView owner;
    {
        std::scoped_lock lock(state_mutex);
        if (!read_only_binding) return true;
        owner = read_only_view;
    }
    return EngineContext::EcsComponentWriteSupported(owner, component.index);
}

extern "C" bool __cdecl KfcRuntimeEcsWrite(std::uint32_t handle, const char* name,
                                            const void* mask, const void* value,
                                            std::size_t size) {
    operation_counters.writes.fetch_add(1, std::memory_order_relaxed);
    if (!name || !mask || !value || !size || size > (1u << 20) || !KfcRuntimeEcsCanWrite()) {
        operation_counters.write_failures.fetch_add(1, std::memory_order_relaxed);
        return false;
    }
    auto operation = prepare_component_write_buffers(handle, name, mask, value, size);
    const bool written = invoke_write_operation(operation) && operation->result;
    if (written) operation_counters.write_successes.fetch_add(1, std::memory_order_relaxed);
    else operation_counters.write_failures.fetch_add(1, std::memory_order_relaxed);
    return written;
}

extern "C" std::uint32_t __cdecl KfcRuntimeEcsCompareExchange(std::uint32_t handle, const char* name,
    const void* expected, const void* mask, const void* value, std::size_t size) {
    if (!name || !expected || !mask || !value || !size || size > (1u << 20) || !KfcRuntimeEcsCanWrite()) return 0;
    auto operation = prepare_component_write_buffers(handle, name, mask, value, size, expected);
    if (!invoke_write_operation(operation)) return 0;
    return operation->uncertain ? 3 : operation->result ? 1 : operation->conflict ? 2 : 0;
}

namespace EcsImplementation {
std::atomic<const char*> last_write_reason{"not-attempted"};
}
