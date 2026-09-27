#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <array>
#include <charconv>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <string>
#include <vector>

namespace {
template<class T> bool read(HANDLE process, std::uintptr_t address, T& value) {
    SIZE_T received{};
    return address && ReadProcessMemory(process, reinterpret_cast<const void*>(address),
        &value, sizeof(value), &received) && received == sizeof(value);
}

bool read_bytes(HANDLE process, std::uintptr_t address, void* value, std::size_t size) {
    SIZE_T received{};
    return address && ReadProcessMemory(process, reinterpret_cast<const void*>(address),
        value, size, &received) && received == size;
}
}

int main(int argc, char** argv) {
    if (argc != 3) return 2;
    DWORD pid{};
    std::uintptr_t table{};
    const auto pid_end = argv[1] + std::strlen(argv[1]);
    const auto table_end = argv[2] + std::strlen(argv[2]);
    if (std::from_chars(argv[1], pid_end, pid).ec != std::errc{} ||
        std::from_chars(argv[2], table_end, table, 16).ec != std::errc{}) return 2;
    const auto process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, pid);
    if (!process) return 3;

    constexpr std::size_t sample_count = 1024;
    constexpr std::size_t inspect_size = 0x80;
    struct Sample {
        std::size_t registry_index{};
        std::array<std::uint8_t, inspect_size> metadata{};
    };
    std::vector<Sample> samples;
    for (std::size_t index = 0; index < 1024 && samples.size() < sample_count; ++index) {
        std::uintptr_t metadata{};
        if (!read(process, table + index * sizeof(metadata), metadata) || !metadata) continue;
        Sample sample{};
        sample.registry_index = index;
        if (!read_bytes(process, metadata, sample.metadata.data(), sample.metadata.size())) continue;
        samples.push_back(sample);
    }
    std::cout << "loaded_non_null_entries=" << samples.size() << '\n';
    if (samples.empty()) {
        CloseHandle(process);
        return 4;
    }
    for (std::size_t offset = 0; offset + 2 <= inspect_size; offset += 2) {
        std::size_t matches{};
        for (const auto& sample : samples) {
            std::uint16_t value{};
            std::memcpy(&value, sample.metadata.data() + offset, sizeof(value));
            matches += value == sample.registry_index;
        }
        if (matches >= samples.size() * 3 / 4)
            std::cout << "u16 offset=0x" << std::hex << offset << std::dec
                      << " matches=" << matches << '/' << samples.size() << '\n';
    }
    for (std::size_t offset = 0; offset + 4 <= inspect_size; offset += 4) {
        std::size_t matches{};
        for (const auto& sample : samples) {
            std::uint32_t value{};
            std::memcpy(&value, sample.metadata.data() + offset, sizeof(value));
            matches += value == sample.registry_index;
        }
        if (matches >= samples.size() * 3 / 4)
            std::cout << "u32 offset=0x" << std::hex << offset << std::dec
                      << " matches=" << matches << '/' << samples.size() << '\n';
    }
    CloseHandle(process);
}
