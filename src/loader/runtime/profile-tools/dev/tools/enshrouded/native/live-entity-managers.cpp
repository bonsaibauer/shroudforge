#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <algorithm>
#include <array>
#include <charconv>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <vector>

namespace {
template<class T>
bool read(HANDLE process, std::uintptr_t address, T& value) {
    SIZE_T received{};
    return address && ReadProcessMemory(process, reinterpret_cast<const void*>(address),
        &value, sizeof(value), &received) && received == sizeof(value);
}

bool read_bytes(HANDLE process, std::uintptr_t address, void* value, std::size_t size) {
    SIZE_T received{};
    return address && value && size && ReadProcessMemory(process,
        reinterpret_cast<const void*>(address), value, size, &received) && received == size;
}

struct EntityHeader {
    std::uint8_t padding[0x10];
    std::uint32_t id;
    std::uint32_t unused;
    std::uintptr_t layout;
    std::uintptr_t storage;
    std::uintptr_t definition;
    std::uint64_t row;
    std::uint64_t component_count;
};

bool validate_manager(HANDLE process, std::uintptr_t candidate,
                      std::uint64_t count, std::uintptr_t table) {
    if (count < 16 || count > (1u << 20) || table < 0x10000) return false;
    const auto sample_count = static_cast<std::size_t>((std::min)(count, std::uint64_t{4096}));
    std::vector<std::uintptr_t> pointers(sample_count);
    if (!read_bytes(process, table, pointers.data(), pointers.size() * sizeof(pointers[0]))) return false;
    std::size_t occupied{}, valid{};
    std::vector<std::uint32_t> ids;
    for (const auto pointer : pointers) {
        if (!pointer) continue;
        ++occupied;
        EntityHeader header{};
        if (!read(process, pointer, header) || !header.id || !header.layout ||
            !header.storage || !header.definition || !header.component_count ||
            header.component_count > 1024 || header.row > (1u << 24)) continue;
        std::uintptr_t definition_name{};
        std::uint64_t definition_name_size{};
        if (!read(process, header.definition + 0x10, definition_name) ||
            !read(process, header.definition + 0x18, definition_name_size) ||
            !definition_name || !definition_name_size || definition_name_size > 512) continue;
        if (std::find(ids.begin(), ids.end(), header.id) != ids.end()) continue;
        ids.push_back(header.id);
        ++valid;
        if (valid >= 8) break;
    }
    if (valid < 2) return false;
    std::cout << "manager=0x" << std::hex << candidate << " table=0x" << table
              << std::dec << " slots=" << count << " sampled=" << occupied
              << " structurally_valid_entities=" << valid << '\n';
    return true;
}
}

int main(int argc, char** argv) {
    if (argc != 2) return 2;
    DWORD pid{};
    const auto end = argv[1] + std::strlen(argv[1]);
    if (std::from_chars(argv[1], end, pid).ec != std::errc{}) return 2;
    const auto process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, pid);
    if (!process) return 3;

    std::vector<std::uint8_t> bytes(16 * 1024 * 1024 + 0x190);
    struct Candidate { std::uintptr_t address; std::uint64_t count; std::uintptr_t table; };
    std::vector<Candidate> candidates;
    std::uintptr_t cursor{};
    MEMORY_BASIC_INFORMATION memory{};
    while (VirtualQueryEx(process, reinterpret_cast<const void*>(cursor), &memory, sizeof(memory)) == sizeof(memory)) {
        const auto region = reinterpret_cast<std::uintptr_t>(memory.BaseAddress);
        const auto end_address = region + memory.RegionSize;
        const bool readable_private = memory.State == MEM_COMMIT && memory.Type == MEM_PRIVATE &&
            !(memory.Protect & (PAGE_GUARD | PAGE_NOACCESS)) &&
            (memory.Protect & (PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READWRITE |
                               PAGE_EXECUTE_WRITECOPY));
        if (readable_private && memory.RegionSize >= 0x190) {
            constexpr std::size_t chunk = 16 * 1024 * 1024;
            for (std::size_t offset = 0; offset < memory.RegionSize; offset += chunk) {
                const auto wanted = (std::min)(bytes.size(), memory.RegionSize - offset);
                SIZE_T received{};
                if (!ReadProcessMemory(process, reinterpret_cast<const void*>(region + offset),
                                       bytes.data(), wanted, &received) || received < 0x190) continue;
                for (std::size_t local = 0; local + 0x190 <= received; local += 8) {
                    std::uint64_t count{};
                    std::uintptr_t table{};
                    std::memcpy(&count, bytes.data() + local + 0x158, sizeof(count));
                    if (count < 16 || count > 100'000) continue;
                    std::memcpy(&table, bytes.data() + local + 0x188, sizeof(table));
                    if (table < 0x1'0000'0000ULL || table >= 0x0000'8000'0000'0000ULL || (table & 7)) continue;
                    candidates.push_back({region + offset + local, count, table});
                }
            }
        }
        if (end_address <= cursor) break;
        cursor = end_address;
    }
    std::size_t found{};
    for (const auto& candidate : candidates) {
        if (validate_manager(process, candidate.address, candidate.count, candidate.table)) ++found;
    }
    std::cerr << "candidates=" << candidates.size() << " verified=" << found << '\n';
    CloseHandle(process);
    return found ? 0 : 4;
}
