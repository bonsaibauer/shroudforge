#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <tlhelp32.h>

#include <algorithm>
#include <array>
#include <charconv>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <vector>

namespace {
struct Target { const char* name; std::uintptr_t rva; };
constexpr std::array targets{
    Target{"keen::ecs::CurrentTransform", 0x1689080},
    Target{"keen::ecs::Flying", 0x17b9120},
    Target{"keen::ecs::Inventory", 0x17e9b40},
    Target{"keen::ecs::StaminaDepletion", 0x17a58e0},
    Target{"keen::ecs::FallDamage", 0x17bf090},
};

std::uintptr_t image_base(DWORD pid) {
    const auto snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid);
    if (snapshot == INVALID_HANDLE_VALUE) return 0;
    MODULEENTRY32W module{sizeof(module)};
    const auto found = Module32FirstW(snapshot, &module);
    CloseHandle(snapshot);
    return found ? reinterpret_cast<std::uintptr_t>(module.modBaseAddr) : 0;
}
}

int main(int argc, char** argv) {
    if (argc != 2) return 2;
    DWORD pid{};
    const auto end = argv[1] + std::strlen(argv[1]);
    if (std::from_chars(argv[1], end, pid).ec != std::errc{}) return 2;
    const auto base = image_base(pid);
    const auto process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, pid);
    if (!base || !process) return 3;

    std::array<std::size_t, targets.size()> totals{};
    std::vector<std::uint8_t> bytes(16 * 1024 * 1024);
    std::uintptr_t cursor{};
    MEMORY_BASIC_INFORMATION memory{};
    while (VirtualQueryEx(process, reinterpret_cast<const void*>(cursor), &memory, sizeof(memory)) == sizeof(memory)) {
        const auto region = reinterpret_cast<std::uintptr_t>(memory.BaseAddress);
        const auto end_address = region + memory.RegionSize;
        const bool readable = memory.State == MEM_COMMIT &&
            !(memory.Protect & (PAGE_GUARD | PAGE_NOACCESS)) &&
            (memory.Protect & (PAGE_READONLY | PAGE_READWRITE | PAGE_WRITECOPY |
                               PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY));
        if (readable) {
            for (std::size_t offset = 0; offset < memory.RegionSize; offset += bytes.size()) {
                const auto wanted = (std::min)(bytes.size(), memory.RegionSize - offset);
                SIZE_T received{};
                if (!ReadProcessMemory(process, reinterpret_cast<const void*>(region + offset),
                                       bytes.data(), wanted, &received)) continue;
                for (std::size_t local = 0; local + sizeof(std::uintptr_t) <= received;
                     local += alignof(std::uintptr_t)) {
                    std::uintptr_t value{};
                    std::memcpy(&value, bytes.data() + local, sizeof(value));
                    for (std::size_t index = 0; index < targets.size(); ++index) {
                        if (value != base + targets[index].rva) continue;
                        ++totals[index];
                        std::cout << targets[index].name << " 0x" << std::hex
                                  << region + offset + local << " region=0x" << region
                                  << " size=0x" << memory.RegionSize << " protect=0x"
                                  << memory.Protect << std::dec << '\n';
                    }
                }
            }
        }
        if (end_address <= cursor) break;
        cursor = end_address;
    }
    for (std::size_t index = 0; index < targets.size(); ++index) {
        std::cout << targets[index].name << " total=" << totals[index] << '\n';
    }
    CloseHandle(process);
    return 0;
}
