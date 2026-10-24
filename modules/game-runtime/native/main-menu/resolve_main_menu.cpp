#include "main-menu/resolve_main_menu.h"
#include "named-services/named_service_contract.h"
#include <algorithm>
#include <cstring>
#include <iterator>
#include <map>
#include <set>

namespace NativeMainMenu {
namespace {
using OriginalCode::Instruction;
using OriginalCode::Procedure;
using NamedServiceImplementation::copy;
constexpr char menu_anchor[] = "mainMenuEntries";

bool reg(const ZydisDecodedOperand& operand, ZydisRegister value) {
    return operand.type == ZYDIS_OPERAND_TYPE_REGISTER && operand.reg.value == value;
}
bool direct_target(const Instruction& instruction, std::uintptr_t& target) {
    if (instruction.decoded.mnemonic != ZYDIS_MNEMONIC_CALL || instruction.decoded.length != 5 ||
        instruction.bytes[0] != 0xe8 || !instruction.decoded.operand_count_visible ||
        instruction.operands[0].type != ZYDIS_OPERAND_TYPE_IMMEDIATE || !instruction.operands[0].imm.is_relative) return false;
    ZyanU64 address{};
    if (!ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&instruction.decoded, &instruction.operands[0], instruction.address, &address)) ||
        !NamedServiceImplementation::executable_callback(static_cast<std::uintptr_t>(address))) return false;
    target = static_cast<std::uintptr_t>(address);
    return true;
}
bool lea_target(const Instruction& instruction, std::uintptr_t address) {
    if (instruction.decoded.mnemonic != ZYDIS_MNEMONIC_LEA || instruction.decoded.operand_count_visible != 2 ||
        instruction.operands[1].type != ZYDIS_OPERAND_TYPE_MEMORY || instruction.operands[1].mem.base != ZYDIS_REGISTER_RIP) return false;
    ZyanU64 target{};
    return ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&instruction.decoded, &instruction.operands[1], instruction.address, &target)) &&
        static_cast<std::uintptr_t>(target) == address;
}
bool test_bool(const Instruction& instruction) {
    return instruction.decoded.mnemonic == ZYDIS_MNEMONIC_TEST && instruction.decoded.operand_count_visible == 2 &&
        reg(instruction.operands[0], ZYDIS_REGISTER_AL) && reg(instruction.operands[1], ZYDIS_REGISTER_AL);
}
bool consecutive(const Instruction& a, const Instruction& b) {
    return a.address + a.decoded.length == b.address;
}
bool string_reference(const Procedure& code, const char* name) {
    const auto size = std::strlen(name) + 1;
    for (const auto& instruction : code.instructions) {
        if (instruction.decoded.mnemonic != ZYDIS_MNEMONIC_LEA || instruction.decoded.operand_count_visible != 2 ||
            instruction.operands[1].type != ZYDIS_OPERAND_TYPE_MEMORY || instruction.operands[1].mem.base != ZYDIS_REGISTER_RIP) continue;
        ZyanU64 address{};
        if (!ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&instruction.decoded, &instruction.operands[1], instruction.address, &address))) continue;
        std::array<char, 64> text{};
        if (size <= text.size() && copy(text.data(), reinterpret_cast<const void*>(static_cast<std::uintptr_t>(address)), size) &&
            std::memcmp(text.data(), name, size) == 0) return true;
    }
    return false;
}

// The reviewed two-pointer helper preserves the incoming state and descriptor
// in nonvolatile registers. It consumes a 24-byte string view followed by byte
// flags, updates state hover bounds, and produces a byte activation result.
bool entry_contract(const Procedure& code) {
    ZydisRegister state = ZYDIS_REGISTER_NONE, descriptor = ZYDIS_REGISTER_NONE;
    for (const auto& instruction : code.instructions) {
        if (instruction.decoded.meta.category == ZYDIS_CATEGORY_CALL ||
            instruction.decoded.meta.category == ZYDIS_CATEGORY_COND_BR ||
            instruction.decoded.meta.category == ZYDIS_CATEGORY_UNCOND_BR) break;
        if (instruction.decoded.mnemonic != ZYDIS_MNEMONIC_MOV || instruction.decoded.operand_count_visible != 2 ||
            instruction.operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER || instruction.operands[0].size != 64) continue;
        if (reg(instruction.operands[1], ZYDIS_REGISTER_RCX)) state = instruction.operands[0].reg.value;
        if (reg(instruction.operands[1], ZYDIS_REGISTER_RDX)) descriptor = instruction.operands[0].reg.value;
    }
    const auto nonvolatile = [](ZydisRegister value) {
        return value == ZYDIS_REGISTER_RBX || value == ZYDIS_REGISTER_RBP || value == ZYDIS_REGISTER_RSI ||
            value == ZYDIS_REGISTER_RDI || value == ZYDIS_REGISTER_R12 || value == ZYDIS_REGISTER_R13 ||
            value == ZYDIS_REGISTER_R14 || value == ZYDIS_REGISTER_R15;
    };
    if (!nonvolatile(state) || !nonvolatile(descriptor) || state == descriptor) return false;
    bool string_head{}, string_hash{}, state_byte{}, state_bounds{}, bool_result{}, returns{};
    std::set<std::int64_t> flags;
    for (const auto& instruction : code.instructions) {
        if (instruction.decoded.mnemonic == ZYDIS_MNEMONIC_MOVZX && instruction.decoded.operand_count_visible == 2 &&
            reg(instruction.operands[0], ZYDIS_REGISTER_EAX) && instruction.operands[1].type == ZYDIS_OPERAND_TYPE_REGISTER &&
            instruction.operands[1].size == 8) bool_result = true;
        if (instruction.decoded.meta.category == ZYDIS_CATEGORY_RET) returns = true;
        for (unsigned index = 0; index < instruction.decoded.operand_count; ++index) {
            const auto& operand = instruction.operands[index];
            if (operand.type != ZYDIS_OPERAND_TYPE_MEMORY || operand.mem.index != ZYDIS_REGISTER_NONE) continue;
            const auto displacement = operand.mem.disp.value;
            const bool read = (operand.actions & ZYDIS_OPERAND_ACTION_MASK_READ) != 0;
            const bool write = (operand.actions & ZYDIS_OPERAND_ACTION_MASK_WRITE) != 0;
            if (operand.mem.base == descriptor && read) {
                if (!displacement && operand.size == 128) string_head = true;
                if (displacement == 16 && operand.size == 64) string_hash = true;
                if (displacement >= 24 && displacement < 32 && operand.size == 8) flags.insert(displacement);
            }
            if (operand.mem.base == state && !displacement && operand.size == 8 && write) state_byte = true;
            if (operand.mem.base == state && displacement == 4 && operand.size == 128 && write) state_bounds = true;
        }
    }
    return string_head && string_hash && state_byte && state_bounds && bool_result && returns && flags.size() >= 4 &&
        string_reference(code, "interactionFrame") && string_reference(code, "interactionFrameLayoutLimiter");
}

bool pointer_arguments(const Procedure& builder, std::size_t call_index) {
    bool state{}, descriptor{};
    for (auto index = call_index; index > 0 && call_index - index < 32;) {
        const auto& instruction = builder.instructions[--index];
        if (!consecutive(instruction, builder.instructions[index + 1]) ||
            instruction.decoded.meta.category == ZYDIS_CATEGORY_CALL ||
            instruction.decoded.meta.category == ZYDIS_CATEGORY_COND_BR ||
            instruction.decoded.meta.category == ZYDIS_CATEGORY_UNCOND_BR) break;
        for (unsigned operand_index = 0; operand_index < instruction.decoded.operand_count; ++operand_index) {
            const auto& operand = instruction.operands[operand_index];
            if (operand.type != ZYDIS_OPERAND_TYPE_REGISTER || !(operand.actions & ZYDIS_OPERAND_ACTION_MASK_WRITE)) continue;
            const auto full = ZydisRegisterGetLargestEnclosing(ZYDIS_MACHINE_MODE_LONG_64, operand.reg.value);
            if (!state && full == ZYDIS_REGISTER_RCX) {
                if (instruction.decoded.operand_count_visible != 2 ||
                    !reg(instruction.operands[0], ZYDIS_REGISTER_RCX)) return false;
                const auto& source = instruction.operands[1];
                const auto nonvolatile_base = [](ZydisRegister base) {
                    return base == ZYDIS_REGISTER_RBX || base == ZYDIS_REGISTER_RBP || base == ZYDIS_REGISTER_RSI ||
                        base == ZYDIS_REGISTER_RDI || base == ZYDIS_REGISTER_R12 || base == ZYDIS_REGISTER_R13 ||
                        base == ZYDIS_REGISTER_R14 || base == ZYDIS_REGISTER_R15;
                };
                // The two engine paths pass either a retained pointer or the
                // address of an owner's selection subobject. Forward it as-is.
                const bool retained = instruction.decoded.mnemonic == ZYDIS_MNEMONIC_MOV &&
                    source.type == ZYDIS_OPERAND_TYPE_REGISTER && source.size == 64 && nonvolatile_base(source.reg.value);
                const bool subobject = instruction.decoded.mnemonic == ZYDIS_MNEMONIC_LEA &&
                    source.type == ZYDIS_OPERAND_TYPE_MEMORY && nonvolatile_base(source.mem.base) &&
                    source.mem.index == ZYDIS_REGISTER_NONE && source.mem.disp.value >= 0;
                if (!retained && !subobject) return false;
                state = true;
            }
            if (!descriptor && full == ZYDIS_REGISTER_RDX) {
                if (instruction.decoded.mnemonic != ZYDIS_MNEMONIC_LEA || instruction.decoded.operand_count_visible != 2 ||
                    !reg(instruction.operands[0], ZYDIS_REGISTER_RDX) || instruction.operands[1].type != ZYDIS_OPERAND_TYPE_MEMORY ||
                    (instruction.operands[1].mem.base != ZYDIS_REGISTER_RBP && instruction.operands[1].mem.base != ZYDIS_REGISTER_RSP) ||
                    instruction.operands[1].mem.index != ZYDIS_REGISTER_NONE) return false;
                descriptor = true;
            }
        }
        if (state && descriptor) return true;
    }
    return false;
}

bool select_binding(const Procedure& builder, std::uintptr_t anchor, MenuBinding& selected) {
    std::map<std::uintptr_t, std::vector<std::size_t>> groups;
    const auto& code = builder.instructions;
    for (std::size_t index = 0; index + 1 < code.size(); ++index) {
        std::uintptr_t target{};
        if (code[index].address < anchor || !direct_target(code[index], target) ||
            !consecutive(code[index], code[index + 1]) || !test_bool(code[index + 1])) continue;
        groups[target].push_back(index);
    }
    std::uintptr_t entry{};
    std::size_t maximum{};
    bool tied{};
    for (const auto& [target, calls] : groups) {
        if (calls.size() > maximum) { entry = target; maximum = calls.size(); tied = false; }
        else if (calls.size() == maximum) tied = true;
    }
    if (!entry || tied || maximum < 4) return false;
    // Both UI branches must reach the same original navigation operation with
    // the same immediate state. The activation guard must skip that operation.
    std::map<std::pair<std::uintptr_t, std::uint64_t>, std::vector<std::size_t>> transitions;
    for (const auto index : groups.at(entry)) {
        if (index + 5 >= code.size()) continue;
        const auto& branch = code[index + 2];
        const auto& value = code[index + 3];
        const auto& owner = code[index + 4];
        const auto& navigate = code[index + 5];
        std::uintptr_t navigation{};
        ZyanU64 skip{};
        if (!consecutive(code[index + 1], branch) || !consecutive(branch, value) ||
            !consecutive(value, owner) || !consecutive(owner, navigate) ||
            branch.decoded.mnemonic != ZYDIS_MNEMONIC_JZ || value.decoded.mnemonic != ZYDIS_MNEMONIC_MOV ||
            value.decoded.operand_count_visible != 2 || !reg(value.operands[0], ZYDIS_REGISTER_EDX) ||
            value.operands[1].type != ZYDIS_OPERAND_TYPE_IMMEDIATE || value.operands[1].imm.is_relative ||
            owner.decoded.mnemonic != ZYDIS_MNEMONIC_MOV || owner.decoded.operand_count_visible != 2 ||
            !reg(owner.operands[0], ZYDIS_REGISTER_RCX) || owner.operands[1].type != ZYDIS_OPERAND_TYPE_REGISTER ||
            owner.operands[1].size != 64 || !direct_target(navigate, navigation) ||
            !branch.decoded.operand_count_visible || branch.operands[0].type != ZYDIS_OPERAND_TYPE_IMMEDIATE ||
            !branch.operands[0].imm.is_relative ||
            !ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&branch.decoded, &branch.operands[0], branch.address, &skip)) ||
            skip != navigate.address + navigate.decoded.length) continue;
        transitions[{navigation, value.operands[1].imm.value.u}].push_back(index);
    }
    const std::vector<std::size_t>* insertion{};
    maximum = 0; tied = false;
    for (const auto& [transition, calls] : transitions) {
        if (calls.size() > maximum) { insertion = &calls; maximum = calls.size(); tied = false; }
        else if (calls.size() == maximum) tied = true;
    }
    if (!insertion || tied || maximum < 2) return false;
    Procedure helper;
    if (!OriginalCode::ReadProcedure(entry, helper) || !entry_contract(helper)) return false;
    MenuBinding result;
    result.original_entry = entry;
    result.menu_calls = groups.at(entry).size();
    for (const auto index : *insertion) {
        if (!pointer_arguments(builder, index)) return false;
        MenuCall call; call.address = code[index].address;
        std::copy_n(code[index].bytes.begin(), call.original.size(), call.original.begin());
        result.insertion_calls.push_back(call);
    }
    if (!OriginalCode::AppendEvidence(builder, result.evidence) || !OriginalCode::AppendEvidence(helper, result.evidence) ||
        !OriginalCode::EvidenceUnchanged(result.evidence)) return false;
    // The character cache observes this same original UI callback. Failure to
    // establish its read-only contract does not disable the Mods menu entry.
    ResolveCharacterSelection(builder, result.character);
    selected = std::move(result);
    return true;
}
}

bool ResolveMainMenu(MenuBinding& output) {
    const auto fail = [&](const char* error) { output = {}; output.error = error; return false; };
    try {
        std::vector<OriginalCode::ImageSection> sections;
        if (!OriginalCode::ReadImageSections(sections)) return fail("Could not read the original executable sections");
        std::set<std::uintptr_t> strings;
        for (const auto& section : sections) {
            if ((section.flags & IMAGE_SCN_MEM_EXECUTE) || section.bytes.size() < sizeof(menu_anchor)) continue;
            auto from = section.bytes.begin();
            for (;;) {
                const auto found = std::search(from, section.bytes.end(), std::begin(menu_anchor), std::end(menu_anchor));
                if (found == section.bytes.end()) break;
                strings.insert(section.address + static_cast<std::size_t>(found - section.bytes.begin()));
                from = std::next(found);
            }
        }
        if (strings.empty()) return fail("The original mainMenuEntries UI name was not found");
        MenuBinding selected;
        std::set<std::uintptr_t> visited_builders;
        for (const auto& section : sections) {
            if (!(section.flags & IMAGE_SCN_MEM_EXECUTE) || section.bytes.size() < 7) continue;
            // Byte search only proposes a candidate. The canonical unwind CFG
            // must decode that exact address as a LEA before it can select code.
            for (std::size_t offset = 0; offset + 7 <= section.bytes.size(); ++offset) {
                const auto* bytes = section.bytes.data() + offset;
                if ((bytes[0] != 0x48 && bytes[0] != 0x4c) || bytes[1] != 0x8d || (bytes[2] & 0xc7) != 5) continue;
                std::int32_t displacement{}; std::memcpy(&displacement, bytes + 3, sizeof(displacement));
                const auto address = section.address + offset;
                const auto target = static_cast<std::uintptr_t>(static_cast<std::int64_t>(address + 7) + displacement);
                if (!strings.contains(target)) continue;
                Procedure builder;
                if (!OriginalCode::ReadContainingProcedure(address, builder)) continue;
                const auto instruction = std::find_if(builder.instructions.begin(), builder.instructions.end(),
                    [address](const Instruction& item) { return item.address == address; });
                if (instruction == builder.instructions.end() || !lea_target(*instruction, target) ||
                    !visited_builders.insert(builder.entry).second) continue;
                MenuBinding candidate;
                if (!select_binding(builder, address, candidate)) continue;
                if (selected.original_entry) return fail("The original main menu binding is ambiguous");
                selected = std::move(candidate);
            }
        }
        if (!selected.original_entry) return fail("The original main menu entry and two-pointer activation contract could not be established");
        output = std::move(selected);
        return true;
    } catch (...) { return fail("The original main menu resolver failed"); }
}
}
