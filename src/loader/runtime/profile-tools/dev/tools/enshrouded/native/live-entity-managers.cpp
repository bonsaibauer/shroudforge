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
    std::uint32_t row;
};

bool validate_manager(HANDLE process, std::uintptr_t candidate,
                      std::uint64_t count, std::uintptr_t table) {
    if (count < 16 || count > (1u << 20) || table < 0x10000) return false;
    // A table containing entity pointers alone also matches unrelated vectors.
    // Require the manager's component registry and its parallel storage arrays.
    std::uintptr_t owner{}, records{}, sizes{}, types{}, callbacks{};
    std::uint64_t registrations{}, capacity{}, size_count{}, type_count{}, callback_count{};
    if (!read(process, candidate, owner) || owner < 0x10000 ||
        !read(process, owner + 8, records) || !records ||
        !read(process, owner + 16, registrations) || registrations < 16 || registrations > 1024 ||
        !read(process, owner + 24, capacity) || capacity < registrations || capacity > 4096 ||
        !read(process, owner + 232, sizes) || !sizes ||
        !read(process, owner + 240, size_count) || size_count != registrations ||
        !read(process, owner + 256, types) || !types ||
        !read(process, owner + 264, type_count) || type_count != registrations ||
        !read(process, owner + 280, callbacks) || !callbacks ||
        !read(process, owner + 288, callback_count) || callback_count != registrations) return false;
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
            !header.storage || header.row > (1u << 24)) continue;
        std::array<std::uint64_t, 16> bits{};
        std::array<std::uint16_t, 1024> strides{}, registered_sizes{};
        const auto words = static_cast<std::size_t>((registrations + 63) / 64);
        if (!read_bytes(process, header.layout, bits.data(), words * sizeof(bits[0])) ||
            !read_bytes(process, header.layout + 2692, strides.data(), registrations * sizeof(strides[0])) ||
            !read_bytes(process, sizes, registered_sizes.data(), registrations * sizeof(strides[0]))) continue;
        bool has_component = false, consistent = true;
        for (std::size_t index = 0; index < registrations; ++index) {
            if (!(bits[index / 64] & (std::uint64_t{1} << (index % 64)))) continue;
            has_component = true;
            if (!strides[index] || strides[index] != registered_sizes[index]) { consistent = false; break; }
        }
        if (!has_component || !consistent) continue;
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
                    if (count < 16 || count > (1u << 20)) continue;
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
