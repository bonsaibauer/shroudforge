#include "code-inventory/resolve_original_call_graph.h"
#include "code-inventory/inspect_game_functions.h"
#include "named-services/named_service_contract.h"
#include <deque>
#include <map>
#include <set>

namespace OriginalCode {
namespace {
using NamedServiceImplementation::copy;
struct Range { std::uint32_t begin{}, end{}; };
struct Image {
    std::uintptr_t base{};
    std::uint32_t size{};
    std::map<std::uint32_t, std::vector<Range>> procedures;
    std::vector<Range> ranges;
    std::vector<IMAGE_SECTION_HEADER> sections;

    bool Read(std::uint32_t offset, void* out, std::size_t bytes) const {
        return offset <= size && bytes <= size - offset &&
            copy(out, reinterpret_cast<const void*>(base + offset), bytes);
    }
    Image() {
        base = reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
        IMAGE_DOS_HEADER dos{};
        IMAGE_NT_HEADERS64 nt{};
        // Header reads precede the bounded image reader.
        if (!copy(&dos, reinterpret_cast<const void*>(base), sizeof(dos)) ||
            dos.e_magic != IMAGE_DOS_SIGNATURE || dos.e_lfanew < 0 || dos.e_lfanew > 0x100000 ||
            !copy(&nt, reinterpret_cast<const void*>(base + dos.e_lfanew), sizeof(nt)) ||
            nt.Signature != IMAGE_NT_SIGNATURE || nt.FileHeader.Machine != IMAGE_FILE_MACHINE_AMD64 ||
            nt.OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR64_MAGIC) return;
        size = nt.OptionalHeader.SizeOfImage;
        const auto section_table = static_cast<std::size_t>(dos.e_lfanew) +
            offsetof(IMAGE_NT_HEADERS64, OptionalHeader) + nt.FileHeader.SizeOfOptionalHeader;
        if (!size || size > 1024u * 1024u * 1024u || !nt.FileHeader.NumberOfSections ||
            nt.FileHeader.NumberOfSections > 128 || section_table > size ||
            nt.FileHeader.NumberOfSections * sizeof(IMAGE_SECTION_HEADER) > size - section_table)
            throw std::runtime_error("invalid original section table");
        sections.resize(nt.FileHeader.NumberOfSections);
        if (!Read(static_cast<std::uint32_t>(section_table), sections.data(), sections.size() * sizeof(IMAGE_SECTION_HEADER)))
            throw std::runtime_error("unreadable original section table");
        for (const auto& section : sections)
            if (section.VirtualAddress > size || section.Misc.VirtualSize > size - section.VirtualAddress)
                throw std::runtime_error("invalid original section range");
        const auto dir = nt.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION];
        if (!size || dir.Size % sizeof(RUNTIME_FUNCTION) || !dir.Size || dir.Size > 16 * 1024 * 1024)
            throw std::runtime_error("invalid original unwind directory");
        std::vector<RUNTIME_FUNCTION> functions(dir.Size / sizeof(RUNTIME_FUNCTION));
        if (!Read(dir.VirtualAddress, functions.data(), dir.Size))
            throw std::runtime_error("unreadable original unwind directory");
        std::uint32_t previous_end{};
        for (const auto& record : functions) {
            if (record.BeginAddress < previous_end)
                throw std::runtime_error("overlapping original unwind ranges");
            previous_end = record.EndAddress;
            auto root = record;
            std::set<std::uint32_t> seen;
            for (;;) {
                std::array<std::uint8_t, 4> unwind{};
                if (root.BeginAddress >= root.EndAddress || root.EndAddress > size ||
                    !seen.insert(root.UnwindData).second || seen.size() > 128 ||
                    !Read(root.UnwindData, unwind.data(), unwind.size()) ||
                    ((unwind[0] & 7) != 1 && (unwind[0] & 7) != 2))
                    throw std::runtime_error("invalid original unwind chain");
                const auto flags = unwind[0] >> 3;
                if (flags & ~(UNW_FLAG_CHAININFO | UNW_FLAG_EHANDLER | UNW_FLAG_UHANDLER))
                    throw std::runtime_error("unknown original unwind flags");
                if (!(flags & UNW_FLAG_CHAININFO)) break;
                if (flags & (UNW_FLAG_EHANDLER | UNW_FLAG_UHANDLER))
                    throw std::runtime_error("invalid original unwind flags");
                const std::uint64_t tail = static_cast<std::uint64_t>(root.UnwindData) + 4ull +
                    static_cast<std::uint64_t>((unwind[2] + 1u) & ~1u) * 2ull;
                if (tail > UINT32_MAX || !Read(static_cast<std::uint32_t>(tail), &root, sizeof(root)))
                    throw std::runtime_error("unreadable original unwind chain");
            }
            if (record.BeginAddress >= record.EndAddress || record.EndAddress > size)
                throw std::runtime_error("invalid original code range");
            const Range range{record.BeginAddress, record.EndAddress};
            procedures[root.BeginAddress].push_back(range);
            ranges.push_back(range);
        }
        for (auto& [entry, parts] : procedures)
            std::sort(parts.begin(), parts.end(), [](const Range& a, const Range& b) { return a.begin < b.begin; });
    }
};
const Image& image() { static const Image value; return value; }

bool executable_span(std::uintptr_t at, std::size_t size) {
    if (!size || size > UINTPTR_MAX - at) return false;
    const auto& im = image();
    for (auto cursor = at; cursor < at + size;) {
        MEMORY_BASIC_INFORMATION page{};
        if (!NamedServiceImplementation::executable_callback(cursor) ||
            VirtualQuery(reinterpret_cast<const void*>(cursor), &page, sizeof(page)) != sizeof(page)) return false;
        const auto start = reinterpret_cast<std::uintptr_t>(page.BaseAddress);
        if (page.RegionSize > UINTPTR_MAX - start || start + page.RegionSize <= cursor) return false;
        cursor = (std::min)(at + size, start + page.RegionSize);
    }
    return at >= im.base && at - im.base <= im.size && size <= im.size - (at - im.base);
}

bool decode(const ZydisDecoder& decoder, std::uintptr_t at, const std::uint8_t* bytes,
    std::size_t length, Instruction& out) {
    out.address = at;
    if (!ZYAN_SUCCESS(ZydisDecoderDecodeFull(&decoder, bytes, length, &out.decoded, out.operands.data())) ||
        !out.decoded.length || out.decoded.length > length || out.decoded.length > out.bytes.size()) return false;
    std::copy_n(bytes,out.decoded.length,out.bytes.begin());
    return true;
}
bool target(const Instruction& ins, std::uintptr_t& address) {
    if (!ins.decoded.operand_count_visible || ins.operands[0].type != ZYDIS_OPERAND_TYPE_IMMEDIATE ||
        !ins.operands[0].imm.is_relative) return false;
    ZyanU64 absolute{};
    if (!ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&ins.decoded, &ins.operands[0], ins.address, &absolute))) return false;
    address = static_cast<std::uintptr_t>(absolute);
    return NamedServiceImplementation::executable_callback(address);
}
bool read_leaf(std::uintptr_t entry, const ZydisDecoder& decoder, Procedure& result) {
    // A missing unwind entry only admits a small call-free control-flow graph.
    // Every edge must terminate in this graph or return. Internal labels of an
    // unwind-owned function are never reinterpreted as leaf procedure entries.
    constexpr std::size_t limit = 512;
    const auto& im = image();
    const auto offset = entry - im.base;
    for (const auto& r : im.ranges) if (r.begin <= offset && offset < r.end) return false;
    const auto count = (std::min)(limit, static_cast<std::size_t>(im.size - offset));
    if (!executable_span(entry, count)) return false;
    std::vector<std::uint8_t> bytes(count), repeated(count);
    if (!copy(bytes.data(), reinterpret_cast<const void*>(entry), count)) return false;
    std::set<std::uintptr_t> pending{entry};
    std::map<std::uintptr_t, Instruction> instructions;
    while (!pending.empty()) {
        auto at = *pending.begin(); pending.erase(pending.begin());
        while (!instructions.contains(at)) {
            if (at < entry || at - entry >= count) return false;
            Instruction ins;
            if (!decode(decoder, at, bytes.data() + (at - entry), count - (at - entry), ins) ||
                ins.decoded.meta.category == ZYDIS_CATEGORY_CALL) return false;
            for (const auto& r : im.ranges)
                if (at < im.base + r.end && im.base + r.begin < at + ins.decoded.length) return false;
            for (const auto& [other, previous] : instructions)
                if (at < other + previous.decoded.length && other < at + ins.decoded.length) return false;
            instructions.emplace(at, ins);
            const auto category = ins.decoded.meta.category;
            if (category == ZYDIS_CATEGORY_RET || category == ZYDIS_CATEGORY_INTERRUPT ||
                ins.decoded.mnemonic == ZYDIS_MNEMONIC_UD2) break;
            if (category == ZYDIS_CATEGORY_UNCOND_BR || category == ZYDIS_CATEGORY_COND_BR) {
                std::uintptr_t next{};
                if (!target(ins, next)) {
                    // An indirect tail jump is a terminal edge just like an
                    // indirect call in an unwind-owned procedure. It can never
                    // satisfy a typed contract requiring direct native callees.
                    if (category != ZYDIS_CATEGORY_UNCOND_BR ||
                        (ins.operands[0].type == ZYDIS_OPERAND_TYPE_IMMEDIATE && ins.operands[0].imm.is_relative)) return false;
                    result.indirect_calls = true;
                    break;
                }
                const auto next_offset = next - im.base;
                const bool unwind_target = std::any_of(im.ranges.begin(), im.ranges.end(),
                    [next_offset](const Range& range) { return range.begin <= next_offset && next_offset < range.end; });
                if (next < entry || next - entry >= count || unwind_target) {
                    result.calls.push_back(next);
                } else pending.insert(next);
                if (category == ZYDIS_CATEGORY_UNCOND_BR) break;
            }
            at += ins.decoded.length;
        }
    }
    if (instructions.empty() || !copy(repeated.data(), reinterpret_cast<const void*>(entry), count) || repeated != bytes) return false;
    for (auto& [at, ins] : instructions) result.instructions.push_back(std::move(ins));
    return true;
}
}

bool AppendEvidence(const Procedure& procedure, CodeEvidence& output) {
    constexpr std::size_t maximum_block = 4096;
    CodeEvidence prepared;
    const auto append = [&](std::uintptr_t address, const std::uint8_t* bytes, std::size_t size) {
        if (!address || !size || size > UINTPTR_MAX - address) return false;
        while (size) {
            const auto count = (std::min)(size, maximum_block);
            if (!prepared.empty() && prepared.back().address + prepared.back().bytes.size() == address &&
                prepared.back().bytes.size() + count <= maximum_block) {
                prepared.back().bytes.insert(prepared.back().bytes.end(), bytes, bytes + count);
            } else prepared.push_back({address, {bytes, bytes + count}});
            address += count; bytes += count; size -= count;
        }
        return true;
    };
    for (const auto& instruction : procedure.instructions) {
        const auto size = instruction.decoded.length;
        if (!size || size > instruction.bytes.size() ||
            !append(instruction.address, instruction.bytes.data(), size)) return false;
    }
    for (const auto& data : procedure.read_only_data)
        if (!append(data.address, data.bytes.data(), data.bytes.size())) return false;
    if (prepared.empty()) return false;
    output.reserve(output.size() + prepared.size());
    for (auto& block : prepared) output.push_back(std::move(block));
    return true;
}

bool EvidenceUnchanged(const CodeEvidence& evidence) {
    if (evidence.empty()) return false;
    std::array<std::uint8_t, 4096> observed{};
    for (const auto& block : evidence) {
        if (!block.address || block.bytes.empty() || block.bytes.size() > observed.size() ||
            !copy(observed.data(), reinterpret_cast<const void*>(block.address), block.bytes.size()) ||
            !std::equal(block.bytes.begin(), block.bytes.end(), observed.begin())) return false;
    }
    return true;
}

bool ReadImageSections(std::vector<ImageSection>& output) {
    try {
        const auto& im = image();
        std::vector<ImageSection> result;
        for (const auto& header : im.sections) {
            if (!(header.Characteristics & IMAGE_SCN_MEM_READ) || !header.Misc.VirtualSize) continue;
            ImageSection section{im.base + header.VirtualAddress, header.Misc.VirtualSize, header.Characteristics, {}};
            if (!(header.Characteristics & IMAGE_SCN_MEM_WRITE) &&
                (header.Characteristics & (IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_CNT_INITIALIZED_DATA))) {
                section.bytes.resize(section.size);
                if (!im.Read(header.VirtualAddress, section.bytes.data(), section.size)) return false;
            }
            result.push_back(std::move(section));
        }
        if (result.empty()) return false;
        output = std::move(result);
        return true;
    } catch (...) { return false; }
}

bool ReadProcedure(std::uintptr_t entry, Procedure& output) {
    try {
        const auto& im = image();
        if (!im.size || entry < im.base || entry - im.base >= im.size ||
            !NamedServiceImplementation::executable_callback(entry)) return false;
        ZydisDecoder decoder;
        if (!ZYAN_SUCCESS(ZydisDecoderInit(&decoder, ZYDIS_MACHINE_MODE_LONG_64, ZYDIS_STACK_WIDTH_64))) return false;
        Procedure result; result.entry = entry;
        const auto found = im.procedures.find(static_cast<std::uint32_t>(entry - im.base));
        if (found == im.procedures.end()) {
            if (!read_leaf(entry, decoder, result)) return false;
        } else {
            struct Snapshot { Range range; std::vector<std::uint8_t> bytes; };
            std::vector<Snapshot> snapshots;
            for (const auto& range : found->second) {
                if (!snapshots.empty() && snapshots.back().range.end == range.begin)
                    snapshots.back().range.end = range.end;
                else snapshots.push_back({range, {}});
            }
            std::size_t total{};
            for (auto& snapshot : snapshots) {
                const auto size = snapshot.range.end - snapshot.range.begin;
                total += size;
                if (total > 1024 * 1024 || !executable_span(im.base + snapshot.range.begin, size)) return false;
                snapshot.bytes.resize(size);
                if (!im.Read(snapshot.range.begin, snapshot.bytes.data(), size)) return false;
            }
            const auto containing = [&](std::uintptr_t at) -> const Snapshot* {
                for (const auto& snapshot : snapshots)
                    if (im.base + snapshot.range.begin <= at && at < im.base + snapshot.range.end) return &snapshot;
                return nullptr;
            };
            // Unwind extents may also contain padding or inline jump tables.
            // Follow decoded control flow instead of reinterpreting those bytes
            // as instructions. Both sides of every direct conditional edge are
            // visited. Indirect transfers remain explicit in the evidence.
            std::set<std::uintptr_t> pending{entry};
            std::map<std::uintptr_t, Instruction> decoded;
            const auto resolve_switch = [&](const Instruction& jump) {
                const auto full = [](ZydisRegister r) { return ZydisRegisterGetLargestEnclosing(ZYDIS_MACHINE_MODE_LONG_64, r); };
                const auto writes = [&](const Instruction& ins, ZydisRegister r) {
                    for (unsigned i=0;i<ins.decoded.operand_count;++i)
                        if(ins.operands[i].type==ZYDIS_OPERAND_TYPE_REGISTER &&
                           (ins.operands[i].actions & ZYDIS_OPERAND_ACTION_MASK_WRITE) && full(ins.operands[i].reg.value)==r) return true;
                    return false;
                };
                std::vector<const Instruction*> block;
                auto it=decoded.find(jump.address);
                block.push_back(&it->second);
                while(it!=decoded.begin() && block.size()<48) {
                    const auto previous=std::prev(it);
                    if(previous->first+previous->second.decoded.length!=it->first)break;
                    it=previous;block.push_back(&it->second);
                }
                std::reverse(block.begin(),block.end());
                if(block.size()<5 || jump.operands[0].type!=ZYDIS_OPERAND_TYPE_REGISTER)return false;
                const auto& add=*block[block.size()-2];
                const auto& load=*block[block.size()-3];
                const auto dest=full(jump.operands[0].reg.value);
                if(add.decoded.mnemonic!=ZYDIS_MNEMONIC_ADD || add.operands[0].type!=ZYDIS_OPERAND_TYPE_REGISTER ||
                   full(add.operands[0].reg.value)!=dest || add.operands[1].type!=ZYDIS_OPERAND_TYPE_REGISTER ||
                   (load.decoded.mnemonic!=ZYDIS_MNEMONIC_MOV && load.decoded.mnemonic!=ZYDIS_MNEMONIC_MOVSXD) ||
                   load.operands[0].type!=ZYDIS_OPERAND_TYPE_REGISTER || full(load.operands[0].reg.value)!=dest ||
                   load.operands[1].type!=ZYDIS_OPERAND_TYPE_MEMORY || load.operands[1].size!=32 ||
                   load.operands[1].mem.scale!=4 || load.operands[1].mem.index==ZYDIS_REGISTER_NONE)return false;
                const auto base_register=full(add.operands[1].reg.value);
                const auto index_register=full(load.operands[1].mem.index);
                if(full(load.operands[1].mem.base)!=base_register || dest==base_register ||
                   index_register==base_register || index_register==dest)return false;
                std::uintptr_t origin{};
                for(std::size_t i=block.size()-3;i-->0;) {
                    const auto& ins=*block[i];
                    if(!writes(ins,base_register))continue;
                    ZyanU64 absolute{};
                    if(ins.decoded.mnemonic!=ZYDIS_MNEMONIC_LEA || ins.operands[1].type!=ZYDIS_OPERAND_TYPE_MEMORY ||
                       ins.operands[1].mem.base!=ZYDIS_REGISTER_RIP ||
                       !ZYAN_SUCCESS(ZydisCalcAbsoluteAddress(&ins.decoded,&ins.operands[1],ins.address,&absolute)))return false;
                    origin=static_cast<std::uintptr_t>(absolute);break;
                }
                if(origin<im.base || origin-im.base>=im.size)return false;
                std::size_t count{};
                for(std::size_t i=block.size()-3;i-->0;) {
                    const auto& ins=*block[i];
                    if(writes(ins,index_register))return false;
                    if(ins.decoded.mnemonic!=ZYDIS_MNEMONIC_CMP || ins.operands[0].type!=ZYDIS_OPERAND_TYPE_REGISTER ||
                       full(ins.operands[0].reg.value)!=index_register || ins.operands[1].type!=ZYDIS_OPERAND_TYPE_IMMEDIATE)continue;
                    const auto& guard=*block[i+1];
                    const auto* name=ZydisMnemonicGetString(guard.decoded.mnemonic);
                    if(!name || (std::strcmp(name,"jnbe") && std::strcmp(name,"jnb")))return false;
                    std::uintptr_t default_target{};
                    if(!target(guard,default_target) || (ins.address<=default_target && default_target<=jump.address))return false;
                    const auto bound=ins.operands[1].imm.value.u;
                    if(bound>255 || (!std::strcmp(name,"jnb") && !bound))return false;
                    count=static_cast<std::size_t>(bound)+(!std::strcmp(name,"jnbe")?1:0);break;
                }
                if(!count)return false;
                const auto displacement=load.operands[1].mem.disp.value;
                std::uintptr_t table{};
                if(displacement>=0) {
                    if(static_cast<std::uint64_t>(displacement)>UINTPTR_MAX-origin)return false;
                    table=origin+static_cast<std::uintptr_t>(displacement);
                } else {
                    const auto distance=std::uint64_t{0}-static_cast<std::uint64_t>(displacement);
                    if(distance>origin)return false;table=origin-static_cast<std::uintptr_t>(distance);
                }
                if(table<im.base || table-im.base>im.size || count*4>im.size-(table-im.base))return false;
                MEMORY_BASIC_INFORMATION page{};
                if(VirtualQuery(reinterpret_cast<const void*>(table),&page,sizeof(page))!=sizeof(page) ||
                   page.Type!=MEM_IMAGE || reinterpret_cast<std::uintptr_t>(page.AllocationBase)!=im.base ||
                   (page.Protect!=PAGE_READONLY && page.Protect!=PAGE_EXECUTE_READ) ||
                   table+count*4>reinterpret_cast<std::uintptr_t>(page.BaseAddress)+page.RegionSize)return false;
                Procedure::ReadOnlyData data{table,std::vector<std::uint8_t>(count*4)};
                if(!copy(data.bytes.data(),reinterpret_cast<const void*>(table),data.bytes.size()))return false;
                std::vector<std::uintptr_t> targets;
                for(std::size_t i=0;i<count;++i) {
                    std::uint32_t relative{};std::memcpy(&relative,data.bytes.data()+i*4,4);
                    const auto delta=load.decoded.mnemonic==ZYDIS_MNEMONIC_MOVSXD
                        ? static_cast<std::int64_t>(static_cast<std::int32_t>(relative)) : static_cast<std::int64_t>(relative);
                    const auto destination=static_cast<std::uintptr_t>(static_cast<std::int64_t>(origin)+delta);
                    if(!containing(destination))return false;
                    targets.push_back(destination);
                }
                for(const auto destination:targets)pending.insert(destination);
                result.read_only_data.push_back(std::move(data));
                return true;
            };
            while (!pending.empty()) {
                auto at = *pending.begin(); pending.erase(pending.begin());
                while (!decoded.contains(at)) {
                    const auto* snapshot = containing(at);
                    if (!snapshot) return false;
                    const auto offset = at - im.base - snapshot->range.begin;
                    Instruction ins;
                    if (!decode(decoder, at, snapshot->bytes.data() + offset, snapshot->bytes.size() - offset, ins)) return false;
                    for (const auto& [other, previous] : decoded)
                        if (at < other + previous.decoded.length && other < at + ins.decoded.length) return false;
                    decoded.emplace(at, ins);
                    const auto category = ins.decoded.meta.category;
                    if (category == ZYDIS_CATEGORY_CALL) {
                        std::uintptr_t callee{};
                        if (target(ins, callee)) result.calls.push_back(callee);
                        else result.indirect_calls = true;
                    }
                    if (category == ZYDIS_CATEGORY_RET || category == ZYDIS_CATEGORY_INTERRUPT ||
                        ins.decoded.mnemonic == ZYDIS_MNEMONIC_UD2) break;
                    if (category == ZYDIS_CATEGORY_COND_BR || category == ZYDIS_CATEGORY_UNCOND_BR) {
                        std::uintptr_t next{};
                        if (!target(ins, next)) {
                            if (category != ZYDIS_CATEGORY_UNCOND_BR ||
                                (ins.operands[0].type == ZYDIS_OPERAND_TYPE_IMMEDIATE && ins.operands[0].imm.is_relative)) return false;
                            result.indirect_calls = true;
                            resolve_switch(ins);
                        } else if (containing(next)) pending.insert(next);
                        else result.calls.push_back(next);
                        if (category == ZYDIS_CATEGORY_UNCOND_BR) break;
                    }
                    at += ins.decoded.length;
                }
            }
            for (auto& [at, ins] : decoded) result.instructions.push_back(std::move(ins));
            // Call positions are canonical instruction order, independent of
            // the order in which conditional paths entered the worklist.
            result.calls.clear();
            for (const auto& ins : result.instructions) {
                const auto category = ins.decoded.meta.category;
                if (category != ZYDIS_CATEGORY_CALL && category != ZYDIS_CATEGORY_COND_BR && category != ZYDIS_CATEGORY_UNCOND_BR) continue;
                std::uintptr_t callee{};
                if (target(ins, callee) && (category == ZYDIS_CATEGORY_CALL || !containing(callee))) result.calls.push_back(callee);
            }
            for (const auto& snapshot : snapshots) {
                std::vector<std::uint8_t> repeated(snapshot.bytes.size());
                if (!im.Read(snapshot.range.begin, repeated.data(), repeated.size()) || repeated != snapshot.bytes) return false;
            }
            for(const auto& data:result.read_only_data) {
                std::vector<std::uint8_t> repeated(data.bytes.size());
                if(!copy(repeated.data(),reinterpret_cast<const void*>(data.address),repeated.size()) || repeated!=data.bytes)return false;
            }
        }
        std::sort(result.instructions.begin(), result.instructions.end(), [](const auto& a, const auto& b) { return a.address < b.address; });
        output = std::move(result);
        return true;
    } catch (...) { return false; }
}

bool ReadContainingProcedure(std::uintptr_t address, Procedure& output) {
    try {
        const auto& im = image();
        if (!im.size || address < im.base || address - im.base >= im.size) return false;
        const auto offset = static_cast<std::uint32_t>(address - im.base);
        for (const auto& [entry, ranges] : im.procedures) {
            if (!std::any_of(ranges.begin(), ranges.end(), [offset](const Range& range) {
                return range.begin <= offset && offset < range.end;
            })) continue;
            Procedure result;
            if (!ReadProcedure(im.base + entry, result) ||
                !std::any_of(result.instructions.begin(), result.instructions.end(),
                    [address](const Instruction& instruction) { return instruction.address == address; })) return false;
            output = std::move(result);
            return true;
        }
        return false;
    } catch (...) { return false; }
}

bool ResolveNamedSystemCode(const char* exact_name, std::uintptr_t& output) {
    if (!exact_name || !*exact_name || std::strlen(exact_name) > 96) return false;
    try {
        // Reuse the canonical original descriptor inventory. Its names and
        // function pointers come from the loaded image, not an RVA catalog.
        static const auto catalog = nlohmann::json::parse(FunctionInventory::Build());
        std::uintptr_t selected{};
        for (const auto& function : catalog.at("entries"))
            for (const auto& descriptor : function.value("engine_descriptors", nlohmann::json::array()))
                if (descriptor.at("name") == exact_name) {
                    const auto address = image().base + function.at("rva").get<std::uint32_t>();
                    if (selected && selected != address) return false;
                    selected = address;
                }
        Procedure code;
        if (!selected || !ReadProcedure(selected, code)) return false;
        output = selected;
        return true;
    } catch (...) { return false; }
}

bool FindUniqueReachable(std::uintptr_t root, unsigned maximum_depth,
    const std::function<bool(const Procedure&)>& predicate, std::uintptr_t& output) {
    if (!root || maximum_depth > 8 || !predicate) return false;
    try {
        constexpr std::size_t maximum_functions = 4096;
        std::deque<std::pair<std::uintptr_t, unsigned>> pending{{root, 0}};
        std::set<std::uintptr_t> visited;
        std::uintptr_t selected{};
        while (!pending.empty()) {
            const auto [entry, depth] = pending.front(); pending.pop_front();
            if (!visited.insert(entry).second) continue;
            if (visited.size() > maximum_functions) return false;
            Procedure code;
            if (!ReadProcedure(entry, code)) return false;
            if (predicate(code)) {
                if (selected && selected != entry) return false;
                selected = entry;
            }
            if (depth < maximum_depth)
                for (const auto callee : code.calls) if (!visited.contains(callee)) pending.emplace_back(callee, depth + 1);
        }
        if (!selected) return false;
        output = selected;
        return true;
    } catch (...) { return false; }
}
}
