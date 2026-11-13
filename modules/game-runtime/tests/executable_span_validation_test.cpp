#include "host/executable_span_validation.h"
#include <array>
#include <atomic>
#include <condition_variable>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <mutex>
#include <string>
#include <thread>

namespace {
constexpr std::uint32_t Base = 0x1000;
std::array<std::uint8_t, 512> code{};
std::atomic<bool> registration_allowed{true};

bool ReadCode(std::uint32_t rva, std::uint8_t* output, std::size_t size) noexcept {
    if (!output || rva < Base || size > code.size() || rva - Base > code.size() - size) return false;
    std::memcpy(output, code.data() + (rva - Base), size);
    return true;
}
bool RegistrationAllowed() noexcept { return registration_allowed.load(); }
bool CallbackExecutable(SFExecutableSpanValidationCallbackV1 callback) noexcept { return callback != nullptr; }

struct Check {
    int failures{};
    void ThatAt(int line, bool ok) {
        if (!ok) { ++failures; std::fprintf(stderr, "span validation assertion failed at line %d\n", line); }
    }
} check;
#define That(expression) ThatAt(__LINE__, (expression))
struct CallbackState {
    ExecutableSpanValidation::Registry* registry{};
    std::string package;
    std::uint64_t generation{};
    std::atomic<unsigned> calls{};
    std::atomic<std::uint32_t> phase{};
    std::atomic<std::uint32_t> result{SF_EXECUTABLE_SPAN_VALIDATION_VERIFIED_OWNER};
    std::atomic<bool> malformed{};
    std::atomic<bool> refuse{};
    std::atomic<bool> mutate{};
    std::mutex mutex;
    std::condition_variable changed;
    bool block{}, entered{}, release{};
};

int32_t SF_EXECUTABLE_SPAN_VALIDATION_CALL Validate(
    void* opaque, const SFExecutableSpanValidationRequestV1* request,
    SFExecutableSpanValidationResponseV1* response) {
    auto& state = *static_cast<CallbackState*>(opaque);
    ++state.calls;
    state.phase = request->phase;
    if (state.block) {
        std::unique_lock lock(state.mutex);
        state.entered = true;
        state.changed.notify_all();
        state.changed.wait(lock, [&] { return state.release; });
    }
    if (state.mutate) code[0] ^= 0x40;
    if (state.refuse) return 9;
    if (state.malformed) {
        response->size = sizeof(*response);
        response->version = 1;
        response->result = SF_EXECUTABLE_SPAN_VALIDATION_VERIFIED_OWNER;
        response->reserved = 1;
        response->owner_identity[0] = 1;
        return 0;
    }
    response->size = sizeof(*response);
    response->version = 1;
    response->result = state.result;
    response->reserved = 0;
    std::memset(response->owner_identity, 0, sizeof(response->owner_identity));
    if (state.result == SF_EXECUTABLE_SPAN_VALIDATION_VERIFIED_OWNER) response->owner_identity[0] = 0xA7;
    return 0;
}

SFExecutableSpanRegistrationV1 registration(std::uint32_t rva, std::uint32_t count,
        CallbackState& state) {
    SFExecutableSpanRegistrationV1 value{};
    value.size = sizeof(value); value.version = 1; value.rva = rva; value.byte_count = count;
    std::memcpy(value.expected, code.data() + (rva - Base), count);
    value.callback = Validate; value.callback_context = &state;
    return value;
}

bool Register(ExecutableSpanValidation::Registry& registry, const char* package,
        std::uint64_t generation, std::uint32_t rva, std::uint32_t count,
        CallbackState& state) {
    SFExecutableSpanValidationServiceV1 service{};
    if (registry.Query(package, generation, 1, sizeof(service), &service) != 0) return false;
    auto value = registration(rva, count, state);
    return service.register_span(service.context, &value) == 0;
}

ExecutableSpanValidation::Registry MakeRegistry() {
    return ExecutableSpanValidation::Registry(ReadCode, RegistrationAllowed, CallbackExecutable);
}

void abi_query_and_registration() {
    code.fill(0);
    auto registry = MakeRegistry();
    SFExecutableSpanValidationServiceV1 service{};
    service.size = 0xA5A5A5A5; service.version = 0x5A5A5A5A;
    service.context = reinterpret_cast<void*>(std::uintptr_t(0x1234));
    const auto before = service;
    check.That(registry.Query("owner", 0, 1, sizeof(service), &service) != 0);
    check.That(std::memcmp(&service, &before, sizeof(service)) == 0);
    check.That(registry.Query("owner", 1, 2, sizeof(service), &service) != 0);
    check.That(std::memcmp(&service, &before, sizeof(service)) == 0);

    CallbackState state{};
    code[0] = 0x11; code[1] = 0x22;
    registration_allowed = false;
    check.That(!Register(registry, "owner", 1, Base, 2, state));
    registration_allowed = true;
    check.That(Register(registry, "owner", 1, Base, 2, state));
    check.That(!Register(registry, "owner", 1, Base, 2, state));
    check.That(!Register(registry, "second", 2, Base + 1, 1, state));
}

void normalizes_only_registered_prefix() {
    code.fill(0);
    code[0] = 0x11; code[1] = 0x22; code[2] = 0x33; code[3] = 0x44;
    auto registry = MakeRegistry();
    CallbackState state{};
    check.That(Register(registry, "owner", 3, Base, 2, state));
    const std::uint8_t expected[]{0x11,0x22,0x33,0x44};
    code[0] = 0x90; code[1] = 0x90;
    const std::uint8_t observed[]{0x90,0x90,0x33,0x44};
    std::uint8_t identity[32]{};
    check.That(registry.ValidateGuard(Base, expected, observed, sizeof(observed),
        SF_EXECUTABLE_SPAN_PHASE_NORMALIZE, identity));
    check.That(identity[0] == 0xA7 && state.calls == 1);
    std::array<std::uint8_t, sizeof(observed)> suffix_changed{};
    std::memcpy(suffix_changed.data(), observed, sizeof(observed)); suffix_changed[3] ^= 1;
    check.That(!registry.ValidateGuard(Base, expected, suffix_changed.data(), sizeof(suffix_changed),
        SF_EXECUTABLE_SPAN_PHASE_NORMALIZE, identity));
    check.That(state.calls == 1);
}

void phases_reject_malformed_unchanged_and_toctou() {
    code.fill(0);
    code[0] = 0x11; code[1] = 0x22;
    auto registry = MakeRegistry();
    CallbackState state{};
    check.That(Register(registry, "owner", 4, Base, 2, state));
    const std::uint8_t expected[]{0x11,0x22};
    std::uint8_t identity_before[32]{}, identity_after[32]{};
    check.That(registry.ValidateGuard(Base, expected, expected, 2,
        SF_EXECUTABLE_SPAN_PHASE_BEFORE_ORIGINAL, identity_before));
    check.That(registry.ValidateGuard(Base, expected, expected, 2,
        SF_EXECUTABLE_SPAN_PHASE_AFTER_ORIGINAL, identity_after));
    check.That(state.calls == 2 && identity_before[0] == identity_after[0]);

    state.result = SF_EXECUTABLE_SPAN_VALIDATION_UNCHANGED;
    std::memset(identity_before, 0xA5, sizeof(identity_before));
    check.That(registry.ValidateGuard(Base, expected, expected, 2,
        SF_EXECUTABLE_SPAN_PHASE_NORMALIZE, identity_before));
    check.That(identity_before[0] == 0);
    code[0] = 0x90;
    const std::uint8_t modified[]{0x90,0x22};
    check.That(!registry.ValidateGuard(Base, expected, modified, 2,
        SF_EXECUTABLE_SPAN_PHASE_NORMALIZE, identity_before));

    state.result = SF_EXECUTABLE_SPAN_VALIDATION_VERIFIED_OWNER;
    state.malformed = true;
    code[0] = 0x11;
    check.That(!registry.ValidateGuard(Base, expected, expected, 2,
        SF_EXECUTABLE_SPAN_PHASE_BEFORE_ORIGINAL, identity_before));
    state.malformed = false; state.mutate = true;
    check.That(!registry.ValidateGuard(Base, expected, expected, 2,
        SF_EXECUTABLE_SPAN_PHASE_BEFORE_ORIGINAL, identity_before));
}

void revalidation_runs_even_for_unchanged_bytes_and_revocation_drains() {
    code.fill(0);
    code[0] = 0x11;
    auto registry = MakeRegistry();
    CallbackState state{};
    check.That(Register(registry, "owner", 5, Base, 1, state));
    state.result = SF_EXECUTABLE_SPAN_VALIDATION_UNCHANGED;
    check.That(registry.RevalidateAllRegistered());
    check.That(state.calls == 1 && state.phase == SF_EXECUTABLE_SPAN_PHASE_NORMALIZE);

    state.result = SF_EXECUTABLE_SPAN_VALIDATION_VERIFIED_OWNER; state.mutate = false; state.block = true;
    std::atomic<bool> accepted{true};
    std::thread in_flight([&] {
        const std::uint8_t expected[]{0x11};
        accepted = registry.ValidateGuard(Base, expected, expected, 1,
            SF_EXECUTABLE_SPAN_PHASE_BEFORE_ORIGINAL);
    });
    {
        std::unique_lock lock(state.mutex);
        state.changed.wait(lock, [&] { return state.entered; });
    }
    check.That(registry.Revoke("owner", 5) == 0);
    check.That(registry.Drain("owner", 5, 0) == 6);
    {
        std::lock_guard lock(state.mutex); state.release = true;
    }
    state.changed.notify_all(); in_flight.join();
    check.That(!accepted.load());
    check.That(registry.Drain("owner", 5, 100) == 0);
    check.That(!Register(registry, "owner", 5, Base + 1, 1, state));
}

void revalidation_rejects_callback_refusal() {
    code.fill(0);
    auto registry = MakeRegistry();
    CallbackState state{};
    check.That(Register(registry, "owner", 6, Base, 1, state));
    state.refuse = true;
    check.That(!registry.RevalidateAllRegistered());
}
}

int main() {
    abi_query_and_registration();
    normalizes_only_registered_prefix();
    phases_reject_malformed_unchanged_and_toctou();
    revalidation_runs_even_for_unchanged_bytes_and_revocation_drains();
    revalidation_rejects_callback_refusal();
    return check.failures ? 1 : 0;
}
