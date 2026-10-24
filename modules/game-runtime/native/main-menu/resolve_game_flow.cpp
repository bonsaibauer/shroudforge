#include "main-menu/resolve_game_flow.h"
#include "named-services/named_service_contract.h"
#include <algorithm>
#include <cstring>
#include <map>
#include <set>
#include <string_view>

namespace NativeMainMenu {
namespace {
using OriginalCode::Instruction;
using OriginalCode::Procedure;
using OriginalCode::ImageSection;
using NamedServiceImplementation::read;
using NamedServiceImplementation::field;

bool reg(const ZydisDecodedOperand& op, ZydisRegister value) {
    return op.type == ZYDIS_OPERAND_TYPE_REGISTER && op.reg.value == value;
}
bool memory(const ZydisDecodedOperand& op, ZydisRegister base, std::size_t size) {
    return op.type == ZYDIS_OPERAND_TYPE_MEMORY && op.mem.base == base &&
        op.mem.index == ZYDIS_REGISTER_NONE && op.mem.disp.value >= 0 && op.size == size;
}
bool next(const Instruction& a, const Instruction& b) { return a.address + a.decoded.length == b.address; }
bool absolute(const Instruction& ins, unsigned operand, std::uintptr_t& value) {
    if (operand >= ins.decoded.operand_count_visible) return false;
    ZyanU64 target{};
    if (!ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&ins.decoded, &ins.operands[operand], ins.address, &target))) return false;
    value = static_cast<std::uintptr_t>(target);
    return true;
}
bool call(const Instruction& ins, std::uintptr_t& target) {
    return ins.decoded.mnemonic == ZYDIS_MNEMONIC_CALL &&
        ins.operands[0].type == ZYDIS_OPERAND_TYPE_IMMEDIATE && ins.operands[0].imm.is_relative &&
        absolute(ins, 0, target);
}
bool lea(const Instruction& ins, std::uintptr_t& target) {
    return ins.decoded.mnemonic == ZYDIS_MNEMONIC_LEA && ins.decoded.operand_count_visible == 2 &&
        ins.operands[1].type == ZYDIS_OPERAND_TYPE_MEMORY && ins.operands[1].mem.base == ZYDIS_REGISTER_RIP &&
        absolute(ins, 1, target);
}
bool writes(const Instruction& ins, ZydisRegister value) {
    for (unsigned n = 0; n < ins.decoded.operand_count; ++n) {
        const auto& op = ins.operands[n];
        if (op.type == ZYDIS_OPERAND_TYPE_REGISTER && (op.actions & ZYDIS_OPERAND_ACTION_MASK_WRITE) &&
            ZydisRegisterGetLargestEnclosing(ZYDIS_MACHINE_MODE_LONG_64, op.reg.value) == value) return true;
    }
    return false;
}
bool nonvolatile(ZydisRegister value) {
    return value == ZYDIS_REGISTER_RBX || value == ZYDIS_REGISTER_RBP || value == ZYDIS_REGISTER_RSI ||
        value == ZYDIS_REGISTER_RDI || value == ZYDIS_REGISTER_R12 || value == ZYDIS_REGISTER_R13 ||
        value == ZYDIS_REGISTER_R14 || value == ZYDIS_REGISTER_R15;
}
const ImageSection* section_at(const std::vector<ImageSection>& sections, std::uintptr_t at, std::size_t size) {
    for (const auto& section : sections)
        if (at >= section.address && at - section.address <= section.size && size <= section.size - (at - section.address))
            return &section;
    return nullptr;
}
bool immutable(const std::vector<ImageSection>& sections, std::uintptr_t at, void* output, std::size_t size) {
    const auto* section = section_at(sections, at, size);
    if (!section || (section->flags & (IMAGE_SCN_MEM_WRITE | IMAGE_SCN_MEM_EXECUTE)) || section->bytes.empty()) return false;
    std::memcpy(output, section->bytes.data() + (at - section->address), size);
    return true;
}

// Byte candidates are admitted only after decoding their exact instruction in
// the canonical unwind CFG. This is the same discovery contract as the menu.
std::map<std::uintptr_t, Procedure> references(const std::vector<ImageSection>& sections,
        const std::set<std::uintptr_t>& targets) {
    std::map<std::uintptr_t, Procedure> result;
    for (const auto& section : sections) {
        if (!(section.flags & IMAGE_SCN_MEM_EXECUTE)) continue;
        for (std::size_t offset = 0; offset + 7 <= section.bytes.size(); ++offset) {
            const auto* bytes = section.bytes.data() + offset;
            if ((bytes[0] != 0x48 && bytes[0] != 0x4c) || bytes[1] != 0x8d || (bytes[2] & 0xc7) != 5) continue;
            std::int32_t displacement{}; std::memcpy(&displacement, bytes + 3, sizeof(displacement));
            const auto at = section.address + offset;
            const auto target = static_cast<std::uintptr_t>(static_cast<std::int64_t>(at + 7) + displacement);
            if (!targets.contains(target)) continue;
            Procedure code;
            if (!OriginalCode::ReadContainingProcedure(at, code)) continue;
            const auto found = std::find_if(code.instructions.begin(), code.instructions.end(),
                [at](const Instruction& ins) { return ins.address == at; });
            std::uintptr_t decoded{};
            if (found != code.instructions.end() && lea(*found, decoded) && decoded == target)
                result.emplace(code.entry, std::move(code));
        }
    }
    return result;
}
bool anchored(const std::vector<ImageSection>& sections, const std::vector<std::string_view>& names, Procedure& output) {
    std::map<std::uintptr_t, std::size_t> strings;
    for (const auto& section : sections) {
        if (section.flags & IMAGE_SCN_MEM_EXECUTE) continue;
        for (std::size_t name = 0; name < names.size(); ++name) {
            auto from = section.bytes.begin();
            while (from != section.bytes.end()) {
                const auto found = std::search(from, section.bytes.end(), names[name].begin(), names[name].end());
                if (found == section.bytes.end()) break;
                const auto offset = static_cast<std::size_t>(found - section.bytes.begin());
                if (offset + names[name].size() < section.bytes.size() && !section.bytes[offset + names[name].size()])
                    strings.emplace(section.address + offset, name);
                from = std::next(found);
            }
        }
    }
    std::set<std::uintptr_t> targets;
    for (const auto& [address, name] : strings) targets.insert(address);
    bool found{};
    for (auto& [entry, code] : references(sections, targets)) {
        std::set<std::size_t> matched;
        for (const auto& ins : code.instructions) {
            std::uintptr_t target{};
            if (lea(ins, target) && strings.contains(target)) matched.insert(strings.at(target));
        }
        if (matched.size() != names.size()) continue;
        if (found) return false;
        found = true; output = std::move(code);
    }
    return found;
}

bool root_binding(const std::vector<ImageSection>& sections, const Procedure& init, const Procedure& step,
        GameFlowBinding& result, OriginalCode::CodeEvidence& evidence) {
    std::set<std::uintptr_t> vtables;
    for (const auto& section : sections) {
        if (section.flags & IMAGE_SCN_MEM_EXECUTE) continue;
        for (std::size_t offset = 0; offset + sizeof(std::uintptr_t) <= section.bytes.size(); offset += sizeof(std::uintptr_t)) {
            std::uintptr_t value{}; std::memcpy(&value, section.bytes.data() + offset, sizeof(value));
            if (value != init.entry) continue;
            // A bounded vtable candidate still needs the constructor's direct
            // global store and both named methods in its contiguous code slots.
            for (std::size_t back = 0; back <= offset && back < 64 * sizeof(std::uintptr_t); back += sizeof(std::uintptr_t)) {
                const auto candidate = section.address + offset - back;
                bool has_init{}, has_step{};
                for (std::size_t n = 0; n < 64; ++n) {
                    std::uintptr_t method{};
                    if (!immutable(sections, candidate + n * sizeof(method), &method, sizeof(method)) ||
                        !NamedServiceImplementation::executable_callback(method)) break;
                    has_init |= method == init.entry; has_step |= method == step.entry;
                }
                if (has_init && has_step) vtables.insert(candidate);
            }
        }
    }
    bool found{};
    for (const auto& [entry, ctor] : references(sections, vtables)) {
        for (std::size_t n = 0; n + 1 < ctor.instructions.size(); ++n) {
            const auto& load = ctor.instructions[n]; const auto& store = ctor.instructions[n + 1];
            std::uintptr_t vt{}, root{};
            if (!lea(load, vt) || !vtables.contains(vt) || load.operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER ||
                !next(load, store) || store.decoded.mnemonic != ZYDIS_MNEMONIC_MOV ||
                !reg(store.operands[1], load.operands[0].reg.value) ||
                !memory(store.operands[0], ZYDIS_REGISTER_RIP, 64) || !absolute(store, 0, root)) continue;
            const auto* writable = section_at(sections, root, sizeof(std::uintptr_t));
            if (!writable || !(writable->flags & IMAGE_SCN_MEM_WRITE)) continue;
            if (found && (result.root != root || result.root_vtable != vt)) return false;
            found = true; result.root = root; result.root_vtable = vt;
            if (!OriginalCode::AppendEvidence(ctor, evidence)) return false;
        }
    }
    return found;
}

bool current_getter(const Procedure& code, std::size_t& offset) {
    if (code.instructions.size() != 3) return false;
    const auto& cmp = code.instructions[0]; const auto& set = code.instructions[1]; const auto& ret = code.instructions[2];
    if (cmp.decoded.mnemonic != ZYDIS_MNEMONIC_CMP || !memory(cmp.operands[0], ZYDIS_REGISTER_RCX, 32) ||
        cmp.operands[1].type != ZYDIS_OPERAND_TYPE_IMMEDIATE ||
        static_cast<std::uint32_t>(cmp.operands[1].imm.value.u) != UINT32_MAX ||
        set.decoded.mnemonic != ZYDIS_MNEMONIC_SETZ || !reg(set.operands[0], ZYDIS_REGISTER_AL) ||
        ret.decoded.meta.category != ZYDIS_CATEGORY_RET || !next(cmp, set) || !next(set, ret)) return false;
    offset = static_cast<std::size_t>(cmp.operands[0].mem.disp.value);
    return offset < 0x10000;
}

// Original state metadata is {id,parent,name span,step span}. The query's
// indexed parent read must prove this exact row stride before names admit IDs.
struct StateRow { std::int32_t id, parent; std::uintptr_t name; std::uint64_t length, steps, count; };
struct Descriptor { std::uintptr_t rows; std::uint64_t count; std::uintptr_t transitions; std::uint64_t transition_count, initial; };
static_assert(sizeof(StateRow) == 40 && sizeof(Descriptor) == 40);
bool state_query(const Procedure& code, std::size_t current_offset) {
    bool current{};
    for (const auto& ins : code.instructions)
        if (ins.decoded.mnemonic == ZYDIS_MNEMONIC_MOV && memory(ins.operands[1], ZYDIS_REGISTER_RCX, 32) &&
            static_cast<std::size_t>(ins.operands[1].mem.disp.value) == current_offset) current = true;
    for (std::size_t n = 0; n + 2 < code.instructions.size(); ++n) {
        const auto& scale = code.instructions[n]; const auto& rows = code.instructions[n + 1]; const auto& parent = code.instructions[n + 2];
        if (!current || scale.decoded.mnemonic != ZYDIS_MNEMONIC_LEA ||
            scale.operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER || scale.operands[1].type != ZYDIS_OPERAND_TYPE_MEMORY ||
            scale.operands[1].mem.base != scale.operands[1].mem.index || scale.operands[1].mem.disp.value ||
            rows.decoded.mnemonic != ZYDIS_MNEMONIC_MOV || rows.operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER ||
            rows.operands[1].type != ZYDIS_OPERAND_TYPE_MEMORY || rows.operands[1].mem.disp.value ||
            parent.decoded.mnemonic != ZYDIS_MNEMONIC_MOV || parent.operands[1].type != ZYDIS_OPERAND_TYPE_MEMORY ||
            parent.operands[1].size != 32 || parent.operands[1].mem.base != rows.operands[0].reg.value ||
            parent.operands[1].mem.index != scale.operands[0].reg.value ||
            parent.operands[1].mem.disp.value != offsetof(StateRow, parent) ||
            (1 + scale.operands[1].mem.scale) * parent.operands[1].mem.scale != sizeof(StateRow) ||
            !next(scale, rows) || !next(rows, parent)) continue;
        return true;
    }
    return false;
}
bool descriptor_copy(const Procedure& ctor) {
    ZydisRegister descriptor = ZYDIS_REGISTER_NONE;
    for (const auto& ins : ctor.instructions) {
        if (ins.decoded.meta.category == ZYDIS_CATEGORY_CALL) break;
        if (ins.decoded.mnemonic == ZYDIS_MNEMONIC_MOV && reg(ins.operands[1], ZYDIS_REGISTER_R8) &&
            ins.operands[0].type == ZYDIS_OPERAND_TYPE_REGISTER && nonvolatile(ins.operands[0].reg.value))
            descriptor = ins.operands[0].reg.value;
    }
    if (descriptor == ZYDIS_REGISTER_NONE) return false;
    for (std::size_t n = 0; n + 1 < ctor.instructions.size(); ++n) {
        const auto& load = ctor.instructions[n]; const auto& store = ctor.instructions[n + 1];
        if (load.decoded.mnemonic == ZYDIS_MNEMONIC_MOVUPS && memory(load.operands[1], descriptor, 128) &&
            !load.operands[1].mem.disp.value && next(load, store) && store.decoded.mnemonic == ZYDIS_MNEMONIC_MOVUPS &&
            store.operands[0].type == ZYDIS_OPERAND_TYPE_MEMORY && !store.operands[0].mem.disp.value &&
            nonvolatile(store.operands[0].mem.base) && store.operands[0].mem.index == ZYDIS_REGISTER_NONE &&
            reg(store.operands[1], load.operands[0].reg.value)) return true;
    }
    return false;
}
bool select_flow(const std::vector<ImageSection>& sections, const Procedure& step,
        GameFlowBinding& result, OriginalCode::CodeEvidence& evidence) {
    ZydisRegister owner = ZYDIS_REGISTER_NONE;
    for (const auto& ins : step.instructions) {
        if (ins.decoded.meta.category == ZYDIS_CATEGORY_COND_BR || ins.decoded.meta.category == ZYDIS_CATEGORY_CALL) break;
        if (ins.decoded.mnemonic == ZYDIS_MNEMONIC_MOV && reg(ins.operands[1], ZYDIS_REGISTER_RCX) &&
            ins.operands[0].type == ZYDIS_OPERAND_TYPE_REGISTER && nonvolatile(ins.operands[0].reg.value))
            owner = ins.operands[0].reg.value;
    }
    if (owner == ZYDIS_REGISTER_NONE) return false;
    std::set<std::pair<std::size_t, std::size_t>> getters;
    for (std::size_t n = 1; n < step.instructions.size(); ++n) {
        const auto& load = step.instructions[n - 1]; std::uintptr_t target{}; Procedure getter;
        if (!call(step.instructions[n], target) || !next(load, step.instructions[n]) ||
            load.decoded.mnemonic != ZYDIS_MNEMONIC_MOV || !reg(load.operands[0], ZYDIS_REGISTER_RCX) ||
            !(memory(load.operands[1], owner, 64) || memory(load.operands[1], ZYDIS_REGISTER_RCX, 64)) ||
            !OriginalCode::ReadProcedure(target, getter)) continue;
        std::size_t current{};
        if (current_getter(getter, current)) {
            getters.emplace(static_cast<std::size_t>(load.operands[1].mem.disp.value), current);
            if (!OriginalCode::AppendEvidence(getter, evidence)) return false;
        }
    }
    if (getters.size() != 1) return false;
    result.fsm_offset = getters.begin()->first; result.current_offset = getters.begin()->second;
    bool query{};
    const std::set<std::uintptr_t> callees(step.calls.begin(), step.calls.end());
    for (const auto target : callees) {
        Procedure code;
        if (OriginalCode::ReadProcedure(target, code) && state_query(code, result.current_offset)) {
            if (query || !OriginalCode::AppendEvidence(code, evidence)) return false;
            query = true;
        }
    }
    if (!query) return false;
    bool selected{};
    for (std::size_t n = 0; n + 1 < step.instructions.size(); ++n) {
        std::uintptr_t target{};
        const auto& invoke = step.instructions[n]; const auto& store = step.instructions[n + 1];
        if (!call(invoke, target) || !next(invoke, store) || store.decoded.mnemonic != ZYDIS_MNEMONIC_MOV ||
            !memory(store.operands[0], owner, 64) ||
            static_cast<std::size_t>(store.operands[0].mem.disp.value) != result.fsm_offset ||
            !reg(store.operands[1], ZYDIS_REGISTER_RAX)) continue;
        std::uintptr_t descriptor{};
        for (std::size_t back = n; back > 0 && n - back < 8;) {
            const auto& ins = step.instructions[--back];
            if (!next(ins, step.instructions[back + 1]) || ins.decoded.meta.category == ZYDIS_CATEGORY_CALL ||
                ins.decoded.meta.category == ZYDIS_CATEGORY_COND_BR || ins.decoded.meta.category == ZYDIS_CATEGORY_UNCOND_BR) break;
            if (!writes(ins, ZYDIS_REGISTER_R8)) continue;
            if (reg(ins.operands[0], ZYDIS_REGISTER_R8)) lea(ins, descriptor);
            break;
        }
        Descriptor data{}; Procedure creator;
        if (!descriptor || !immutable(sections, descriptor, &data, sizeof(data)) || !data.count || data.count > 64 ||
            data.initial >= data.count || !OriginalCode::ReadProcedure(target, creator) || !descriptor_copy(creator)) continue;
        std::set<std::string> names;
        std::int32_t pregame = -1;
        bool valid = true;
        for (std::size_t index = 0; index < data.count; ++index) {
            StateRow row{}; std::array<char, 64> text{};
            if (!immutable(sections, data.rows + index * sizeof(row), &row, sizeof(row)) || row.id != index ||
                row.parent < -1 || row.parent >= static_cast<std::int32_t>(index) || !row.length || row.length >= text.size() ||
                !immutable(sections, row.name, text.data(), static_cast<std::size_t>(row.length)) ||
                !names.emplace(text.data(), static_cast<std::size_t>(row.length)).second) { valid = false; break; }
            if (std::string_view(text.data(), static_cast<std::size_t>(row.length)) == "PreGame") pregame = row.id;
        }
        if (!valid || pregame < 0 || !names.contains("Root") || !names.contains("Application") || !names.contains("Play")) continue;
        if (selected || !OriginalCode::AppendEvidence(creator, evidence)) return false;
        selected = true; result.state_rows = data.rows; result.state_count = data.count; result.pregame_state = pregame;
        evidence.push_back({descriptor, std::vector<std::uint8_t>(sizeof(data))});
        std::memcpy(evidence.back().bytes.data(), &data, sizeof(data));
        const auto* rows = section_at(sections, data.rows, static_cast<std::size_t>(data.count) * sizeof(StateRow));
        if (!rows || rows->bytes.empty()) return false;
        const auto begin = rows->bytes.begin() + (data.rows - rows->address);
        evidence.push_back({data.rows, {begin, begin + data.count * sizeof(StateRow)}});
    }
    return selected;
}
}

bool ResolveGameFlow(GameFlowBinding& output) {
    const auto fail = [&](const char* error) { output = {}; output.error = error; return false; };
    try {
        std::vector<ImageSection> sections; Procedure init, step;
        if (!OriginalCode::ReadImageSections(sections)) return fail("original-game-flow-image-unavailable");
        if (!anchored(sections, {"--use-save-beta", "--no-save-beta", "enshrouded.kfc", "--kfc-path"}, init) ||
            !anchored(sections, {"[game] Could not initialize GameFlowSystem."}, step))
            return fail("original-game-flow-application-is-missing-or-ambiguous");
        GameFlowBinding result; OriginalCode::CodeEvidence evidence;
        if (!root_binding(sections, init, step, result, evidence)) return fail("original-game-flow-root-is-missing-or-ambiguous");
        if (!select_flow(sections, step, result, evidence)) return fail("original-game-flow-state-contract-is-missing-or-ambiguous");
        if (!OriginalCode::AppendEvidence(init, evidence) || !OriginalCode::AppendEvidence(step, evidence) ||
            !OriginalCode::EvidenceUnchanged(evidence)) return fail("original-game-flow-evidence-changed");
        output = std::move(result); return true;
    } catch (...) { return fail("original-game-flow-resolution-failed"); }
}

std::uint32_t ReadGameFlowState(const GameFlowBinding& binding) noexcept {
    if (!binding.root || !binding.state_rows || !binding.state_count || binding.pregame_state < 0) return 0;
    struct Snapshot {
        std::uintptr_t root_vtable{}, fsm{}, rows{};
        std::uint64_t count{};
        std::int32_t current{-1};
        bool operator==(const Snapshot&) const = default;
    };
    const auto sample = [&](Snapshot& value) {
        return read(binding.root, value.root_vtable) && value.root_vtable == binding.root_vtable &&
            field(binding.root, binding.fsm_offset, value.fsm) && value.fsm && !(value.fsm % alignof(std::uintptr_t)) &&
            read(value.fsm, value.rows) && value.rows == binding.state_rows &&
            field(value.fsm, sizeof(std::uintptr_t), value.count) && value.count == binding.state_count &&
            field(value.fsm, binding.current_offset, value.current) && value.current >= 0 &&
            static_cast<std::uint64_t>(value.current) < binding.state_count;
    };
    Snapshot first, again;
    if (!sample(first) || !sample(again) || first != again) return 0;
    return first.current == binding.pregame_state ? 1u : 2u;
}
}
