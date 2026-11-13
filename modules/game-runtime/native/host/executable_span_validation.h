#pragma once

#include "abi/executable_span_validation_api.h"
#include <condition_variable>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <mutex>
#include <string>
#include <string_view>
#include <thread>
#include <vector>

namespace ExecutableSpanValidation {
class Registry final {
public:
    struct Owner;
    struct Span;
    using CodeReader = bool (*)(std::uint32_t, std::uint8_t*, std::size_t) noexcept;
    using RegistrationGate = bool (*)() noexcept;
    using CallbackAddressValidator = bool (*)(SFExecutableSpanValidationCallbackV1) noexcept;

    Registry(CodeReader reader, RegistrationGate gate,
        CallbackAddressValidator callback_validator) noexcept;
    ~Registry();
    Registry(const Registry&) = delete;
    Registry& operator=(const Registry&) = delete;

    std::int32_t Query(std::string_view package, std::uint64_t generation,
        std::uint32_t version, std::uint32_t capacity, void* output) noexcept;
    std::int32_t Revoke(std::string_view package, std::uint64_t generation) noexcept;
    std::int32_t Drain(std::string_view package, std::uint64_t generation,
        std::uint32_t timeout_ms) noexcept;

    // Compares one host-owned engine guard. A registered prefix is the only
    // bytes a callback may authorize; every byte after it remains exact.
    bool ValidateGuard(std::uint32_t rva, const std::uint8_t* expected,
        const std::uint8_t* observed, std::size_t guard_size, std::uint32_t phase,
        std::uint8_t owner_identity[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES] = nullptr) noexcept;

    // Calls every live registration, including exact/unchanged spans. Intended
    // for the selected-startup activation gate after restored-entry callbacks.
    bool RevalidateAllRegistered() noexcept;

private:
    static std::int32_t SF_EXECUTABLE_SPAN_VALIDATION_CALL RegisterSpan(
        void* context, const SFExecutableSpanRegistrationV1* registration) noexcept;
    std::int32_t Register(Owner* context,
        const SFExecutableSpanRegistrationV1* registration) noexcept;
    bool Invoke(Owner* owner, const Span& span, const std::uint8_t* observed,
        std::size_t guard_size, std::uint32_t phase, std::uint8_t identity[
            SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES]) noexcept;
    Owner* FindLocked(std::string_view package, std::uint64_t generation) const noexcept;
    Owner* FindContextLocked(const void* context) const noexcept;
    Owner* CreateLocked(std::string_view package, std::uint64_t generation) noexcept;

    CodeReader reader_{};
    RegistrationGate registration_gate_{};
    CallbackAddressValidator callback_validator_{};
    std::mutex mutex_;
    std::condition_variable changed_;
    std::vector<std::unique_ptr<Owner>> owners_;
    std::size_t span_count_{};
};

Registry& Services() noexcept;
bool RevalidateAllRegistered() noexcept;
}
