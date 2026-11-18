#include "abi/runtime_api.h"
#include "engine-hooks/scan_executable_signature.h"
#include "engine-hooks/allocate_near_code.h"
#include "engine-hooks/encode_relative_jump.h"
#include "engine-hooks/write_executable_bytes.h"
#include "patches/set_runtime_patch.h"
#include "code-inventory/resolve_original_call_graph.h"
#include "loaded-resources/loaded_resource_access.h"

#include "game-thread/run_game_thread_operations.h"
#include "profiles/game_profile.h"

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
#include <array>
#include <mutex>

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

// This intervention is resolved from the original registered actor_rotation
// system. It does not depend on an execution-profile RVA or a mod-owned AOB.
struct GliderPitchPatch {
    bool attempted{}, available{}, enabled{}, fault{};
    std::uintptr_t target{};
    std::array<std::uint8_t, 8> original{}, replacement{};
    OriginalCode::CodeEvidence evidence;
    void* trampoline{};
    const char* status{"unresolved"};
};
GliderPitchPatch glider;
std::mutex glider_mutex;
constexpr auto glider_name = "runtime.patch.glider_pitch";

bool scalar_reference(const OriginalCode::Instruction& ins, ZydisRegister output,
                      std::uint32_t expected_bits) {
    if (ins.decoded.mnemonic != ZYDIS_MNEMONIC_MOVSS ||
        ins.decoded.operand_count_visible != 2 ||
        ins.operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER ||
        ins.operands[0].reg.value != output ||
        ins.operands[1].type != ZYDIS_OPERAND_TYPE_MEMORY ||
        ins.operands[1].mem.base != ZYDIS_REGISTER_RIP || ins.operands[1].size != 32)
        return false;
    ZyanU64 address{};
    std::uint32_t bits{};
    SIZE_T count{};
    MEMORY_BASIC_INFORMATION page{};
    return ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&ins.decoded, &ins.operands[1], ins.address, &address)) &&
        VirtualQuery(reinterpret_cast<const void*>(address), &page, sizeof(page)) == sizeof(page) &&
        page.Type == MEM_IMAGE && page.AllocationBase == GetModuleHandleW(nullptr) &&
        (page.Protect == PAGE_READONLY || page.Protect == PAGE_EXECUTE_READ) &&
        address <= reinterpret_cast<std::uintptr_t>(page.BaseAddress) + page.RegionSize - sizeof(bits) &&
        ReadProcessMemory(GetCurrentProcess(), reinterpret_cast<const void*>(address), &bits, sizeof(bits), &count) &&
        count == sizeof(bits) && bits == expected_bits;
}

bool registers(const OriginalCode::Instruction& ins, ZydisMnemonic mnemonic,
               ZydisRegister left, ZydisRegister right) {
    return ins.decoded.mnemonic == mnemonic && ins.decoded.operand_count_visible == 2 &&
        ins.operands[0].type == ZYDIS_OPERAND_TYPE_REGISTER && ins.operands[0].reg.value == left &&
        ins.operands[1].type == ZYDIS_OPERAND_TYPE_REGISTER && ins.operands[1].reg.value == right;
}

bool branch_to(const OriginalCode::Instruction& ins, std::uintptr_t target) {
    if (ins.decoded.operand_count_visible != 1 || ins.operands[0].type != ZYDIS_OPERAND_TYPE_IMMEDIATE ||
        !ins.operands[0].imm.is_relative) return false;
    ZyanU64 destination{};
    return ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&ins.decoded, &ins.operands[0], ins.address, &destination)) &&
        destination == target;
}

bool resolve_glider_locked() {
    if (glider.attempted) return glider.available;
    // A loaded read-owner profile binds this process to its exact image. It
    // does not itself authorize this intervention; the code contract below
    // must independently identify the original operation.
    if (!ResourceRuntime::ProfileLoaded()) {
        glider.status = "exact-image-profile-unavailable";
        return false;
    }
    glider.attempted = true;
    std::uintptr_t root{};
    OriginalCode::Procedure procedure;
    if (!OriginalCode::ResolveNamedSystemCode("actor_rotation", root) ||
        !OriginalCode::ReadProcedure(root, procedure)) {
        glider.status = "actor-rotation-unavailable"; return false;
    }
    const auto& code = procedure.instructions;
    std::uintptr_t match{};
    for (std::size_t i = 0; i + 8 < code.size(); ++i) {
        const auto& a = code[i];
        // Confirm the two original angle constants and the entire conditional
        // clamp, including its branch destinations and instruction boundaries.
        if (!scalar_reference(a, ZYDIS_REGISTER_XMM0, 0x3db2b8c2) || a.decoded.length != 8 ||
            code[i+1].decoded.mnemonic != ZYDIS_MNEMONIC_MOVSD ||
            code[i+1].decoded.operand_count_visible != 2 ||
            code[i+1].operands[0].type != ZYDIS_OPERAND_TYPE_MEMORY ||
            code[i+1].operands[0].mem.base != ZYDIS_REGISTER_RSP ||
            code[i+1].operands[0].mem.disp.value != 0x40 ||
            code[i+1].operands[1].type != ZYDIS_OPERAND_TYPE_REGISTER ||
            code[i+1].operands[1].reg.value != ZYDIS_REGISTER_XMM1 ||
            code[i+2].decoded.mnemonic != ZYDIS_MNEMONIC_JNBE ||
            !registers(code[i+3], ZYDIS_MNEMONIC_COMISS, ZYDIS_REGISTER_XMM0, ZYDIS_REGISTER_XMM8) ||
            code[i+4].decoded.mnemonic != ZYDIS_MNEMONIC_JNBE ||
            !scalar_reference(code[i+5], ZYDIS_REGISTER_XMM6, 0x3fb2b8c2) ||
            !registers(code[i+6], ZYDIS_MNEMONIC_MINSS, ZYDIS_REGISTER_XMM6, ZYDIS_REGISTER_XMM8) ||
            code[i+7].decoded.mnemonic != ZYDIS_MNEMONIC_JMP ||
            !registers(code[i+8], ZYDIS_MNEMONIC_MOVAPS, ZYDIS_REGISTER_XMM6, ZYDIS_REGISTER_XMM0) ||
            !branch_to(code[i+2], code[i+8].address) || !branch_to(code[i+4], code[i+8].address) ||
            !branch_to(code[i+7], code[i+8].address + code[i+8].decoded.length)) continue;
        bool contiguous = true;
        for (std::size_t j = i; j < i + 8; ++j)
            contiguous &= code[j].address + code[j].decoded.length == code[j+1].address;
        if (!contiguous) continue;
        if (match) { glider.status = "glider-clamp-ambiguous"; return false; }
        match = a.address;
        std::copy_n(a.bytes.begin(), glider.original.size(), glider.original.begin());
    }
    if (!match) { glider.status = "glider-clamp-unavailable"; return false; }
    if (!OriginalCode::AppendEvidence(procedure, glider.evidence) ||
        !OriginalCode::EvidenceUnchanged(glider.evidence)) {
        glider.status = "actor-rotation-code-changed"; return false;
    }
    // Original MOVSS xmm0,[RIP+disp32] becomes a jump to a local constant.
    // The other instructions and actor-specific input remain engine-owned.
    std::array<std::uint8_t, 17> payload{0xf3,0x0f,0x10,0x05,0x05,0,0,0,
        0xe9,0,0,0,0,0xc3,0xf5,0xc8,0xbf};
    auto* allocation = static_cast<std::uint8_t*>(EngineHooks::AllocateNearCode(match, payload.size()));
    if (!allocation) { glider.status = "trampoline-allocation-failed"; return false; }
    std::int32_t ret{}, detour{};
    if (!EngineHooks::EncodeRelativeJump(reinterpret_cast<std::uintptr_t>(allocation) + 8, match + 8, ret) ||
        !EngineHooks::EncodeRelativeJump(match, reinterpret_cast<std::uintptr_t>(allocation), detour)) {
        VirtualFree(allocation, 0, MEM_RELEASE); glider.status = "trampoline-out-of-range"; return false;
    }
    std::memcpy(payload.data() + 9, &ret, sizeof(ret));
    std::memcpy(allocation, payload.data(), payload.size());
    FlushInstructionCache(GetCurrentProcess(), allocation, payload.size());
    glider.replacement = {0xe9,0,0,0,0,0x90,0x90,0x90};
    std::memcpy(glider.replacement.data() + 1, &detour, sizeof(detour));
    glider.target = match;
    glider.trampoline = allocation;
    glider.status = "resolved";
    return glider.available = true;
}

bool glider_available() {
    std::scoped_lock lock(glider_mutex);
    return resolve_glider_locked();
}

bool set_glider(bool enabled, std::uint32_t* outcome) {
    std::scoped_lock lock(glider_mutex);
    if (!resolve_glider_locked()) return false;
    if (glider.fault) return false;
    if (glider.enabled == enabled) {
        const auto* expected = enabled ? glider.replacement.data() : glider.original.data();
        if (std::memcmp(reinterpret_cast<const void*>(glider.target), expected, glider.original.size())) {
            glider.fault = true; glider.status = "live-bytes-changed"; return false;
        }
        *outcome = 0; return true;
    }
    if (enabled && !OriginalCode::EvidenceUnchanged(glider.evidence)) {
        glider.status = "actor-rotation-code-changed"; return false;
    }
    const auto* expected = enabled ? glider.original.data() : glider.replacement.data();
    const auto* replacement = enabled ? glider.replacement.data() : glider.original.data();
    if (std::memcmp(reinterpret_cast<const void*>(glider.target), expected, glider.original.size())) {
        glider.fault = true; glider.status = "live-bytes-changed"; return false;
    }
    if (!EngineHooks::ReplaceExecutableBytes(glider.target, expected, replacement, glider.original.size())) {
        // The writer can report failure after publication or rollback. Do not
        // retry an outcome that does not establish the final code protection.
        glider.fault = true; glider.status = "code-write-outcome-uncertain"; return false;
    }
    if (std::memcmp(reinterpret_cast<const void*>(glider.target), replacement, glider.original.size())) {
        glider.fault = true; glider.status = "code-write-readback-uncertain"; return false;
    }
    glider.enabled = enabled;
    glider.status = enabled ? "enabled" : "resolved";
    *outcome = 0;
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

bool resolve(PatchState& state, std::uint8_t* base) {
    const auto& spec = *state.profile;
    const auto parsed_signature = EngineHooks::ParseExecutableSignature(spec.signature);
    if (parsed_signature.size() < spec.overwrite || (spec.kind == "detour" && spec.overwrite < 5)) {
        state.status = "invalid-signature-or-overwrite-length";
        return false;
    }
    const auto match = EngineHooks::FindExecutableSignature(base, spec.signature);
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
    const bool association_matches = state.function_begin_rva == spec.function_begin_rva &&
        state.function_end_rva == spec.function_end_rva && state.target_offset == spec.target_offset;
    if (!association_matches && KfcRuntimeCompatibility::EnshroudedClient::exact_build_match) {
        state.status = "function-association-mismatch";
        return false;
    }
    if (!association_matches) state.status = "verified-build-diff-function-association";
    state.original.assign(reinterpret_cast<const std::uint8_t*>(match.address),
        reinterpret_cast<const std::uint8_t*>(match.address) + spec.overwrite);
    if (spec.kind == "bytes") {
        state.replacement = spec.payload;
        if (association_matches) state.status = "verified";
        return true;
    }

    std::vector<std::uint8_t> trampoline = spec.payload;
    for (const auto& reference : spec.inline_references) {
        const auto displacement = static_cast<std::int32_t>(reference.data) - static_cast<std::int32_t>(reference.next);
        std::memcpy(trampoline.data() + reference.displacement, &displacement, sizeof(displacement));
    }
    std::int32_t return_delta{};
    const auto trampoline_size = trampoline.size() + 16;
    auto* allocation = static_cast<std::uint8_t*>(EngineHooks::AllocateNearCode(state.target, trampoline_size));
    if (!allocation) { state.status = "trampoline-allocation-failed"; return false; }
    state.trampoline = allocation;
    if (!EngineHooks::EncodeRelativeJump(reinterpret_cast<std::uintptr_t>(allocation) + spec.return_rel32_offset - 1,
        state.target + spec.overwrite, return_delta)) { state.status = "return-jump-out-of-range"; return false; }
    std::memcpy(trampoline.data() + spec.return_rel32_offset, &return_delta, sizeof(return_delta));
    std::memcpy(allocation, trampoline.data(), trampoline.size());
    FlushInstructionCache(GetCurrentProcess(), allocation, trampoline.size());
    state.replacement.assign(spec.overwrite, 0x90);
    state.replacement[0] = 0xe9;
    std::int32_t detour{};
    if (!EngineHooks::EncodeRelativeJump(state.target, reinterpret_cast<std::uintptr_t>(allocation), detour)) { state.status = "detour-out-of-range"; return false; }
    std::memcpy(state.replacement.data() + 1, &detour, sizeof(detour));
    if (association_matches) state.status = "verified";
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
    {
        std::scoped_lock lock(glider_mutex);
        if (glider.enabled && !glider.fault) {
            if (EngineHooks::ReplaceExecutableBytes(glider.target, glider.replacement.data(),
                    glider.original.data(), glider.original.size()) &&
                !std::memcmp(reinterpret_cast<const void*>(glider.target), glider.original.data(), glider.original.size())) {
                glider.enabled = false;
                glider.status = "resolved";
            } else {
                glider.fault = true;
                glider.status = "shutdown-restore-uncertain";
            }
        }
        // A running thread can still return through the published trampoline.
        // Its allocation remains mapped until process exit.
    }
    for (auto& patch : patches) {
        if (patch.enabled && EngineHooks::ReplaceExecutableBytes(patch.target, patch.replacement.data(), patch.original.data(), patch.original.size()))
            patch.enabled = false;
    }
}

bool Available(const char* name) {
    if (!name) return false;
    if (std::strcmp(name, glider_name) == 0) return glider_available();
    for (const auto& patch : patches)
        if (patch.profile->name == name) return patch.target && patch.status.starts_with("verified");
    return false;
}

bool AnyAvailable() {
    for (const auto& patch : patches)
        if (patch.target && patch.status.starts_with("verified")) return true;
    return glider_available();
}

bool SetEnabled(const char* name, bool enabled, std::uint32_t* outcome) {
    if (outcome) *outcome = 1;
    if (!name || !outcome) return false;
    if (std::strcmp(name, glider_name) == 0) return set_glider(enabled, outcome);
    for (auto& patch : patches) {
        if (patch.profile->name != name) continue;
        if (!patch.target || !patch.status.starts_with("verified")) return false;
        if (patch.enabled == enabled) { *outcome = 0; return true; }
        const auto* expected = enabled ? patch.original.data() : patch.replacement.data();
        const auto* replacement = enabled ? patch.replacement.data() : patch.original.data();
        if (!EngineHooks::ReplaceExecutableBytes(patch.target, expected, replacement, patch.original.size())) {
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
                {"verified", association_verified}, {"buildDiffWarning", !association_verified && patch.status.starts_with("verified")}}},
            {"overwriteBytes", patch.original.size()}});
    }
    {
        std::scoped_lock lock(glider_mutex);
        rows.push_back({{"name", glider_name}, {"status", glider.status},
            {"resolved", glider.available}, {"enabled", glider.enabled}, {"fault", glider.fault},
            {"targetRva", glider.target && glider.target >= image_base ?
                nlohmann::json(glider.target - image_base) : nlohmann::json(nullptr)}});
    }
    return rows.dump();
}
}

extern "C" KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimePatchAvailable(const char* name) {
    try {
        if (name && std::strcmp(name, "runtime.gameplay.patch") == 0) return PatchRuntime::AnyAvailable();
        return PatchRuntime::Available(name);
    } catch (...) { return false; }
}

extern "C" KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimePatchSetEnabled(const char* name, bool enabled, std::uint32_t* outcome) {
    try { return PatchRuntime::SetEnabled(name, enabled, outcome); }
    catch (...) { return false; }
}
