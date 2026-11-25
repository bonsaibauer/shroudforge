#include "impact/submit_entity_destruction.h"
#include "impact/resolve_registered_operation.h"
#include "impact/owned_operation_arguments.h"
#include "ecs/resolve_entity_command_queue.h"
#include "named-services/named_service_contract.h"

namespace ImpactRuntime {
namespace {
using NamedServiceImplementation::copy;
using EcsRuntime::EntityCommandQueueSelection;

// destroyEntity's complete transitive read set uses only QueryContext.queue
// and QueryIterator.current_index. Index zero is the original source-id-zero
// branch and never reads an entity list. These are owned objects, not a borrowed
// scheduler frame. No arbitrary system entry or queue method is invoked.
struct QueryContext {
    std::uintptr_t reserved{}, queue{};
    bool operator==(const QueryContext&) const = default;
};
struct QueryIterator {
    const QueryContext* context{};
    std::uint32_t current_index{}, reserved{};
    bool operator==(const QueryIterator&) const = default;
};
struct ImpactContext {
    const QueryIterator* iterator{};
    bool operator==(const ImpactContext&) const = default;
};
struct DestroyCommand {
    std::uint16_t length{};
    std::uint8_t type{}, flags{};
    std::uint16_t system_id{}, subtask_id{};
    std::uint32_t source_id{}, target_id{};
    bool operator==(const DestroyCommand&) const = default;
};
static_assert(sizeof(QueryContext) == 16 && offsetof(QueryContext, queue) == 8);
static_assert(sizeof(QueryIterator) == 16 && offsetof(QueryIterator, current_index) == 8);
static_assert(sizeof(DestroyCommand) == 16 && offsetof(DestroyCommand, target_id) == 12);

struct Bindings {
    NamedServiceImplementation::BindingContract impact =
        NamedServiceImplementation::binding_contract("impact::ImpactSystem");
    NamedServiceImplementation::BindingContract tasks =
        NamedServiceImplementation::binding_contract("ecs::Tasks");
};

bool operation(const Bindings& bindings, const EngineContext::EcsView& view,
    RegisteredOperation& found) {
    return ResolveRegisteredOperationPinned(bindings.impact, view, "destroyEntity", found) ==
        ResolveResult::Found && found.arguments == 1;
}

bool untouched_result(const std::array<std::uint8_t, 44>& storage) {
    for (std::size_t i = 0; i < storage.size(); ++i) {
        const bool written = i == 16 || (i >= 20 && i < 24) || i == 24;
        if (!written && storage[i] != 0xa5) return false;
    }
    return true;
}
}

bool EntityDestructionAvailable() {
    try {
        const Bindings bindings;
        EngineContext::EcsView view;
        if (!EngineContext::ResolveEcs(view) || view.role != EngineContext::EcsRole::LocalAuthority)
            return false;
        // Page queries before the Scene gate, which holds the game's frame.
        EntityCommandQueueSelection checked;
        if (!EcsRuntime::CheckEntityCommandQueuePages(view, checked)) return false;
        // Availability can be called by an ECS status reader holding its state
        // mutex. Never wait for another operation or acquire SessionGuard here.
        std::unique_lock operation_lock(NamedServiceImplementation::operation_mutex, std::try_to_lock);
        if (!operation_lock.owns_lock()) return false;
        EngineContext::EcsWriteGuard scene;
        if (!scene.TryAcquire(view) || !scene.Validate()) return false;
        RegisteredOperation found;
        EntityCommandQueueSelection queue;
        return operation(bindings, view, found) &&
            EcsRuntime::ResolveEntityCommandQueue(bindings.tasks, view, checked, queue) && scene.Validate();
    } catch (...) { return false; }
}

DestructionResult SubmitEntityDestruction(std::uint64_t expected_session,
    std::uint32_t entity_handle) {
    if (!expected_session || !entity_handle) return DestructionResult::Rejected;
    bool invocation_started{};
    try {
        const Bindings bindings;
        // Page queries before the Scene gate, which holds the game's frame.
        EngineContext::EcsView checked_view;
        EntityCommandQueueSelection checked;
        if (!EngineContext::ResolveEcs(checked_view) ||
            !EcsRuntime::CheckEntityCommandQueuePages(checked_view, checked))
            return DestructionResult::Unavailable;
        std::scoped_lock operation_lock(NamedServiceImplementation::operation_mutex);
        EcsRuntime::SessionGuard session;
        if (!session.TryAcquire(expected_session)) return DestructionResult::StaleSession;
        if (!session.WritesAllowed()) return DestructionResult::Uncertain;
        EngineContext::EcsWriteGuard scene;
        if (!scene.TryAcquire(session.View()) || !scene.Validate()) return DestructionResult::Unavailable;
        RegisteredOperation found;
        EntityCommandQueueSelection queue, repeated;
        std::uint32_t id{}, generation{};
        if (!operation(bindings, session.View(), found) ||
            !EcsRuntime::ResolveEntityCommandQueue(bindings.tasks, session.View(), checked, queue))
            return DestructionResult::Unavailable;
        if (!session.ResolveEntityIdentity(entity_handle, id, generation)) return DestructionResult::Rejected;

        std::array<OwnedVm::Variable, 2> variables{};
        variables[1].size = sizeof(id);
        std::array<std::uint32_t, 1> data{id}, indices{1};
        OwnedVm::Arguments arguments{indices.data(), indices.size()};
        QueryContext query{0, queue.queue_address};
        QueryIterator iterator{&query, 0, 0};
        ImpactContext impact{&iterator};
        OwnedVm::Context context{variables.data(), variables.size(), data.data(), sizeof(data), {}, &impact};
        const auto expected_variables = variables;
        const auto expected_data = data;
        const auto expected_indices = indices;
        const auto expected_arguments = arguments;
        const auto expected_context = context;
        const auto expected_query = query;
        const auto expected_iterator = iterator;
        const auto expected_impact = impact;
        alignas(8) std::array<std::uint8_t, 44> storage;
        storage.fill(0xa5);
        auto* result = storage.data() + 16;
        std::array<std::uint8_t, 32> before{}, after{};
        if (!copy(before.data(), reinterpret_cast<const void*>(queue.header.buffer), before.size()) ||
            !EcsRuntime::RereadEntityCommandQueue(bindings.tasks, session.View(), queue, repeated) ||
            repeated.header != queue.header || !scene.Validate()) return DestructionResult::Unavailable;
        std::uint32_t current_id{}, current_generation{};
        if (!session.ResolveEntityIdentity(entity_handle, current_id, current_generation) ||
            current_id != id || current_generation != generation) return DestructionResult::Rejected;

        void* returned{};
        invocation_started = true;
        const bool invoked = OwnedVm::Invoke(found.callback, result, &arguments, &context, returned);
        const bool stable = scene.Validate() &&
            EcsRuntime::RereadEntityCommandQueue(bindings.tasks, session.View(), queue, repeated) &&
            copy(after.data(), reinterpret_cast<const void*>(queue.header.buffer), after.size()) &&
            session.ResolveEntityIdentity(entity_handle, current_id, current_generation) &&
            current_id == id && current_generation == generation;
        std::uint32_t detail{};
        std::memcpy(&detail, result + 4, sizeof(detail));
        const bool owned_intact = invoked && returned == result && !detail && untouched_result(storage) &&
            variables == expected_variables && data == expected_data && indices == expected_indices &&
            arguments == expected_arguments && context == expected_context && query == expected_query &&
            iterator == expected_iterator && impact == expected_impact;
        if (stable && owned_intact && result[0] == 1 && result[8] == 0 &&
            repeated.header == queue.header && before == after) return DestructionResult::Rejected;

        DestroyCommand appended;
        std::memcpy(&appended, after.data(), sizeof(appended));
        const DestroyCommand expected{16, 1, 0, queue.header.system_id, queue.header.subtask_id, 0, id};
        auto expected_header = queue.header;
        expected_header.byte_cursor = 16;
        expected_header.command_count = 1;
        if (stable && owned_intact && !result[0] && result[8] == 1 &&
            repeated.header == expected_header && appended == expected &&
            std::equal(before.begin() + sizeof(appended), before.end(), after.begin() + sizeof(appended)))
            return DestructionResult::Submitted;

        // An original callback may already have published a command. Preserve
        // the queue and stop all further writes for this session. Never rollback
        // or retry an uncertain original-engine operation.
        scene.Release();
        session.MarkWriteUncertain("registered-destroy-command-outcome-uncertain");
        return DestructionResult::Uncertain;
    } catch (...) {
        if (!invocation_started) return DestructionResult::Unavailable;
        // Guards have unwound. An exception after entering original code must
        // never permit a fallback or retry of a possibly published command.
        EcsRuntime::MarkSessionWriteUncertain(expected_session,
            "registered-destroy-command-exception");
        return DestructionResult::Uncertain;
    }
}
}
