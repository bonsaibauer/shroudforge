#pragma once
#include <windows.h>
#include <nlohmann/json.hpp>
#include <map>
#include <set>
#include <cstring>
#include <stdexcept>
#include <algorithm>
#include <vector>
#include "profile.h"

namespace FunctionInventory {
// Windows unwind metadata supplies code extents, never native call signatures.
inline std::string Build(const std::uint8_t* image_override = nullptr, std::uintptr_t address_base = 0) {
    using namespace KfcRuntimeCompatibility::EnshroudedClient;
    const auto* image = image_override ? image_override : reinterpret_cast<const std::uint8_t*>(GetModuleHandleW(nullptr));
    if (!image || !image_size) throw std::runtime_error("executable identity unavailable");
    if (!address_base) address_base = reinterpret_cast<std::uintptr_t>(image);
    auto bytes = [&](std::uint32_t rva, std::size_t length) {
        if (length > image_size || rva > image_size - length) throw std::runtime_error("unwind metadata outside image");
        return image + rva;
    };
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(bytes(0, sizeof(IMAGE_DOS_HEADER)));
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(bytes(dos->e_lfanew, sizeof(IMAGE_NT_HEADERS64)));
    const auto directory = nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION];
    if (directory.Size % sizeof(RUNTIME_FUNCTION)) throw std::runtime_error("truncated exception directory");
    const auto* functions = reinterpret_cast<const RUNTIME_FUNCTION*>(bytes(directory.VirtualAddress, directory.Size));
    const auto count = directory.Size / sizeof(RUNTIME_FUNCTION);
    std::map<std::uint32_t, nlohmann::json> groups;
    std::size_t unknown_versions{};
    for (std::size_t i = 0; i < count; ++i) {
        auto root = functions[i];
        std::set<std::uint32_t> seen;
        bool chain_resolved = true;
        for (;;) {
            if (root.BeginAddress >= root.EndAddress || root.EndAddress > image_size ||
                !seen.insert(root.UnwindData).second || seen.size() > 128)
                throw std::runtime_error("invalid or cyclic unwind chain");
            const auto* unwind = bytes(root.UnwindData, 4);
            const auto version = unwind[0] & 7;
            const auto flags = unwind[0] >> 3;
            if (version != 1 && version != 2) { ++unknown_versions; chain_resolved = false; break; }
            if (!(flags & 4)) break;
            if (flags & 3) throw std::runtime_error("invalid chained unwind flags");
            const auto tail = root.UnwindData + 4 + ((unwind[2] + 1u) & ~1u) * 2;
            std::memcpy(&root, bytes(tail, sizeof(root)), sizeof(root));
        }
        const auto rva = chain_resolved ? root.BeginAddress : functions[i].BeginAddress;
        auto& group = groups[rva];
        if (group.is_null()) group = {{"id","native:" + std::to_string(rva)}, {"rva",rva},
            {"callable",false}, {"abi",nullptr}, {"chain_resolved",chain_resolved}, {"address_evidence","unwind-entry"},
            {"reason","native ABI, context and effects are not resolved"},
            {"code_bytes",0}, {"ranges",nlohmann::json::array()}};
        group["ranges"].push_back({{"begin_rva",functions[i].BeginAddress},{"end_rva",functions[i].EndAddress}});
        group["code_bytes"] = group["code_bytes"].get<std::uint64_t>() + functions[i].EndAddress - functions[i].BeginAddress;
    }
    const auto unwind_count = groups.size();
    const auto* sections = IMAGE_FIRST_SECTION(nt);
    std::vector<std::pair<std::uint32_t,std::uint32_t>> executable;
    for (unsigned i = 0; i < nt->FileHeader.NumberOfSections; ++i) {
        const auto& section = sections[i];
        bytes(section.VirtualAddress, section.Misc.VirtualSize);
        if (section.Characteristics & IMAGE_SCN_MEM_EXECUTE)
            executable.emplace_back(section.VirtualAddress, section.VirtualAddress + section.Misc.VirtualSize);
    }
    auto is_code = [&](std::uintptr_t pointer) {
        if (pointer < address_base || pointer - address_base >= image_size) return false;
        const auto rva = pointer - address_base;
        for (const auto& [begin,end] : executable) if (begin <= rva && rva < end) return true;
        return false;
    };
    std::size_t pointer_slots{}, unreadable_pages{};
    std::set<std::uint32_t> pointer_targets;
    for (unsigned i = 0; i < nt->FileHeader.NumberOfSections; ++i) {
        const auto& section = sections[i];
        if (section.Characteristics & IMAGE_SCN_MEM_EXECUTE) continue;
        for (std::uint32_t offset = 0; offset < section.Misc.VirtualSize; offset += 4096) {
            std::uint8_t page[4096]{};
            const auto length = (std::min)(DWORD{4096}, section.Misc.VirtualSize - offset);
            const auto rva = section.VirtualAddress + offset;
            if (image_override) std::memcpy(page, bytes(rva,length),length);
            else {
                SIZE_T actual{};
                if (!ReadProcessMemory(GetCurrentProcess(), image + rva, page, length, &actual) || actual != length) {
                    ++unreadable_pages; continue;
                }
            }
            for (std::size_t slot = (8 - (rva % 8)) % 8; slot + 8 <= length; slot += 8) {
                std::uintptr_t pointer{};
                std::memcpy(&pointer,page + slot,8);
                if (!is_code(pointer)) continue;
                const auto target = static_cast<std::uint32_t>(pointer - address_base);
                ++pointer_slots; pointer_targets.insert(target);
                auto& group = groups[target];
                if (group.is_null()) group = {{"id","native:" + std::to_string(target)}, {"rva",target},
                    {"callable",false}, {"abi",nullptr}, {"chain_resolved",nullptr}, {"address_evidence","image-code-pointer"},
                    {"reason","code pointer target; function boundary, ABI and effects are unresolved"},
                    {"code_bytes",nullptr}, {"ranges",nlohmann::json::array()}};
                if (!group.contains("pointer_slots")) group["pointer_slots"] = nlohmann::json::array();
                group["pointer_slots"].push_back(rva + slot);
            }
        }
    }
    nlohmann::json entries = nlohmann::json::array();
    for (auto& [rva, group] : groups) entries.push_back(std::move(group));
    return nlohmann::json({{"schema_version",1}, {"image",{
        {"sha256",image_sha256},{"target",image_target},{"timestamp",image_timestamp},{"size",image_size}}},
        {"source","loaded-executable-unwind-and-code-pointers"}, {"count",entries.size()}, {"range_count",count},
        {"unwind_count",unwind_count}, {"pointer_target_count",pointer_targets.size()},
        {"pointer_slot_count",pointer_slots}, {"unreadable_pointer_pages",unreadable_pages},
        {"unknown_unwind_versions",unknown_versions}, {"complete_engine_api",false},
        {"limitations",{"unreferenced leaf and inlined functions can be absent", "code pointers can target internal labels; an entry point is not implied", "code extents do not establish function signatures"}},
        {"entries",std::move(entries)}}).dump();
}
}
