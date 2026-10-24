#include "main-menu/resolve_character_selection.h"
#include "named-services/named_service_contract.h"
#include <algorithm>
#include <deque>
#include <limits>
#include <map>
#include <optional>
#include <set>

namespace NativeMainMenu {
namespace {
using OriginalCode::Instruction;
using OriginalCode::Procedure;
using NamedServiceImplementation::copy;

ZydisRegister full(ZydisRegister value) {
    return ZydisRegisterGetLargestEnclosing(ZYDIS_MACHINE_MODE_LONG_64, value);
}
bool reg(const ZydisDecodedOperand& operand, ZydisRegister value) {
    return operand.type == ZYDIS_OPERAND_TYPE_REGISTER && operand.reg.value == value;
}
bool nonvolatile(ZydisRegister value) {
    return value == ZYDIS_REGISTER_RBX || value == ZYDIS_REGISTER_RBP || value == ZYDIS_REGISTER_RSI ||
        value == ZYDIS_REGISTER_RDI || value == ZYDIS_REGISTER_R12 || value == ZYDIS_REGISTER_R13 ||
        value == ZYDIS_REGISTER_R14 || value == ZYDIS_REGISTER_R15;
}
bool writes(const Instruction& instruction, ZydisRegister value) {
    for (unsigned index = 0; index < instruction.decoded.operand_count; ++index) {
        const auto& operand = instruction.operands[index];
        if (operand.type == ZYDIS_OPERAND_TYPE_REGISTER &&
            (operand.actions & ZYDIS_OPERAND_ACTION_MASK_WRITE) && full(operand.reg.value) == full(value)) return true;
    }
    return false;
}
bool straight(const Instruction& instruction) {
    const auto category = instruction.decoded.meta.category;
    return category != ZYDIS_CATEGORY_CALL && category != ZYDIS_CATEGORY_COND_BR &&
        category != ZYDIS_CATEGORY_UNCOND_BR && category != ZYDIS_CATEGORY_RET &&
        category != ZYDIS_CATEGORY_INTERRUPT && instruction.decoded.mnemonic != ZYDIS_MNEMONIC_UD2;
}
std::optional<std::uintptr_t> target(const Instruction& instruction) {
    if (!instruction.decoded.operand_count_visible || instruction.operands[0].type != ZYDIS_OPERAND_TYPE_IMMEDIATE ||
        !instruction.operands[0].imm.is_relative) return {};
    ZyanU64 address{};
    if (!ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&instruction.decoded, &instruction.operands[0], instruction.address, &address))) return {};
    return static_cast<std::uintptr_t>(address);
}
std::optional<std::uintptr_t> call(const Instruction& instruction) {
    return instruction.decoded.mnemonic == ZYDIS_MNEMONIC_CALL ? target(instruction) : std::nullopt;
}
std::optional<std::int64_t> memory(const Instruction& instruction, unsigned index, ZydisRegister base, unsigned bits) {
    const auto& operand = instruction.operands[index];
    if (operand.type != ZYDIS_OPERAND_TYPE_MEMORY || operand.mem.base != base ||
        operand.mem.index != ZYDIS_REGISTER_NONE || operand.size != bits) return {};
    return operand.mem.disp.value;
}
std::optional<std::int64_t> immediate(const Instruction& instruction, unsigned index) {
    const auto& operand = instruction.operands[index];
    if (operand.type != ZYDIS_OPERAND_TYPE_IMMEDIATE || operand.imm.is_relative) return {};
    if (operand.imm.is_signed) return operand.imm.value.s;
    if (operand.imm.value.u > static_cast<std::uint64_t>((std::numeric_limits<std::int64_t>::max)())) return {};
    return static_cast<std::int64_t>(operand.imm.value.u);
}
std::optional<std::size_t> positive(std::int64_t value) {
    if (value <= 0 || value > 16 * 1024 * 1024) return {};
    return static_cast<std::size_t>(value);
}
std::optional<std::size_t> offset(std::optional<std::int64_t> value) {
    if (!value || *value < 0 || *value > 16 * 1024 * 1024) return {};
    return static_cast<std::size_t>(*value);
}
bool pair(const Instruction& instruction, ZydisMnemonic mnemonic, ZydisRegister left, ZydisRegister right) {
    return instruction.decoded.mnemonic == mnemonic && reg(instruction.operands[0], left) && reg(instruction.operands[1], right);
}
bool branch(const Instruction& instruction, ZydisMnemonic mnemonic, std::uintptr_t destination) {
    return instruction.decoded.mnemonic == mnemonic && instruction.decoded.meta.category == ZYDIS_CATEGORY_COND_BR &&
        target(instruction) == std::optional<std::uintptr_t>(destination);
}
std::optional<std::uintptr_t> rip_address(const Instruction& instruction, unsigned index) {
    const auto& operand = instruction.operands[index];
    if (operand.type != ZYDIS_OPERAND_TYPE_MEMORY || operand.mem.base != ZYDIS_REGISTER_RIP) return {};
    ZyanU64 address{};
    if (!ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&instruction.decoded, &operand, instruction.address, &address))) return {};
    return static_cast<std::uintptr_t>(address);
}
bool named_anchor(const Instruction& instruction, const char* name) {
    if (instruction.decoded.mnemonic != ZYDIS_MNEMONIC_LEA) return false;
    const auto address = rip_address(instruction, 1);
    const auto length = std::strlen(name) + 1;
    std::array<char, 64> text{};
    return address && length <= text.size() && copy(text.data(), reinterpret_cast<const void*>(*address), length) &&
        std::memcmp(text.data(), name, length) == 0;
}
std::optional<std::size_t> preceding_call(const std::vector<Instruction>& code, std::size_t before) {
    auto next = code[before].address;
    for (auto index = before; index > 0 && before - index < 8;) {
        const auto& instruction = code[--index];
        if (instruction.address + instruction.decoded.length != next) return {};
        if (call(instruction)) return index;
        if (!straight(instruction)) return {};
        next = instruction.address;
    }
    return {};
}
bool argument_from(const std::vector<Instruction>& code, std::size_t at, ZydisRegister value) {
    auto next = code[at].address;
    for (auto index = at; index > 0 && at - index < 12;) {
        const auto& instruction = code[--index];
        if (instruction.address + instruction.decoded.length != next || !straight(instruction)) return false;
        if (writes(instruction, ZYDIS_REGISTER_RDX))
            return pair(instruction, ZYDIS_MNEMONIC_MOV, ZYDIS_REGISTER_RDX, value);
        next = instruction.address;
    }
    return false;
}
std::vector<std::uintptr_t> successors(const Instruction& instruction) {
    const auto category = instruction.decoded.meta.category;
    if (category == ZYDIS_CATEGORY_RET || category == ZYDIS_CATEGORY_INTERRUPT || instruction.decoded.mnemonic == ZYDIS_MNEMONIC_UD2) return {};
    if (category == ZYDIS_CATEGORY_UNCOND_BR) {
        const auto destination = target(instruction);
        return destination ? std::vector<std::uintptr_t>{*destination} : std::vector<std::uintptr_t>{};
    }
    std::vector<std::uintptr_t> result{instruction.address + instruction.decoded.length};
    if (category == ZYDIS_CATEGORY_COND_BR) {
        if (const auto destination = target(instruction)) result.push_back(*destination);
    }
    return result;
}
bool retained_result_reaches(const Procedure& builder, std::uintptr_t retained_at, std::uintptr_t formatter, ZydisRegister value) {
    std::map<std::uintptr_t, const Instruction*> code;
    std::map<std::uintptr_t, std::vector<std::uintptr_t>> predecessors;
    for (const auto& instruction : builder.instructions) {
        code.emplace(instruction.address, &instruction);
        for (const auto next : successors(instruction)) predecessors[next].push_back(instruction.address);
    }
    std::set<std::uintptr_t> ancestors;
    std::deque<std::uintptr_t> pending{formatter};
    while (!pending.empty()) {
        const auto at = pending.front(); pending.pop_front();
        if (!ancestors.insert(at).second) continue;
        for (const auto previous : predecessors[at]) pending.push_back(previous);
    }
    if (!ancestors.contains(retained_at)) return false;
    std::set<std::uintptr_t> visited;
    pending = {builder.entry};
    while (!pending.empty()) {
        const auto at = pending.front(); pending.pop_front();
        if (at == retained_at || !visited.insert(at).second) continue;
        if (at == formatter) return false;
        const auto instruction = code.find(at);
        if (instruction != code.end()) for (const auto next : successors(*instruction->second)) pending.push_back(next);
    }
    const auto retained = code.find(retained_at);
    if (retained == code.end()) return false;
    visited.clear();
    for (const auto next : successors(*retained->second)) pending.push_back(next);
    while (!pending.empty()) {
        const auto at = pending.front(); pending.pop_front();
        if (at == formatter || !ancestors.contains(at) || !visited.insert(at).second) continue;
        const auto instruction = code.find(at);
        if (instruction == code.end() || writes(*instruction->second, value)) return false;
        for (const auto next : successors(*instruction->second)) pending.push_back(next);
    }
    return true;
}
bool resolve_name_span(const Procedure& formatter, std::size_t& result) {
    std::vector<std::size_t> spans;
    bool input_valid = true;
    for (std::size_t index = 0; index < formatter.instructions.size(); ++index) {
        const auto& instruction = formatter.instructions[index];
        if (!straight(instruction)) break;
        const auto mnemonic = instruction.decoded.mnemonic;
        const auto span = offset(memory(instruction, 1, ZYDIS_REGISTER_RDX, 128));
        if (input_valid && span && (mnemonic == ZYDIS_MNEMONIC_MOVUPS || mnemonic == ZYDIS_MNEMONIC_MOVDQU) &&
            instruction.operands[0].type == ZYDIS_OPERAND_TYPE_REGISTER && instruction.operands[0].size == 128) {
            const auto vector = instruction.operands[0].reg.value;
            for (auto next = index + 1; next < formatter.instructions.size(); ++next) {
                const auto& following = formatter.instructions[next];
                if (!straight(following) || writes(following, vector)) break;
                const auto& destination = following.operands[0];
                if ((following.decoded.mnemonic == ZYDIS_MNEMONIC_MOVUPS || following.decoded.mnemonic == ZYDIS_MNEMONIC_MOVDQU) &&
                    destination.type == ZYDIS_OPERAND_TYPE_MEMORY && destination.size == 128 &&
                    (destination.mem.base == ZYDIS_REGISTER_RSP || destination.mem.base == ZYDIS_REGISTER_RBP) &&
                    destination.mem.index == ZYDIS_REGISTER_NONE && reg(following.operands[1], vector)) {
                    spans.push_back(*span); break;
                }
            }
        }
        if (writes(instruction, ZYDIS_REGISTER_RDX)) input_valid = false;
    }
    if (spans.size() != 1) return false;
    result = spans.front();
    return true;
}
struct LoopFields {
    std::size_t source_stride{}, source_count{}, pointer{}, count{}, stride{}, selected{}, entry{};
};
std::optional<LoopFields> selection_loop(const std::vector<Instruction>& instructions, std::size_t start,
    ZydisRegister owner, ZydisRegister sources, ZydisRegister end, std::size_t span) {
    if (start >= instructions.size() || instructions.size() - start < 24) return {};
    const auto* code = instructions.data() + start;
    const auto size = instructions.size() - start;
    if (code[0].decoded.mnemonic != ZYDIS_MNEMONIC_MOV || code[0].operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER ||
        code[0].operands[0].size != 64 || code[2].decoded.mnemonic != ZYDIS_MNEMONIC_MOV ||
        !reg(code[2].operands[0], ZYDIS_REGISTER_RAX)) return {};
    const auto count_register = code[0].operands[0].reg.value;
    const auto count = offset(memory(code[0], 1, sources, 64));
    const auto pointer = offset(memory(code[2], 1, sources, 64));
    if (code[1].operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER) return {};
    const auto index = code[1].operands[0].reg.value;
    if (!count || !pointer || code[1].operands[0].size != 32 || !pair(code[1], ZYDIS_MNEMONIC_XOR, index, index) ||
        !pair(code[3], ZYDIS_MNEMONIC_TEST, count_register, count_register) ||
        !branch(code[4], ZYDIS_MNEMONIC_JZ, code[19].address) || code[5].decoded.mnemonic != ZYDIS_MNEMONIC_MOVZX ||
        code[7].decoded.mnemonic != ZYDIS_MNEMONIC_MOVZX || code[5].operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER ||
        code[7].operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER || code[5].operands[0].size != 32 || code[7].operands[0].size != 32) return {};
    const auto selected = offset(memory(code[5], 1, owner, 8));
    const auto entry_relative = memory(code[7], 1, ZYDIS_REGISTER_RAX, 8);
    const auto bias_value = immediate(code[6], 1);
    if (!selected || !entry_relative || !bias_value || !positive(*bias_value) ||
        code[6].decoded.mnemonic != ZYDIS_MNEMONIC_ADD || !reg(code[6].operands[0], ZYDIS_REGISTER_RAX)) return {};
    const auto bias = *positive(*bias_value);
    if (*entry_relative < -static_cast<std::int64_t>(bias) || *entry_relative > 16 * 1024 * 1024) return {};
    const auto entry = static_cast<std::size_t>(static_cast<std::int64_t>(bias) + *entry_relative);
    if (code[8].operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER || code[8].operands[1].type != ZYDIS_OPERAND_TYPE_REGISTER) return {};
    const auto selected_byte = code[8].operands[1].reg.value;
    const auto entry_byte = code[8].operands[0].reg.value;
    const std::set<ZydisRegister> roles{owner, sources, end, ZYDIS_REGISTER_RAX, count_register, full(index),
        full(code[5].operands[0].reg.value), full(code[7].operands[0].reg.value)};
    if (roles.size() != 8 || full(selected_byte) != full(code[5].operands[0].reg.value) ||
        full(entry_byte) != full(code[7].operands[0].reg.value) ||
        !pair(code[8], ZYDIS_MNEMONIC_CMP, entry_byte, selected_byte) ||
        !branch(code[9], ZYDIS_MNEMONIC_JNZ, code[15].address) || !pair(code[10], ZYDIS_MNEMONIC_TEST, entry_byte, entry_byte)) return {};
    const auto found = target(code[11]);
    if (!found || !branch(code[11], ZYDIS_MNEMONIC_JZ, *found) || code[12].decoded.mnemonic != ZYDIS_MNEMONIC_MOVZX ||
        !reg(code[12].operands[0], code[7].operands[0].reg.value) ||
        memory(code[12], 1, owner, 8) != std::optional<std::int64_t>(static_cast<std::int64_t>(*selected + 1)) ||
        code[13].decoded.mnemonic != ZYDIS_MNEMONIC_CMP ||
        memory(code[13], 0, ZYDIS_REGISTER_RAX, 8) != std::optional<std::int64_t>(*entry_relative + 1) ||
        !reg(code[13].operands[1], entry_byte) || !branch(code[14], ZYDIS_MNEMONIC_JZ, *found) ||
        code[15].decoded.mnemonic != ZYDIS_MNEMONIC_INC || !reg(code[15].operands[0], full(index)) ||
        code[16].decoded.mnemonic != ZYDIS_MNEMONIC_ADD || !reg(code[16].operands[0], ZYDIS_REGISTER_RAX) ||
        !pair(code[17], ZYDIS_MNEMONIC_CMP, full(index), count_register) || !branch(code[18], ZYDIS_MNEMONIC_JB, code[7].address) ||
        code[19].decoded.mnemonic != ZYDIS_MNEMONIC_ADD || !reg(code[19].operands[0], sources) ||
        !pair(code[20], ZYDIS_MNEMONIC_CMP, sources, end) || !branch(code[21], ZYDIS_MNEMONIC_JNZ, code[0].address) ||
        !pair(code[22], ZYDIS_MNEMONIC_XOR, ZYDIS_REGISTER_EAX, ZYDIS_REGISTER_EAX)) return {};
    const auto stride_value = immediate(code[16], 1), source_value = immediate(code[19], 1);
    if (!stride_value || !source_value) return {};
    const auto stride = positive(*stride_value), source_stride = positive(*source_value);
    if (!stride || !source_stride || span % *source_stride || span / *source_stride > 64) return {};
    bool null_return{};
    for (std::size_t at = 23; at < size && at < 35; ++at) {
        if (code[at].decoded.meta.category == ZYDIS_CATEGORY_RET) { null_return = true; break; }
        if (!straight(code[at]) || writes(code[at], ZYDIS_REGISTER_RAX)) return {};
    }
    if (!null_return) return {};
    const auto found_instruction = std::find_if(code, code + size, [&](const Instruction& instruction) { return instruction.address == *found; });
    if (found_instruction == code + size) return {};
    bool unbias{}, found_return{};
    for (auto at = found_instruction; at != code + size && at - found_instruction < 12; ++at) {
        if (at->decoded.meta.category == ZYDIS_CATEGORY_RET) { found_return = true; break; }
        if (!straight(*at)) return {};
        if (writes(*at, ZYDIS_REGISTER_RAX)) {
            if (unbias || at->decoded.mnemonic != ZYDIS_MNEMONIC_ADD || !reg(at->operands[0], ZYDIS_REGISTER_RAX) ||
                immediate(*at, 1) != std::optional<std::int64_t>(-static_cast<std::int64_t>(bias))) return {};
            unbias = true;
        }
    }
    if (!unbias || !found_return) return {};
    return LoopFields{*source_stride, span / *source_stride, *pointer, *count, *stride, *selected, entry};
}
bool writable_image_slot(std::uintptr_t address) {
    MEMORY_BASIC_INFORMATION page{};
    if (VirtualQuery(reinterpret_cast<const void*>(address), &page, sizeof(page)) != sizeof(page) ||
        page.State != MEM_COMMIT || page.Type != MEM_IMAGE || page.AllocationBase != GetModuleHandleW(nullptr) ||
        (page.Protect & (PAGE_GUARD | PAGE_NOACCESS))) return false;
    const auto protection = page.Protect & 0xff;
    const auto begin = reinterpret_cast<std::uintptr_t>(page.BaseAddress);
    return (protection == PAGE_READWRITE || protection == PAGE_WRITECOPY) && address >= begin &&
        page.RegionSize >= sizeof(std::uintptr_t) && address - begin <= page.RegionSize - sizeof(std::uintptr_t);
}
bool resolve_getter(const Procedure& getter, CharacterSelectionBinding& result) {
    std::vector<Instruction> code;
    for (const auto& instruction : getter.instructions)
        if (instruction.decoded.mnemonic != ZYDIS_MNEMONIC_NOP) code.push_back(instruction);
    std::vector<std::size_t> globals;
    for (std::size_t index = 0; index < code.size(); ++index) {
        const auto& instruction = code[index];
        if (instruction.decoded.mnemonic == ZYDIS_MNEMONIC_MOV && instruction.operands[0].type == ZYDIS_OPERAND_TYPE_REGISTER &&
            instruction.operands[0].size == 64 && instruction.operands[1].size == 64 && rip_address(instruction, 1)) globals.push_back(index);
    }
    if (globals.size() != 1 || globals.front() + 2 >= code.size()) return false;
    const auto global = globals.front();
    const auto context = rip_address(code[global], 1);
    if (!context || !writable_image_slot(*context)) return false;
    const auto& first = code[global + 1];
    const auto& second = code[global + 2];
    if (first.decoded.mnemonic != ZYDIS_MNEMONIC_MOV || first.operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER ||
        first.operands[0].size != 64 || second.decoded.mnemonic != ZYDIS_MNEMONIC_MOV ||
        second.operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER || second.operands[0].size != 64) return false;
    const auto first_offset = offset(memory(first, 1, code[global].operands[0].reg.value, 64));
    const auto second_offset = offset(memory(second, 1, first.operands[0].reg.value, 64));
    const auto owner = second.operands[0].reg.value;
    std::vector<std::size_t> owners;
    for (auto index = global + 3; index < code.size(); ++index) {
        const auto value = memory(code[index], 1, owner, 64);
        if (code[index].decoded.mnemonic == ZYDIS_MNEMONIC_MOV && reg(code[index].operands[0], owner) &&
            value && *value > 0) owners.push_back(index);
    }
    if (!first_offset || !second_offset || owners.size() != 1 || owners.front() + 2 >= code.size()) return false;
    const auto owner_index = owners.front();
    for (auto index = global + 3; index < owner_index; ++index) if (writes(code[index], owner)) return false;
    const auto owner_offset = offset(memory(code[owner_index], 1, owner, 64));
    const auto& source = code[owner_index + 1];
    const auto& end = code[owner_index + 2];
    if (!owner_offset || source.decoded.mnemonic != ZYDIS_MNEMONIC_LEA || source.operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER ||
        source.operands[0].size != 64 || end.decoded.mnemonic != ZYDIS_MNEMONIC_LEA ||
        end.operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER || end.operands[0].size != 64) return false;
    const auto sources_offset = memory(source, 1, owner, 64);
    const auto span_value = memory(end, 1, source.operands[0].reg.value, 64);
    if (!sources_offset || !span_value) return false;
    const auto sources = positive(*sources_offset), span = positive(*span_value);
    if (!sources || !span) return false;
    std::vector<LoopFields> loops;
    for (auto index = owner_index + 3; index + 23 < code.size(); ++index)
        if (auto loop = selection_loop(code, index, owner, source.operands[0].reg.value, end.operands[0].reg.value, *span)) loops.push_back(*loop);
    if (loops.size() != 1) return false;
    const auto& loop = loops.front();
    if (loop.selected < *sources + *span || loop.stride < loop.entry + 2 || loop.pointer == loop.count) return false;
    result.context_slot = *context;
    result.owner_path = {*first_offset, *second_offset, *owner_offset};
    result.sources_offset = *sources; result.source_stride = loop.source_stride; result.source_count = loop.source_count;
    result.entries_pointer_offset = loop.pointer; result.entries_count_offset = loop.count; result.entry_stride = loop.stride;
    result.selected_id_offset = loop.selected; result.entry_id_offset = loop.entry;
    return true;
}
}

bool ResolveCharacterSelection(const Procedure& builder, CharacterSelectionBinding& output) {
    const auto fail = [&](const char* error) { output = {}; output.error = error; return false; };
    try {
        std::vector<std::size_t> anchors;
        std::size_t menu_anchors{};
        for (std::size_t index = 0; index < builder.instructions.size(); ++index) {
            if (named_anchor(builder.instructions[index], "characterTextGroup")) anchors.push_back(index);
            if (named_anchor(builder.instructions[index], "mainMenuEntries")) ++menu_anchors;
        }
        if (anchors.size() != 1 || menu_anchors != 1) return fail("The original menu and character-name builder is missing or ambiguous");
        const auto anchor = anchors.front();
        const auto getter_call = preceding_call(builder.instructions, anchor);
        if (!getter_call) return fail("The original character-name group has no preceding selected-character getter");
        const auto getter_address = call(builder.instructions[*getter_call]);
        if (!getter_address || !NamedServiceImplementation::executable_callback(*getter_address))
            return fail("The selected-character getter is outside original executable code");
        std::vector<std::size_t> copies;
        for (auto index = *getter_call + 1; index < anchor; ++index) {
            const auto& instruction = builder.instructions[index];
            if (instruction.decoded.mnemonic == ZYDIS_MNEMONIC_MOV && instruction.operands[0].type == ZYDIS_OPERAND_TYPE_REGISTER &&
                instruction.operands[0].size == 64 && nonvolatile(instruction.operands[0].reg.value) &&
                reg(instruction.operands[1], ZYDIS_REGISTER_RAX)) copies.push_back(index);
        }
        if (copies.size() != 1) return fail("The original menu does not retain exactly one character-getter result");
        const auto retained_at = copies.front();
        const auto retained = builder.instructions[retained_at].operands[0].reg.value;
        for (auto index = *getter_call + 1; index < retained_at; ++index)
            if (writes(builder.instructions[index], ZYDIS_REGISTER_RAX)) return fail("The original menu replaces the selected-character return");
        std::set<std::uintptr_t> formatters;
        for (auto index = anchor + 1; index < builder.instructions.size(); ++index) {
            const auto& instruction = builder.instructions[index];
            if (instruction.address - builder.instructions[anchor].address > 8192) break;
            const auto destination = call(instruction);
            if (destination && argument_from(builder.instructions, index, retained) &&
                retained_result_reaches(builder, builder.instructions[retained_at].address, instruction.address, retained)) formatters.insert(*destination);
        }
        if (formatters.size() != 1) return fail("The original selected-character name formatter is missing or ambiguous");
        Procedure getter, formatter;
        if (!OriginalCode::ReadProcedure(*getter_address, getter) || !OriginalCode::ReadProcedure(*formatters.begin(), formatter))
            return fail("The original character getter or formatter has no complete compiler contract");
        CharacterSelectionBinding result;
        if (!resolve_name_span(formatter, result.name_span_offset)) return fail("The original name formatter does not copy one UTF-8 pointer-length span");
        if (!resolve_getter(getter, result)) return fail("The original character getter has an unsupported owner, identity or container contract");
        if (result.name_span_offset > result.entry_stride || 16 > result.entry_stride - result.name_span_offset)
            return fail("The original character-name span is outside its entry");
        if (!OriginalCode::AppendEvidence(getter, result.evidence) || !OriginalCode::AppendEvidence(formatter, result.evidence) ||
            !OriginalCode::EvidenceUnchanged(result.evidence)) return fail("The original character getter or formatter changed during discovery");
        output = std::move(result);
        return true;
    } catch (...) { return fail("The original character-selection resolver failed"); }
}
}
