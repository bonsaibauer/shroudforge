#pragma once
#include <Zydis/Zydis.h>
#include <array>
#include <cstdint>
#include <functional>
#include <vector>

namespace OriginalCode {
// Read-only instruction evidence. Resolving code does not grant an invocation
// contract. Each typed adapter validates its own arguments, effects and owners.
struct Instruction {
    std::uintptr_t address{};
    ZydisDecodedInstruction decoded{};
    std::array<ZydisDecodedOperand, ZYDIS_MAX_OPERAND_COUNT> operands{};
    std::array<std::uint8_t,15> bytes{};
};
struct Procedure {
    struct ReadOnlyData { std::uintptr_t address{}; std::vector<std::uint8_t> bytes; };
    std::uintptr_t entry{};
    std::vector<Instruction> instructions;
    std::vector<std::uintptr_t> calls;
    bool indirect_calls{};
    std::vector<ReadOnlyData> read_only_data;
};

// Loaded-image section metadata. Immutable readable sections carry copied
// bytes. Writable sections expose bounds only, never a retained data snapshot.
struct ImageSection {
    std::uintptr_t address{};
    std::size_t size{};
    std::uint32_t flags{};
    std::vector<std::uint8_t> bytes;
};
bool ReadImageSections(std::vector<ImageSection>& output);

// Exact decoded bytes, coalesced into bounded contiguous reads. Both native
// adapters use this same evidence owner. Validation allocates no memory and
// compares all retained bytes, including decoded switch tables.
struct EvidenceBlock { std::uintptr_t address{}; std::vector<std::uint8_t> bytes; };
using CodeEvidence = std::vector<EvidenceBlock>;
bool AppendEvidence(const Procedure& procedure, CodeEvidence& output);
bool EvidenceUnchanged(const CodeEvidence& evidence);

// Complete chained PE unwind ranges, or a bounded call-free leaf CFG. Never
// treats E8 bytes inside operands as calls and never scans for byte signatures.
bool ReadProcedure(std::uintptr_t entry, Procedure& output);
// Resolves an interior instruction through the canonical chained unwind owner.
// The address must be an instruction boundary reached by that owner's CFG.
bool ReadContainingProcedure(std::uintptr_t address, Procedure& output);
bool ResolveNamedSystemCode(const char* exact_name, std::uintptr_t& output);
bool FindUniqueReachable(std::uintptr_t root, unsigned maximum_depth,
    const std::function<bool(const Procedure&)>& predicate, std::uintptr_t& output);
}
