#include "impact/submit_building_prop_spawn.h"
#include "code-inventory/resolve_original_call_graph.h"
#include "ecs/resolve_entity_command_queue.h"
#include "named-services/named_service_contract.h"
#include "ecs/registry/read_component_registry.h"
#include <cmath>
#include <sstream>
#include <string_view>
#include <algorithm>
#include <cstring>
#include <limits>
#include <charconv>

namespace ImpactRuntime {
namespace {
using OriginalCode::Instruction;
using OriginalCode::Procedure;
using NamedServiceImplementation::copy;
using EcsRuntime::EntityCommandQueueSelection;

// Complete reviewed instruction contracts, selected through original named
// descriptors and decoded call edges. Relative targets are instruction labels
// or separately validated callees. Component identities are obtained by name.
// No function address, byte signature or function digest selects the entry.
static constexpr std::string_view helper_recipe = R"abi(mov|m64:rsp:-:0:16|rbx
push|rbp
push|rsi
push|rdi
push|r14
push|r15
lea|rbp|m0:rsp:-:0:-112
sub|rsp|368
mov|rbx|rcx
mov|rdi|r9
mov|rcx|m64:rcx:-:0:0
mov|rsi|r8
mov|r14|rdx
call|external
mov|rcx|m64:rbx:-:0:0
lea|rdx|m0:rbp:-:0:184
mov|r15|rax
call|external
movdqa|xmm1|rip128
lea|rax|m0:rsp:-:0:120
xor|ebx|ebx
mov|m64:rsp:-:0:80|rax
lea|rax|rip64
mov|m64:rsp:-:0:112|rbx
xorps|xmm0|xmm0
mov|m64:rsp:-:0:104|rax
lea|r9|m0:rsp:-:0:64
mov|m64:rsp:-:0:88|rbx
mov|r8|rdi
mov|m64:rsp:-:0:96|5
mov|rdx|rsi
mov|m64:rbp:-:0:64|rbx
lea|rcx|m0:rbp:-:0:48
mov|m32:rbp:-:0:88|1065353216
movdqu|m128:rbp:-:0:48|xmm0
mov|m32:rbp:-:0:92|1065353216
movups|m128:rbp:-:0:72|xmm1
mov|m32:rbp:-:0:96|1065353216
mov|m32:rsp:-:0:64|1065353216
mov|m32:rsp:-:0:68|1065353216
mov|m32:rsp:-:0:72|1065353216
call|external
mov|rdx|m64:rsp:-:0:88
lea|rcx|m0:rdx:-:0:1
mov|m64:rsp:-:0:64|rcx
cmp|rcx|m64:rsp:-:0:96
jbe|label60
mov|rax|m64:rsp:-:0:104
test|rax|rax
jz|label208
neg|rcx
xor|r8d|r8d
mov|rdx|rcx
lea|rcx|m0:rsp:-:0:80
call|rax
test|al|al
jnz|label59
mov|eax|ebx
jmp|label65
mov|rdx|m64:rsp:-:0:88
mov|rax|m64:rsp:-:0:80
lea|rcx|m0:rdx:rdx:2:0
inc|rdx
mov|m64:rsp:-:0:88|rdx
lea|rax|m0:rax:rcx:8:0
mov|m32:rax:-:0:0|#CurrentTransform
lea|rcx|m0:rbp:-:0:48
mov|m64:rax:-:0:8|rcx
mov|m64:rax:-:0:16|56
xor|eax|eax
mov|rdx|m64:rsp:-:0:88
mov|m8:rbp:-:0:160|al
lea|rax|m0:rdx:-:0:1
mov|m64:rsp:-:0:64|rax
cmp|rax|m64:rsp:-:0:96
jbe|label89
mov|r9|m64:rsp:-:0:104
test|r9|r9
jz|label215
neg|rax
lea|rcx|m0:rsp:-:0:80
mov|rdx|rax
xor|r8d|r8d
call|r9
test|al|al
jnz|label88
mov|rax|rbx
jmp|label94
mov|rdx|m64:rsp:-:0:88
mov|rax|m64:rsp:-:0:80
lea|rcx|m0:rdx:rdx:2:0
inc|rdx
mov|m64:rsp:-:0:88|rdx
lea|rax|m0:rax:rcx:8:0
lea|rcx|m0:rbp:-:0:160
mov|m32:rax:-:0:0|#DisableRandomLootOnDestroy
mov|m64:rax:-:0:8|rcx
mov|m64:rax:-:0:16|1
mov|eax|m32:rbp:-:0:192
test|eax|eax
jz|label129
mov|rdx|m64:rsp:-:0:88
mov|m32:rbp:-:0:176|eax
lea|rax|m0:rdx:-:0:1
mov|m64:rsp:-:0:64|rax
cmp|rax|m64:rsp:-:0:96
jbe|label120
mov|r9|m64:rsp:-:0:104
test|r9|r9
jz|label222
neg|rax
lea|rcx|m0:rsp:-:0:80
mov|rdx|rax
xor|r8d|r8d
call|r9
test|al|al
jnz|label119
mov|rax|rbx
jmp|label125
mov|rdx|m64:rsp:-:0:88
mov|rax|m64:rsp:-:0:80
lea|rcx|m0:rdx:rdx:2:0
inc|rdx
mov|m64:rsp:-:0:88|rdx
lea|rax|m0:rax:rcx:8:0
lea|rcx|m0:rbp:-:0:176
mov|m32:rax:-:0:0|#UsedItem
mov|m64:rax:-:0:8|rcx
mov|m64:rax:-:0:16|4
mov|rcx|m64:rbp:-:0:208
mov|rax|m64:rcx:-:0:8
or|rax|m64:rcx:-:0:0
jz|label165
mov|eax|m32:rbp:-:0:200
xorps|xmm0|xmm0
mov|rdx|m64:rsp:-:0:88
movdqu|m128:rbp:-:0:20|xmm0
mov|m32:rbp:-:0:16|eax
movups|xmm0|m128:rcx:-:0:0
lea|rax|m0:rdx:-:0:1
mov|m64:rbp:-:0:36|rbx
mov|m64:rsp:-:0:64|rax
movups|m128:rbp:-:0:0|xmm0
cmp|rax|m64:rsp:-:0:96
jbe|label156
mov|r9|m64:rsp:-:0:104
test|r9|r9
jz|label201
neg|rax
lea|rcx|m0:rsp:-:0:80
mov|rdx|rax
xor|r8d|r8d
call|r9
test|al|al
jz|label161
mov|rdx|m64:rsp:-:0:88
mov|rax|m64:rsp:-:0:80
lea|rcx|m0:rdx:rdx:2:0
inc|rdx
mov|m64:rsp:-:0:88|rdx
lea|rbx|m0:rax:rcx:8:0
lea|rax|m0:rbp:-:0:0
mov|m32:rbx:-:0:0|#CollisionFeedbackMaterialComponent
mov|m64:rbx:-:0:8|rax
mov|m64:rbx:-:0:16|44
mov|rax|m64:rsp:-:0:80
lea|rdx|m0:rsp:-:0:64
mov|r8d|m32:rbp:-:0:184
mov|rcx|r15
mov|m64:rbp:-:0:-16|rax
mov|r9d|r8d
mov|rax|m64:rsp:-:0:88
mov|m64:rbp:-:0:-8|rax
movzx|eax|rip8
mov|m8:rsp:-:0:56|al
lea|rax|m0:rbp:-:0:-16
mov|m8:rsp:-:0:48|0
mov|m64:rsp:-:0:40|rax
mov|m64:rsp:-:0:32|r14
call|external
mov|ebx|m32:rax:-:0:0
mov|rax|m64:rsp:-:0:96
cmp|rax|6
jb|label192
mov|rdx|m64:rsp:-:0:80
test|rdx|rdx
jz|label192
cmp|rax|5
jbe|label192
mov|rcx|m64:rsp:-:0:112
and|rcx|18446744073709551614
call|external
mov|eax|ebx
mov|rbx|m64:rsp:-:0:424
add|rsp|368
pop|r15
pop|r14
pop|rdi
pop|rsi
pop|rbp
ret
lea|r9|m0:rsp:-:0:64
xor|ecx|ecx
lea|r8|m0:rsp:-:0:96
lea|rdx|rip64
call|external
call|external
int3
lea|r9|m0:rsp:-:0:64
xor|ecx|ecx
lea|r8|m0:rsp:-:0:96
lea|rdx|rip64
call|external
call|external
int3
lea|r9|m0:rsp:-:0:64
xor|ecx|ecx
lea|r8|m0:rsp:-:0:96
lea|rdx|rip64
call|external
call|external
int3
lea|r9|m0:rsp:-:0:64
xor|ecx|ecx
lea|r8|m0:rsp:-:0:96
lea|rdx|rip64
call|external
call|external
int3
)abi";

static constexpr std::string_view queue_recipe = R"abi(mov|rax|m64:rcx:-:0:0
mov|rax|m64:rax:-:0:8
ret
)abi";

static constexpr std::string_view source_recipe = R"abi(mov|eax|m32:rcx:-:0:8
mov|r8|rdx
test|eax|eax
jnz|label7
mov|m32:rdx:-:0:0|eax
mov|rax|rdx
ret
lea|edx|m0:rax:-:0:-1
mov|rax|m64:rcx:-:0:0
add|rdx|rdx
mov|rcx|m64:rax:-:0:@sourceEntityIdsOffset
mov|eax|m32:rcx:rdx:8:0
mov|m32:r8:-:0:0|eax
mov|rax|r8
ret
)abi";

static constexpr std::string_view transform_recipe = R"abi(sub|rsp|24
movsd|xmm2|m64:rdx:-:0:0
mov|r10d|m32:rdx:-:0:8
movaps|xmm0|xmm2
movss|xmm4|rip32
movaps|xmm1|xmm2
movss|xmm3|rip32
mulss|xmm0|xmm4
shufps|xmm1|xmm1|85
mulss|xmm1|xmm4
addss|xmm0|xmm3
addss|xmm1|xmm3
cvttss2si|rax|xmm0
movd|xmm0|r10d
mov|m64:rcx:-:0:0|rax
mulss|xmm0|xmm4
cvttss2si|rax|xmm1
addss|xmm0|xmm3
mov|m64:rcx:-:0:8|rax
cvttss2si|rax|xmm0
mov|m64:rcx:-:0:16|rax
movups|xmm0|m128:r8:-:0:0
movups|m128:rcx:-:0:24|xmm0
movsd|xmm0|m64:r9:-:0:0
movsd|m64:rcx:-:0:40|xmm0
mov|eax|m32:r9:-:0:8
mov|m32:rcx:-:0:48|eax
add|rsp|24
ret
)abi";

static constexpr std::string_view prepare_recipe = R"abi(push|rbx
sub|rsp|96
xor|eax|eax
mov|m32:rsp:-:0:48|r9d
mov|r9d|rip32
mov|rbx|rdx
mov|m16:rsp:-:0:90|ax
movzx|eax|m8:rsp:-:0:160
mov|m8:rsp:-:0:92|al
mov|rax|m64:rsp:-:0:144
movups|xmm0|m128:rax:-:0:0
mov|rax|m64:rsp:-:0:152
movups|m128:rsp:-:0:52|xmm0
movups|xmm1|m128:rax:-:0:0
lea|rax|m0:rsp:-:0:48
mov|m64:rsp:-:0:32|rax
movups|m128:rsp:-:0:72|xmm1
call|external
mov|rax|rbx
add|rsp|96
pop|rbx
ret
)abi";

static constexpr std::string_view append_recipe = R"abi(mov|m64:rsp:-:0:24|rbx
mov|m64:rsp:-:0:32|rsi
push|rdi
push|r14
push|r15
sub|rsp|80
mov|r14|m64:rsp:-:0:144
xor|esi|esi
mov|ebx|r9d
mov|r15|rdx
mov|rdi|rcx
mov|r11d|esi
mov|r10|m64:r14:-:0:32
test|r10|r10
jz|label24
mov|r9|m64:r14:-:0:24
add|r9|16
nop|m16:rax:rax:1:0|ax
movzx|eax|m16:r9:-:0:0
lea|r9|m0:r9:-:0:24
add|r11d|6
add|r11d|eax
sub|r10|1
jnz|label18
mov|rax|m64:r14:-:0:12
or|rax|m64:r14:-:0:4
jnz|label38
movups|xmm0|rip128
xor|r9d|r9d
lea|r8|m0:rsp:-:0:48
lea|rdx|rip64
mov|cl|1
movups|m128:rsp:-:0:48|xmm0
movsd|xmm0|rip64
movsd|m64:rsp:-:0:64|xmm0
call|external
mov|m32:r15:-:0:0|esi
jmp|label92
mov|m32:rsp:-:0:32|r8d
lea|r9d|m0:r11:-:0:44
xor|r8d|r8d
mov|m64:rsp:-:0:112|rbp
lea|rdx|m0:rcx:-:0:16
call|external
mov|rbp|rax
test|rax|rax
jnz|label49
mov|m32:r15:-:0:0|esi
jmp|label91
test|ebx|ebx
jnz|label55
mov|rax|m64:rdi:-:0:56
mov|ebx|1
lock_xadd|m32:rax:-:0:0|ebx
inc|ebx
mov|m32:rbp:-:0:12|ebx
lea|rdi|m0:rbp:-:0:44
mov|eax|m32:r14:-:0:0
mov|m32:rbp:-:0:16|eax
movups|xmm0|m128:r14:-:0:4
movups|m128:rbp:-:0:20|xmm0
movzx|eax|m8:r14:-:0:44
mov|m8:rbp:-:0:37|al
movzx|eax|m16:r14:-:0:42
mov|m16:rbp:-:0:38|ax
movzx|eax|m8:r14:-:0:32
mov|m8:rbp:-:0:40|al
cmp|m64:r14:-:0:32|rsi
jbe|label89
mov|m64:rsp:-:0:120|r12
mov|r12|rsi
mov|rdx|m64:r14:-:0:24
add|rdx|r12
mov|eax|m32:rdx:-:0:0
mov|m32:rdi:-:0:0|eax
movzx|eax|m16:rdx:-:0:16
mov|m16:rdi:-:0:4|ax
add|rdi|6
mov|rbx|m64:rdx:-:0:16
mov|rcx|rdi
mov|rdx|m64:rdx:-:0:8
mov|r8|rbx
call|external
add|rdi|rbx
inc|rsi
add|r12|24
cmp|rsi|m64:r14:-:0:32
jb|label71
mov|r12|m64:rsp:-:0:120
mov|eax|m32:rbp:-:0:12
mov|m32:r15:-:0:0|eax
mov|rbp|m64:rsp:-:0:112
lea|r11|m0:rsp:-:0:80
mov|rax|r15
mov|rbx|m64:r11:-:0:48
mov|rsi|m64:r11:-:0:56
mov|rsp|r11
pop|r15
pop|r14
pop|rdi
ret
)abi";

static constexpr std::string_view allocate_recipe = R"abi(mov|m64:rsp:-:0:16|rsi
push|rdi
sub|rsp|32
mov|r10d|m32:rdx:-:0:16
mov|rdi|rdx
mov|eax|r10d
and|eax|15
jz|label10
sub|r10d|eax
add|r10d|16
lea|esi|m0:r10:r9:1:0
mov|eax|esi
cmp|rax|m64:rdx:-:0:8
jbe|label19
xor|eax|eax
mov|rsi|m64:rsp:-:0:56
add|rsp|32
pop|rdi
ret
mov|edx|r10d
mov|m64:rsp:-:0:48|rbx
mov|rbx|m64:rdi:-:0:0
add|rbx|rdx
mov|m8:rbx:-:0:2|r8b
mov|m16:rbx:-:0:0|r9w
mov|m8:rbx:-:0:3|0
movzx|eax|m16:rcx:-:0:40
mov|m16:rbx:-:0:4|ax
movzx|ecx|m16:rcx:-:0:42
mov|m16:rbx:-:0:6|cx
mov|ecx|m32:rsp:-:0:80
mov|m32:rbx:-:0:8|ecx
mov|rcx|m64:rdi:-:0:0
add|rcx|12
mov|r8d|r9d
add|rcx|rdx
sub|r8|12
xor|edx|edx
call|external
inc|m32:rdi:-:0:20
mov|rax|rbx
mov|rbx|m64:rsp:-:0:48
mov|m32:rdi:-:0:16|esi
mov|rsi|m64:rsp:-:0:56
add|rsp|32
pop|rdi
ret
)abi";


// Component tags are original full-name identities. The instruction contracts
// contain symbolic names, never a manually selected numeric type hash.
std::string component_tag(std::uint64_t value) {
    for (const char* name : {"CurrentTransform", "DisableRandomLootOnDestroy", "UsedItem",
                            "CollisionFeedbackMaterialComponent"}) {
        if (value == ComponentRegistry::hash_name(std::string("keen::ecs::") + name))
            return std::string("#") + name;
    }
    return std::to_string(value);
}

bool absolute(const Instruction& ins, const ZydisDecodedOperand& operand,
              std::uintptr_t& output) {
    ZyanU64 address{};
    if (!ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&ins.decoded, &operand, ins.address, &address)))
        return false;
    output = static_cast<std::uintptr_t>(address);
    return true;
}

std::string canonical(const Procedure& code) {
    std::ostringstream out;
    for (const auto& ins : code.instructions) {
        const auto* mnemonic = ZydisMnemonicGetString(ins.decoded.mnemonic);
        if (!mnemonic) return {};
        if (ins.decoded.attributes & ZYDIS_ATTRIB_HAS_LOCK) out << "lock_";
        out << mnemonic;
        for (unsigned index = 0; index < ins.decoded.operand_count_visible; ++index) {
            const auto& operand = ins.operands[index];
            out << '|';
            if (operand.type == ZYDIS_OPERAND_TYPE_REGISTER) {
                out << ZydisRegisterGetString(operand.reg.value);
            } else if (operand.type == ZYDIS_OPERAND_TYPE_IMMEDIATE) {
                if (operand.imm.is_relative) {
                    std::uintptr_t address{};
                    if (!absolute(ins, operand, address)) return {};
                    const auto found = std::find_if(code.instructions.begin(), code.instructions.end(),
                        [address](const auto& value) { return value.address == address; });
                    if (found == code.instructions.end()) out << "external";
                    else out << "label" << (found - code.instructions.begin());
                } else {
                    out << component_tag(operand.imm.value.u);
                }
            } else if (operand.type == ZYDIS_OPERAND_TYPE_MEMORY) {
                if (operand.mem.segment == ZYDIS_REGISTER_FS || operand.mem.segment == ZYDIS_REGISTER_GS)
                    return {};
                if (operand.mem.base == ZYDIS_REGISTER_RIP) {
                    out << "rip" << operand.size;
                } else {
                    const auto reg = [](ZydisRegister value) -> const char* {
                        return value == ZYDIS_REGISTER_NONE ? "-" : ZydisRegisterGetString(value);
                    };
                    out << 'm' << (ins.decoded.mnemonic == ZYDIS_MNEMONIC_LEA ? 0 : operand.size)
                        << ':' << reg(operand.mem.base) << ':' << reg(operand.mem.index)
                        << ':' << (operand.mem.index == ZYDIS_REGISTER_NONE ? 0 : operand.mem.scale)
                        << ':' << (operand.mem.disp.has_displacement ? operand.mem.disp.value : 0);
                }
            } else return {};
        }
        out << '\n';
    }
    return out.str();
}

bool matches(const Procedure& code, std::string_view expected) {
    return canonical(code) == expected;
}

std::string source_contract_recipe(const EngineContext::BuildingPropSpawnLayout& layout) {
    std::string expected(source_recipe);
    constexpr std::string_view field = "@sourceEntityIdsOffset";
    expected.replace(expected.find(field), field.size(), std::to_string(layout.source_entity_ids));
    return expected;
}

std::string helper_contract_recipe(const EngineContext::BuildingPropSpawnLayout& layout) {
    if (!layout.allocation_failure_marker) return std::string(helper_recipe);
    std::vector<std::string_view> lines;
    for (std::size_t begin = 0; begin < helper_recipe.size();) {
        const auto end = helper_recipe.find('\n', begin);
        if (end == std::string_view::npos) return {};
        lines.push_back(helper_recipe.substr(begin, end - begin));
        begin = end + 1;
    }
    std::vector<std::size_t> insertions;
    for (std::size_t index = 1; index + 1 < lines.size(); ++index)
        if (lines[index - 1] == "call|external" && lines[index] == "call|external" && lines[index + 1] == "int3")
            insertions.push_back(index);
    if (insertions.size() != 4) return {};
    std::ostringstream expected;
    for (std::size_t index = 0; index < lines.size(); ++index) {
        if (std::find(insertions.begin(), insertions.end(), index) != insertions.end())
            expected << "mov|rax|rip64\nmov|m32:rax:-:0:0|" << layout.allocation_failure_marker << '\n';
        const auto line = lines[index];
        const auto label = line.find("|label");
        if (label == std::string_view::npos) expected << line;
        else {
            std::size_t target{};
            const auto digits = line.substr(label + 6);
            const auto parsed = std::from_chars(digits.data(), digits.data() + digits.size(), target);
            if (parsed.ec != std::errc{} || parsed.ptr != digits.data() + digits.size() || target >= lines.size()) return {};
            const auto preceding = std::count_if(insertions.begin(), insertions.end(),
                [target](std::size_t insertion) { return insertion <= target; });
            expected << line.substr(0, label + 6) << target + preceding * 2;
        }
        expected << '\n';
    }
    return expected.str();
}

struct Contract {
    std::uintptr_t helper{}, spawn_flag{}, explicit_id{};
    OriginalCode::CodeEvidence code;
    EngineContext::BuildingPropSpawnLayout layout;
};

bool capture_code(const Procedure& code, Contract& contract) {
    return OriginalCode::AppendEvidence(code, contract.code);
}

bool validate_code(const Contract& contract) {
    return OriginalCode::EvidenceUnchanged(contract.code);
}

bool rip_operands(const Procedure& code, ZydisMnemonic mnemonic,
                  std::vector<std::uintptr_t>& addresses) {
    for (const auto& ins : code.instructions) {
        if (ins.decoded.mnemonic != mnemonic) continue;
        for (unsigned index = 0; index < ins.decoded.operand_count_visible; ++index) {
            const auto& operand = ins.operands[index];
            if (operand.type != ZYDIS_OPERAND_TYPE_MEMORY || operand.mem.base != ZYDIS_REGISTER_RIP)
                continue;
            std::uintptr_t address{};
            if (!absolute(ins, operand, address)) return false;
            addresses.push_back(address);
        }
    }
    return true;
}

// Resolve and allocate every code/descriptor buffer before retaining original
// Scene gates. The complete instruction recipes establish call positions and
// operand data flow before those call targets are assigned semantic roles.
bool resolve_contract(Contract& output) {
    const auto layout = EngineContext::ConfiguredBuildingPropSpawnLayout();
    if (!layout) return false;
    static std::mutex mutex;
    static Contract cached;
    std::scoped_lock lock(mutex);
    if(cached.helper && cached.layout == *layout && validate_code(cached)) { output=cached;return true; }
    const auto expected_helper = helper_contract_recipe(*layout);
    const auto expected_source = source_contract_recipe(*layout);
    if (expected_helper.empty() || expected_source.empty()) return false;
    std::uintptr_t root{}, helper{};
    if (!OriginalCode::ResolveNamedSystemCode("player_building_place_prop", root) ||
        !OriginalCode::FindUniqueReachable(root, 2,
            [&expected_helper](const Procedure& code) { return matches(code, expected_helper); }, helper)) return false;
    Procedure body, queue, source, transform, prepare, append, allocate;
    if (!OriginalCode::ReadProcedure(helper, body) || !matches(body, expected_helper) || body.calls.size() != 13 ||
        !OriginalCode::ReadProcedure(body.calls[0], queue) || !matches(queue, queue_recipe) ||
        !OriginalCode::ReadProcedure(body.calls[1], source) || !matches(source, expected_source) ||
        !OriginalCode::ReadProcedure(body.calls[2], transform) || !matches(transform, transform_recipe) ||
        !OriginalCode::ReadProcedure(body.calls[3], prepare) || !matches(prepare, prepare_recipe) ||
        prepare.calls.size() != 1 || !OriginalCode::ReadProcedure(prepare.calls[0], append) ||
        !matches(append, append_recipe) || append.calls.size() != 3 ||
        !OriginalCode::ReadProcedure(append.calls[1], allocate) || !matches(allocate, allocate_recipe)) return false;
    // The two first leaf reads are exactly the same source-zero queue/iterator
    // contract used by the separately admitted registered destruction adapter.
    // The transform helper performs only fixed-point conversion and POD copies.
    std::vector<std::uintptr_t> constants, flags, defaults;
    if (!rip_operands(transform, ZYDIS_MNEMONIC_MOVSS, constants) || constants.size() != 2 ||
        !rip_operands(body, ZYDIS_MNEMONIC_MOVZX, flags) || flags.size() != 1 ||
        !rip_operands(prepare, ZYDIS_MNEMONIC_MOV, defaults) || defaults.size() != 1) return false;
    float multiplier{}, rounding{};
    if (!NamedServiceImplementation::read(constants[0], multiplier) || multiplier != 4294967296.0f ||
        !NamedServiceImplementation::read(constants[1], rounding) || rounding != 0.5f) return false;
    Contract found;
    found.layout = *layout;
    found.helper = helper;
    found.spawn_flag = flags[0];
    found.explicit_id = defaults[0];
    for (const auto* procedure : {&body, &queue, &source, &transform, &prepare, &append, &allocate})
        if (!capture_code(*procedure, found)) return false;
    Procedure root_body;
    if(!OriginalCode::ReadProcedure(root,root_body) || !capture_code(root_body,found))return false;
    bool linked=std::find(root_body.calls.begin(),root_body.calls.end(),helper)!=root_body.calls.end();
    for(const auto callee:root_body.calls) {
        Procedure intermediate;
        if(OriginalCode::ReadProcedure(callee,intermediate) &&
           std::find(intermediate.calls.begin(),intermediate.calls.end(),helper)!=intermediate.calls.end()) {
            if(!capture_code(intermediate,found))return false;linked=true;
        }
    }
    if(!linked || !validate_code(found))return false;
    cached=found;
    output = std::move(found);
    return true;
}

struct QueryContext {
    std::uintptr_t reserved{}, queue{};
    bool operator==(const QueryContext&) const = default;
};
struct QueryIterator {
    const QueryContext* context{};
    std::uint32_t current_index{}, reserved{};
    bool operator==(const QueryIterator&) const = default;
};
struct BuildingContext {
    const QueryIterator* iterator{};
    bool operator==(const BuildingContext&) const = default;
};
struct Transform {
    std::array<std::int64_t, 3> position{};
    std::array<float, 4> rotation{};
    std::array<float, 3> scale{1, 1, 1};
    std::uint32_t padding{};
};
static_assert(sizeof(QueryContext) == 16 && offsetof(QueryContext, queue) == 8);
static_assert(sizeof(QueryIterator) == 16 && offsetof(QueryIterator, current_index) == 8);
static_assert(sizeof(Transform) == 56);

using NativeCreate = std::uint32_t(__fastcall*)(const BuildingContext*, const std::uint64_t*,
    const float*, const float*, std::uint32_t, std::uint32_t, const std::uint64_t*);

bool invoke(std::uintptr_t callback, const BuildingContext* context, const std::uint64_t* uuid,
    const float* position, const float* rotation, std::uint32_t tracking,
    const std::uint64_t* auxiliary, std::uint32_t& output) {
    __try {
        output = reinterpret_cast<NativeCreate>(callback)(context, uuid, position, rotation, tracking, 0, auxiliary);
        return true;
    } __except (EXCEPTION_EXECUTE_HANDLER) { return false; }
}

struct OwnedInputs {
    std::array<std::uint64_t, 2> uuid{}, auxiliary{};
    std::array<float, 3> position{};
    Transform transform;
    std::uint32_t tracking{};
    bool operator==(const OwnedInputs& other) const {
        return uuid == other.uuid && auxiliary == other.auxiliary && position == other.position &&
            std::memcmp(&transform, &other.transform, sizeof(transform)) == 0 && tracking == other.tracking;
    }
};

bool prepare_inputs(const std::uint64_t uuid[2], const double position[3],
    const double rotation[4], std::uint32_t tracking, std::uint32_t flags, OwnedInputs& input) {
    if (!uuid || !position || !rotation || !(uuid[0] || uuid[1]) || !tracking || flags) return false;
    input.uuid = {uuid[0], uuid[1]};
    input.tracking = tracking;
    for (std::size_t axis = 0; axis < input.position.size(); ++axis) {
        if (!std::isfinite(position[axis])) return false;
        input.position[axis] = static_cast<float>(position[axis]);
        const float scaled = input.position[axis] * 4294967296.0f;
        const float rounded = scaled + 0.5f;
        if (!std::isfinite(rounded) || static_cast<long double>(rounded) < INT64_MIN ||
            static_cast<long double>(rounded) >= 9223372036854775808.0L) return false;
        input.transform.position[axis] = static_cast<std::int64_t>(rounded);
    }
    long double norm{};
    for (std::size_t axis = 0; axis < input.transform.rotation.size(); ++axis) {
        if (!std::isfinite(rotation[axis])) return false;
        norm += static_cast<long double>(rotation[axis]) * rotation[axis];
    }
    if (!std::isfinite(norm) || norm < 1e-12L) return false;
    const auto inverse = 1.0L / std::sqrt(norm);
    for (std::size_t axis = 0; axis < input.transform.rotation.size(); ++axis)
        input.transform.rotation[axis] = static_cast<float>(rotation[axis] * inverse);
    return true;
}

constexpr std::size_t command_size = 123;
using CommandBytes = std::array<std::uint8_t, 160>;
template<class T> void store(CommandBytes& bytes, std::size_t offset, const T& value) {
    std::memcpy(bytes.data() + offset, &value, sizeof(value));
}

CommandBytes expected_command(const EntityCommandQueueSelection& queue, const OwnedInputs& input,
    std::uint32_t entity_id, std::uint8_t flag, const CommandBytes& after) {
    CommandBytes expected{};
    store(expected, 0, std::uint16_t{command_size});
    store(expected, 4, queue.header.system_id);
    store(expected, 6, queue.header.subtask_id);
    store(expected, 12, entity_id);
    std::memcpy(expected.data() + 20, input.uuid.data(), sizeof(input.uuid));
    expected[37] = flag;
    expected[40] = 3;
    store(expected, 44, ComponentRegistry::hash_name("keen::ecs::CurrentTransform"));
    store(expected, 48, std::uint16_t{sizeof(Transform)});
    std::memcpy(expected.data() + 50, &input.transform, sizeof(Transform));
    // The original helper deliberately copies a 56-byte transform whose last
    // four padding bytes are unspecified. They carry no transform semantics.
    std::copy(after.begin() + 102, after.begin() + 106, expected.begin() + 102);
    store(expected, 106, ComponentRegistry::hash_name("keen::ecs::DisableRandomLootOnDestroy"));
    store(expected, 110, std::uint16_t{1});
    store(expected, 113, ComponentRegistry::hash_name("keen::ecs::UsedItem"));
    store(expected, 117, std::uint16_t{4});
    store(expected, 119, input.tracking);
    return expected;
}

bool globals(const Contract& contract, std::uint8_t& flag) {
    std::uint32_t explicit_id{};
    return NamedServiceImplementation::read(contract.spawn_flag, flag) && flag <= 1 &&
        NamedServiceImplementation::read(contract.explicit_id, explicit_id) && !explicit_id;
}
}

bool BuildingPropSpawnAvailable() {
    try {
        Contract contract;
        if (!resolve_contract(contract)) return false;
        const auto binding = NamedServiceImplementation::binding_contract("ecs::Tasks");
        EngineContext::EcsView view;
        if (!EngineContext::ResolveEcs(view) || view.role != EngineContext::EcsRole::LocalAuthority)
            return false;
        std::unique_lock operation_lock(NamedServiceImplementation::operation_mutex, std::try_to_lock);
        if (!operation_lock.owns_lock()) return false;
        EngineContext::EcsWriteGuard scene;
        if (!scene.TryAcquire(view) || !scene.Validate()) return false;
        EntityCommandQueueSelection queue;
        std::uint8_t flag{};
        return validate_code(contract) && globals(contract, flag) &&
            EcsRuntime::ResolveEntityCommandQueue(binding, view, queue) && scene.Validate();
    } catch (...) { return false; }
}

BuildingPropSpawnResult SubmitBuildingPropSpawn(std::uint64_t expected_session,
    const std::uint64_t template_uuid[2], const double position[3], const double rotation[4],
    std::uint32_t tracking, std::uint32_t flags, BuildingPropSpawnReceipt& output) {
    if (!expected_session) return BuildingPropSpawnResult::Rejected;
    bool invocation_started{};
    try {
        OwnedInputs input;
        Contract contract;
        if (!prepare_inputs(template_uuid, position, rotation, tracking, flags, input))
            return BuildingPropSpawnResult::Rejected;
        if (!resolve_contract(contract)) return BuildingPropSpawnResult::Unavailable;
        const auto binding = NamedServiceImplementation::binding_contract("ecs::Tasks");
        std::scoped_lock operation_lock(NamedServiceImplementation::operation_mutex);
        EcsRuntime::SessionGuard session;
        if (!session.TryAcquire(expected_session)) return BuildingPropSpawnResult::StaleSession;
        if (!session.WritesAllowed()) return BuildingPropSpawnResult::Uncertain;
        EngineContext::EcsWriteGuard scene;
        if (!scene.TryAcquire(session.View()) || !scene.Validate()) return BuildingPropSpawnResult::Unavailable;
        EntityCommandQueueSelection queue, repeated;
        std::uint8_t flag{}, after_flag{};
        std::uint32_t counter{}, after_counter{};
        if (!validate_code(contract) || !globals(contract, flag) ||
            !EcsRuntime::ResolveEntityCommandQueue(binding, session.View(), queue) ||
            !NamedServiceImplementation::read(queue.header.entity_id_counter, counter) || counter == UINT32_MAX)
            return BuildingPropSpawnResult::Unavailable;
        ULONG_PTR stack_low{}, stack_high{};
        GetCurrentThreadStackLimits(&stack_low, &stack_high);
        if (!stack_low || !stack_high || stack_low >= stack_high ||
            (queue.header.buffer < stack_high && stack_low < queue.header.buffer + queue.header.capacity))
            return BuildingPropSpawnResult::Unavailable;
        CommandBytes before{}, after{};
        if (!copy(before.data(), reinterpret_cast<const void*>(queue.header.buffer), before.size()) ||
            !EcsRuntime::RereadEntityCommandQueue(binding, session.View(), queue, repeated) ||
            repeated.header != queue.header || !scene.Validate()) return BuildingPropSpawnResult::Unavailable;
        QueryContext query{0, queue.queue_address};
        QueryIterator iterator{&query, 0, 0};
        BuildingContext context{&iterator};
        const auto expected_input = input;
        const auto expected_query = query;
        const auto expected_iterator = iterator;
        const auto expected_context = context;
        std::uint32_t returned{};
        invocation_started = true;
        const bool invoked = invoke(contract.helper, &context, input.uuid.data(), input.position.data(),
            input.transform.rotation.data(), input.tracking, input.auxiliary.data(), returned);
        const bool stable = scene.Validate() && validate_code(contract) && globals(contract, after_flag) &&
            after_flag == flag && EcsRuntime::RereadEntityCommandQueue(binding, session.View(), queue, repeated) &&
            copy(after.data(), reinterpret_cast<const void*>(queue.header.buffer), after.size()) &&
            NamedServiceImplementation::read(queue.header.entity_id_counter, after_counter);
        const bool intact = invoked && input == expected_input && query == expected_query &&
            iterator == expected_iterator && context == expected_context;
        if (stable && intact && !returned && repeated.header == queue.header && before == after &&
            after_counter == counter) return BuildingPropSpawnResult::Rejected;
        auto expected_header = queue.header;
        expected_header.byte_cursor = command_size;
        expected_header.command_count = 1;
        const auto expected = expected_command(queue, expected_input, counter + 1, flag, after);
        if (stable && intact && returned == counter + 1 && after_counter == returned &&
            repeated.header == expected_header && std::equal(expected.begin(), expected.begin() + command_size, after.begin()) &&
            std::equal(before.begin() + command_size, before.end(), after.begin() + command_size)) {
            output = {returned, input.transform.position, input.transform.rotation};
            return BuildingPropSpawnResult::Submitted;
        }
        scene.Release();
        session.MarkWriteUncertain("native-building-spawn-command-outcome-uncertain");
        return BuildingPropSpawnResult::Uncertain;
    } catch (...) {
        if (!invocation_started) return BuildingPropSpawnResult::Unavailable;
        EcsRuntime::MarkSessionWriteUncertain(expected_session, "native-building-spawn-command-exception");
        return BuildingPropSpawnResult::Uncertain;
    }
}
}
