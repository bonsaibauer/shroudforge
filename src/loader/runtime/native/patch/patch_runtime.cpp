#include "patch_runtime.h"

#include "dispatcher.h"
#include "profile.h"

#include <windows.h>
#include <nlohmann/json.hpp>
#include <algorithm>
#include <charconv>
#include <cstring>
#include <limits>
#include <optional>
#include <sstream>
#include <string_view>
#include <vector>

namespace {
struct PatchState {
    const KfcRuntimeCompatibility::EnshroudedClient::RuntimePatch* profile{};
    std::uintptr_t target{};
    std::uint32_t function_begin_rva{}, function_end_rva{};
    std::size_t target_offset{};
    std::size_t signature_matches{};
    std::vector<std::uint8_t> original, replacement;
    void* trampoline{};
    std::string status{"unresolved"};
    bool enabled{};
};
std::vector<PatchState> patches;

std::vector<int> parse_signature(std::string_view text) {
    std::vector<int> bytes;
    std::istringstream input{std::string(text)};
    std::string token;
    while (input >> token) {
        if (token == "?" || token == "??") { bytes.push_back(-1); continue; }
        unsigned value{};
        const auto parsed = std::from_chars(token.data(), token.data() + token.size(), value, 16);
        if (parsed.ec != std::errc{} || parsed.ptr != token.data() + token.size() || value > 255) return {};
        bytes.push_back(static_cast<int>(value));
    }
    return bytes;
}

struct SignatureMatch {
    std::uintptr_t address{};
    std::size_t count{};
};

SignatureMatch find_unique(std::uint8_t* base, std::string_view text) {
    const auto signature = parse_signature(text);
    if (signature.empty()) return {};
    const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base);
    const auto nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(base + dos->e_lfanew);
    const auto sections = IMAGE_FIRST_SECTION(nt);
    SignatureMatch result{};
    for (WORD i = 0; i < nt->FileHeader.NumberOfSections; ++i) {
        const auto& section = sections[i];
        if (!(section.Characteristics & IMAGE_SCN_MEM_EXECUTE)) continue;
        const auto length = (std::min)(static_cast<std::size_t>(section.Misc.VirtualSize),
            static_cast<std::size_t>(nt->OptionalHeader.SizeOfImage - section.VirtualAddress));
        if (length < signature.size()) continue;
        auto* begin = base + section.VirtualAddress;
        for (std::size_t offset = 0; offset <= length - signature.size(); ++offset) {
            bool matches = true;
            for (std::size_t j = 0; j < signature.size(); ++j)
                if (signature[j] >= 0 && begin[offset + j] != signature[j]) { matches = false; break; }
            if (!matches) continue;
            if (!result.count) result.address = reinterpret_cast<std::uintptr_t>(begin + offset);
            ++result.count;
        }
    }
    return result;
}

bool relative(std::uintptr_t instruction, std::uintptr_t destination, std::int32_t& result) {
    const auto delta = static_cast<std::int64_t>(destination) - static_cast<std::int64_t>(instruction + 5);
    if (delta < (std::numeric_limits<std::int32_t>::min)() || delta > (std::numeric_limits<std::int32_t>::max)()) return false;
    result = static_cast<std::int32_t>(delta);
    return true;
}

std::optional<std::pair<std::uint32_t, std::uint32_t>> function_range(
    std::uintptr_t image_base, std::uintptr_t target) {
    if (!image_base || target < image_base) return std::nullopt;
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(image_base);
    if (dos->e_magic != IMAGE_DOS_SIGNATURE || dos->e_lfanew <= 0) return std::nullopt;
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(image_base + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE || nt->OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR64_MAGIC)
        return std::nullopt;
    const auto directory = nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION];
    if (!directory.VirtualAddress || directory.Size < 12 ||
        directory.VirtualAddress > nt->OptionalHeader.SizeOfImage ||
        directory.Size > nt->OptionalHeader.SizeOfImage - directory.VirtualAddress)
        return std::nullopt;
    struct FunctionEntry { std::uint32_t begin, end, unwind; };
    const auto* entries = reinterpret_cast<const FunctionEntry*>(image_base + directory.VirtualAddress);
    const auto count = directory.Size / sizeof(FunctionEntry);
    const auto target_rva = target - image_base;
    for (std::size_t index = 0; index < count; ++index) {
        if (entries[index].begin <= target_rva && target_rva < entries[index].end)
            return std::pair{entries[index].begin, entries[index].end};
    }
    return std::nullopt;
}

void* allocate_near(std::uintptr_t target, std::size_t size) {
    SYSTEM_INFO system{};
    GetSystemInfo(&system);
    const auto granularity = static_cast<std::uintptr_t>(system.dwAllocationGranularity);
    constexpr std::uintptr_t range = 0x7fff0000;
    const auto minimum = (std::max)(reinterpret_cast<std::uintptr_t>(system.lpMinimumApplicationAddress), target > range ? target - range : 0);
    const auto maximum = (std::min)(reinterpret_cast<std::uintptr_t>(system.lpMaximumApplicationAddress), target <= UINTPTR_MAX - range ? target + range : target);
    for (auto cursor = minimum; cursor < maximum;) {
        MEMORY_BASIC_INFORMATION memory{};
        if (!VirtualQuery(reinterpret_cast<void*>(cursor), &memory, sizeof(memory))) break;
        const auto region = reinterpret_cast<std::uintptr_t>(memory.BaseAddress);
        const auto end = region + memory.RegionSize;
        if (memory.State == MEM_FREE) {
            const auto candidate = (region + granularity - 1) & ~(granularity - 1);
            if (candidate >= minimum && candidate + size <= end && candidate + size <= maximum)
                if (auto allocation = VirtualAlloc(reinterpret_cast<void*>(candidate), size, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE)) return allocation;
        }
        if (end <= cursor) break;
        cursor = end;
    }
    return nullptr;
}

bool resolve(PatchState& state, std::uint8_t* base) {
    const auto& spec = *state.profile;
    const auto parsed_signature = parse_signature(spec.signature);
    if (parsed_signature.size() < spec.overwrite || (spec.kind == "detour" && spec.overwrite < 5)) {
        state.status = "invalid-signature-or-overwrite-length";
        return false;
    }
    const auto match = find_unique(base, spec.signature);
    state.signature_matches = match.count;
    if (match.count != 1 || !match.address) {
        state.status = match.count == 0 ? "signature-missing" : "signature-ambiguous";
        return false;
    }
    state.target = match.address;
    const auto image_base = reinterpret_cast<std::uintptr_t>(base);
    const auto actual_range = function_range(image_base, state.target);
    if (!actual_range) {
        state.status = "function-boundary-unavailable";
        return false;
    }
    state.function_begin_rva = actual_range->first;
    state.function_end_rva = actual_range->second;
    state.target_offset = static_cast<std::size_t>((state.target - image_base) - actual_range->first);
    if (state.function_begin_rva != spec.function_begin_rva ||
        state.function_end_rva != spec.function_end_rva ||
        state.target_offset != spec.target_offset) {
        state.status = "function-association-mismatch";
        return false;
    }
    state.original.assign(reinterpret_cast<const std::uint8_t*>(match.address),
        reinterpret_cast<const std::uint8_t*>(match.address) + spec.overwrite);
    if (spec.kind == "bytes") {
        state.replacement = spec.payload;
        state.status = "verified";
        return true;
    }

    std::vector<std::uint8_t> trampoline = spec.payload;
    std::int32_t return_delta{};
    const auto trampoline_size = trampoline.size() + 16;
    auto* allocation = static_cast<std::uint8_t*>(allocate_near(state.target, trampoline_size));
    if (!allocation) { state.status = "trampoline-allocation-failed"; return false; }
    state.trampoline = allocation;
    if (!relative(reinterpret_cast<std::uintptr_t>(allocation) + spec.return_rel32_offset - 1,
        state.target + spec.overwrite, return_delta)) { state.status = "return-jump-out-of-range"; return false; }
    std::memcpy(trampoline.data() + spec.return_rel32_offset, &return_delta, sizeof(return_delta));
    std::memcpy(allocation, trampoline.data(), trampoline.size());
    FlushInstructionCache(GetCurrentProcess(), allocation, trampoline.size());
    state.replacement.assign(spec.overwrite, 0x90);
    state.replacement[0] = 0xe9;
    std::int32_t detour{};
    if (!relative(state.target, reinterpret_cast<std::uintptr_t>(allocation), detour)) { state.status = "detour-out-of-range"; return false; }
    std::memcpy(state.replacement.data() + 1, &detour, sizeof(detour));
    state.status = "verified";
    return true;
}
}

namespace PatchRuntime {
bool Initialize() {
    patches.clear();
    const auto base = reinterpret_cast<std::uint8_t*>(GetModuleHandleW(nullptr));
    if (!base) return false;
    for (const auto& profile : KfcRuntimeCompatibility::EnshroudedClient::runtime_patches) {
        PatchState state{};
        state.profile = &profile;
        resolve(state, base);
        patches.push_back(std::move(state));
    }
    return !patches.empty();
}

void Shutdown() {
    for (auto& patch : patches) {
        if (patch.enabled && GameThreadDispatcher::WriteCode(patch.target, patch.replacement.data(), patch.original.data(), patch.original.size()))
            patch.enabled = false;
    }
}

bool Available(const char* name) {
    if (!name) return false;
    for (const auto& patch : patches)
        if (patch.profile->name == name) return patch.target && patch.status == "verified";
    return false;
}

bool AnyAvailable() {
    for (const auto& patch : patches)
        if (patch.target && patch.status == "verified") return true;
    return false;
}

bool SetEnabled(const char* name, bool enabled, std::uint32_t* outcome) {
    if (outcome) *outcome = 1;
    if (!name || !outcome) return false;
    for (auto& patch : patches) {
        if (patch.profile->name != name) continue;
        if (!patch.target || patch.status != "verified") return false;
        if (patch.enabled == enabled) { *outcome = 0; return true; }
        const auto* expected = enabled ? patch.original.data() : patch.replacement.data();
        const auto* replacement = enabled ? patch.replacement.data() : patch.original.data();
        if (!GameThreadDispatcher::WriteCode(patch.target, expected, replacement, patch.original.size())) {
            patch.status = "live-bytes-changed-or-thread-unsafe";
            return false;
        }
        patch.enabled = enabled;
        *outcome = 0;
        return true;
    }
    return false;
}

std::string Diagnostics() {
    nlohmann::json rows = nlohmann::json::array();
    const auto image_base = reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
    for (const auto& patch : patches) {
        const auto& spec = *patch.profile;
        const bool association_verified = patch.target && patch.function_begin_rva == spec.function_begin_rva &&
            patch.function_end_rva == spec.function_end_rva && patch.target_offset == spec.target_offset;
        rows.push_back({{"name", patch.profile->name}, {"status", patch.status},
            {"resolved", patch.target != 0}, {"enabled", patch.enabled},
            {"signatureMatches", patch.signature_matches},
            {"targetRva", patch.target && patch.target >= image_base ?
                nlohmann::json(patch.target - image_base) : nlohmann::json(nullptr)},
            {"functionRva", patch.target ? nlohmann::json{{"begin", patch.function_begin_rva},
                {"end", patch.function_end_rva}} : nlohmann::json(nullptr)},
            {"functionAssociation", {{"id", spec.function_id},
                {"expectedBeginRva", spec.function_begin_rva}, {"expectedEndRva", spec.function_end_rva},
                {"expectedTargetOffset", spec.target_offset}, {"actualBeginRva", patch.target ?
                    nlohmann::json(patch.function_begin_rva) : nlohmann::json(nullptr)},
                {"actualEndRva", patch.target ? nlohmann::json(patch.function_end_rva) : nlohmann::json(nullptr)},
                {"actualTargetOffset", patch.target ? nlohmann::json(patch.target_offset) : nlohmann::json(nullptr)},
                {"verified", association_verified}}},
            {"overwriteBytes", patch.original.size()}});
    }
    return rows.dump();
}
}
