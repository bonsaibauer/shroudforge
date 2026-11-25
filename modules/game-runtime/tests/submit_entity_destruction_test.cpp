// Exercise the production adapter against owned queue/VM memory and controlled
// ownership boundaries. No process is opened and no game function is invoked.
#include "impact/submit_entity_destruction.cpp"
#include <cstdio>
#include <cstdlib>
#include <source_location>

namespace {
unsigned checks{};
void check(bool value, std::source_location location = std::source_location::current()) {
    ++checks;
    if (!value) {
        std::fprintf(stderr, "entity destruction check %u failed at line %u\n", checks, location.line());
        std::exit(1);
    }
}

enum class CallbackMode {
    Append, Reject, RejectAfterAppend, SuccessWithoutAppend, Exception,
    NullReturn, ResultDetail, ResultValue, PrefixGuard, SuffixGuard,
    Variable, Data, Index, Arguments, Context, Query, Iterator, Impact,
    CommandTarget, CommandFlags, HeaderMetadata, TrailingBuffer, SceneLost,
};

struct Fixture {
    EngineContext::EcsView view;
    EcsRuntime::EntityCommandQueueSelection selection;
    std::array<std::uint8_t, 64> buffer{};
    EcsRuntime::EntityCommandQueueHeader header;
    std::uint64_t session{77};
    std::uint32_t handle{9}, entity_id{0x10203040}, generation{6};
    bool owner_available{true}, scene_available{true}, scene_valid{true}, writes_allowed{true};
    bool registry_available{true}, queue_available{true}, handle_available{true};
    std::uint8_t operation_arity{1};
    unsigned fail_reread_call{}, fail_copy_call{}, throw_copy_call{}, change_identity_call{}, fail_identity_call{};
    unsigned binding_calls{}, session_calls{}, scene_calls{}, resolve_calls{}, reread_calls{}, identity_calls{};
    unsigned check_calls{};
    unsigned callback_calls{}, registry_calls{}, copy_calls{}, uncertain_calls{}, scene_releases{};
    bool session_held{}, scene_held{};
    CallbackMode mode{CallbackMode::Append};
    std::string uncertainty_reason;
};
Fixture fixture;

void reset() {
    check(!fixture.session_held && !fixture.scene_held);
    fixture = {};
    fixture.view.role = EngineContext::EcsRole::LocalAuthority;
    fixture.view.manager = 0x1000;
    fixture.view.registry_owner = 0x2000;
    fixture.buffer.fill(0x6d);
    fixture.header.vtable = 0x3000;
    fixture.header.allocator = 0x4000;
    fixture.header.buffer = reinterpret_cast<std::uintptr_t>(fixture.buffer.data());
    fixture.header.capacity = fixture.buffer.size();
    fixture.header.system_id = 0x1357;
    fixture.header.subtask_id = 0x2468;
    fixture.header.entity_id_counter = 0x5000;
    fixture.selection.manager = fixture.view.manager;
    fixture.selection.tasks_service = 0x6000;
    fixture.selection.task_state = 0x7000;
    fixture.selection.task_records = 0x8000;
    fixture.selection.queue_records = reinterpret_cast<std::uintptr_t>(&fixture.header);
    fixture.selection.queue_count = 1;
    fixture.selection.queue_address = fixture.selection.queue_records;
    fixture.selection.task_record = 0x8000;
    fixture.selection.header = fixture.header;
}

// These independently declared fixture layouts verify what the real adapter
// hands to the original callback, rather than reusing its private C++ types.
struct FixtureQuery { std::uintptr_t reserved{}, queue{}; };
struct FixtureIterator { const FixtureQuery* query{}; std::uint32_t index{}, reserved{}; };
struct FixtureImpact { const FixtureIterator* iterator{}; };
struct FixtureCommand {
    std::uint16_t length{};
    std::uint8_t type{}, flags{};
    std::uint16_t system{}, subtask{};
    std::uint32_t source{}, target{};
};
static_assert(sizeof(FixtureCommand) == 16);

void* __fastcall original_destroy_fixture(void* output, const void* argument_view, const void* vm_view) {
    ++fixture.callback_calls;
    check(fixture.session_held && fixture.scene_held && fixture.writes_allowed);
    check(fixture.registry_calls == 1 && fixture.resolve_calls == 1 && fixture.reread_calls == 1);
    check(fixture.identity_calls == 2);
    auto* result = static_cast<std::uint8_t*>(output);
    const auto* args = static_cast<const ImpactRuntime::OwnedVm::Arguments*>(argument_view);
    const auto* vm = static_cast<const ImpactRuntime::OwnedVm::Context*>(vm_view);
    check(args && args->count == 1 && args->indices && args->indices[0] == 1);
    check(vm && vm->variable_count == 2 && vm->variables && vm->data && vm->data_extent == 4);
    check(vm->variables[0] == ImpactRuntime::OwnedVm::Variable{});
    auto expected_variable = ImpactRuntime::OwnedVm::Variable{};
    expected_variable.size = 4;
    check(vm->variables[1] == expected_variable && vm->data[0] == fixture.entity_id);
    check(vm->reserved == std::array<std::uintptr_t, 2>{});
    check(vm->execution_context != nullptr);
    FixtureImpact impact;
    std::memcpy(&impact, vm->execution_context, sizeof(impact));
    check(impact.iterator != nullptr);
    FixtureIterator iterator;
    std::memcpy(&iterator, impact.iterator, sizeof(iterator));
    check(iterator.query != nullptr);
    FixtureQuery query;
    std::memcpy(&query, iterator.query, sizeof(query));
    check(iterator.index == 0 && iterator.reserved == 0 && query.reserved == 0);
    check(query.queue == reinterpret_cast<std::uintptr_t>(&fixture.header));
    check(fixture.header.byte_cursor == 0 && fixture.header.command_count == 0);

    if (fixture.mode == CallbackMode::Exception) RaiseException(0xe0000042, 0, 0, nullptr);
    const bool append = fixture.mode != CallbackMode::Reject && fixture.mode != CallbackMode::SuccessWithoutAppend;
    if (append) {
        FixtureCommand command{16, 1, 0, fixture.header.system_id, fixture.header.subtask_id, 0, vm->data[0]};
        if (fixture.mode == CallbackMode::CommandTarget) ++command.target;
        if (fixture.mode == CallbackMode::CommandFlags) ++command.flags;
        std::memcpy(fixture.buffer.data(), &command, sizeof(command));
        fixture.header.byte_cursor = 16;
        fixture.header.command_count = 1;
    }
    result[0] = fixture.mode == CallbackMode::Reject || fixture.mode == CallbackMode::RejectAfterAppend ? 1 : 0;
    const std::uint32_t detail = fixture.mode == CallbackMode::ResultDetail ? 9 : 0;
    std::memcpy(result + 4, &detail, sizeof(detail));
    result[8] = fixture.mode == CallbackMode::Reject ? 0 : 1;
    switch (fixture.mode) {
    case CallbackMode::NullReturn: return nullptr;
    case CallbackMode::ResultValue: result[8] = 2; break;
    case CallbackMode::PrefixGuard: result[-1] = 0; break;
    case CallbackMode::SuffixGuard: result[9] = 0; break;
    case CallbackMode::Variable: ++const_cast<ImpactRuntime::OwnedVm::Variable*>(vm->variables)[1].offset; break;
    case CallbackMode::Data: ++const_cast<std::uint32_t*>(vm->data)[0]; break;
    case CallbackMode::Index: ++const_cast<std::uint32_t*>(args->indices)[0]; break;
    case CallbackMode::Arguments: ++const_cast<ImpactRuntime::OwnedVm::Arguments*>(args)->count; break;
    case CallbackMode::Context: ++const_cast<ImpactRuntime::OwnedVm::Context*>(vm)->reserved[0]; break;
    case CallbackMode::Query:
        ++query.reserved;
        std::memcpy(const_cast<FixtureQuery*>(iterator.query), &query, sizeof(query));
        break;
    case CallbackMode::Iterator:
        ++iterator.index;
        std::memcpy(const_cast<FixtureIterator*>(impact.iterator), &iterator, sizeof(iterator));
        break;
    case CallbackMode::Impact:
        impact.iterator = nullptr;
        std::memcpy(const_cast<void*>(vm->execution_context), &impact, sizeof(impact));
        break;
    case CallbackMode::HeaderMetadata: ++fixture.header.system_id; break;
    case CallbackMode::TrailingBuffer: ++fixture.buffer[20]; break;
    case CallbackMode::SceneLost: fixture.scene_valid = false; break;
    default: break;
    }
    return result;
}

ImpactRuntime::DestructionResult submit() {
    const auto result = ImpactRuntime::SubmitEntityDestruction(fixture.session, fixture.handle);
    check(!fixture.session_held && !fixture.scene_held);
    return result;
}
void expect_no_callback(ImpactRuntime::DestructionResult result) {
    check(submit() == result && fixture.callback_calls == 0 && fixture.uncertain_calls == 0);
    check(fixture.header.byte_cursor == 0 && fixture.header.command_count == 0);
    check(std::all_of(fixture.buffer.begin(), fixture.buffer.end(), [](auto byte) { return byte == 0x6d; }));
}
}

namespace NamedServiceImplementation {
std::mutex operation_mutex;
BindingContract binding_contract(const char* name) {
    ++fixture.binding_calls;
    check(!fixture.session_held && !fixture.scene_held);
    check(name && (!std::strcmp(name, "impact::ImpactSystem") || !std::strcmp(name, "ecs::Tasks")));
    BindingContract result;
    result.key = name;
    return result;
}
bool copy(void* target, const void* source, std::size_t size) {
    ++fixture.copy_calls;
    check(fixture.scene_held && target && source && size == 32);
    if (fixture.copy_calls == fixture.throw_copy_call) throw std::runtime_error("private-copy-exception");
    if (fixture.copy_calls == fixture.fail_copy_call) return false;
    std::memcpy(target, source, size);
    return true;
}
}

namespace EngineContext {
struct EcsWriteGuard::State {};
EcsWriteGuard::EcsWriteGuard() = default;
EcsWriteGuard::~EcsWriteGuard() { Release(); }
bool EcsWriteGuard::TryAcquire(const EcsView& view, const char**) {
    ++fixture.scene_calls;
    check(!fixture.scene_held && view == fixture.view);
    if (!fixture.scene_available) return false;
    state_ = std::make_unique<State>();
    fixture.scene_held = true;
    return true;
}
bool EcsWriteGuard::Validate() const {
    check(fixture.scene_held && state_ != nullptr);
    return fixture.scene_valid;
}
void EcsWriteGuard::Release() noexcept {
    if (state_) {
        ++fixture.scene_releases;
        fixture.scene_held = false;
        state_.reset();
    }
}
bool EcsWriteGuard::ComponentSupported(std::uint16_t) const { return false; }
bool EcsWriteGuard::TrackChange(std::uintptr_t, std::uint16_t) const { return false; }
bool ResolveEcs(EcsView& view, std::string*) {
    check(!fixture.session_held && !fixture.scene_held);
    if (!fixture.owner_available) return false;
    view = fixture.view;
    return true;
}
}

namespace EcsRuntime {
struct SessionGuard::State { bool acquired{}; };
SessionGuard::SessionGuard() : state_(std::make_unique<State>()) {}
SessionGuard::~SessionGuard() {
    if (state_->acquired) {
        check(!fixture.scene_held && fixture.session_held);
        fixture.session_held = false;
    }
}
bool SessionGuard::TryAcquire(std::uint64_t expected) {
    ++fixture.session_calls;
    check(!fixture.session_held && !fixture.scene_held);
    if (expected != fixture.session) return false;
    fixture.session_held = state_->acquired = true;
    return true;
}
const EngineContext::EcsView& SessionGuard::View() const {
    check(state_->acquired && fixture.session_held);
    return fixture.view;
}
bool SessionGuard::WritesAllowed() const {
    check(state_->acquired && fixture.session_held);
    return fixture.writes_allowed;
}
bool SessionGuard::ResolveEntityIdentity(std::uint32_t handle, std::uint32_t& id, std::uint32_t& generation) const {
    ++fixture.identity_calls;
    check(state_->acquired && fixture.session_held && fixture.scene_held);
    if (!fixture.handle_available || handle != fixture.handle || fixture.identity_calls == fixture.fail_identity_call)
        return false;
    id = fixture.entity_id;
    generation = fixture.generation + (fixture.identity_calls >= fixture.change_identity_call && fixture.change_identity_call ? 1 : 0);
    return true;
}
void SessionGuard::MarkWriteUncertain(const char* reason) {
    ++fixture.uncertain_calls;
    check(state_->acquired && fixture.session_held && !fixture.scene_held);
    fixture.writes_allowed = false;
    fixture.uncertainty_reason = reason;
}
bool MarkSessionWriteUncertain(std::uint64_t expected, const char* reason) {
    check(!fixture.session_held && !fixture.scene_held && fixture.scene_releases == 1);
    std::unique_lock operation(NamedServiceImplementation::operation_mutex,std::try_to_lock);
    check(operation.owns_lock());
    if (!expected || expected != fixture.session || !reason) return false;
    ++fixture.uncertain_calls;
    fixture.writes_allowed = false;
    fixture.uncertainty_reason = reason;
    return true;
}
bool CheckEntityCommandQueuePages(const EngineContext::EcsView& view, EntityCommandQueueSelection& checked) {
    ++fixture.check_calls;
    // The page queries are made before SessionGuard and the Scene gate.
    check(!fixture.scene_held && !fixture.session_held && view == fixture.view);
    checked = fixture.selection;
    return true;
}
bool ResolveEntityCommandQueue(const NamedServiceImplementation::BindingContract& binding,
    const EngineContext::EcsView& view, const EntityCommandQueueSelection& checked,
    EntityCommandQueueSelection& output) {
    ++fixture.resolve_calls;
    check(fixture.scene_held && binding.key == "ecs::Tasks" && view == fixture.view);
    check(checked.queue_records == fixture.selection.queue_records);
    if (!fixture.queue_available) return false;
    output = fixture.selection;
    output.header = fixture.header;
    return true;
}
bool RereadEntityCommandQueue(const NamedServiceImplementation::BindingContract& binding,
    const EngineContext::EcsView& view, const EntityCommandQueueSelection& expected,
    EntityCommandQueueSelection& output) {
    ++fixture.reread_calls;
    check(fixture.scene_held && fixture.session_held && binding.key == "ecs::Tasks" && view == fixture.view);
    check(expected.queue_address == fixture.selection.queue_address && expected.header == fixture.selection.header);
    if (fixture.reread_calls == fixture.fail_reread_call) return false;
    output = expected;
    output.header = fixture.header;
    return true;
}
}

namespace ImpactRuntime {
ResolveResult ResolveRegisteredOperationPinned(const NamedServiceImplementation::BindingContract& binding,
    const EngineContext::EcsView& view, const char* name, RegisteredOperation& result) {
    ++fixture.registry_calls;
    check(fixture.scene_held && binding.key == "impact::ImpactSystem" && view == fixture.view);
    check(name && !std::strcmp(name, "destroyEntity"));
    if (!fixture.registry_available) return ResolveResult::RegistryUnavailable;
    result = {reinterpret_cast<std::uintptr_t>(original_destroy_fixture), fixture.operation_arity};
    return ResolveResult::Found;
}
}

int main() {
    using ImpactRuntime::DestructionResult;
    reset();
    check(ImpactRuntime::EntityDestructionAvailable());
    check(fixture.session_calls == 0 && fixture.callback_calls == 0 && fixture.identity_calls == 0);
    check(!fixture.scene_held && fixture.scene_releases == 1);
    reset();
    fixture.view.role = EngineContext::EcsRole::ClientReplica;
    check(!ImpactRuntime::EntityDestructionAvailable() && fixture.scene_calls == 0 && fixture.session_calls == 0);
    reset(); fixture.owner_available = false;
    check(!ImpactRuntime::EntityDestructionAvailable() && fixture.scene_calls == 0);
    reset(); fixture.operation_arity = 2;
    check(!ImpactRuntime::EntityDestructionAvailable() && fixture.resolve_calls == 0 && fixture.session_calls == 0);
    reset(); fixture.queue_available = false;
    check(!ImpactRuntime::EntityDestructionAvailable() && fixture.session_calls == 0);
    reset();
    {
        std::unique_lock held(NamedServiceImplementation::operation_mutex);
        check(!ImpactRuntime::EntityDestructionAvailable() && fixture.scene_calls == 0 && fixture.session_calls == 0);
    }

    reset();
    check(ImpactRuntime::SubmitEntityDestruction(0, fixture.handle) == DestructionResult::Rejected);
    check(ImpactRuntime::SubmitEntityDestruction(fixture.session, 0) == DestructionResult::Rejected);
    check(fixture.binding_calls == 0 && fixture.session_calls == 0);
    reset();
    check(ImpactRuntime::SubmitEntityDestruction(fixture.session + 1, fixture.handle) == DestructionResult::StaleSession);
    check(fixture.scene_calls == 0 && fixture.callback_calls == 0);
    reset(); fixture.writes_allowed = false;
    expect_no_callback(DestructionResult::Uncertain);
    check(fixture.scene_calls == 0);
    reset(); fixture.scene_available = false; expect_no_callback(DestructionResult::Unavailable);
    reset(); fixture.scene_valid = false; expect_no_callback(DestructionResult::Unavailable);
    reset(); fixture.registry_available = false; expect_no_callback(DestructionResult::Unavailable);
    reset(); fixture.operation_arity = 0; expect_no_callback(DestructionResult::Unavailable);
    reset(); fixture.operation_arity = 2; expect_no_callback(DestructionResult::Unavailable);
    reset(); fixture.queue_available = false; expect_no_callback(DestructionResult::Unavailable);
    reset(); fixture.handle_available = false; expect_no_callback(DestructionResult::Rejected);
    reset(); fixture.change_identity_call = 2; expect_no_callback(DestructionResult::Rejected);
    reset(); fixture.fail_identity_call = 2; expect_no_callback(DestructionResult::Rejected);
    reset(); fixture.fail_reread_call = 1; expect_no_callback(DestructionResult::Unavailable);
    reset(); fixture.fail_copy_call = 1; expect_no_callback(DestructionResult::Unavailable);

    reset();
    check(submit() == DestructionResult::Submitted);
    check(fixture.callback_calls == 1 && fixture.uncertain_calls == 0 && fixture.writes_allowed);
    check(fixture.header.byte_cursor == 16 && fixture.header.command_count == 1);
    FixtureCommand command;
    std::memcpy(&command, fixture.buffer.data(), sizeof(command));
    check(command.length == 16 && command.type == 1 && command.flags == 0 && command.source == 0);
    check(command.target == fixture.entity_id && command.system == 0x1357 && command.subtask == 0x2468);
    check(std::all_of(fixture.buffer.begin() + 16, fixture.buffer.end(), [](auto byte) { return byte == 0x6d; }));
    // Submission reports only the exact queued command. Identity is still
    // present. The adapter has not asserted that native deletion completed.
    check(fixture.handle_available && fixture.identity_calls == 3);

    reset(); fixture.mode = CallbackMode::Reject;
    check(submit() == DestructionResult::Rejected && fixture.callback_calls == 1 && fixture.uncertain_calls == 0);
    check(fixture.header.byte_cursor == 0 && fixture.header.command_count == 0);
    check(std::all_of(fixture.buffer.begin(), fixture.buffer.end(), [](auto byte) { return byte == 0x6d; }));

    for (const auto mode : {CallbackMode::RejectAfterAppend, CallbackMode::SuccessWithoutAppend,
        CallbackMode::Exception, CallbackMode::NullReturn, CallbackMode::ResultDetail, CallbackMode::ResultValue,
        CallbackMode::PrefixGuard, CallbackMode::SuffixGuard, CallbackMode::Variable, CallbackMode::Data,
        CallbackMode::Index, CallbackMode::Arguments, CallbackMode::Context, CallbackMode::Query,
        CallbackMode::Iterator, CallbackMode::Impact, CallbackMode::CommandTarget, CallbackMode::CommandFlags,
        CallbackMode::HeaderMetadata, CallbackMode::TrailingBuffer, CallbackMode::SceneLost}) {
        reset(); fixture.mode = mode;
        check(submit() == DestructionResult::Uncertain);
        check(fixture.callback_calls == 1 && fixture.uncertain_calls == 1 && !fixture.writes_allowed);
        check(fixture.uncertainty_reason == "registered-destroy-command-outcome-uncertain");
        const auto bytes = fixture.buffer;
        const auto header = fixture.header;
        check(submit() == DestructionResult::Uncertain && fixture.callback_calls == 1 && fixture.uncertain_calls == 1);
        check(fixture.buffer == bytes && fixture.header == header);
    }
    for (unsigned after_failure = 0; after_failure < 4; ++after_failure) {
        reset();
        if (after_failure == 0) fixture.fail_reread_call = 2;
        if (after_failure == 1) fixture.fail_copy_call = 2;
        if (after_failure == 2) fixture.fail_identity_call = 3;
        if (after_failure == 3) fixture.change_identity_call = 3;
        check(submit() == DestructionResult::Uncertain);
        check(fixture.callback_calls == 1 && fixture.uncertain_calls == 1 && !fixture.writes_allowed);
        check(fixture.header.byte_cursor == 16 && fixture.header.command_count == 1);
    }
    reset(); fixture.throw_copy_call = 1;
    expect_no_callback(DestructionResult::Unavailable);
    reset(); fixture.throw_copy_call = 2;
    check(submit() == DestructionResult::Uncertain);
    check(fixture.callback_calls == 1 && fixture.uncertain_calls == 1 && !fixture.writes_allowed);
    check(fixture.uncertainty_reason == "registered-destroy-command-exception");
    check(fixture.scene_releases == 1 && fixture.header.byte_cursor == 16 && fixture.header.command_count == 1);
    const auto preserved_buffer = fixture.buffer;
    const auto preserved_header = fixture.header;
    check(submit() == DestructionResult::Uncertain && fixture.callback_calls == 1 && fixture.uncertain_calls == 1);
    check(fixture.buffer == preserved_buffer && fixture.header == preserved_header);
    std::printf("entity destruction checks passed: %u\n", checks);
}
