#include "rebase_profile_contract.h"
#include "profile_rebase_analysis.h"
#include "prove_profile_layout.h"
#include <windows.h>
#include <Zydis/Zydis.h>
#include <algorithm>
#include <array>
#include <cstring>
#include <map>
#include <set>
#include <stdexcept>
#include <unordered_map>
#include <optional>
#include <cctype>
#include <tuple>
#include <numeric>
#include <deque>

namespace GameProfileDiscovery::Detail {
namespace {
using json = nlohmann::json;

struct Pe {
    const Image& image;
    IMAGE_NT_HEADERS64 nt{};
    explicit Pe(const Image& value) : image(value) {
        IMAGE_DOS_HEADER dos{};
        std::memcpy(&dos, image.file_bytes.data(), sizeof(dos));
        std::memcpy(&nt, image.file_bytes.data() + dos.e_lfanew, sizeof(nt));
    }
    std::uint64_t base() const { return nt.OptionalHeader.ImageBase; }
    const Section* section(std::uint32_t rva, std::size_t size = 1) const {
        for (const auto& part : image.sections) {
            const auto extent = std::max<std::size_t>(part.virtual_size, part.bytes.size());
            if (rva >= part.rva && rva - part.rva <= extent && size <= extent - (rva - part.rva)) return &part;
        }
        return nullptr;
    }
    bool executable(std::uint32_t rva) const {
        const auto* part = section(rva);
        return part && (part->flags & IMAGE_SCN_MEM_EXECUTE);
    }
    bool in_image(std::uint64_t va) const { return va >= base() && va - base() < image.identity.size; }
    std::uint32_t rva(std::uint64_t va) const {
        if (!in_image(va)) throw std::runtime_error("pointer-outside-original-image");
        return static_cast<std::uint32_t>(va - base());
    }
    std::vector<std::uint8_t> bytes(std::uint32_t rva, std::size_t size) const {
        const auto* part = section(rva, size);
        if (!part) throw std::runtime_error("rva-outside-mapped-section");
        std::vector<std::uint8_t> result(size);
        const auto offset = static_cast<std::size_t>(rva - part->rva);
        if (offset < part->bytes.size()) {
            const auto copied = std::min(size, part->bytes.size() - offset);
            std::copy_n(part->bytes.begin() + offset, copied, result.begin());
        }
        return result;
    }
    template<class T> T read(std::uint32_t rva) const {
        const auto data = bytes(rva, sizeof(T));
        T result{};
        std::memcpy(&result, data.data(), sizeof(result));
        return result;
    }
    std::string text(std::uint64_t va, std::uint64_t length, std::size_t maximum = 4096) const {
        if (!length || length > maximum) throw std::runtime_error("invalid-original-string-span");
        const auto data = bytes(rva(va), static_cast<std::size_t>(length));
        if (std::find(data.begin(), data.end(), std::uint8_t{}) != data.end())
            throw std::runtime_error("embedded-nul-in-original-string");
        return {data.begin(), data.end()};
    }
    template<class Action> void pointer_slots(Action action) const {
        for (const auto& part : image.sections) {
            if (part.flags & IMAGE_SCN_MEM_EXECUTE) continue;
            const auto start = (8 - part.rva % 8) % 8;
            for (std::size_t at = start; at + 8 <= part.bytes.size(); at += 8) {
                std::uint64_t value{};
                std::memcpy(&value, part.bytes.data() + at, sizeof(value));
                action(part.rva + static_cast<std::uint32_t>(at), value);
            }
        }
    }
};

class ImageBaseTrace {
    std::set<ZydisRegister> bases;
    struct CodeValues {
        const std::vector<std::uint32_t>* targets{};
        bool absolute{};
        bool operator==(const CodeValues& other) const {
            return absolute == other.absolute && targets && other.targets && *targets == *other.targets;
        }
    };
    std::map<ZydisRegister, CodeValues> code_values;
    struct Bound {
        std::uint64_t maximum{}; std::uint32_t guard{}, fallthrough{}; unsigned bits{64};
        bool operator==(const Bound&) const = default;
    };
    std::map<ZydisRegister, std::uint64_t> maxima;
    std::map<ZydisRegister, Bound> bounds;
    struct Comparison {
        ZydisRegister reg; std::uint64_t maximum; unsigned bits;
        bool operator==(const Comparison&) const = default;
    };
    std::optional<Comparison> comparison;
    static ZydisRegister reg(ZydisRegister value) {
        return ZydisRegisterGetLargestEnclosing(ZYDIS_MACHINE_MODE_LONG_64, value);
    }
public:
    struct Table { unsigned entries{}; std::uint32_t guard{}, fallthrough{}; };
    ImageBaseTrace fork() const { auto result = *this; result.comparison.reset(); return result; }
    std::uint32_t base_mask() const {
        std::uint32_t result{};
        for (const auto value : bases) if (value >= ZYDIS_REGISTER_RAX && value <= ZYDIS_REGISTER_R15)
            result |= 1u << (value - ZYDIS_REGISTER_RAX);
        return result;
    }
    bool retains_bases(std::uint32_t previous) const {
        return (base_mask() & previous) == previous;
    }
    void merge(const ImageBaseTrace& other) {
        std::erase_if(bases, [&](auto value) { return !other.bases.contains(value); });
        std::erase_if(code_values, [&](const auto& item) {
            const auto found = other.code_values.find(item.first);
            return found == other.code_values.end() || !(found->second == item.second);
        });
        std::erase_if(maxima, [&](const auto& item) { return !other.maxima.contains(item.first); });
        for (auto& [key, value] : maxima) value = std::max(value, other.maxima.at(key));
        std::erase_if(bounds, [&](const auto& item) {
            const auto found = other.bounds.find(item.first);
            return found == other.bounds.end() || found->second.guard != item.second.guard ||
                found->second.fallthrough != item.second.fallthrough || found->second.bits != item.second.bits;
        });
        for (auto& [key, bound] : bounds) bound.maximum = std::max(bound.maximum, other.bounds.at(key).maximum);
        comparison.reset();
    }
    bool merge_dispatch(const ImageBaseTrace& other) {
        const auto prior_bases = bases;
        const auto prior_values = code_values;
        const auto prior_maxima = maxima;
        const auto prior_bounds = bounds;
        const auto prior_comparison = comparison;
        const auto shared_comparison = comparison == other.comparison ? comparison : std::nullopt;
        merge(other);
        comparison = shared_comparison;
        return prior_bases != bases || prior_values != code_values || prior_maxima != maxima ||
            prior_bounds != bounds || prior_comparison != comparison;
    }
    void loaded_code_values(ZydisRegister destination, const std::vector<std::uint32_t>& targets) {
        code_values[reg(destination)] = {&targets, false};
    }
    const std::vector<std::uint32_t>* branch_targets(ZydisRegister input) const {
        const auto found = code_values.find(reg(input));
        return found != code_values.end() && found->second.absolute ? found->second.targets : nullptr;
    }
    bool indexed_table(const ZydisDecodedOperand& operand) const {
        return operand.type == ZYDIS_OPERAND_TYPE_MEMORY && bases.contains(reg(operand.mem.base)) &&
            operand.mem.index != ZYDIS_REGISTER_NONE &&
            operand.mem.base == reg(operand.mem.base) && operand.mem.index == reg(operand.mem.index) &&
            ((operand.mem.scale == 4 && operand.size == 32) || (operand.mem.scale == 1 && operand.size == 8)) &&
            operand.mem.disp.has_displacement && operand.mem.disp.value > 0;
    }
    Table table_bound(const ZydisDecodedOperand& operand, std::uint32_t instruction) const {
        const auto found = bounds.find(reg(operand.mem.index));
        if (found == bounds.end() || found->second.bits < 64 || found->second.maximum >= 4096 ||
            instruction - found->second.fallthrough > 64) return {};
        return {static_cast<unsigned>(found->second.maximum + 1), found->second.guard, found->second.fallthrough};
    }
    void loaded_byte_values(ZydisRegister destination, unsigned maximum, const Table& input) {
        maxima[reg(destination)] = maximum;
        bounds[reg(destination)] = {maximum, input.guard, input.fallthrough, 64};
    }
    void observe(std::uint32_t rva, const ZydisDecodedInstruction& instruction, const ZydisDecodedOperand* operands) {
        if (instruction.mnemonic == ZYDIS_MNEMONIC_CMP && instruction.operand_count_visible == 2 &&
            operands[0].type == ZYDIS_OPERAND_TYPE_REGISTER && operands[1].type == ZYDIS_OPERAND_TYPE_IMMEDIATE) {
            const auto key = reg(operands[0].reg.value);
            const auto width = operands[0].size;
            const auto known = maxima.find(key);
            const bool full = width >= 64 || (known != maxima.end() && known->second < (std::uint64_t{1} << width));
            comparison = Comparison{key, operands[1].imm.value.u, full ? 64u : width};
            return;
        }
        if (instruction.meta.category == ZYDIS_CATEGORY_COND_BR) {
            if (comparison && (instruction.mnemonic == ZYDIS_MNEMONIC_JNBE || instruction.mnemonic == ZYDIS_MNEMONIC_JNB)) {
                const auto maximum = comparison->maximum - (instruction.mnemonic == ZYDIS_MNEMONIC_JNB ? 1 : 0);
                bounds[comparison->reg] = {maximum, rva, rva + instruction.length, comparison->bits};
            }
            comparison.reset();
            return;
        }
        comparison.reset();
        if (instruction.meta.category == ZYDIS_CATEGORY_CALL) {
            for (const auto value : {ZYDIS_REGISTER_RAX, ZYDIS_REGISTER_RCX, ZYDIS_REGISTER_RDX, ZYDIS_REGISTER_R8,
                ZYDIS_REGISTER_R9, ZYDIS_REGISTER_R10, ZYDIS_REGISTER_R11}) {
                bases.erase(value); bounds.erase(value); maxima.erase(value);
                code_values.erase(value);
            }
            return;
        }
        if (!instruction.operand_count_visible || operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER ||
            !(operands[0].actions & ZYDIS_OPERAND_ACTION_MASK_WRITE)) return;
        bool image_base{};
        if (instruction.operand_count_visible >= 2) {
            const auto& source = operands[1];
            if (instruction.mnemonic == ZYDIS_MNEMONIC_MOV && source.type == ZYDIS_OPERAND_TYPE_REGISTER)
                image_base = operands[0].size == 64 && source.size == 64 && bases.contains(reg(source.reg.value));
            else if (instruction.mnemonic == ZYDIS_MNEMONIC_LEA && source.type == ZYDIS_OPERAND_TYPE_MEMORY &&
                operands[0].size == 64 && source.mem.base == ZYDIS_REGISTER_RIP) {
                ZyanU64 target{};
                image_base = ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&instruction, &source, rva, &target)) && target == 0;
            }
        }
        const auto destination = reg(operands[0].reg.value);
        std::optional<CodeValues> copied_code;
        if (instruction.operand_count_visible >= 2) {
            const auto& input = operands[1];
            if (instruction.mnemonic == ZYDIS_MNEMONIC_MOV && input.type == ZYDIS_OPERAND_TYPE_REGISTER) {
                const auto found = code_values.find(reg(input.reg.value));
                if (found != code_values.end() && operands[0].size >= 32 && input.size >= 32 &&
                    (operands[0].size == 64 || !found->second.absolute))
                    copied_code = found->second;
            } else if (instruction.mnemonic == ZYDIS_MNEMONIC_ADD && operands[0].size == 64 &&
                input.type == ZYDIS_OPERAND_TYPE_REGISTER) {
                const auto left = code_values.find(destination), right = code_values.find(reg(input.reg.value));
                if (left != code_values.end() && !left->second.absolute && bases.contains(reg(input.reg.value)))
                    copied_code = CodeValues{left->second.targets, true};
                else if (right != code_values.end() && !right->second.absolute && bases.contains(destination))
                    copied_code = CodeValues{right->second.targets, true};
            } else if (instruction.mnemonic == ZYDIS_MNEMONIC_LEA && operands[0].size == 64 &&
                input.type == ZYDIS_OPERAND_TYPE_MEMORY && input.mem.scale == 1 &&
                input.mem.base == reg(input.mem.base) && input.mem.index == reg(input.mem.index) &&
                (!input.mem.disp.has_displacement || !input.mem.disp.value)) {
                const auto left = code_values.find(reg(input.mem.base)), right = code_values.find(reg(input.mem.index));
                if (left != code_values.end() && !left->second.absolute && bases.contains(reg(input.mem.index)))
                    copied_code = CodeValues{left->second.targets, true};
                else if (right != code_values.end() && !right->second.absolute && bases.contains(reg(input.mem.base)))
                    copied_code = CodeValues{right->second.targets, true};
            }
        }
        std::optional<Bound> copied_bound;
        if (instruction.operand_count_visible >= 2 && operands[1].type == ZYDIS_OPERAND_TYPE_REGISTER &&
            (instruction.mnemonic == ZYDIS_MNEMONIC_MOV || instruction.mnemonic == ZYDIS_MNEMONIC_MOVZX ||
                instruction.mnemonic == ZYDIS_MNEMONIC_MOVSXD)) {
            const auto input = bounds.find(reg(operands[1].reg.value));
            if (input != bounds.end() && input->second.bits >= operands[1].size) {
                copied_bound = input->second;
                if (instruction.mnemonic == ZYDIS_MNEMONIC_MOVSXD && copied_bound->maximum > INT32_MAX)
                    copied_bound.reset();
                else copied_bound->bits = instruction.mnemonic != ZYDIS_MNEMONIC_MOV || operands[0].size == 32 ? 64 : operands[0].size;
            }
        }
        if (image_base) bases.insert(destination); else bases.erase(destination);
        code_values.erase(destination);
        if (copied_code) code_values[destination] = *copied_code;
        bounds.erase(destination);
        maxima.erase(destination);
        if (copied_bound) bounds[destination] = *copied_bound;
        if (operands[0].size == 32 && destination >= ZYDIS_REGISTER_RAX && destination <= ZYDIS_REGISTER_R15)
            maxima[destination] = UINT32_MAX;
        if (instruction.mnemonic == ZYDIS_MNEMONIC_MOVZX && instruction.operand_count_visible >= 2 && operands[1].size < 32)
            maxima[destination] = (std::uint64_t{1} << operands[1].size) - 1;
    }
};


// Local register expressions retain original argument aliases, nested header
// addresses and element-byte arithmetic. They never infer an address from a
// nearby constant or search for an arbitrary sum in unrelated procedures.
class LayoutTrace {
    struct Expression {
        std::int64_t symbol{-1};
        std::int64_t coefficient{}, offset{};
        bool valid{};
        std::int64_t second_symbol{-1};
        std::int64_t second_coefficient{};
        unsigned input_mask{}, symbol_input_mask{}, second_symbol_input_mask{};
        bool operator==(const Expression&) const = default;
    };
    std::map<ZydisRegister, Expression> registers;
    static ZydisRegister reg(ZydisRegister value) { return ZydisRegisterGetLargestEnclosing(ZYDIS_MACHINE_MODE_LONG_64, value); }
    Expression get(ZydisRegister value) {
        if (value == ZYDIS_REGISTER_NONE || value == ZYDIS_REGISTER_RIP) return {};
        const auto key = reg(value);
        auto found = registers.find(key);
        if (found == registers.end()) found = registers.emplace(key,
            Expression{1000 + static_cast<int>(key), 1, 0, true}).first;
        return found->second;
    }
    static bool bounded(const Expression& value) {
        constexpr std::int64_t limit = 1ll << 40;
        return value.coefficient > -limit && value.coefficient < limit &&
            value.second_coefficient > -limit && value.second_coefficient < limit &&
            value.offset > -limit && value.offset < limit;
    }
    static Expression add(Expression a, Expression b) {
        const auto mask = a.input_mask | b.input_mask;
        if (!a.valid || !b.valid) { Expression unknown; unknown.input_mask = mask; return unknown; }
        // Keep at most two independent terms. Canonical ordering makes joins
        // compare the complete address, regardless of ADD/LEA operand order.
        using Term = std::tuple<std::int64_t, std::int64_t, unsigned>;
        std::array<Term, 4> terms{{
            {a.symbol, a.coefficient, a.symbol_input_mask}, {a.second_symbol, a.second_coefficient, a.second_symbol_input_mask},
            {b.symbol, b.coefficient, b.symbol_input_mask}, {b.second_symbol, b.second_coefficient, b.second_symbol_input_mask}}};
        std::sort(terms.begin(), terms.end());
        Expression result{-1, 0, a.offset + b.offset, true};
        result.input_mask = mask;
        unsigned retained{};
        for (std::size_t i = 0; i < terms.size();) {
            const auto symbol = std::get<0>(terms[i]);
            std::int64_t coefficient{};
            unsigned term_mask{};
            do { coefficient += std::get<1>(terms[i]); term_mask |= std::get<2>(terms[i++]); }
            while (i < terms.size() && std::get<0>(terms[i]) == symbol);
            if (!coefficient) continue;
            if (symbol < 0 || retained == 2) { Expression unknown; unknown.input_mask = mask; return unknown; }
            if (retained++ == 0) { result.symbol = symbol; result.coefficient = coefficient; result.symbol_input_mask = term_mask; }
            else { result.second_symbol = symbol; result.second_coefficient = coefficient; result.second_symbol_input_mask = term_mask; }
        }
        if (bounded(result)) return result;
        Expression unknown; unknown.input_mask = mask; return unknown;
    }
    static Expression multiply(Expression a, std::int64_t factor) {
        if (!a.valid || factor < -4096 || factor > 4096) {
            Expression unknown; unknown.input_mask = a.input_mask; return unknown;
        }
        a.coefficient *= factor; a.second_coefficient *= factor; a.offset *= factor;
        if (!a.coefficient) {
            a.symbol = -1; a.second_symbol = -1; a.second_coefficient = 0;
            a.input_mask = a.symbol_input_mask = a.second_symbol_input_mask = 0;
        }
        if (bounded(a)) return a;
        Expression unknown; unknown.input_mask = a.input_mask; return unknown;
    }
    Expression address(const ZydisDecodedOperand& operand) {
        auto value = operand.mem.base == ZYDIS_REGISTER_NONE ? Expression{-1, 0, 0, true} : get(operand.mem.base);
        if (operand.mem.index != ZYDIS_REGISTER_NONE) value = add(value, multiply(get(operand.mem.index), operand.mem.scale));
        if (operand.mem.disp.has_displacement) value = add(value, Expression{-1, 0, operand.mem.disp.value, true});
        return value;
    }
public:
    LayoutTrace() {
        const std::array<ZydisRegister, 4> arguments{ZYDIS_REGISTER_RCX, ZYDIS_REGISTER_RDX, ZYDIS_REGISTER_R8, ZYDIS_REGISTER_R9};
        for (unsigned i = 0; i != arguments.size(); ++i)
            registers[arguments[i]] = {static_cast<int>(i), 1, 0, true, -1, 0, 1u << i, 1u << i};
    }
    bool merge(const LayoutTrace& other, std::uint32_t instruction) {
        bool changed{};
        auto inputs = other.registers;
        for (const auto& [key, value] : registers) inputs.try_emplace(key, value);
        for (const auto& [key, unused] : inputs) {
            const auto before = get(key);
            auto incoming = other.registers.find(key);
            const auto after = incoming == other.registers.end() ?
                Expression{1000 + static_cast<int>(key), 1, 0, true} : incoming->second;
            // A phi names this instruction's newly joined value. It preserves
            // arithmetic performed on that same value, without identifying
            // either incoming pointer as an original argument. Once formed it
            // absorbs further incoming values, including changing backedges.
            const auto phi = (std::int64_t{1} << 40) + (std::int64_t{instruction} << 10) + static_cast<int>(key);
            const auto mask = before.input_mask | after.input_mask;
            if (before == after) continue;
            if (before.valid && before.symbol == phi && before.coefficient == 1 && !before.offset && !before.second_coefficient &&
                before.input_mask == mask) continue;
            registers[key] = {phi, 1, 0, true, -1, 0, mask, mask};
            changed = true;
        }
        return changed;
    }
    void observe(Procedure& function, std::uint32_t rva, const ZydisDecodedInstruction& instruction,
                 const ZydisDecodedOperand* operands, bool emit = true, bool aliases = true) {
        if (instruction.mnemonic == ZYDIS_MNEMONIC_NOP) return;
        for (unsigned index = 0; emit && index < instruction.operand_count_visible; ++index) {
            const auto& operand = operands[index];
            if (operand.type == ZYDIS_OPERAND_TYPE_MEMORY && operand.mem.base != ZYDIS_REGISTER_RIP) {
                const auto base = get(operand.mem.base);
                const bool stack = reg(operand.mem.base) == ZYDIS_REGISTER_RSP ||
                    (base.valid && (base.symbol == 1000 + static_cast<int>(ZYDIS_REGISTER_RSP) ||
                        (reg(operand.mem.base) == ZYDIS_REGISTER_RBP && base.symbol == 1000 + static_cast<int>(ZYDIS_REGISTER_RBP))));
                if (stack || (!operand.actions && instruction.mnemonic != ZYDIS_MNEMONIC_LEA)) continue;
                const auto expression = address(operand);
                const auto width = instruction.mnemonic == ZYDIS_MNEMONIC_LEA ? 0u : operand.size / 8u;
                const bool read = (operand.actions & ZYDIS_OPERAND_ACTION_MASK_READ) != 0;
                const bool write = (operand.actions & ZYDIS_OPERAND_ACTION_MASK_WRITE) != 0;
                function.layout_uses.push_back({rva, index,
                    operand.mem.disp.has_displacement ? operand.mem.disp.value : 0, -1, "original-memory-displacement", width, read, write});
                if (aliases && expression.valid && !expression.second_coefficient &&
                    expression.coefficient == 1 && expression.symbol >= 0 && expression.symbol < 4)
                    function.layout_uses.push_back({rva, index, expression.offset, static_cast<int>(expression.symbol), "original-argument-relative-field", width, read, write});
                if (aliases && expression.valid && expression.coefficient == 1 &&
                    expression.symbol >= 0 && expression.symbol < 4 &&
                    expression.second_symbol >= 2000 && expression.second_coefficient > 1 && !expression.second_symbol_input_mask) {
                    // The second term must have an explicit instruction origin.
                    // Another original argument or an undefined entry register
                    // cannot be reinterpreted as a record index.
                    LayoutUse use{rva, index, expression.offset, static_cast<int>(expression.symbol),
                        "original-indexed-argument-relative-field", width, read, write};
                    use.index_stride = expression.second_coefficient;
                    function.layout_uses.push_back(std::move(use));
                }
                if (width && operand.mem.index != ZYDIS_REGISTER_NONE && operand.mem.scale > 1)
                    function.layout_uses.push_back({rva, index, operand.mem.scale, -1, "original-address-index-scale", width, read, write});
            } else if (operand.type == ZYDIS_OPERAND_TYPE_IMMEDIATE && !operand.imm.is_relative)
                function.layout_uses.push_back({rva, index, operand.imm.value.s, -1, "original-immediate"});
        }
        if (instruction.meta.category == ZYDIS_CATEGORY_CALL) {
            if (emit && instruction.operand_count_visible == 1 && operands[0].type == ZYDIS_OPERAND_TYPE_IMMEDIATE) {
                ZyanU64 target{};
                if (ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&instruction, operands, rva, &target)) && target <= UINT32_MAX) {
                    CallArguments call{rva, static_cast<std::uint32_t>(target), {}};
                    const std::array<ZydisRegister, 4> arguments{ZYDIS_REGISTER_RCX, ZYDIS_REGISTER_RDX, ZYDIS_REGISTER_R8, ZYDIS_REGISTER_R9};
                    for (unsigned i = 0; i != arguments.size(); ++i) {
                        const auto expression = get(arguments[i]);
                        if (aliases && expression.valid && !expression.second_coefficient && expression.coefficient == 1) {
                            call.offsets[i] = expression.offset;
                            if (expression.symbol >= 0 && expression.symbol < 4)
                                call.original_arguments[i] = static_cast<unsigned>(expression.symbol);
                        }
                        if (aliases && expression.valid && !expression.second_coefficient && expression.offset == 0 && expression.coefficient > 1)
                            function.layout_uses.push_back({rva, i, expression.coefficient, -1, "original-call-argument-element-stride"});
                    }
                    function.call_arguments.push_back(call);
                }
            }
            for (const auto volatile_reg : {ZYDIS_REGISTER_RAX, ZYDIS_REGISTER_RCX, ZYDIS_REGISTER_RDX,
                ZYDIS_REGISTER_R8, ZYDIS_REGISTER_R9, ZYDIS_REGISTER_R10, ZYDIS_REGISTER_R11})
                registers[volatile_reg] = {2000 + static_cast<int>(rva), 1, 0, true};
            return;
        }
        if (!instruction.operand_count_visible || operands[0].type != ZYDIS_OPERAND_TYPE_REGISTER ||
            !(operands[0].actions & ZYDIS_OPERAND_ACTION_MASK_WRITE)) return;
        const auto destination = reg(operands[0].reg.value);
        Expression value;
        // Unsupported register arithmetic still carries input taint into a
        // later phi. Renaming an input must never turn it into a local index.
        for (unsigned i = 0; i < instruction.operand_count_visible; ++i)
            if (operands[i].type == ZYDIS_OPERAND_TYPE_REGISTER && (operands[i].actions & ZYDIS_OPERAND_ACTION_MASK_READ))
                value.input_mask |= get(operands[i].reg.value).input_mask;
        if (instruction.mnemonic == ZYDIS_MNEMONIC_MOV && instruction.operand_count_visible >= 2) {
            if (operands[1].type == ZYDIS_OPERAND_TYPE_REGISTER) value = get(operands[1].reg.value);
            else if (operands[1].type == ZYDIS_OPERAND_TYPE_IMMEDIATE) value = {-1, 0, operands[1].imm.value.s, true};
            else value = {2000 + static_cast<int>(rva), 1, 0, true};
        } else if (instruction.mnemonic == ZYDIS_MNEMONIC_LEA && instruction.operand_count_visible >= 2 &&
                   operands[1].type == ZYDIS_OPERAND_TYPE_MEMORY && operands[1].mem.base != ZYDIS_REGISTER_RIP)
            value = address(operands[1]);
        else if ((instruction.mnemonic == ZYDIS_MNEMONIC_ADD || instruction.mnemonic == ZYDIS_MNEMONIC_SUB) &&
                 instruction.operand_count_visible >= 2 && operands[1].type == ZYDIS_OPERAND_TYPE_IMMEDIATE)
            value = add(get(destination), {-1, 0, operands[1].imm.value.s * (instruction.mnemonic == ZYDIS_MNEMONIC_SUB ? -1 : 1), true});
        else if (instruction.mnemonic == ZYDIS_MNEMONIC_SHL && instruction.operand_count_visible >= 2 &&
                 operands[1].type == ZYDIS_OPERAND_TYPE_IMMEDIATE && operands[1].imm.value.u <= 11) {
            value = multiply(get(destination), std::int64_t{1} << operands[1].imm.value.u);
            if (emit && aliases && value.valid && !value.second_coefficient && value.coefficient > 1 && value.offset == 0)
                function.layout_uses.push_back({rva, 0, value.coefficient, -1, "original-index-byte-stride"});
        }
        else if (instruction.mnemonic == ZYDIS_MNEMONIC_IMUL && instruction.operand_count_visible >= 2) {
            const auto& factor = operands[instruction.operand_count_visible - 1];
            if (factor.type == ZYDIS_OPERAND_TYPE_IMMEDIATE)
                value = multiply(instruction.operand_count_visible == 3 && operands[1].type == ZYDIS_OPERAND_TYPE_REGISTER ?
                    get(operands[1].reg.value) : get(destination), factor.imm.value.s);
            if (emit && aliases && value.valid && !value.second_coefficient && value.coefficient > 1 && value.offset == 0)
                function.layout_uses.push_back({rva, 0, value.coefficient, -1, "original-index-byte-stride"});
        }
        registers[destination] = value;
    }
};

struct LayoutInstruction {
    std::uint32_t rva{};
    std::array<std::uint8_t, ZYDIS_MAX_INSTRUCTION_LENGTH> bytes{};
    unsigned length{};
    ZydisInstructionCategory category{};
    std::optional<std::uint32_t> branch;
    std::vector<std::uint32_t> table_targets;
    ImageBaseTrace::Table table_bound;
    std::optional<unsigned> byte_maximum;
    bool indirect_branch{}, stop{};
};

LayoutInstruction layout_instruction(std::uint32_t rva, const std::uint8_t* code,
    const ZydisDecodedInstruction& instruction, const ZydisDecodedOperand* operands) {
    LayoutInstruction result;
    result.rva = rva; result.length = instruction.length; result.category = instruction.meta.category;
    std::copy_n(code, instruction.length, result.bytes.begin());
    result.stop = instruction.meta.category == ZYDIS_CATEGORY_RET ||
        instruction.mnemonic == ZYDIS_MNEMONIC_INT3 || instruction.mnemonic == ZYDIS_MNEMONIC_UD2;
    if (instruction.meta.category == ZYDIS_CATEGORY_COND_BR || instruction.meta.category == ZYDIS_CATEGORY_UNCOND_BR) {
        ZyanU64 target{};
        if (instruction.operand_count_visible == 1 && operands[0].type == ZYDIS_OPERAND_TYPE_IMMEDIATE &&
            operands[0].imm.is_relative &&
            ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&instruction, operands, rva, &target)) && target <= UINT32_MAX)
            result.branch = static_cast<std::uint32_t>(target);
        else result.indirect_branch = true;
    }
    return result;
}

// Propagate argument aliases along actual instruction edges. A return path
// cannot clobber an alternative block located later in the compiler range.
// At joins original aliases survive only when shared by every incoming path.
// Otherwise an opaque phi retains arithmetic on the new value without claiming
// either origin, including loop backedges. Raw operands and calls remain separate
// from these dataflow claims, even for unreachable or indirect-branch code.
void trace_layout(Procedure& function, const std::vector<LayoutInstruction>& instructions) {
    if (instructions.empty()) return;
    ZydisDecoder decoder;
    ZydisDecoderInit(&decoder, ZYDIS_MACHINE_MODE_LONG_64, ZYDIS_STACK_WIDTH_64);
    std::unordered_map<std::uint32_t, std::size_t> indices;
    bool aliases = true;
    for (std::size_t i = 0; i < instructions.size(); ++i) {
        indices.emplace(instructions[i].rva, i);
    }
    for (const auto& site : instructions) if (site.branch && !indices.contains(*site.branch))
        for (const auto& [begin, end] : function.ranges)
            if (*site.branch >= begin && *site.branch < end) aliases = false;
    struct State {
        LayoutTrace layout;
        ImageBaseTrace dispatch;
        bool merge(const State& other, std::uint32_t instruction) {
            const auto layout_changed = layout.merge(other.layout, instruction);
            const auto dispatch_changed = dispatch.merge_dispatch(other.dispatch);
            return layout_changed || dispatch_changed;
        }
    };
    std::vector<std::optional<State>> incoming(instructions.size());
    std::deque<std::size_t> pending;
    std::vector<bool> queued(instructions.size());
    const auto entry = indices.find(function.root);
    if (entry == indices.end()) aliases = false;
    else { incoming[entry->second].emplace(); pending.push_back(entry->second); queued[entry->second] = true; }
    std::size_t visits{};
    while (aliases && !pending.empty()) {
        const auto index = pending.front(); pending.pop_front(); queued[index] = false;
        if (++visits > instructions.size() * 128) { aliases = false; break; }
        const auto& site = instructions[index];
        ZydisDecodedInstruction instruction;
        ZydisDecodedOperand operands[ZYDIS_MAX_OPERAND_COUNT];
        if (!ZYAN_SUCCESS(ZydisDecoderDecodeFull(&decoder, site.bytes.data(), site.length, &instruction, operands))) {
            aliases = false; break;
        }
        auto state = *incoming[index];
        const auto branch_dispatch = state.dispatch.fork();
        bool proven_table{};
        ImageBaseTrace::Table bound;
        if (site.table_bound.entries && instruction.operand_count_visible == 2 &&
            operands[0].type == ZYDIS_OPERAND_TYPE_REGISTER && state.dispatch.indexed_table(operands[1])) {
            bound = state.dispatch.table_bound(operands[1], site.rva);
            proven_table = bound.entries == site.table_bound.entries && bound.guard == site.table_bound.guard &&
                bound.fallthrough == site.table_bound.fallthrough;
        }
        state.layout.observe(function, site.rva, instruction, operands, false);
        state.dispatch.observe(site.rva, instruction, operands);
        if (proven_table && !site.table_targets.empty())
            state.dispatch.loaded_code_values(operands[0].reg.value, site.table_targets);
        if (proven_table && site.byte_maximum)
            state.dispatch.loaded_byte_values(operands[0].reg.value, *site.byte_maximum, bound);
        const auto propagate = [&](std::uint32_t target, const State& output) {
            const auto found = indices.find(target);
            if (found == indices.end()) {
                for (const auto& [begin, end] : function.ranges)
                    if (target >= begin && target < end) aliases = false;
                return;
            }
            const auto next = found->second;
            bool changed = !incoming[next];
            if (changed) incoming[next] = output;
            else changed = incoming[next]->merge(output, target);
            if (changed && !queued[next]) { queued[next] = true; pending.push_back(next); }
        };
        if (site.branch) {
            auto taken = state;
            if (site.category == ZYDIS_CATEGORY_COND_BR) taken.dispatch = branch_dispatch;
            propagate(*site.branch, taken);
        }
        if (site.indirect_branch) {
            const auto* targets = instruction.operand_count_visible == 1 &&
                operands[0].type == ZYDIS_OPERAND_TYPE_REGISTER ? state.dispatch.branch_targets(operands[0].reg.value) : nullptr;
            if (!targets) { aliases = false; break; }
            for (const auto target : *targets) propagate(target, state);
        }
        if (!site.stop && site.category != ZYDIS_CATEGORY_UNCOND_BR)
            propagate(site.rva + site.length, state);
    }
    for (std::size_t i = 0; i < instructions.size(); ++i) {
        const auto& site = instructions[i];
        ZydisDecodedInstruction instruction;
        ZydisDecodedOperand operands[ZYDIS_MAX_OPERAND_COUNT];
        if (!ZYAN_SUCCESS(ZydisDecoderDecodeFull(&decoder, site.bytes.data(), site.length, &instruction, operands))) continue;
        if (aliases && incoming[i]) {
            auto state = incoming[i]->layout;
            state.observe(function, site.rva, instruction, operands);
        } else {
            LayoutTrace state;
            state.observe(function, site.rva, instruction, operands, true, false);
        }
    }
}

template<class T> void append(std::string& output, const T& value) {
    output.append(reinterpret_cast<const char*>(&value), sizeof(value));
}

std::uint32_t primary(const Pe& pe, const RUNTIME_FUNCTION& entry) {
    auto current = entry;
    std::set<std::uint32_t> seen;
    for (unsigned depth = 0; depth != 64; ++depth) {
        if (!seen.insert(current.BeginAddress).second) throw std::runtime_error("cyclic-unwind-chain");
        const auto flags = pe.read<std::uint8_t>(current.UnwindData);
        if ((flags & 7) != 1 && (flags & 7) != 2) throw std::runtime_error("unknown-unwind-version");
        if (!(flags >> 3 & UNW_FLAG_CHAININFO)) return current.BeginAddress;
        const auto count = pe.read<std::uint8_t>(current.UnwindData + 2);
        const auto tail = current.UnwindData + 4 + 2 * ((static_cast<unsigned>(count) + 1) & ~1u);
        current = pe.read<RUNTIME_FUNCTION>(tail);
    }
    throw std::runtime_error("unwind-chain-too-deep");
}

std::map<std::uint32_t, Procedure> procedures(const Pe& pe) {
    std::map<std::uint32_t, Procedure> result;
    const auto& directory = pe.nt.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION];
    if (!directory.VirtualAddress || !directory.Size || directory.Size % sizeof(RUNTIME_FUNCTION))
        throw std::runtime_error("compiler-function-table-unavailable");
    if (directory.Size / sizeof(RUNTIME_FUNCTION) > 1000000)
        throw std::runtime_error("compiler-function-table-too-large");
    for (std::uint32_t offset = 0; offset < directory.Size; offset += sizeof(RUNTIME_FUNCTION)) {
        const auto entry = pe.read<RUNTIME_FUNCTION>(directory.VirtualAddress + offset);
        if (entry.BeginAddress >= entry.EndAddress || !pe.executable(entry.BeginAddress) ||
            !pe.section(entry.BeginAddress, entry.EndAddress - entry.BeginAddress))
            throw std::runtime_error("invalid-compiler-function-range");
        const auto root = primary(pe, entry);
        auto& function = result[root];
        function.root = root;
        function.ranges.emplace_back(entry.BeginAddress, entry.EndAddress);
    }
    ZydisDecoder decoder;
    ZydisDecoderInit(&decoder, ZYDIS_MACHINE_MODE_LONG_64, ZYDIS_STACK_WIDTH_64);
    for (auto& [root, function] : result) {
        std::sort(function.ranges.begin(), function.ranges.end());
        function.ranges.erase(std::unique(function.ranges.begin(), function.ranges.end()), function.ranges.end());
        struct TableRegion { unsigned entries{}, width{4}; std::uint32_t guard{}, fallthrough{}, dispatch{}; };
        std::map<std::uint32_t, TableRegion> embedded_tables;
        std::map<std::uint32_t, ImageBaseTrace> incoming_states;
        std::map<std::uint32_t, std::uint32_t> observed_states;
        ImageBaseTrace continuation;
        bool has_continuation{};
        std::vector<LayoutInstruction> layout_instructions;
        for (unsigned ordinal = 0; ordinal < function.ranges.size(); ++ordinal) {
            const auto [begin, end] = function.ranges[ordinal];
            auto code = pe.bytes(begin, end - begin);
            function.code_bytes += code.size();
            std::size_t offset{};
            ImageBaseTrace image_base_trace = has_continuation ? continuation : ImageBaseTrace{};
            bool reachable = begin == root || has_continuation;
            while (offset < code.size()) {
                const auto instruction_rva = begin + static_cast<std::uint32_t>(offset);
                const auto incoming = incoming_states.find(instruction_rva);
                if (incoming != incoming_states.end()) {
                    if (reachable) image_base_trace.merge(incoming->second);
                    else { image_base_trace = incoming->second; reachable = true; }
                } else if (!reachable) image_base_trace = {};
                observed_states[instruction_rva] = image_base_trace.base_mask();
                const auto table = embedded_tables.find(instruction_rva);
                if (table != embedded_tables.end()) {
                    const auto length = table->second.entries * table->second.width;
                    if (length > code.size() - offset) {
                        function.complete = false;
                        function.references_complete = false;
                        function.reference_failure_rva = instruction_rva;
                        function.reference_failure = "embedded-table-out-of-range";
                        break;
                    }
                    if (table->second.width == 4) {
                        for (unsigned entry = 0; entry < table->second.entries; ++entry) {
                            const auto destination = pe.read<std::uint32_t>(instruction_rva + entry * 4);
                            function.references.push_back({ordinal, static_cast<unsigned>(offset + entry * 4), destination, true});
                        }
                        std::fill_n(code.begin() + offset, length, std::uint8_t{});
                    }
                    offset += length;
                    continue;
                }
                ZydisDecodedInstruction instruction;
                ZydisDecodedOperand operands[ZYDIS_MAX_OPERAND_COUNT];
                if (!ZYAN_SUCCESS(ZydisDecoderDecodeFull(&decoder, code.data() + offset, code.size() - offset,
                    &instruction, operands))) {
                    function.complete = false;
                    function.references_complete = false;
                    function.reference_failure_rva = instruction_rva;
                    function.reference_failure = "instruction-decode-failed";
                    break;
                }
                layout_instructions.push_back(layout_instruction(instruction_rva, code.data() + offset, instruction, operands));
                std::vector<std::uint32_t> table_targets;
                struct ByteValues { ZydisRegister destination; unsigned maximum; ImageBaseTrace::Table bound; };
                std::optional<ByteValues> loaded_byte_values;
                for (unsigned index = 0; index < instruction.operand_count_visible; ++index) {
                    const auto& operand = operands[index];
                    bool relative{};
                    unsigned displacement{}, width{};
                    std::optional<std::uint32_t> indexed_rva;
                    unsigned table_entries{};
                    const unsigned table_width = operand.size == 8 ? 1u : 4u;
                    if (operand.type == ZYDIS_OPERAND_TYPE_MEMORY && operand.mem.base == ZYDIS_REGISTER_RIP) {
                        relative = true;
                        displacement = instruction.raw.disp.offset;
                        width = instruction.raw.disp.size / 8;
                    } else if (image_base_trace.indexed_table(operand) && operand.mem.disp.value <= UINT32_MAX) {
                        const auto candidate = static_cast<std::uint32_t>(operand.mem.disp.value);
                        const auto* section = pe.section(candidate, 8);
                        const auto bound = image_base_trace.table_bound(operand, instruction_rva);
                        bool embedded{};
                        if (section && !(section->flags & IMAGE_SCN_MEM_WRITE)) {
                            if (section->flags & IMAGE_SCN_MEM_EXECUTE) {
                                for (const auto& range : function.ranges) {
                                    if (candidate <= instruction_rva + instruction.length || candidate < range.first || candidate >= range.second) continue;
                                    if (bound.entries >= 2 && candidate + bound.entries * table_width <= range.second) {
                                        embedded = true; table_entries = bound.entries;
                                    } else if (table_width == 4) {
                                        unsigned count{};
                                        while (count < 4096 && candidate + count * 4 + 4 <= range.second &&
                                            pe.executable(pe.read<std::uint32_t>(candidate + count * 4))) ++count;
                                        const auto limit = candidate + count * 4;
                                        if (count >= 2 && count < 4096 && (limit == range.second || embedded_tables.contains(limit))) {
                                            embedded = true; table_entries = count;
                                        }
                                    }
                                }
                            } else if (table_width == 4) {
                                for (; table_entries != 4096; ++table_entries) {
                                    if (!pe.section(candidate + table_entries * 4, 4) ||
                                        !pe.executable(pe.read<std::uint32_t>(candidate + table_entries * 4))) break;
                                }
                                if (table_entries == 4096) table_entries = 0;
                            } else table_entries = bound.entries;
                            if (table_entries >= 2 && pe.section(candidate, table_entries * table_width)) {
                                bool code_targets = true;
                                if (table_width == 4) for (unsigned entry = 0; entry < table_entries; ++entry)
                                    code_targets &= pe.executable(pe.read<std::uint32_t>(candidate + entry * 4));
                                if (code_targets) {
                                    indexed_rva = candidate; relative = true;
                                    displacement = instruction.raw.disp.offset; width = instruction.raw.disp.size / 8;
                                    if (embedded) embedded_tables[candidate] = {table_entries, table_width, bound.guard, bound.fallthrough, instruction_rva};
                                    if (table_width == 4) {
                                        for (unsigned entry = 0; entry < table_entries; ++entry)
                                            table_targets.push_back(pe.read<std::uint32_t>(candidate + entry * 4));
                                        if (bound.guard && bound.entries >= 2 && bound.entries <= table_entries &&
                                            index == 1 && instruction.operand_count_visible == 2 &&
                                            operands[0].type == ZYDIS_OPERAND_TYPE_REGISTER && instruction.mnemonic == ZYDIS_MNEMONIC_MOV) {
                                            auto& layout = layout_instructions.back();
                                            layout.table_bound = bound;
                                            layout.table_targets.assign(table_targets.begin(), table_targets.begin() + bound.entries);
                                        }
                                    } else if (instruction.operand_count_visible >= 2 && operands[0].type == ZYDIS_OPERAND_TYPE_REGISTER &&
                                        instruction.mnemonic == ZYDIS_MNEMONIC_MOVZX) {
                                        const auto values = pe.bytes(candidate, table_entries);
                                        loaded_byte_values = ByteValues{operands[0].reg.value,
                                            *std::max_element(values.begin(), values.end()), bound};
                                        if (bound.guard && bound.entries >= 2 && bound.entries == table_entries && index == 1 &&
                                            instruction.operand_count_visible == 2) {
                                            auto& layout = layout_instructions.back();
                                            layout.table_bound = bound;
                                            layout.byte_maximum = loaded_byte_values->maximum;
                                        }
                                    }
                                }
                            }
                        }
                    } else if (operand.type == ZYDIS_OPERAND_TYPE_IMMEDIATE && operand.imm.is_relative) {
                        relative = true;
                        for (const auto& immediate : instruction.raw.imm) {
                            if (immediate.is_relative) { displacement = immediate.offset; width = immediate.size / 8; break; }
                        }
                    } else if (operand.type == ZYDIS_OPERAND_TYPE_MEMORY) {
                        function.constants.insert(operand.mem.disp.has_displacement ? operand.mem.disp.value : 0);
                    } else if (operand.type == ZYDIS_OPERAND_TYPE_IMMEDIATE) {
                        function.constants.insert(operand.imm.value.s);
                    }
                    if (!relative) continue;
                    ZyanU64 target = indexed_rva.value_or(0);
                    if (!width || displacement + width > instruction.length ||
                        (!indexed_rva && !ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&instruction, &operand, begin + offset, &target)))) {
                        function.complete = false;
                        function.references_complete = false;
                        function.reference_failure_rva = instruction_rva;
                        function.reference_failure = "relative-target-unresolved";
                        break;
                    }
                    // The decoder's runtime address is an RVA here.
                    if (target >= pe.image.identity.size) {
                        function.complete = false;
                        function.references_complete = false;
                        function.reference_failure_rva = instruction_rva;
                        function.reference_failure = "relative-target-outside-image";
                        break;
                    }
                    const bool branch = operand.type == ZYDIS_OPERAND_TYPE_IMMEDIATE;
                    if (branch && instruction.meta.category != ZYDIS_CATEGORY_CALL) {
                        const auto state = image_base_trace.fork();
                        const auto prior = observed_states.find(static_cast<std::uint32_t>(target));
                        if (prior != observed_states.end() && !state.retains_bases(prior->second)) {
                            for (const auto& earlier : function.references) {
                                const auto site = function.ranges.at(earlier.range).first + earlier.offset;
                                if (earlier.indexed_image && site >= target && site <= instruction_rva)
                                    function.complete = false;
                            }
                        }
                        const auto known = incoming_states.find(static_cast<std::uint32_t>(target));
                        if (known == incoming_states.end()) incoming_states.emplace(static_cast<std::uint32_t>(target), state);
                        else known->second.merge(state);
                    }
                    bool internal{};
                    if (branch) for (const auto& range : function.ranges)
                        internal |= target >= range.first && target < range.second;
                    // Internal branch displacements remain part of the whole
                    // procedure signature. They are never wildcarded away.
                    if (!internal) std::fill_n(code.begin() + offset + displacement, width, std::uint8_t{});
                    function.references.push_back({ordinal, static_cast<unsigned>(offset),
                        static_cast<std::uint32_t>(target), branch, indexed_rva.has_value(), table_entries, table_width,
                        operand.type == ZYDIS_OPERAND_TYPE_MEMORY && (operand.actions & ZYDIS_OPERAND_ACTION_MASK_READ) &&
                            instruction.mnemonic != ZYDIS_MNEMONIC_LEA ? operand.size / 8u : 0u,
                        instruction.mnemonic == ZYDIS_MNEMONIC_LEA,
                        operand.type == ZYDIS_OPERAND_TYPE_MEMORY && (operand.actions & ZYDIS_OPERAND_ACTION_MASK_WRITE) ? operand.size / 8u : 0u});
                }
                if (!function.references_complete) break;
                const auto next_table = embedded_tables.upper_bound(instruction_rva);
                if (next_table != embedded_tables.end() && next_table->first < instruction_rva + instruction.length) {
                    function.complete = false;
                    function.references_complete = false;
                    function.reference_failure_rva = instruction_rva;
                    function.reference_failure = "embedded-table-overlaps-instruction";
                    break;
                }
                image_base_trace.observe(begin + static_cast<std::uint32_t>(offset), instruction, operands);
                if (loaded_byte_values) image_base_trace.loaded_byte_values(loaded_byte_values->destination,
                    loaded_byte_values->maximum, loaded_byte_values->bound);
                for (const auto destination : table_targets) {
                    const auto known = incoming_states.find(destination);
                    const auto state = image_base_trace.fork();
                    if (known == incoming_states.end()) incoming_states.emplace(destination, state); else known->second.merge(state);
                }
                if (instruction.meta.category == ZYDIS_CATEGORY_RET || instruction.meta.category == ZYDIS_CATEGORY_UNCOND_BR)
                    reachable = false;
                offset += instruction.length;
            }
            const auto length = static_cast<std::uint32_t>(code.size());
            append(function.signature, length);
            function.signature.append(reinterpret_cast<const char*>(code.data()), code.size());
            has_continuation = reachable && ordinal + 1 < function.ranges.size() &&
                function.ranges[ordinal + 1].first == end;
            if (has_continuation) continuation = image_base_trace;
        }
        for (const auto& [table, region] : embedded_tables) for (const auto& ref : function.references) {
            if (!region.guard || !ref.branch || ref.target < region.fallthrough || ref.target > region.dispatch) continue;
            const auto origin = function.ranges.at(ref.range).first + ref.offset;
            if (origin < region.guard || origin > region.dispatch) {
                function.complete = false;
            }
        }
        trace_layout(function, layout_instructions);
    }
    return result;
}

bool padding_instruction_stream(const Pe& source, const Pe& target, std::uint32_t begin,
                               std::uint32_t end) {
    if (end <= begin) return false;
    const auto before = source.bytes(begin, end - begin);
    const auto after = target.bytes(begin, end - begin);
    ZydisDecoder decoder;
    ZydisDecoderInit(&decoder, ZYDIS_MACHINE_MODE_LONG_64, ZYDIS_STACK_WIDTH_64);
    std::size_t offset{};
    while (offset < before.size()) {
        ZydisDecodedInstruction old_instruction{}, new_instruction{};
        ZydisDecodedOperand old_operands[ZYDIS_MAX_OPERAND_COUNT], new_operands[ZYDIS_MAX_OPERAND_COUNT];
        const auto remaining = before.size() - offset;
        if (!ZYAN_SUCCESS(ZydisDecoderDecodeFull(&decoder, before.data() + offset, remaining,
                &old_instruction, old_operands)) ||
            !ZYAN_SUCCESS(ZydisDecoderDecodeFull(&decoder, after.data() + offset, remaining,
                &new_instruction, new_operands)) ||
            old_instruction.length != new_instruction.length ||
            (old_instruction.mnemonic != ZYDIS_MNEMONIC_NOP && old_instruction.mnemonic != ZYDIS_MNEMONIC_INT3) ||
            (new_instruction.mnemonic != ZYDIS_MNEMONIC_NOP && new_instruction.mnemonic != ZYDIS_MNEMONIC_INT3))
            return false;
        offset += old_instruction.length;
    }
    return true;
}

bool compiler_padding_equivalent(const Pe& source, const Pe& target,
    const std::map<std::uint32_t, Procedure>& source_functions,
    const std::map<std::uint32_t, Procedure>& target_functions, json& evidence) {
    evidence = { {"method", "unreferenced-compiler-padding-only"}, {"equivalent", false},
        {"changedBytes", 0}, {"changedRanges", json::array()}, {"rejection", "not-padding-only"} };
    if (source.base() != target.base() || source.image.identity.size != target.image.identity.size ||
        source.image.normalized_headers != target.image.normalized_headers ||
        source.image.sections.size() != target.image.sections.size() ||
        source_functions.empty() || source_functions.size() != target_functions.size()) {
        evidence["rejection"] = "mapped-image-layout-differs";
        return false;
    }
    for (std::size_t index = 0; index < source.image.sections.size(); ++index) {
        const auto& before = source.image.sections[index];
        const auto& after = target.image.sections[index];
        if (before.name != after.name || before.rva != after.rva || before.virtual_size != after.virtual_size ||
            before.flags != after.flags || before.bytes.size() != after.bytes.size()) {
            evidence["rejection"] = "mapped-section-layout-differs";
            return false;
        }
        if (!(before.flags & IMAGE_SCN_MEM_EXECUTE) && before.bytes != after.bytes) {
            evidence["rejection"] = "non-executable-section-changed";
            return false;
        }
    }

    std::vector<std::pair<std::uint32_t, std::uint32_t>> ranges;
    json incomplete_reference_samples = json::array();
    std::size_t incomplete_reference_count{};
    for (const auto& [root, before] : source_functions) {
        const auto found = target_functions.find(root);
        if (found == target_functions.end() || before.ranges != found->second.ranges) {
            evidence["rejection"] = "compiler-function-or-reference-inventory-changed";
            return false;
        }
        if (!before.references_complete || !found->second.references_complete) {
            ++incomplete_reference_count;
            if (incomplete_reference_samples.size() < 8)
                incomplete_reference_samples.push_back({{"procedureRva", root},
                    {"sourceReferences", before.references.size()}, {"targetReferences", found->second.references.size()},
                    {"failureRva", before.reference_failure_rva}, {"failure", before.reference_failure}});
        }
        for (const auto& range : before.ranges) {
            if (source.bytes(range.first, range.second - range.first) !=
                target.bytes(range.first, range.second - range.first)) {
                evidence["rejection"] = "compiler-function-bytes-changed";
                return false;
            }
            ranges.push_back(range);
        }
    }
    std::sort(ranges.begin(), ranges.end());

    struct ChangedRange { std::uint32_t begin{}, end{}; };
    std::vector<ChangedRange> changed_ranges;
    std::size_t changed_bytes{};
    auto inspect_gap = [&](const Section& section, std::uint32_t begin, std::uint32_t end) {
        if (begin >= end) return true;
        const auto offset = static_cast<std::size_t>(begin - section.rva);
        const auto length = static_cast<std::size_t>(end - begin);
        const auto& after = target.image.sections[&section - source.image.sections.data()];
        if (offset > section.bytes.size() || length > section.bytes.size() - offset) return false;
        bool changed{};
        for (std::size_t index = 0; index < length; ++index)
            changed |= section.bytes[offset + index] != after.bytes[offset + index];
        if (!changed) return true;
        if (!padding_instruction_stream(source, target, begin, end)) return false;
        for (std::size_t index = 0; index < length;) {
            if (section.bytes[offset + index] == after.bytes[offset + index]) { ++index; continue; }
            const auto first = index;
            while (index < length && section.bytes[offset + index] != after.bytes[offset + index]) {
                ++changed_bytes;
                ++index;
            }
            changed_ranges.push_back({begin + static_cast<std::uint32_t>(first), begin + static_cast<std::uint32_t>(index)});
        }
        return true;
    };

    for (std::size_t section_index = 0; section_index < source.image.sections.size(); ++section_index) {
        const auto& section = source.image.sections[section_index];
        if (!(section.flags & IMAGE_SCN_MEM_EXECUTE)) continue;
        const auto section_begin = section.rva;
        const auto section_end = section.rva + static_cast<std::uint32_t>(section.bytes.size());
        auto cursor = section_begin;
        for (const auto& [begin, end] : ranges) {
            if (begin < section_begin || begin >= section_end) continue;
            if (end > section_end || begin < cursor) {
                evidence["rejection"] = "compiler-function-range-invalid";
                return false;
            }
            if (!inspect_gap(section, cursor, begin)) {
                evidence["rejection"] = "changed-executable-gap-is-not-padding";
                return false;
            }
            cursor = end;
        }
        if (!inspect_gap(section, cursor, section_end)) {
            evidence["rejection"] = "changed-executable-gap-is-not-padding";
            return false;
        }
    }
    if (!changed_bytes) {
        evidence["rejection"] = "no-executable-padding-changes";
        return false;
    }

    const auto changed = [&](std::uint32_t rva) {
        return std::any_of(changed_ranges.begin(), changed_ranges.end(), [&](const auto& range) {
            return rva >= range.begin && rva < range.end;
        });
    };
    for (const auto& [root, function] : source_functions) for (const auto& reference : function.references)
        if (changed(reference.target)) {
            evidence["rejection"] = "code-reference-targets-changed-padding";
            return false;
        }
    bool pointer_targets_padding{};
    source.pointer_slots([&](std::uint32_t, std::uint64_t pointer) {
        if (source.in_image(pointer) && changed(source.rva(pointer))) pointer_targets_padding = true;
    });
    if (pointer_targets_padding) {
        evidence["rejection"] = "data-pointer-targets-changed-padding";
        return false;
    }

    evidence["equivalent"] = true;
    evidence["rejection"] = nullptr;
    evidence["changedBytes"] = changed_bytes;
    evidence["referenceInventoryComplete"] = incomplete_reference_count == 0;
    evidence["incompleteReferenceProcedureCount"] = incomplete_reference_count;
    if (incomplete_reference_count) evidence["incompleteReferenceSamples"] = std::move(incomplete_reference_samples);
    for (const auto& range : changed_ranges)
        evidence["changedRanges"].push_back({{"beginRva", range.begin}, {"endRva", range.end}});
    evidence["verifiedProcedures"] = source_functions.size();
    evidence["checkedReferences"] = std::accumulate(source_functions.begin(), source_functions.end(), std::size_t{},
        [](std::size_t count, const auto& item) { return count + item.second.references.size(); });
    return true;
}

struct Reflection {
    std::map<std::string, ReflectedType> types;
};

Reflection reflection(const Pe& pe) {
    std::array<std::vector<std::uint32_t>, 2> anchors;
    const std::array<std::string, 2> names{"BlobString", "uint32"};
    for (unsigned kind = 0; kind != names.size(); ++kind) {
        std::set<std::uint64_t> strings;
        for (const auto& part : pe.image.sections) {
            const auto& data = part.bytes;
            for (std::size_t at = 1; at + names[kind].size() < data.size(); ++at)
                if (!data[at - 1] && !data[at + names[kind].size()] &&
                    !std::memcmp(data.data() + at, names[kind].data(), names[kind].size()))
                    strings.insert(pe.base() + part.rva + at);
        }
        pe.pointer_slots([&](std::uint32_t slot, std::uint64_t pointer) {
            if (strings.contains(pointer) && pe.read<std::uint64_t>(slot + 8) == names[kind].size())
                anchors[kind].push_back(slot);
        });
    }
    std::set<std::pair<std::uint32_t, std::uint64_t>> tables;
    for (const auto first : anchors[0]) for (const auto second : anchors[1]) {
        std::vector<std::uint32_t> possible;
        pe.pointer_slots([&](std::uint32_t slot, std::uint64_t pointer) {
            if (pointer == pe.base() + first && pe.read<std::uint64_t>(slot + 8) == pe.base() + second)
                possible.push_back(slot);
        });
        for (const auto table : possible) pe.pointer_slots([&](std::uint32_t slot, std::uint64_t pointer) {
            if (pointer != pe.base() + table) return;
            const auto count = pe.read<std::uint64_t>(slot + 8);
            if (count < 2 || count > 100000) return;
            try {
                std::set<std::uint64_t> distinct;
                for (std::uint64_t i = 0; i < count; ++i) {
                    const auto metadata = pe.read<std::uint64_t>(table + static_cast<std::uint32_t>(i * 8));
                    if (!distinct.insert(metadata).second || !pe.section(pe.rva(metadata), 0x90)) return;
                }
                tables.emplace(table, count);
            } catch (const std::exception&) {}
        });
    }
    if (tables.size() != 1) throw std::runtime_error("reflection-registry-missing-or-ambiguous");
    const auto [table, count] = *tables.begin();
    std::map<std::uint64_t, std::string> type_names;
    for (std::uint64_t index = 0; index < count; ++index) {
        const auto metadata = pe.read<std::uint64_t>(table + static_cast<std::uint32_t>(index * 8));
        const auto address = pe.rva(metadata);
        const auto name = pe.text(pe.read<std::uint64_t>(address + 0x20), pe.read<std::uint64_t>(address + 0x28));
        if (!type_names.emplace(metadata, name).second) throw std::runtime_error("duplicate-reflection-descriptor");
    }
    const auto type_name = [&](std::uint64_t pointer) -> std::string {
        if (!pointer) return {};
        const auto found = type_names.find(pointer);
        if (found == type_names.end()) throw std::runtime_error("unknown-reflected-field-type");
        return found->second;
    };
    Reflection result;
    for (std::uint64_t index = 0; index < count; ++index) {
        const auto pointer = pe.read<std::uint64_t>(table + static_cast<std::uint32_t>(index * 8));
        const auto metadata = pe.rva(pointer);
        ReflectedType type;
        type.metadata = metadata; type.slot = table + static_cast<std::uint32_t>(index * 8);
        type.name = type_names.at(pointer); type.hash = pe.read<std::uint32_t>(metadata + 0x50);
        type.size = pe.read<std::uint32_t>(metadata + 0x40);
        const auto fields = pe.read<std::uint64_t>(metadata + 0x58);
        const auto enums = pe.read<std::uint64_t>(metadata + 0x60);
        const auto field_count = pe.read<std::uint32_t>(metadata + 0x48);
        if (field_count > 100000) throw std::runtime_error("reflected-field-count-out-of-bounds");
        type.layout = {{"size", type.size}, {"alignment", pe.read<std::uint16_t>(metadata + 0x44)},
            {"elementAlignment", pe.read<std::uint16_t>(metadata + 0x46)},
            {"primitive", pe.read<std::uint8_t>(metadata + 0x4c)}, {"flags", pe.read<std::uint8_t>(metadata + 0x4d)},
            {"innerType", type_name(pe.read<std::uint64_t>(metadata + 0x38))},
            {"fields", json::object()}, {"enums", json::object()}};
        for (std::uint32_t field = 0; fields && field < field_count; ++field) {
            const auto row = pe.rva(fields) + field * 48;
            const auto name = pe.text(pe.read<std::uint64_t>(row), pe.read<std::uint64_t>(row + 8));
            if (type.layout["fields"].contains(name)) throw std::runtime_error("duplicate-reflected-field-name");
            type.layout["fields"][name] = {{"type", type_name(pe.read<std::uint64_t>(row + 16))},
                {"offset", pe.read<std::uint64_t>(row + 24)}};
        }
        for (std::uint32_t field = 0; enums && field < field_count; ++field) {
            const auto row = pe.rva(enums) + field * 40;
            const auto name = pe.text(pe.read<std::uint64_t>(row), pe.read<std::uint64_t>(row + 8));
            if (type.layout["enums"].contains(name)) throw std::runtime_error("duplicate-reflected-enum-name");
            type.layout["enums"][name] = pe.read<std::uint64_t>(row + 16);
        }
        if (!result.types.emplace(type.name, std::move(type)).second)
            throw std::runtime_error("duplicate-qualified-reflection-name");
    }
    return result;
}

struct Descriptor { std::uint32_t rva{}, function{}; std::string name; json layout; };
std::map<std::string, std::vector<Descriptor>> descriptors(const Pe& pe,
    const std::map<std::uint32_t, Procedure>& functions) {
    std::map<std::string, std::vector<Descriptor>> result;
    pe.pointer_slots([&](std::uint32_t slot, std::uint64_t pointer) {
        if (!pe.in_image(pointer) || !functions.contains(pe.rva(pointer)) || slot < 16) return;
        try {
            Descriptor value;
            value.rva = slot - 16; value.function = pe.rva(pointer);
            value.name = pe.text(pe.read<std::uint64_t>(value.rva), pe.read<std::uint64_t>(value.rva + 8), 96);
            if (value.name.size() < 3 || value.name[0] < 'a' || value.name[0] > 'z' ||
                !std::all_of(value.name.begin(), value.name.end(), [](char c) {
                    return (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '_'; })) return;
            value.layout = json::array();
            bool any{};
            for (const auto offset : {24u, 40u, 56u, 72u, 88u, 104u, 152u, 176u}) {
                const auto data = pe.read<std::uint64_t>(value.rva + offset);
                const auto count = pe.read<std::uint64_t>(value.rva + offset + 8);
                if (count > 256 || static_cast<bool>(data) != static_cast<bool>(count)) return;
                auto names = json::array();
                for (std::uint64_t i = 0; i < count; ++i) {
                    const auto row = pe.rva(data) + static_cast<std::uint32_t>(i * 16);
                    names.push_back(pe.text(pe.read<std::uint64_t>(row), pe.read<std::uint64_t>(row + 8), 256));
                }
                any |= count != 0;
                value.layout.push_back({{"offset", offset}, {"entries", names}});
            }
            if (any) result[value.name].push_back(std::move(value));
        } catch (const std::exception&) {}
    });
    return result;
}

struct Claim { std::set<std::uint32_t> targets; std::set<std::string> methods; std::size_t witnesses{}; json evidence = json::array(); };
struct Mapping {
    std::map<std::uint32_t, Claim> claims;
    std::map<std::uint32_t, std::uint32_t> functions;
    std::set<std::uint32_t> profile_rvas;
    void add(std::uint32_t source, std::uint32_t target, const char* method, const json& witness = json::object()) {
        auto& claim = claims[source];
        claim.targets.insert(target); claim.methods.insert(method); ++claim.witnesses;
        if (profile_rvas.contains(source) && claim.evidence.size() < 8 && !witness.empty())
            claim.evidence.push_back(witness);
    }
};

void vtable_claim(const Pe&, const Pe&, std::uint32_t, Mapping&);

std::map<std::uint32_t, std::string> imports(const Pe& pe) {
    std::map<std::uint32_t, std::string> result;
    const auto text = [&](std::uint32_t rva) {
        std::string value;
        for (unsigned i = 0; i < 512; ++i) {
            const auto byte = pe.read<char>(rva + i);
            if (!byte) return value;
            if (static_cast<unsigned char>(byte) < 32 || static_cast<unsigned char>(byte) >= 127)
                throw std::runtime_error("invalid-import-name");
            value.push_back(byte);
        }
        throw std::runtime_error("import-name-too-long");
    };
    const auto& directory = pe.nt.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT];
    if (!directory.VirtualAddress) return result;
    for (unsigned ordinal = 0; ordinal != 512; ++ordinal) {
        const auto descriptor = pe.read<IMAGE_IMPORT_DESCRIPTOR>(directory.VirtualAddress + ordinal * sizeof(IMAGE_IMPORT_DESCRIPTOR));
        if (!descriptor.Name && !descriptor.FirstThunk) return result;
        auto library = text(descriptor.Name);
        std::transform(library.begin(), library.end(), library.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
        const auto table = descriptor.OriginalFirstThunk ? descriptor.OriginalFirstThunk : descriptor.FirstThunk;
        for (unsigned i = 0; i != 65536; ++i) {
            const auto name = pe.read<std::uint64_t>(table + i * 8);
            if (!name) break;
            const auto key = library + "!" + ((name & IMAGE_ORDINAL_FLAG64) ?
                ("#" + std::to_string(name & 0xffff)) : text(static_cast<std::uint32_t>(name) + 2));
            result.emplace(descriptor.FirstThunk + i * 8, key);
        }
    }
    throw std::runtime_error("import-descriptor-table-unterminated");
}

std::optional<Procedure> leaf_procedure(const Pe& pe, std::uint32_t begin) {
    if (!pe.executable(begin)) return std::nullopt;
    const auto* part = pe.section(begin);
    if (begin - part->rva >= part->bytes.size()) return std::nullopt;
    const auto extent = std::min<std::size_t>(4096, part->bytes.size() - (begin - part->rva));
    auto code = pe.bytes(begin, extent);
    ZydisDecoder decoder;
    ZydisDecoderInit(&decoder, ZYDIS_MACHINE_MODE_LONG_64, ZYDIS_STACK_WIDTH_64);
    Procedure result; result.root = begin;
    LayoutTrace layout_trace;
    std::size_t offset{}; bool ended{};
    while (offset < code.size()) {
        ZydisDecodedInstruction instruction;
        ZydisDecodedOperand operands[ZYDIS_MAX_OPERAND_COUNT];
        if (!ZYAN_SUCCESS(ZydisDecoderDecodeFull(&decoder, code.data() + offset, code.size() - offset,
            &instruction, operands))) return std::nullopt;
        // The bound is a rejection limit, never an inferred function end.
        // Decode the complete fallthrough path through RET or explicit tail
        // JMP. Every conditional target is retained as a separate code edge
        // and verified recursively too, including all returning branches.
        if (instruction.meta.category == ZYDIS_CATEGORY_CALL ||
            instruction.mnemonic == ZYDIS_MNEMONIC_INT3) return std::nullopt;
        layout_trace.observe(result, begin + static_cast<std::uint32_t>(offset), instruction, operands);
        for (unsigned index = 0; index < instruction.operand_count_visible; ++index) {
            const auto& operand = operands[index];
            unsigned displacement{}, width{};
            bool relative{};
            if (operand.type == ZYDIS_OPERAND_TYPE_MEMORY && operand.mem.base == ZYDIS_REGISTER_RIP) {
                relative = true; displacement = instruction.raw.disp.offset; width = instruction.raw.disp.size / 8;
            } else if (operand.type == ZYDIS_OPERAND_TYPE_IMMEDIATE && operand.imm.is_relative) {
                relative = true;
                for (const auto& value : instruction.raw.imm) if (value.is_relative) {
                    displacement = value.offset; width = value.size / 8; break;
                }
            } else if (operand.type == ZYDIS_OPERAND_TYPE_MEMORY)
                result.constants.insert(operand.mem.disp.has_displacement ? operand.mem.disp.value : 0);
            else if (operand.type == ZYDIS_OPERAND_TYPE_IMMEDIATE) result.constants.insert(operand.imm.value.s);
            if (!relative) continue;
            ZyanU64 target{};
            if (!width || displacement + width > instruction.length ||
                !ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&instruction, &operand, begin + offset, &target)) ||
                target >= pe.image.identity.size) return std::nullopt;
            std::fill_n(code.begin() + offset + displacement, width, std::uint8_t{});
            result.references.push_back({0, static_cast<unsigned>(offset), static_cast<std::uint32_t>(target),
                operand.type == ZYDIS_OPERAND_TYPE_IMMEDIATE, false, 0, 4,
                operand.type == ZYDIS_OPERAND_TYPE_MEMORY && (operand.actions & ZYDIS_OPERAND_ACTION_MASK_READ) &&
                    instruction.mnemonic != ZYDIS_MNEMONIC_LEA ? operand.size / 8u : 0u,
                instruction.mnemonic == ZYDIS_MNEMONIC_LEA,
                operand.type == ZYDIS_OPERAND_TYPE_MEMORY && (operand.actions & ZYDIS_OPERAND_ACTION_MASK_WRITE) ? operand.size / 8u : 0u});
        }
        offset += instruction.length;
        if (instruction.meta.category == ZYDIS_CATEGORY_RET || instruction.meta.category == ZYDIS_CATEGORY_UNCOND_BR) {
            ended = true; break;
        }
    }
    if (!ended) return std::nullopt;
    result.code_bytes = offset;
    result.ranges.emplace_back(begin, begin + static_cast<std::uint32_t>(offset));
    append(result.signature, static_cast<std::uint32_t>(offset));
    result.signature.append(reinterpret_cast<const char*>(code.data()), offset);
    return result;
}

class CodeRelations {
    using Key = std::tuple<std::uint32_t, std::uint32_t, bool>;
    const Pe &source, &target;
    const std::map<std::uint32_t, Procedure> &before, &after;
    Mapping& mapping;
    std::map<std::uint32_t, std::string> source_imports, target_imports;
    std::map<Key, int> results;
    std::map<Key, std::set<Key>> parents;
    std::map<std::uint32_t, std::optional<Procedure>> source_leaves, target_leaves;
    std::map<std::uint32_t, std::pair<std::uint32_t, const Procedure*>> source_ranges, target_ranges;
    const Procedure* containing(const std::map<std::uint32_t, Procedure>& list, std::uint32_t site) const {
        // Calls ordinarily name a primary entry. Chained unwind fragments and
        // legal interior entrypoints are bound to their complete group too.
        const auto direct = list.find(site);
        if (direct != list.end()) return &direct->second;
        const auto& ranges = &list == &before ? source_ranges : target_ranges;
        auto found = ranges.upper_bound(site);
        if (found != ranges.begin()) {
            --found;
            if (site < found->second.first) return found->second.second;
        }
        return nullptr;
    }
    void invalidate(const Key& key) {
        if (results[key] == -1) return;
        results[key] = -1;
        const auto dependencies = parents.find(key);
        if (dependencies != parents.end()) for (const auto& parent : dependencies->second) invalidate(parent);
    }
    const Procedure* procedure(const Pe& pe, const std::map<std::uint32_t, Procedure>& compiled,
        std::map<std::uint32_t, std::optional<Procedure>>& leaves, std::uint32_t site) {
        if (const auto* known = containing(compiled, site)) return known;
        auto found = leaves.find(site);
        if (found == leaves.end()) found = leaves.emplace(site, leaf_procedure(pe, site)).first;
        return found->second ? &*found->second : nullptr;
    }
    bool corresponding_code_site(std::uint32_t from, std::uint32_t to) const {
        const auto* original = containing(before, from);
        if (!original) return false;
        const auto match = mapping.functions.find(original->root);
        if (match == mapping.functions.end()) return false;
        const auto& replacement = after.at(match->second);
        for (unsigned i = 0; i < original->ranges.size(); ++i) {
            const auto [begin, end] = original->ranges[i];
            if (from >= begin && from < end && to == replacement.ranges[i].first + from - begin) return true;
        }
        return false;
    }
    bool indexed_table(const Reference& left, const Reference& right) const {
        if (!left.indexed_image || !right.indexed_image || left.table_entries < 2 ||
            left.table_entries != right.table_entries || left.table_width != right.table_width) return false;
        if (left.table_width == 1)
            return source.bytes(left.target, left.table_entries) == target.bytes(right.target, right.table_entries);
        for (unsigned i = 0; i < left.table_entries; ++i)
            if (!corresponding_code_site(source.read<std::uint32_t>(left.target + i * 4),
                target.read<std::uint32_t>(right.target + i * 4))) return false;
        return true;
    }
public:
    std::size_t verified{}, leaf_pairs{}, import_pairs{}, independent_callee_bindings{};
    json failures = json::array();
    CodeRelations(const Pe& a, const Pe& b, const std::map<std::uint32_t, Procedure>& left,
        const std::map<std::uint32_t, Procedure>& right, Mapping& claims)
        : source(a), target(b), before(left), after(right), mapping(claims), source_imports(imports(a)), target_imports(imports(b)) {
        for (const auto& [root, function] : before) for (const auto& [begin, end] : function.ranges)
            source_ranges.emplace(begin, std::pair{end, &function});
        for (const auto& [root, function] : after) for (const auto& [begin, end] : function.ranges)
            target_ranges.emplace(begin, std::pair{end, &function});
    }
    bool indexed_tables_correspond(std::uint32_t from, std::uint32_t to) const {
        const auto& a = before.at(from); const auto& b = after.at(to);
        if (a.references.size() != b.references.size()) return false;
        for (unsigned i = 0; i < a.references.size(); ++i)
            if ((a.references[i].indexed_image || b.references[i].indexed_image) &&
                !indexed_table(a.references[i], b.references[i])) return false;
        return true;
    }
    void export_leaves(std::map<std::uint32_t, Procedure>& source_functions,
                       std::map<std::uint32_t, Procedure>& target_functions) const {
        for (const auto& [rva, function] : source_leaves) if (function) source_functions.try_emplace(rva, *function);
        for (const auto& [rva, function] : target_leaves) if (function) target_functions.try_emplace(rva, *function);
    }
    void export_verified_pairs(std::map<std::uint32_t, std::uint32_t>& pairs) {
        std::set<std::uint32_t> existing_targets;
        for (const auto& [source_root, target_root] : pairs) existing_targets.insert(target_root);
        std::map<std::uint32_t, std::uint32_t> candidates;
        std::map<std::uint32_t, std::uint32_t> candidate_sources_by_target;
        std::set<std::uint32_t> ambiguous_sources, ambiguous_targets;
        for (const auto& [key, accepted] : results) {
            if (accepted != 1) continue;
            const auto* source_function = procedure(source, before, source_leaves, std::get<0>(key));
            const auto* target_function = procedure(target, after, target_leaves, std::get<1>(key));
            if (!source_function || !target_function || !source_function->complete || !target_function->complete ||
                source_function->signature != target_function->signature) continue;
            const auto [source_pair, inserted] = candidates.emplace(source_function->root, target_function->root);
            if (!inserted && source_pair->second != target_function->root) ambiguous_sources.insert(source_function->root);
            const auto [target_pair, target_inserted] = candidate_sources_by_target.emplace(
                target_function->root, source_function->root);
            if (!target_inserted && target_pair->second != source_function->root)
                ambiguous_targets.insert(target_function->root);
        }
        for (const auto& [source_root, target_root] : candidates) {
            if (ambiguous_sources.contains(source_root) || ambiguous_targets.contains(target_root) ||
                pairs.contains(source_root) || existing_targets.contains(target_root)) continue;
            pairs.emplace(source_root, target_root);
        }
    }
    bool verify(std::uint32_t from, std::uint32_t to, bool execution_contract = false, unsigned depth = 0) {
        if (depth > 256) return false;
        const Key key{from, to, execution_contract};
        const auto old = results.find(key);
        if (old != results.end()) return old->second != -1;
        results[key] = 0; // Coinductive cycle, closed only after every edge checks.
        const auto reject = [&](const char* reason) {
            invalidate(key);
            if (failures.size() < 128) failures.push_back({{"sourceRva", from}, {"targetRva", to}, {"reason", reason}});
            return false;
        };
        const auto* a = procedure(source, before, source_leaves, from);
        const auto* b = procedure(target, after, target_leaves, to);
        if (!a || !b || !a->complete || !b->complete || a->signature != b->signature ||
            a->ranges.size() != b->ranges.size() || a->references.size() != b->references.size())
            return reject(!a || !b ? "complete-callee-extent-unavailable" : "complete-callee-code-differs");
        bool same_site{};
        for (unsigned i = 0; i < a->ranges.size(); ++i) {
            const auto [begin, end] = a->ranges[i];
            if (from >= begin && from < end && to == b->ranges[i].first + from - begin) same_site = true;
        }
        if (!same_site) return reject("corresponding-procedure-entry-differs");
        std::vector<unsigned> reference_order;
        for (unsigned i = 0; i < a->references.size(); ++i) reference_order.push_back(i);
        std::stable_sort(reference_order.begin(), reference_order.end(), [&](unsigned x, unsigned y) {
            return source.executable(a->references[x].target) > source.executable(a->references[y].target);
        });
        for (const auto i : reference_order) {
            const auto& left = a->references[i]; const auto& right = b->references[i];
            if (left.range != right.range || left.offset != right.offset || left.branch != right.branch ||
                left.indexed_image != right.indexed_image) return reject("original-reference-sites-differ");
            if (left.indexed_image || right.indexed_image) {
                if (!indexed_table(left, right)) return reject("original-indexed-jump-table-targets-differ");
                continue;
            }
            if (source.executable(left.target) || target.executable(right.target)) {
                if (!source.executable(left.target) || !target.executable(right.target))
                    return reject("original-code-reference-kind-differs");
                bool independently_bound{};
                if (const auto* callee = containing(before, left.target)) {
                    const auto match = mapping.functions.find(callee->root);
                    if (match != mapping.functions.end()) {
                        const auto& matched = after.at(match->second);
                        bool corresponding_site{};
                        for (unsigned range = 0; range < callee->ranges.size(); ++range) {
                            const auto [begin, end] = callee->ranges[range];
                            if (left.target >= begin && left.target < end &&
                                right.target == matched.ranges[range].first + left.target - begin)
                                corresponding_site = true;
                        }
                        if (!corresponding_site) return reject("original-direct-call-target-changed");
                        independently_bound = true;
                        ++independent_callee_bindings;
                    }
                }
                // An identity/layout anchor ends at a complete independently
                // matched original callee. It does not claim that every CRT
                // implementation transitively reachable from it is unchanged.
                // A native invocation also verifies the immediate helpers'
                // own reference contracts. Unmatched small callees always
                // require complete callsite-bound code/import/path evidence.
                if (!independently_bound || execution_contract) {
                    parents[Key{left.target, right.target, false}].insert(key);
                    if (!verify(left.target, right.target, false, depth + 1))
                        return reject("original-code-reference-contract-unresolved");
                }
            } else {
                const auto import_a = source_imports.find(left.target), import_b = target_imports.find(right.target);
                if (import_a != source_imports.end() || import_b != target_imports.end()) {
                    if (import_a == source_imports.end() || import_b == target_imports.end() || import_a->second != import_b->second) return reject("original-import-name-binding-differs");
                    ++import_pairs;
                } else {
                    auto claim = mapping.claims.find(left.target);
                    if (claim == mapping.claims.end() || claim->second.targets.size() != 1)
                        vtable_claim(source, target, left.target, mapping);
                    claim = mapping.claims.find(left.target);
                    if (claim == mapping.claims.end()) {
                        const auto* source_section = source.section(left.target);
                        const auto* target_section = target.section(right.target);
                        if (source_section && target_section && !(source_section->flags & IMAGE_SCN_MEM_WRITE) &&
                            !(target_section->flags & IMAGE_SCN_MEM_WRITE)) {
                            if (left.read_bytes && left.read_bytes <= 64 && left.read_bytes == right.read_bytes &&
                                source.bytes(left.target, left.read_bytes) == target.bytes(right.target, right.read_bytes)) {
                                bool image_pointer{};
                                for (unsigned offset = 0; offset + 8 <= left.read_bytes; offset += 8)
                                    image_pointer |= source.in_image(source.read<std::uint64_t>(left.target + offset));
                                if (!image_pointer) mapping.add(left.target, right.target, "original-read-only-operand-literal");
                            } else if (left.address_only && right.address_only) {
                                std::string first;
                                for (unsigned character = 0; character != 4096 && source.section(left.target + character) && target.section(right.target + character); ++character) {
                                    const auto a_char = source.read<unsigned char>(left.target + character);
                                    const auto b_char = target.read<unsigned char>(right.target + character);
                                    if (a_char != b_char) break;
                                    if (!a_char) {
                                        if (first.size() >= 3) mapping.add(left.target, right.target, "original-complete-immutable-string");
                                        break;
                                    }
                                    if ((a_char < 32 && a_char != '\n' && a_char != '\r' && a_char != '\t') || a_char > 126) break;
                                    first.push_back(static_cast<char>(a_char));
                                }
                            }
                        } else if (source_section && target_section && (source_section->flags & IMAGE_SCN_MEM_WRITE) &&
                            (target_section->flags & IMAGE_SCN_MEM_WRITE)) {
                            bool read{}, write{}, consistent = true;
                            for (unsigned edge = 0; edge < a->references.size(); ++edge) {
                                const auto& origin = a->references[edge]; const auto& fresh = b->references[edge];
                                if (origin.branch || origin.indexed_image || origin.target != left.target) continue;
                                consistent &= fresh.target == right.target && origin.read_bytes == fresh.read_bytes && origin.write_bytes == fresh.write_bytes;
                                read |= origin.read_bytes != 0; write |= origin.write_bytes != 0;
                            }
                            if (consistent && read && write)
                                mapping.add(left.target, right.target, "original-owner-bound-global-read-and-write-sites");
                        }
                        claim = mapping.claims.find(left.target);
                    }
                    if (claim == mapping.claims.end()) {
                        unsigned methods{}; bool compatible = true;
                        for (unsigned slot = 0; slot < 16 && methods < 3; ++slot) {
                            if (!source.section(left.target + slot * 8, 8) || !target.section(right.target + slot * 8, 8)) break;
                            const auto a_pointer = source.read<std::uint64_t>(left.target + slot * 8);
                            const auto b_pointer = target.read<std::uint64_t>(right.target + slot * 8);
                            const bool a_code = source.in_image(a_pointer) && source.executable(source.rva(a_pointer));
                            const bool b_code = target.in_image(b_pointer) && target.executable(target.rva(b_pointer));
                            if (!a_code && !b_code) break;
                            if (!a_code || !b_code) { compatible = false; break; }
                            const auto a_method = source.rva(a_pointer), b_method = target.rva(b_pointer);
                            parents[Key{a_method, b_method, false}].insert(key);
                            if (!verify(a_method, b_method, false, depth + 1)) { compatible = false; break; }
                            ++methods;
                        }
                        if (compatible && methods >= 3)
                            mapping.add(left.target, right.target, "constructor-bound-original-vtable-method-bodies");
                        claim = mapping.claims.find(left.target);
                    }
                    if (claim == mapping.claims.end() || claim->second.targets.size() != 1 ||
                        *claim->second.targets.begin() != right.target) return reject("original-rip-data-binding-unresolved");
                }
            }
        }
        if (source_leaves.contains(from) || target_leaves.contains(to)) ++leaf_pairs;
        ++verified;
        if (results[key] == -1) return false;
        results[key] = 1;
        return true;
    }
};

bool ends(const std::string& text, const char* suffix) { return text.ends_with(suffix); }

std::set<std::uint32_t> named_procedures(const Pe& pe, const std::map<std::uint32_t, Procedure>& functions,
                                       const std::string& name) {
    std::set<std::uint32_t> texts, result;
    for (const auto& section : pe.image.sections) for (std::size_t at = 0; at + name.size() < section.bytes.size(); ++at)
        if (!section.bytes[at + name.size()] && !std::memcmp(section.bytes.data() + at, name.data(), name.size()))
            texts.insert(section.rva + static_cast<std::uint32_t>(at));
    for (const auto& [root, function] : functions) for (const auto& ref : function.references)
        if (!ref.branch && texts.contains(ref.target)) result.insert(root);
    return result;
}

void field_leaves(const json& value, const std::string& pointer,
                  std::vector<std::pair<std::string, std::uint32_t>>& rvas,
                  std::vector<std::pair<std::string, std::uint64_t>>& offsets) {
    if (value.is_object()) {
        for (const auto& [key, child] : value.items()) {
            auto escaped = key;
            for (std::size_t at = 0; (at = escaped.find('~', at)) != std::string::npos; at += 2) escaped.replace(at, 1, "~0");
            for (std::size_t at = 0; (at = escaped.find('/', at)) != std::string::npos; at += 2) escaped.replace(at, 1, "~1");
            const auto path = pointer + '/' + escaped;
            if (child.is_number_unsigned() || (child.is_number_integer() && child.get<std::int64_t>() >= 0)) {
                const auto number = child.get<std::uint64_t>();
                if (ends(key, "Rva")) {
                    if (number > UINT32_MAX) throw std::runtime_error("profile-rva-out-of-bounds");
                    rvas.emplace_back(path, static_cast<std::uint32_t>(number));
                } else if (key.find("Offset") != std::string::npos || key.ends_with("Stride") ||
                           key == "offset" || key == "stride" ||
                           key == "sceneEntryReadyState" || key == "sceneRunningState" ||
                           key == "executionAuthorityMode" || key == "serviceInitializedValue" ||
                           key == "allocationFailureMarkerValue" || key == "bufferCapacity" ||
                           key == "threadRegistryCapacity" || key == "threadNameCapacity" ||
                           key == "headerSize" || key == "resourceKeySize" || key == "sizeMask" ||
                           key == "entityId" || key == "entityGeneration" || key == "entityLayout" ||
                           key == "entityStorage" || key == "entityDefinitionPointer" || key == "entityRow" ||
                           key == "entityManagerCount" || key == "entityManagerTable" || key == "entityManagerKeys" ||
                           key == "entityManagerOccupancy" || key == "componentBits" || key == "componentOffsets" ||
                           key == "componentStrides") offsets.emplace_back(path, number);
            }
            field_leaves(child, path, rvas, offsets);
        }
    } else if (value.is_array()) for (std::size_t i = 0; i < value.size(); ++i)
        field_leaves(value[i], pointer + '/' + std::to_string(i), rvas, offsets);
}

void vtable_claim(const Pe& source, const Pe& target, std::uint32_t rva, Mapping& mapping) {
    std::vector<std::pair<unsigned, std::uint32_t>> anchors;
    for (unsigned index = 0; index < 16; ++index) {
        try {
            const auto pointer = source.read<std::uint64_t>(rva + index * 8);
            if (!source.in_image(pointer) || !source.executable(source.rva(pointer))) break;
            const auto function = mapping.functions.find(source.rva(pointer));
            if (function != mapping.functions.end()) anchors.emplace_back(index, function->second);
        } catch (const std::exception&) { break; }
    }
    if (anchors.size() < 3) return;
    std::set<std::uint32_t> candidates;
    target.pointer_slots([&](std::uint32_t slot, std::uint64_t pointer) {
        if (pointer != target.base() + anchors[0].second || slot < anchors[0].first * 8) return;
        const auto start = slot - anchors[0].first * 8;
        try {
            for (const auto& [ordinal, function] : anchors)
                if (target.read<std::uint64_t>(start + ordinal * 8) != target.base() + function) return;
            candidates.insert(start);
        } catch (const std::exception&) {}
    });
    json methods = json::array();
    for (const auto& [ordinal, function] : anchors)
        methods.push_back({{"slot", ordinal}, {"sourceProcedureRva", source.rva(source.read<std::uint64_t>(rva + ordinal * 8))},
            {"targetProcedureRva", function}});
    for (const auto candidate : candidates) mapping.add(rva, candidate, "unique-original-vtable-methods",
        {{"sourceVtableRva", rva}, {"targetVtableRva", candidate}, {"originalMethods", methods}});
}
}

bool RebaseProfile(const Image& source_image, const Image& target_image, const json& baseline,
                   json& rebased, json& evidence) {
    evidence = {{"method", "original-image-profile-rebase"}, {"complete", false},
        {"fields", json::array()}, {"unresolved", json::array()},
        {"targetProfileRead", false}, {"gameplayEffectsTested", false}};
    try {
        const Pe source(source_image), target(target_image);
        std::vector<std::pair<std::string, std::uint32_t>> rvas;
        std::vector<std::pair<std::string, std::uint64_t>> offsets;
        field_leaves(baseline, "", rvas, offsets);
        auto source_functions = procedures(source), target_functions = procedures(target);
        json padding_evidence;
        if (compiler_padding_equivalent(source, target, source_functions, target_functions, padding_evidence)) {
            rebased = baseline;
            evidence["method"] = "unchanged-profile-contract-except-unreferenced-compiler-padding";
            evidence["paddingEquivalence"] = std::move(padding_evidence);
            evidence["complete"] = true;
            evidence["reason"] = "mapped-addresses-code-data-and-profile-layouts-are-unchanged";
            return true;
        }
        evidence["paddingEquivalence"] = std::move(padding_evidence);
        Mapping mapping;
        for (const auto& [path, value] : rvas) if (value) mapping.profile_rvas.insert(value);
        bool same_section_layout = source.base() == target.base() &&
            source.image.identity.size == target.image.identity.size &&
            source.image.sections.size() == target.image.sections.size();
        for (std::size_t index = 0; same_section_layout && index < source.image.sections.size(); ++index) {
            const auto& before = source.image.sections[index];
            const auto& after = target.image.sections[index];
            same_section_layout = before.name == after.name && before.rva == after.rva &&
                before.virtual_size == after.virtual_size && before.flags == after.flags;
        }
        std::size_t same_rva_procedures{};
        if (same_section_layout) for (const auto& [rva, before] : source_functions) {
            const auto found = target_functions.find(rva);
            if (!before.complete || found == target_functions.end() || !found->second.complete ||
                before.ranges != found->second.ranges || before.references.size() != found->second.references.size())
                continue;
            bool identical = true;
            for (std::size_t index = 0; identical && index < before.ranges.size(); ++index) {
                const auto [begin, end] = before.ranges[index];
                identical = source.bytes(begin, end - begin) == target.bytes(begin, end - begin);
            }
            for (std::size_t index = 0; identical && index < before.references.size(); ++index) {
                const auto& left = before.references[index];
                const auto& right = found->second.references[index];
                identical = left.range == right.range && left.offset == right.offset &&
                    left.target == right.target && left.branch == right.branch &&
                    left.indexed_image == right.indexed_image && left.table_entries == right.table_entries &&
                    left.table_width == right.table_width && left.read_bytes == right.read_bytes &&
                    left.address_only == right.address_only && left.write_bytes == right.write_bytes;
            }
            if (identical && mapping.functions.emplace(rva, rva).second) ++same_rva_procedures;
        }
        evidence["sameRvaIdenticalProcedures"] = same_rva_procedures;
        std::unordered_map<std::string, std::vector<std::uint32_t>> left, right;
        for (const auto& [rva, function] : source_functions)
            if (function.complete && function.code_bytes >= 32) left[function.signature].push_back(rva);
        for (const auto& [rva, function] : target_functions)
            if (function.complete && function.code_bytes >= 32) right[function.signature].push_back(rva);
        for (const auto& [signature, origins] : left) {
            const auto found = right.find(signature);
            if (origins.size() != 1 || found == right.end() || found->second.size() != 1) continue;
            const auto from = origins.front(), to = found->second.front();
            mapping.functions.emplace(from, to);
        }
        CodeRelations code_relations(source, target, source_functions, target_functions, mapping);
        // A normalized image-relative jump-table address is admitted only
        // when every original table element names the corresponding code
        // site. Recheck after removals because tables can reference another
        // candidate procedure whose own evidence became unresolved.
        bool tables_changed;
        do {
            tables_changed = false;
            for (auto candidate = mapping.functions.begin(); candidate != mapping.functions.end();) {
                if (!code_relations.indexed_tables_correspond(candidate->first, candidate->second)) {
                    candidate = mapping.functions.erase(candidate); tables_changed = true;
                } else ++candidate;
            }
        } while (tables_changed);
        evidence["sourceProcedures"] = source_functions.size();
        evidence["targetProcedures"] = target_functions.size();
        evidence["reciprocalUniqueProcedures"] = mapping.functions.size();
        for (const auto& [from, to] : mapping.functions) {
            const auto& before = source_functions.at(from);
            const auto& after = target_functions.at(to);
            if (before.ranges.size() != after.ranges.size() || before.references.size() != after.references.size())
                throw std::runtime_error("normalized-procedure-evidence-disagrees");
            for (unsigned i = 0; i < before.ranges.size(); ++i)
                mapping.add(before.ranges[i].first, after.ranges[i].first, "reciprocal-unique-whole-procedure",
                    {{"sourceProcedureRva", from}, {"targetProcedureRva", to}, {"compilerRange", i}});
            for (unsigned i = 0; i < before.references.size(); ++i) {
                const auto& a = before.references[i]; const auto& b = after.references[i];
                if (a.range != b.range || a.offset != b.offset || a.branch != b.branch ||
                    a.indexed_image != b.indexed_image || a.table_entries != b.table_entries || a.table_width != b.table_width)
                    throw std::runtime_error("normalized-reference-sites-disagree");
                if (!a.branch && !source.executable(a.target) && !target.executable(b.target))
                    mapping.add(a.target, b.target, "corresponding-rip-reference-in-whole-procedure",
                        {{"sourceProcedureRva", from}, {"targetProcedureRva", to},
                         {"sourceInstructionRva", before.ranges.at(a.range).first + a.offset},
                         {"targetInstructionRva", after.ranges.at(b.range).first + b.offset}});
            }
        }
        const auto old_types = reflection(source), new_types = reflection(target);
        std::size_t unchanged{};
        for (const auto& [name, before] : old_types.types) {
            const auto found = new_types.types.find(name);
            if (found == new_types.types.end() || before.hash != found->second.hash || before.layout != found->second.layout) {
                evidence["unresolved"].push_back({{"scope", "reflected-layout"}, {"name", name},
                    {"reason", found == new_types.types.end() ? "source-type-missing" : "original-type-layout-changed"}});
                continue;
            }
            ++unchanged;
            const json type_witness{{"qualifiedType", name}, {"qualifiedHash", before.hash}, {"size", before.size},
                {"sourceMetadataRva", before.metadata}, {"targetMetadataRva", found->second.metadata},
                {"sourceRegistrySlotRva", before.slot}, {"targetRegistrySlotRva", found->second.slot}};
            mapping.add(before.metadata, found->second.metadata, "exact-qualified-reflected-type-layout", type_witness);
            mapping.add(before.slot, found->second.slot, "original-reflection-registry-slot", type_witness);
        }
        evidence["unchangedReflectedLayouts"] = unchanged;
        evidence["sourceReflectedTypes"] = old_types.types.size();
        evidence["targetReflectedTypes"] = new_types.types.size();
        const auto old_systems = descriptors(source, source_functions), new_systems = descriptors(target, target_functions);
        for (const auto& [name, records] : old_systems) {
            const auto found = new_systems.find(name);
            if (records.size() != 1 || found == new_systems.end() || found->second.size() != 1) continue;
            const auto& before = records.front(); const auto& after = found->second.front();
            const auto function = mapping.functions.find(before.function);
            if (before.layout == after.layout && function != mapping.functions.end() && function->second == after.function)
                mapping.add(before.rva, after.rva, "original-system-name-dependencies-and-function",
                    {{"originalSystemName", name}, {"sourceProcedureRva", before.function}, {"targetProcedureRva", after.function},
                        {"dependencyLayout", before.layout}});
        }
        evidence["sourceSystemNames"] = old_systems.size();
        evidence["targetSystemNames"] = new_systems.size();
        rebased = baseline;
        for (const auto& [path, value] : rvas) {
            if (!value) {
                evidence["fields"].push_back({{"path", path}, {"before", 0}, {"after", 0}, {"method", "absent-optional-rva"}});
                continue;
            }
            if (path.find("VtableRva") != std::string::npos) vtable_claim(source, target, value, mapping);
            bool code_edges_verified = true;
            if (source.executable(value)) {
                // A code address is admitted only by its complete compiler
                // procedure. Paired call operands cannot invent a code match.
                bool located{};
                for (const auto& [root, before] : source_functions) {
                    for (unsigned i = 0; i < before.ranges.size(); ++i) {
                        const auto [begin, end] = before.ranges[i];
                        if (value < begin || value >= end) continue;
                        located = true;
                        const auto match = mapping.functions.find(root);
                        if (match != mapping.functions.end()) {
                            const auto& after = target_functions.at(match->second);
                            for (std::size_t edge = 0; edge < before.references.size(); ++edge) {
                                const auto& a = before.references[edge];
                                const auto& b = after.references[edge];
                                if (!a.branch) continue;
                                const auto callee = mapping.functions.find(a.target);
                                if (callee != mapping.functions.end() && callee->second != b.target) {
                                    code_edges_verified = false;
                                    evidence["unresolved"].push_back({{"path", path},
                                        {"reason", "original-direct-call-target-changed"},
                                        {"sourceCalleeRva", a.target}, {"targetCalleeRva", b.target}});
                                }
                            }
                            mapping.add(value, after.ranges[i].first + value - begin, "complete-procedure-site",
                                {{"sourceProcedureRva", root}, {"targetProcedureRva", match->second},
                                    {"sourceInstructionRva", value}, {"targetInstructionRva", after.ranges[i].first + value - begin}});
                        }
                        break;
                    }
                    if (located) break;
                }
            }
            const auto found = mapping.claims.find(value);
            if (found == mapping.claims.end() || found->second.targets.size() != 1) {
                evidence["unresolved"].push_back({{"path", path}, {"sourceRva", value},
                    {"reason", found == mapping.claims.end() ? "no-independent-address-evidence" : "conflicting-address-evidence"}});
                continue;
            }
            if (!code_edges_verified) continue;
            const auto& claim = found->second;
            const auto target_rva = *claim.targets.begin();
            const bool native_invocation = path == "/ecsWriteLayout/changeTrackerRva";
            if (source.executable(value) && !code_relations.verify(value, target_rva, native_invocation)) {
                evidence["unresolved"].push_back({{"path", path}, {"sourceRva", value}, {"targetRva", target_rva},
                    {"reason", "complete-original-code-and-callee-contract-unresolved"}});
                continue;
            }
            rebased[json::json_pointer(path)] = target_rva;
            evidence["fields"].push_back({{"path", path}, {"before", value}, {"after", target_rva},
                {"role", source.executable(value) ? (native_invocation ? "invoked" : "identity") : "data-identity"},
                {"methods", claim.methods}, {"witnessCount", claim.witnesses}, {"witnesses", claim.evidence}});
        }
        if (baseline.contains("ecsWriteLayout") && baseline.at("ecsWriteLayout").contains("changeTrackerBytes")) {
            const auto& contract = baseline.at("ecsWriteLayout");
            const auto encoded = contract.at("changeTrackerBytes").get<std::string>();
            if (encoded.empty() || encoded.size() % 2 || encoded.size() > 1024)
                throw std::runtime_error("invalid-original-invocation-byte-guard");
            const auto digit = [](char value) -> unsigned {
                if (value >= '0' && value <= '9') return value - '0';
                if (value >= 'a' && value <= 'f') return value - 'a' + 10;
                if (value >= 'A' && value <= 'F') return value - 'A' + 10;
                throw std::runtime_error("invalid-original-invocation-byte-guard");
            };
            std::vector<std::uint8_t> expected;
            for (std::size_t i = 0; i < encoded.size(); i += 2)
                expected.push_back(static_cast<std::uint8_t>(digit(encoded[i]) * 16 + digit(encoded[i + 1])));
            const auto original = contract.at("changeTrackerRva").get<std::uint32_t>();
            const auto replacement = rebased.at("ecsWriteLayout").at("changeTrackerRva").get<std::uint32_t>();
            if (source.bytes(original, expected.size()) != expected ||
                !code_relations.verify(original, replacement, true))
                evidence["unresolved"].push_back({{"path", "/ecsWriteLayout/changeTrackerBytes"},
                    {"reason", "original-invocation-byte-guard-not-established"}});
            else {
                const auto fresh = target.bytes(replacement, expected.size());
                auto changed = encoded;
                if (fresh != expected) {
                    constexpr char hex[] = "0123456789abcdef";
                    changed.clear();
                    for (const auto value : fresh) { changed.push_back(hex[value >> 4]); changed.push_back(hex[value & 15]); }
                    rebased["ecsWriteLayout"]["changeTrackerBytes"] = changed;
                }
                evidence["fields"].push_back({{"path", "/ecsWriteLayout/changeTrackerBytes"},
                    {"before", encoded}, {"after", changed}, {"role", "invoked-guard"},
                    {"method", "exact-source-guard-and-proven-target-procedure-bytes"}});
            }
        }
        code_relations.export_leaves(source_functions, target_functions);
        auto layout_counterparts = mapping.functions;
        code_relations.export_verified_pairs(layout_counterparts);
        ProfileRebaseAnalysis analysis{source_functions, target_functions, layout_counterparts, old_types.types, new_types.types, {}, {}, {}};
        for (const auto& field : evidence.at("fields")) {
            const auto path = field.value("path", std::string{});
            if (path.ends_with("Rva") && field.at("after").is_number_integer())
                analysis.established_rvas.emplace(path, field.at("after").get<std::uint32_t>());
        }
        for (const auto& name : {std::string("ecs::Tasks"), std::string("GameServer"), std::string("KfcResourceContent")})
            analysis.named_procedures[name] = named_procedures(source, source_functions, name);
        for (const auto group : {"ecsWriteLayout", "resourceWriteLayout"}) {
            if (baseline.contains(group) && baseline.at(group).contains("threadName")) {
                const auto name = baseline.at(group).at("threadName").get<std::string>();
                analysis.named_procedures[name] = named_procedures(source, source_functions, name);
            }
        }
        if (baseline.contains("resourceLayout") && baseline.at("resourceLayout").contains("itemIdMap")) {
            const auto& item_map = baseline.at("resourceLayout").at("itemIdMap");
            if (item_map.contains("registry") && item_map.at("registry").is_string()) {
                const auto name = item_map.at("registry").get<std::string>();
                analysis.named_procedures[name] = named_procedures(source, source_functions, name);
            }
        }
        for (const auto& [name, systems] : old_systems) for (const auto& system : systems)
            analysis.named_systems[name].insert(system.function);
        json layouts;
        const bool layouts_proven = ProveProfileLayouts(baseline, analysis, offsets, layouts, layout_counterparts);
        std::map<std::string, std::uint64_t> required_layouts(offsets.begin(), offsets.end());
        std::set<std::string> proven_layouts;
        if (layouts.contains("fields")) for (const auto& field : layouts.at("fields")) {
            const auto path = field.value("path", std::string{});
            const auto original = required_layouts.find(path);
            if (original == required_layouts.end() || !field.contains("before") || !field.contains("after") ||
                field.at("before") != original->second || !field.at("after").is_number_integer() ||
                field.at("after").get<std::int64_t>() < 0 ||
                !proven_layouts.insert(path).second)
                throw std::runtime_error("layout-proof-field-inventory-disagrees");
            bool code_proven = true;
            const auto verify_consumers = [&](const auto& self, const json& value) -> void {
                if (value.is_object()) {
                    if (value.contains("sourceProcedureRva") || value.contains("targetProcedureRva")) {
                        if (!value.contains("sourceProcedureRva") || !value.contains("targetProcedureRva") ||
                            !ExactLayoutConsumerPair(analysis,
                                value.at("sourceProcedureRva").get<std::uint32_t>(),
                                value.at("targetProcedureRva").get<std::uint32_t>())) code_proven = false;
                    }
                    for (const auto& child : value) self(self, child);
                } else if (value.is_array()) for (const auto& child : value) self(self, child);
            };
            verify_consumers(verify_consumers, field);
            if (!code_proven) {
                evidence["unresolved"].push_back({{"path", path},
                    {"reason", "layout-consumer-code-contract-unresolved"}});
                continue;
            }
            rebased[json::json_pointer(path)] = field.at("after");
            auto labeled = field;
            labeled["role"] = "layout";
            evidence["fields"].push_back(std::move(labeled));
        }
        std::set<std::string> unresolved_layouts;
        if (layouts.contains("unresolved")) for (const auto& field : layouts.at("unresolved")) {
            unresolved_layouts.insert(field.value("path", std::string{}));
            evidence["unresolved"].push_back(field);
        }
        for (const auto& [path, number] : offsets) if (!proven_layouts.contains(path) && !unresolved_layouts.contains(path))
            evidence["unresolved"].push_back({{"path", path}, {"before", number},
                {"reason", "profile-layout-field-not-proven"}});
        if (!layouts_proven && (!layouts.contains("unresolved") || layouts.at("unresolved").empty()))
            evidence["unresolved"].push_back({{"scope", "private-layouts"}, {"reason", "layout-prover-did-not-complete"}});
        evidence["layoutProof"] = layouts;
        evidence["verifiedCodeRelations"] = code_relations.verified;
        evidence["verifiedLeafPairs"] = code_relations.leaf_pairs;
        evidence["verifiedImportBindings"] = code_relations.import_pairs;
        evidence["verifiedIndependentCalleeBindings"] = code_relations.independent_callee_bindings;
        if (!code_relations.failures.empty()) evidence["codeRelationFailures"] = code_relations.failures;
        evidence["complete"] = evidence["unresolved"].empty();
        evidence["reason"] = evidence["unresolved"].empty() ? "all-profile-address-and-layout-checks-resolved" : "profile-contract-has-unresolved-evidence";
        return evidence["unresolved"].empty();
    } catch (const std::exception& error) {
        evidence["reason"] = error.what();
        return false;
    }
}
}
