#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <tlhelp32.h>
#include "../../../../../native/ecs/component_registry.h"
#include "../../../../../native/provider/function_inventory.h"
#include <nlohmann/json.hpp>
#include <fstream>
#include <iostream>

int main(int argc, char** argv) {
    if (argc != 4) { std::cerr << "usage: verify-component-registry PID manager-hex output.json\n"; return 2; }
    const auto pid = static_cast<DWORD>(std::stoul(argv[1]));
    const auto manager = static_cast<std::uintptr_t>(std::stoull(argv[2], nullptr, 16));
    const auto process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, pid);
    if (!process) return 3;
    const auto modules = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid);
    MODULEENTRY32W module{sizeof(module)};
    if (modules == INVALID_HANDLE_VALUE || !Module32FirstW(modules, &module)) { CloseHandle(process); return 4; }
    CloseHandle(modules);
    ComponentRegistry::Reader read = [&](std::uintptr_t address, void* destination, std::size_t size) {
        SIZE_T received{};
        return ReadProcessMemory(process, reinterpret_cast<void*>(address), destination, size, &received) && received == size;
    };
    ComponentRegistry::Snapshot snapshot;
    std::string error;
    if (!ComponentRegistry::Read(manager, reinterpret_cast<std::uintptr_t>(module.modBaseAddr),
        module.modBaseSize, read, snapshot, error)) {
        std::cerr << error << '\n'; CloseHandle(process); return 5;
    }
    nlohmann::json rows = nlohmann::json::array();
    std::size_t runtime{}, template_only{}, callback_count{};
    for (const auto& entry : snapshot.entries) {
        runtime += !entry.storage.name.empty();
        template_only += entry.storage.name.empty();
        callback_count += entry.callbacks.size();
        nlohmann::json callbacks = nlohmann::json::array();
        for (const auto& callback : entry.callbacks) {
            std::uint8_t code[512]{};
            std::string code_hex;
            if (read(reinterpret_cast<std::uintptr_t>(module.modBaseAddr) + callback.function_rva, code, sizeof(code))) {
                static constexpr char digits[] = "0123456789abcdef";
                for (const auto byte : code) { code_hex += digits[byte >> 4]; code_hex += digits[byte & 15]; }
            }
            callbacks.push_back({{"slot_offset",callback.slot_offset},{"function_rva",callback.function_rva},
                {"origin",callback.in_record ? "registration-record" : "parallel-callback-table"}, {"code_hex",code_hex}});
        }
        rows.push_back({{"index",entry.index},{"name",entry.name},{"hash",entry.hash},
            {"qualified_name",entry.name},{"qualified_hash",entry.hash},{"callbacks",callbacks},
            {"runtimeType",entry.storage.name},{"templateType",entry.configuration.name},
            {"size",entry.size}});
    }
    std::ofstream output(argv[3]);
    output << nlohmann::json({{"available",true},{"pid",pid},{"runtime",runtime},{"templateOnly",template_only},
        {"callbackPointers",callback_count},{"entries",rows}}).dump(2) << '\n';
    std::cout << "Validated with the production registry reader: " << rows.size() << " registrations, "
        << runtime << " storage layouts, " << template_only << " template-only, " << callback_count << " callback pointers\n";
    // Fault injection affects local copies only. The game is opened VM_READ.
    for (unsigned mode = 0; mode < 3; ++mode) {
        unsigned record_reads{};
        const auto corrupt = [&](std::uintptr_t address, void* destination, std::size_t size) {
            if (!read(address, destination, size)) return false;
            if (mode == 0 && address == snapshot.owner + 16 && size == 8) {
                const std::uint64_t oversized = 1025;
                std::memcpy(destination, &oversized, 8);
            }
            if (address == snapshot.records && size == snapshot.entries.size() * 256) {
                ++record_reads;
                if (mode == 1 || (mode == 2 && record_reads == 2))
                    static_cast<std::uint8_t*>(destination)[32] ^= 1;
            }
            return true;
        };
        ComponentRegistry::Snapshot rejected;
        std::string rejection;
        if (ComponentRegistry::Read(manager, reinterpret_cast<std::uintptr_t>(module.modBaseAddr),
                module.modBaseSize, corrupt, rejected, rejection)) {
            std::cerr << "Fault injection accepted: " << mode << '\n'; CloseHandle(process); return 9;
        }
    }
    std::cout << "Production reader rejected oversized counts, mismatched hashes, and changing readbacks\n";
    // Exercise the actual provider's unwind reader on a read-only image snapshot.
    std::vector<std::uint8_t> image(module.modBaseSize);
    const auto base = reinterpret_cast<std::uintptr_t>(module.modBaseAddr);
    if (!read(base, image.data(), 4096)) { CloseHandle(process); return 7; }
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(image.data());
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(image.data() + dos->e_lfanew);
    const auto* sections = IMAGE_FIRST_SECTION(nt);
    for (std::size_t i = 0; i < nt->FileHeader.NumberOfSections; ++i) {
        const auto& section = sections[i];
        if (section.VirtualAddress > image.size() || section.Misc.VirtualSize > image.size() - section.VirtualAddress) return 8;
        // Only unwind/rdata are required; pages not readable in the running process remain zero.
        for (std::size_t offset = 0; offset < section.Misc.VirtualSize; offset += 4096) {
            const auto size = (std::min)(std::size_t{4096}, section.Misc.VirtualSize - offset);
            read(base + section.VirtualAddress + offset, image.data() + section.VirtualAddress + offset, size);
        }
    }
    KfcRuntimeCompatibility::EnshroudedClient::image_size = module.modBaseSize;
    KfcRuntimeCompatibility::EnshroudedClient::image_timestamp = nt->FileHeader.TimeDateStamp;
    const auto functions = nlohmann::json::parse(FunctionInventory::Build(image.data(), base));
    std::ofstream function_output(std::string(argv[3]) + ".functions.json");
    function_output << functions.dump() << '\n';
    std::cout << "Production function reader: " << functions.at("count") << " candidates, " << functions.at("unwind_count")
        << " unwind groups, " << functions.at("pointer_target_count") << " pointer targets, " << functions.at("range_count") << " ranges\n";
    CloseHandle(process);
    return output ? 0 : 6;
}
