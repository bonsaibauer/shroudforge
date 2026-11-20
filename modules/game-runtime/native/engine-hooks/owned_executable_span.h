#pragma once

#include "../abi/executable_span_api.h"
#include <cstdint>
#include <span>

namespace EngineHooks {
using OwnedExecutableSpanVerifier = bool(SF_EXECUTABLE_SPAN_CALL *)(
    std::uint32_t rva,
    const std::uint8_t* original,
    const std::uint8_t* replacement,
    std::uint32_t byte_count) noexcept;

// Internal framework registration only. Records are retained and immutable;
// no native-mod registration, mutation, restoration, or retirement API exists.
bool PublishOwnedExecutableSpan(std::uint32_t rva,
    std::span<const std::uint8_t> original,
    std::span<const std::uint8_t> replacement,
    OwnedExecutableSpanVerifier verifier) noexcept;

// Reads only a bounded, executable, readable span from the current main image.
// The result is written only when the request is valid and observed_current
// still matches the live bytes. This is evidence, not a patch authorization or
// quiescence guarantee.
SFExecutableSpanStatusV1 QueryMainImageExecutableSpan(
    const SFExecutableSpanQueryV1& request,
    SFExecutableSpanResultV1& result) noexcept;
}
