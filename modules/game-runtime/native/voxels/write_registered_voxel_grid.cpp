#include "voxels/write_registered_voxel_grid.h"
#include "voxels/read_registered_voxel_grid.h"
#include "voxels/voxel_request.h"
#include "impact/resolve_registered_operation.h"
#include "code-inventory/resolve_original_call_graph.h"
#include "named-services/named_service_contract.h"
#include <atomic>
#include <deque>
#include <map>
#include <mutex>
#include <set>
#include <thread>

namespace WorldRuntime::RegisteredVoxelGrid {
namespace {
using OriginalCode::Instruction;
using OriginalCode::Procedure;
using NamedServiceImplementation::field;
using NamedServiceImplementation::copy;

enum class Role { Unknown, Stack, World, Span, Dimensions, Origin, Store, Cells, Flag, Constant, Dimension };
struct Value { Role role{}; std::int64_t offset{}; std::uint64_t number{}; };
using Registers = std::array<Value, ZYDIS_REGISTER_MAX_VALUE + 1>;
ZydisRegister full(ZydisRegister r) {
    return ZydisRegisterGetLargestEnclosing(ZYDIS_MACHINE_MODE_LONG_64, r);
}
Value& reg(Registers& values, ZydisRegister r) { return values[full(r)]; }
bool immediate(const ZydisDecodedOperand& op, std::uint64_t value) {
    return op.type == ZYDIS_OPERAND_TYPE_IMMEDIATE && op.imm.value.u == value;
}
bool memory(const ZydisDecodedOperand& op, ZydisRegister base, std::int64_t displacement) {
    return op.type == ZYDIS_OPERAND_TYPE_MEMORY && op.mem.base == base &&
        op.mem.index == ZYDIS_REGISTER_NONE && op.mem.disp.value == displacement;
}

// This is a small ABI/data-flow verifier for the complete original block
// copier, not a byte pattern or a positional call selection. Every instruction
// is decoded. All its own writes must target its stack. At its sole call the
// four ABI registers and fifth stack argument must be proven values derived
// from the original World, Span, dimensions and origin arguments.
bool writer_contract(const Procedure& code) {
    if (code.calls.size() != 1 || code.indirect_calls || code.instructions.empty()) return false;
    Registers registers{};
    reg(registers, ZYDIS_REGISTER_RSP) = {Role::Stack, 0};
    reg(registers, ZYDIS_REGISTER_RCX) = {Role::World};
    reg(registers, ZYDIS_REGISTER_RDX) = {Role::Span};
    reg(registers, ZYDIS_REGISTER_R8) = {Role::Dimensions};
    reg(registers, ZYDIS_REGISTER_R9) = {Role::Origin};
    std::map<std::int64_t, Value> stack;
    std::set<std::uintptr_t> addresses;
    for (const auto& ins : code.instructions) addresses.insert(ins.address);
    unsigned word_loads{}, calls{}, dimension_reads{}, dimension_loops{};
    std::set<ZydisRegister> steps;
    bool store_loaded{}, flag_loaded{}, block_span{}, returned{};
    const auto load = [&](const ZydisDecodedOperand& op, Value& value) {
        if (op.type == ZYDIS_OPERAND_TYPE_REGISTER) { value = reg(registers, op.reg.value); return true; }
        if (op.type == ZYDIS_OPERAND_TYPE_IMMEDIATE) { value = {Role::Constant, 0, op.imm.value.u}; return true; }
        if (op.type != ZYDIS_OPERAND_TYPE_MEMORY) return false;
        const auto base = reg(registers, op.mem.base);
        const auto displacement = op.mem.disp.value;
        if (base.role == Role::Stack) {
            value = {};
            if (op.mem.index == ZYDIS_REGISTER_NONE) {
                const auto at = stack.find(base.offset + displacement);
                if (at != stack.end()) value = at->second;
            }
            return true;
        }
        if (base.role == Role::Cells && op.size == 16 && op.mem.scale == 2) {
            value = {}; ++word_loads; return true;
        }
        if (op.mem.index != ZYDIS_REGISTER_NONE) return false;
        if (base.role == Role::World && displacement == 0x1220 && op.size == 64) {
            value = {Role::Store}; store_loaded = true; return true;
        }
        if (base.role == Role::Span && displacement == 0 && op.size == 64) {
            value = {Role::Cells}; return true;
        }
        if (base.role == Role::Store && displacement == 0x2c0 && op.size == 8) {
            value = {Role::Flag}; flag_loaded = true; return true;
        }
        if (base.role == Role::Dimensions && op.size == 32 &&
            (displacement == 0 || displacement == 4 || displacement == 8)) {
            value = {Role::Dimension, displacement / 4};
            dimension_reads |= 1u << value.offset;
            return true;
        }
        if (base.role == Role::Origin && ((displacement == 0 && op.size == 64) ||
            (displacement == 8 && op.size == 32))) { value = {}; return true; }
        return false;
    };
    for (const auto& ins : code.instructions) {
        const auto& d = ins.decoded;
        const auto& a = ins.operands[0];
        const auto& b = ins.operands[1];
        const auto n = d.operand_count_visible;
        if (d.meta.category == ZYDIS_CATEGORY_CALL) {
            if (++calls != 1 || reg(registers, ZYDIS_REGISTER_RCX).role != Role::Store ||
                reg(registers, ZYDIS_REGISTER_RDX).role != Role::Stack ||
                reg(registers, ZYDIS_REGISTER_R8).role != Role::Constant ||
                reg(registers, ZYDIS_REGISTER_R8).number != 6 ||
                reg(registers, ZYDIS_REGISTER_R9).role != Role::Stack) return false;
            const auto span = reg(registers, ZYDIS_REGISTER_R9).offset;
            const auto data = stack.find(span), count = stack.find(span + 8);
            const auto flag = stack.find(reg(registers, ZYDIS_REGISTER_RSP).offset + 32);
            if (data == stack.end() || data->second.role != Role::Stack || count == stack.end() ||
                count->second.role != Role::Constant || count->second.number != 512 ||
                flag == stack.end() || flag->second.role != Role::Flag) return false;
            block_span = true;
            for (const auto r : {ZYDIS_REGISTER_RAX, ZYDIS_REGISTER_RCX, ZYDIS_REGISTER_RDX,
                ZYDIS_REGISTER_R8, ZYDIS_REGISTER_R9, ZYDIS_REGISTER_R10, ZYDIS_REGISTER_R11}) reg(registers, r) = {};
            continue;
        }
        if (d.meta.category == ZYDIS_CATEGORY_COND_BR || d.meta.category == ZYDIS_CATEGORY_UNCOND_BR) {
            ZyanU64 target{};
            if (!n || a.type != ZYDIS_OPERAND_TYPE_IMMEDIATE || !a.imm.is_relative ||
                !ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&d, &a, ins.address, &target)) ||
                !addresses.contains(static_cast<std::uintptr_t>(target))) return false;
            continue;
        }
        if (d.mnemonic == ZYDIS_MNEMONIC_RET) { returned = true; continue; }
        if (d.mnemonic == ZYDIS_MNEMONIC_NOP) continue;
        if (d.mnemonic == ZYDIS_MNEMONIC_PUSH || d.mnemonic == ZYDIS_MNEMONIC_POP) {
            auto& sp = reg(registers, ZYDIS_REGISTER_RSP);
            if (sp.role != Role::Stack) return false;
            sp.offset += d.mnemonic == ZYDIS_MNEMONIC_PUSH ? -8 : 8;
            if (d.mnemonic == ZYDIS_MNEMONIC_POP && a.type == ZYDIS_OPERAND_TYPE_REGISTER) reg(registers, a.reg.value) = {};
            continue;
        }
        if (d.mnemonic == ZYDIS_MNEMONIC_LEA) {
            if (n != 2 || a.type != ZYDIS_OPERAND_TYPE_REGISTER || b.type != ZYDIS_OPERAND_TYPE_MEMORY) return false;
            const auto base = reg(registers, b.mem.base);
            reg(registers, a.reg.value) = base.role == Role::Stack && b.mem.index == ZYDIS_REGISTER_NONE
                ? Value{Role::Stack, base.offset + b.mem.disp.value} : Value{};
            continue;
        }
        Value right{};
        for (unsigned i = 0; i < n; ++i) {
            const auto& op = ins.operands[i];
            if (op.type == ZYDIS_OPERAND_TYPE_MEMORY) {
                if ((op.actions & ZYDIS_OPERAND_ACTION_MASK_WRITE) && reg(registers, op.mem.base).role != Role::Stack)
                    return false;
                if (op.actions & ZYDIS_OPERAND_ACTION_MASK_READ) {
                    Value read_value;
                    if (!load(op, read_value)) return false;
                }
            }
        }
        if (d.mnemonic == ZYDIS_MNEMONIC_MOV || d.mnemonic == ZYDIS_MNEMONIC_MOVZX ||
            d.mnemonic == ZYDIS_MNEMONIC_MOVSD) {
            if (n != 2 || !load(b, right)) return false;
            if (a.type == ZYDIS_OPERAND_TYPE_REGISTER) reg(registers, a.reg.value) = right;
            else if (a.type == ZYDIS_OPERAND_TYPE_MEMORY) {
                const auto base = reg(registers, a.mem.base);
                if (base.role != Role::Stack) return false;
                if (a.mem.index == ZYDIS_REGISTER_NONE) stack[base.offset + a.mem.disp.value] = right;
            } else return false;
            continue;
        }
        if ((d.mnemonic == ZYDIS_MNEMONIC_ADD || d.mnemonic == ZYDIS_MNEMONIC_SUB) &&
            n == 2 && a.type == ZYDIS_OPERAND_TYPE_REGISTER && b.type == ZYDIS_OPERAND_TYPE_IMMEDIATE) {
            auto& value = reg(registers, a.reg.value);
            if (value.role == Role::Stack) value.offset += d.mnemonic == ZYDIS_MNEMONIC_ADD
                ? b.imm.value.s : -b.imm.value.s;
            else if (value.role == Role::Constant) {
                if (d.mnemonic == ZYDIS_MNEMONIC_ADD) value.number += b.imm.value.u;
                else value.number -= b.imm.value.u;
            }
            else value = {};
            if (d.mnemonic == ZYDIS_MNEMONIC_ADD && a.size == 32 && immediate(b, 8)) steps.insert(full(a.reg.value));
            continue;
        }
        if (d.mnemonic == ZYDIS_MNEMONIC_CMP) {
            if (n != 2 || !load(b, right)) return false;
            if (a.type == ZYDIS_OPERAND_TYPE_REGISTER && right.role == Role::Dimension && steps.contains(full(a.reg.value)))
                dimension_loops |= 1u << right.offset;
            continue;
        }
        if (d.mnemonic == ZYDIS_MNEMONIC_XOR && n == 2 && a.type == ZYDIS_OPERAND_TYPE_REGISTER &&
            b.type == ZYDIS_OPERAND_TYPE_REGISTER && full(a.reg.value) == full(b.reg.value)) {
            reg(registers, a.reg.value) = {Role::Constant, 0, 0}; continue;
        }
        switch (d.mnemonic) {
        case ZYDIS_MNEMONIC_TEST: case ZYDIS_MNEMONIC_SHR: case ZYDIS_MNEMONIC_IMUL:
        case ZYDIS_MNEMONIC_ADD: case ZYDIS_MNEMONIC_SUB: case ZYDIS_MNEMONIC_INC:
            for (unsigned i = 0; i < n; ++i) if (ins.operands[i].type == ZYDIS_OPERAND_TYPE_REGISTER &&
                (ins.operands[i].actions & ZYDIS_OPERAND_ACTION_MASK_WRITE)) reg(registers, ins.operands[i].reg.value) = {};
            break;
        default: return false;
        }
    }
    // Each source word is encountered once by the general read validation and
    // once by the MOVZX transfer. There are eight unrolled words per row.
    return calls == 1 && returned && store_loaded && flag_loaded && block_span &&
        word_loads == 16 && dimension_reads == 7 && dimension_loops == 7 && steps.size() == 3;
}

bool commit_contract(const Procedure& code) {
    bool gate{}, acquire{}, release{}, span{};
    for (const auto& ins : code.instructions) {
        const auto& d = ins.decoded;
        const auto& a = ins.operands[0]; const auto& b = ins.operands[1];
        gate |= d.mnemonic == ZYDIS_MNEMONIC_LEA && a.type == ZYDIS_OPERAND_TYPE_REGISTER &&
            a.reg.value == ZYDIS_REGISTER_RDI && memory(b, ZYDIS_REGISTER_R13, 0x118);
        if (d.mnemonic == ZYDIS_MNEMONIC_ADD && (d.attributes & ZYDIS_ATTRIB_HAS_LOCK) && memory(a, ZYDIS_REGISTER_RDI, 0)) {
            acquire |= immediate(b, 0x10000); release |= immediate(b, 0x7fff0000);
        }
        span |= d.mnemonic == ZYDIS_MNEMONIC_MOV && a.type == ZYDIS_OPERAND_TYPE_REGISTER &&
            a.reg.value == ZYDIS_REGISTER_R11 && memory(b, ZYDIS_REGISTER_R9, 0);
    }
    return gate && acquire && release && span;
}

bool dirty_commit(const Procedure& code) {
    bool dirty{}, allocator{}, notify{};
    for (const auto& ins : code.instructions) {
        const auto& a = ins.operands[0]; const auto& b = ins.operands[1];
        dirty |= ins.decoded.mnemonic == ZYDIS_MNEMONIC_MOV && a.size == 8 &&
            a.type == ZYDIS_OPERAND_TYPE_MEMORY && a.mem.disp.value == 0x2c1 && immediate(b, 1);
        allocator |= ins.decoded.meta.category == ZYDIS_CATEGORY_CALL && a.type == ZYDIS_OPERAND_TYPE_MEMORY && a.mem.disp.value == 8;
        notify |= ins.decoded.mnemonic == ZYDIS_MNEMONIC_OR && a.type == ZYDIS_OPERAND_TYPE_MEMORY && a.mem.scale == 8;
    }
    return dirty && allocator && notify;
}

struct WriterCode { std::uintptr_t root{}, writer{}; OriginalCode::CodeEvidence bytes; };
std::mutex resolution_mutex;
WriterCode cached;

bool capture_code(const Procedure& code, WriterCode& result) {
    return OriginalCode::AppendEvidence(code, result.bytes);
}
bool code_unchanged(const WriterCode& code) {
    return OriginalCode::EvidenceUnchanged(code.bytes);
}
bool resolve_writer(std::uintptr_t root, WriterCode& output) {
    std::scoped_lock lock(resolution_mutex);
    if (cached.root == root && cached.writer && code_unchanged(cached)) { output = cached; return true; }
    WriterCode result; result.root = root;
    if (!OriginalCode::FindUniqueReachable(root, 6, writer_contract, result.writer)) return false;
    Procedure writer, commit, dirty;
    std::uintptr_t dirty_entry{};
    if (!OriginalCode::ReadProcedure(result.writer, writer) || !writer_contract(writer) ||
        !OriginalCode::ReadProcedure(writer.calls.front(), commit) || !commit_contract(commit) ||
        !OriginalCode::FindUniqueReachable(commit.entry, 2, dirty_commit, dirty_entry) ||
        !OriginalCode::ReadProcedure(dirty_entry, dirty) || !dirty_commit(dirty)) return false;
    // Keep one decoded directed path from the named entry to this unique
    // verified writer. The snapshot is compared again under the Scene gate.
    std::deque<std::vector<std::uintptr_t>> pending{{root}};
    std::set<std::uintptr_t> visited;
    bool found{};
    while (!pending.empty() && visited.size() < 4096) {
        auto path = std::move(pending.front()); pending.pop_front();
        const auto at = path.back();
        if (!visited.insert(at).second) continue;
        Procedure code;
        if (!OriginalCode::ReadProcedure(at, code)) continue;
        if (at == result.writer) {
            for (const auto step : path) {
                Procedure part;
                if (!OriginalCode::ReadProcedure(step, part) || !capture_code(part, result)) return false;
            }
            found = true; break;
        }
        if (path.size() < 7) for (const auto target : code.calls) {
            auto next = path; next.push_back(target); pending.push_back(std::move(next));
        }
    }
    if (!found || !capture_code(commit, result) || !capture_code(dirty, result) || !code_unchanged(result)) return false;
    cached = result; output = std::move(result); return true;
}

bool invoke_write(std::uintptr_t callback, std::uintptr_t world,
    const std::uint16_t* values, std::size_t count, const std::uint32_t* dimensions, const std::int32_t* origin) {
    struct Span { const std::uint16_t* data; std::uint64_t count; } span{values, count};
    using Callback = void(__fastcall*)(void*, const Span*, const std::uint32_t*, const std::int32_t*);
    __try {
        reinterpret_cast<Callback>(callback)(reinterpret_cast<void*>(world), &span, dimensions, origin);
        return true;
    } __except (EXCEPTION_EXECUTE_HANDLER) { return false; }
}

bool registered_root(const NamedServiceImplementation::BindingContract& binding,
    const EngineContext::EcsView& owner, std::uintptr_t& root) {
    ImpactRuntime::RegisteredOperation operation;
    if (ImpactRuntime::ResolveRegisteredOperationPinned(binding, owner, "terraforming", operation) !=
        ImpactRuntime::ResolveResult::Found || operation.arguments != 9) return false;
    root = operation.callback; return true;
}

// The registered root of the local authority's world, read under the Scene
// gate. With a region, that region must also be pinned there.
bool authority_root(PinnedRegion* region, std::uintptr_t& root) {
    const auto binding = NamedServiceImplementation::binding_contract("impact::ImpactSystem");
    EngineContext::EcsView owner;
    if (!EngineContext::ResolveEcs(owner) || owner.role != EngineContext::EcsRole::LocalAuthority) return false;
    std::unique_lock operation(NamedServiceImplementation::operation_mutex, std::try_to_lock);
    if (!operation.owns_lock()) return false;
    EngineContext::EcsWriteGuard scene;
    return scene.TryAcquire(owner) && scene.Validate() && (!region || region->AvailablePinned(owner)) &&
        registered_root(binding, owner, root) && scene.Validate();
}

// The first resolution decodes the code reachable from the root, about 1.2 s
// in game: it would stall the first write's caller (the mods' loop). A worker
// does it once the world accepts writes; it only reads the game's code.
std::mutex preparation_mutex;
std::thread preparation;
std::atomic<bool> prepared{}, preparing{};
std::atomic<std::uint64_t> next_preparation_ms{};
}

bool WriteAvailable() {
    try {
        if (!Healthy()) return false;
        const std::int32_t origin[3]{}; const std::uint32_t dimensions[3]{1, 1, 1};
        PinnedRegion region(origin, dimensions);
        std::uintptr_t root{};
        if (!authority_root(&region, root)) return false;
        WriterCode code;
        return resolve_writer(root, code);
    } catch (...) { return false; }
}

void PrepareWriter() {
    if (prepared.load(std::memory_order_acquire) || preparing.load(std::memory_order_acquire)) return;
    const auto now = GetTickCount64();
    if (now < next_preparation_ms.load(std::memory_order_relaxed)) return;
    next_preparation_ms.store(now + 1000, std::memory_order_relaxed);
    try {
        std::uintptr_t root{};
        if (!Healthy() || !authority_root(nullptr, root)) return;
        std::scoped_lock lock(preparation_mutex);
        if (preparation.joinable()) preparation.join();
        preparing.store(true, std::memory_order_release);
        preparation = std::thread([root] {
            bool resolved{};
            try { WriterCode code; resolved = resolve_writer(root, code); } catch (...) {}
            // Not resolved: tried again later, the write still resolves it itself.
            if (!resolved) next_preparation_ms.store(GetTickCount64() + 30000, std::memory_order_relaxed);
            prepared.store(resolved, std::memory_order_release);
            preparing.store(false, std::memory_order_release);
        });
    } catch (...) { preparing.store(false, std::memory_order_release); }
}

void StopPreparingWriter() {
    std::scoped_lock lock(preparation_mutex);
    if (preparation.joinable()) preparation.join();
}

bool WriteForSession(std::uint64_t expected_session, const std::int32_t origin[3],
    const std::uint32_t dimensions[3], const std::uint16_t* values, std::size_t count, std::uint32_t* outcome) {
    if (!outcome) return false;
    *outcome = 1;
    bool attempted{};
    try {
        std::size_t expected{};
        Implementation::WriteRegion expanded;
        if (!expected_session || !origin || !values || !Implementation::cell_count(dimensions, expected) ||
            count != expected || !Healthy() || !Implementation::plan_write_region(origin, dimensions, expanded)) return false;
        std::vector<std::uint16_t> input(count), original(expanded.count), replacement(expanded.count), observed(expanded.count);
        if (!copy(input.data(), values, count * sizeof(std::uint16_t))) return false;
        PinnedRegion region(expanded.origin.data(), expanded.dimensions.data());
        const auto binding = NamedServiceImplementation::binding_contract("impact::ImpactSystem");
        ImpactRuntime::RegisteredOperation operation;
        if (ImpactRuntime::ResolveRegisteredOperation(expected_session, "terraforming", operation) !=
            ImpactRuntime::ResolveResult::Found || operation.arguments != 9) return false;
        WriterCode code;
        if (!resolve_writer(operation.callback, code)) return false;
        std::scoped_lock operation_lock(NamedServiceImplementation::operation_mutex);
        EcsRuntime::SessionGuard session;
        if (!session.TryAcquire(expected_session) || !session.WritesAllowed()) return false;
        EngineContext::EcsWriteGuard scene;
        std::uintptr_t root{};
        if (!scene.TryAcquire(session.View()) || !scene.Validate() || !registered_root(binding, session.View(), root) ||
            root != code.root || !code_unchanged(code) ||
            region.ReadPinned(session.View(), original.data(), original.size()) != Result::Success || !scene.Validate()) return false;
        std::copy(original.begin(), original.end(), replacement.begin());
        for (std::size_t z = 0; z < dimensions[2]; ++z)
            for (std::size_t y = 0; y < dimensions[1]; ++y)
                for (std::size_t x = 0; x < dimensions[0]; ++x) {
                    const auto source = (z * dimensions[1] + y) * dimensions[0] + x;
                    const auto target = ((z + expanded.offset[2]) * expanded.dimensions[1] + y + expanded.offset[1]) *
                        expanded.dimensions[0] + x + expanded.offset[0];
                    replacement[target] = input[source];
                }
        if (replacement == original) { *outcome = 0; return true; }
        if (!scene.Validate() || !region.ValidatePinned(session.View()) || !code_unchanged(code)) return false;
        attempted = true;
        const auto fence = [&]() {
            *outcome = 3;
            scene.Release();
            session.MarkWriteUncertain("registered-voxel-write-outcome-uncertain");
            return false;
        };
        if (!invoke_write(code.writer, region.WorldPayload(), replacement.data(), replacement.size(),
            expanded.dimensions.data(), expanded.origin.data())) return fence();
        if (scene.Validate() && region.ReadPinned(session.View(), observed.data(), observed.size()) == Result::Success &&
            observed == replacement && scene.Validate() && code_unchanged(code)) { *outcome = 0; return true; }
        // A normal return plus stable owners permits a bounded restoration.
        // A faulting native call never re-enters a possibly held Store gate.
        if (!Healthy() || !scene.Validate() || !region.ValidatePinned(session.View()) || !code_unchanged(code)) return fence();
        if (!invoke_write(code.writer, region.WorldPayload(), original.data(), original.size(),
            expanded.dimensions.data(), expanded.origin.data())) return fence();
        if (scene.Validate() && region.ReadPinned(session.View(), observed.data(), observed.size()) == Result::Success &&
            observed == original && scene.Validate() && code_unchanged(code)) { *outcome = 2; return false; }
        return fence();
    } catch (...) {
        if (attempted) {
            *outcome = 3;
            EcsRuntime::MarkSessionWriteUncertain(expected_session, "registered-voxel-write-exception");
        }
        return false;
    }
}
}
