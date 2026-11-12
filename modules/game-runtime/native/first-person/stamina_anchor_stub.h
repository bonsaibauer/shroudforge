#pragma once

#include "camera_layout.h"

#include <windows.h>

#include <array>
#include <cstdint>
#include <cstring>
#include <initializer_list>
#include <limits>
#include <vector>

namespace FirstPersonControl {

inline constexpr std::size_t CodePageSize = 0x1000;
inline constexpr std::size_t GaugeCodeOffset = CodePageSize;
inline constexpr std::size_t SharedStateOffset = CodePageSize * 2;
inline constexpr std::size_t GaugeDataOffsetFromState = 0x100;
inline constexpr std::size_t GaugeFunctionTableOffset = 0x80;
inline constexpr std::size_t AllocationSize = CodePageSize * 4;

struct GaugeCode {
    std::vector<std::uint8_t> bytes;
    RUNTIME_FUNCTION function{};
    std::size_t tail{};
    std::size_t codeSize{};
};

inline bool RelativeDisplacement(
    std::uintptr_t nextInstruction,
    std::uintptr_t target,
    std::int32_t& result) noexcept {
    const auto delta = static_cast<std::int64_t>(target) -
        static_cast<std::int64_t>(nextInstruction);
    if (delta < (std::numeric_limits<std::int32_t>::min)() ||
        delta > (std::numeric_limits<std::int32_t>::max)()) {
        return false;
    }
    result = static_cast<std::int32_t>(delta);
    return true;
}

inline bool PatchRelativeReference(
    std::vector<std::uint8_t>& bytes,
    std::size_t displacementOffset,
    std::size_t nextOffset,
    std::uintptr_t codeBase,
    std::uintptr_t target) noexcept {
    if (displacementOffset > bytes.size() ||
        sizeof(std::int32_t) > bytes.size() - displacementOffset) {
        return false;
    }
    std::int32_t displacement{};
    if (!RelativeDisplacement(codeBase + nextOffset, target, displacement)) return false;
    std::memcpy(bytes.data() + displacementOffset, &displacement, sizeof(displacement));
    return true;
}

inline bool BuildGaugeCode(
    std::uintptr_t contextSlot,
    std::uintptr_t sharedEnabled,
    std::uintptr_t codeBase,
    GaugeCode& out) {
    out = {};
    auto& bytes = out.bytes;
    auto emit = [&](std::initializer_list<std::uint8_t> values) {
        bytes.insert(bytes.end(), values);
    };

    struct RelativeReference {
        std::size_t displacementOffset{};
        std::size_t nextOffset{};
        std::uintptr_t target{};
    };
    std::vector<RelativeReference> references;
    std::vector<std::size_t> restoreBranches;
    auto rip = [&](std::initializer_list<std::uint8_t> opcode,
                   std::uintptr_t target,
                   std::initializer_list<std::uint8_t> suffix = {}) {
        emit(opcode);
        const auto displacementOffset = bytes.size();
        emit({0, 0, 0, 0});
        emit(suffix);
        references.push_back({displacementOffset, bytes.size(), target});
    };
    auto restoreBranch = [&](std::uint8_t condition) {
        emit({0x0F, condition});
        restoreBranches.push_back(bytes.size());
        emit({0, 0, 0, 0});
    };

    // Save flags, nonvolatile frame state, SIMD inputs, and MXCSR. All error
    // paths join the same unwindable epilogue and native setter tail-jump.
    emit({0x9C, 0x55, 0x48, 0x83, 0xEC, 0x40, 0x48, 0x8D, 0x6C, 0x24, 0x40});
    emit({0x48, 0x89, 0x44, 0x24, 0x20, 0x4C, 0x89, 0x44, 0x24, 0x28});
    emit({0x0F, 0x11, 0x04, 0x24, 0x0F, 0x11, 0x4C, 0x24, 0x10});
    emit({0x0F, 0xAE, 0x5C, 0x24, 0x30, 0x8B, 0x44, 0x24, 0x30});
    emit({0x0D, 0x80, 0x1F, 0, 0, 0x89, 0x44, 0x24, 0x34, 0x0F, 0xAE, 0x54, 0x24, 0x34});
    rip({0x83, 0x3D}, sharedEnabled, {0});
    restoreBranch(0x84);
    emit({0x41, 0x80, 0x7D, 0x09, 0x02});
    restoreBranch(0x85); // Only the original resource-kind value is modified.

    emit({0x48, 0xB8});
    const auto contextImmediate = bytes.size();
    bytes.resize(bytes.size() + sizeof(contextSlot));
    std::memcpy(bytes.data() + contextImmediate, &contextSlot, sizeof(contextSlot));
    emit({0x48, 0x8B, 0x00, 0x48, 0x85, 0xC0});
    restoreBranch(0x84);
    emit({0x48, 0x8B, 0x40, 0x10, 0x48, 0x85, 0xC0});
    restoreBranch(0x84);
    emit({0xF3, 0x0F, 0x7E, 0x40, 0x20}); // Canvas width and height.

    const auto dataBase = codeBase + GaugeCodeOffset + CodePageSize +
        GaugeDataOffsetFromState;
    std::array<std::uintptr_t, 5> data{};
    data[0] = sharedEnabled;
    for (std::size_t index = 1; index < data.size(); ++index) {
        data[index] = dataBase + (index - 1) * 16;
    }

    auto reject = [&](unsigned dataIndex, std::uint8_t comparison) {
        rip({0x0F, 0xC2, 0x0D}, data[dataIndex], {comparison});
        emit({0x44, 0x0F, 0x50, 0xC1, 0x41, 0xF6, 0xC0, 0x03});
        restoreBranch(0x85);
    };
    emit({0x0F, 0x28, 0xC8}); reject(1, 2); // Canvas must be positive.
    emit({0x0F, 0x28, 0xC8}); reject(2, 6); // Canvas must be finite and bounded.
    emit({0x0F, 0x28, 0xCE, 0x0F, 0xC6, 0xC9, 0xEE}); reject(1, 1);
    emit({0x0F, 0x28, 0xCE, 0x0F, 0xC6, 0xC9, 0xEE}); reject(2, 6);
    rip({0x0F, 0x59, 0x05}, data[3]);
    emit({0x0F, 0x28, 0xCE, 0x0F, 0xC6, 0xC9, 0xEE});
    rip({0x0F, 0x59, 0x0D}, data[4]);
    emit({0x0F, 0x5C, 0xC1, 0x66, 0x48, 0x0F, 0x7E, 0xC2});

    const auto restoreOffset = bytes.size();
    out.codeSize = restoreOffset;
    emit({0x0F, 0x10, 0x04, 0x24, 0x0F, 0x10, 0x4C, 0x24, 0x10});
    emit({0x0F, 0xAE, 0x54, 0x24, 0x30});
    emit({0xFF, 0x75, 0x08, 0x9D});
    emit({0x48, 0x8B, 0x45, 0x00, 0x48, 0x89, 0x45, 0x08});
    emit({0x48, 0x8B, 0x44, 0x24, 0x20, 0x4C, 0x8B, 0x44, 0x24, 0x28});
    emit({0x48, 0x8D, 0x65, 0x08, 0x5D, 0xFF, 0x25});
    const auto tailDisplacement = bytes.size();
    emit({0, 0, 0, 0});
    out.codeSize = bytes.size();

    if (out.codeSize >= CodePageSize) return false;
    const auto requiredDataOffset = CodePageSize + GaugeDataOffsetFromState;
    if (bytes.size() > requiredDataOffset) return false;
    bytes.resize(requiredDataOffset, 0x90);

    // Preserve the source algorithm's constants. The first data slot is the
    // shared owner-enabled word and is intentionally not duplicated locally.
    const std::array<std::array<float, 4>, 5> values = {{
        {{0, 0, 0, 0}},
        {{0, 0, 0, 0}},
        {{16384.0f, 16384.0f, 16384.0f, 16384.0f}},
        {{0.45f, 0.5f, 0, 0}},
        {{0.5f, 0.5f, 0.5f, 0.5f}},
    }};
    const auto valuesStart = bytes.size();
    for (std::size_t index = 1; index < values.size(); ++index) {
        const auto begin = bytes.size();
        bytes.resize(begin + sizeof(values[index]));
        std::memcpy(bytes.data() + begin, values[index].data(), sizeof(values[index]));
    }
    out.tail = bytes.size();
    bytes.resize(out.tail + sizeof(std::uintptr_t));
    const auto unwindOffset = bytes.size();
    emit({1, 11, 4, 0x45, 11, 3, 6, 0x72, 2, 0x50, 1, 2});
    out.function = {0, static_cast<DWORD>(out.codeSize), static_cast<DWORD>(unwindOffset)};

    for (const auto& reference : references) {
        if (!PatchRelativeReference(bytes, reference.displacementOffset,
                reference.nextOffset, codeBase + GaugeCodeOffset, reference.target)) {
            return false;
        }
    }
    for (const auto branchOffset : restoreBranches) {
        const auto nextOffset = branchOffset + sizeof(std::int32_t);
        const auto targetAddress = codeBase + GaugeCodeOffset + restoreOffset;
        if (!PatchRelativeReference(bytes, branchOffset, nextOffset,
                codeBase + GaugeCodeOffset, targetAddress)) {
            return false;
        }
    }
    const auto tailSlot = codeBase + GaugeCodeOffset + out.tail;
    if (!PatchRelativeReference(bytes, tailDisplacement, tailDisplacement + sizeof(std::int32_t),
            codeBase + GaugeCodeOffset, tailSlot)) {
        return false;
    }
    if (valuesStart + sizeof(values[1]) * (values.size() - 1) != out.tail) return false;
    return true;
}

} // namespace FirstPersonControl
