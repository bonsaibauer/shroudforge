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

    constexpr std::size_t sample_count = 256;
    constexpr std::size_t inspect_size = 0x80;
    std::vector<std::array<std::uint8_t, inspect_size>> samples(sample_count);
    std::size_t loaded{};
    for (std::size_t index = 0; index < sample_count; ++index) {
        std::uintptr_t metadata{};
        if (!read(process, table + index * sizeof(metadata), metadata) || !metadata) {
            std::cout << "table read failed at index=" << index
                      << " error=" << GetLastError() << '\n';
            break;
        }
        if (!read_bytes(process, metadata, samples[index].data(), inspect_size)) {
            std::cout << "metadata read failed at index=" << index
                      << " address=0x" << std::hex << metadata << std::dec
                      << " error=" << GetLastError() << '\n';
            break;
        }
        ++loaded;
    }
    std::cout << "loaded=" << loaded << '\n';
    if (!loaded) {
        CloseHandle(process);
        return 4;
    }
    for (std::size_t offset = 0; offset + 2 <= inspect_size; offset += 2) {
        std::size_t matches{};
        for (std::size_t index = 0; index < loaded; ++index) {
            std::uint16_t value{};
            std::memcpy(&value, samples[index].data() + offset, sizeof(value));
            matches += value == index;
        }
        if (matches >= loaded * 3 / 4)
            std::cout << "u16 offset=0x" << std::hex << offset << std::dec
                      << " matches=" << matches << '/' << loaded << '\n';
    }
    for (std::size_t offset = 0; offset + 4 <= inspect_size; offset += 4) {
        std::size_t matches{};
        for (std::size_t index = 0; index < loaded; ++index) {
            std::uint32_t value{};
            std::memcpy(&value, samples[index].data() + offset, sizeof(value));
            matches += value == index;
        }
        if (matches >= loaded * 3 / 4)
            std::cout << "u32 offset=0x" << std::hex << offset << std::dec
                      << " matches=" << matches << '/' << loaded << '\n';
    }
    CloseHandle(process);
}
