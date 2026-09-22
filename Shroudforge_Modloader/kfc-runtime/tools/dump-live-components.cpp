#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <tlhelp32.h>

#include <algorithm>
#include <array>
#include <charconv>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <string>
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
    return address && ReadProcessMemory(process, reinterpret_cast<const void*>(address),
        value, size, &received) && received == size;
}

std::uintptr_t image_base(DWORD pid, std::size_t& image_size) {
    const auto snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid);
    if (snapshot == INVALID_HANDLE_VALUE) return 0;
    MODULEENTRY32W module{sizeof(module)};
    const auto found = Module32FirstW(snapshot, &module);
    CloseHandle(snapshot);
    if (!found) return 0;
    image_size = module.modBaseSize;
    return reinterpret_cast<std::uintptr_t>(module.modBaseAddr);
}

std::uintptr_t find_metadata(const std::vector<std::uint8_t>& image,
                             std::uintptr_t base, const char* wanted) {
    const auto length = std::strlen(wanted);
    for (std::size_t offset = 0; offset + length + 1 <= image.size(); ++offset) {
        if (std::memcmp(image.data() + offset, wanted, length) || image[offset + length]) continue;
        const auto text = base + offset;
        for (std::size_t candidate = 0; candidate + 0x50 <= image.size(); candidate += 8) {
            std::uintptr_t pointer{};
            std::uint64_t text_length{};
            std::memcpy(&pointer, image.data() + candidate + 0x20, sizeof(pointer));
            std::memcpy(&text_length, image.data() + candidate + 0x28, sizeof(text_length));
            if (pointer == text && text_length == length) return base + candidate;
        }
    }
    return 0;
}

bool metadata(HANDLE process, std::uintptr_t address, std::string& name, std::uint32_t& size) {
    std::uintptr_t text{};
    std::uint64_t length{};
    if (!read(process, address + 0x20, text) || !read(process, address + 0x28, length) ||
        !length || length > 512 || !read(process, address + 0x40, size)) return false;
    name.resize(static_cast<std::size_t>(length));
    return read_bytes(process, text, name.data(), name.size());
}
}

int main(int argc, char** argv) {
    if (argc != 2) return 2;
    DWORD pid{};
    const auto end = argv[1] + std::strlen(argv[1]);
    if (std::from_chars(argv[1], end, pid).ec != std::errc{}) return 2;

    std::size_t image_size{};
    const auto base = image_base(pid, image_size);
    const auto process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, pid);
    if (!base || !process || !image_size) return 3;
    std::vector<std::uint8_t> image(image_size);
    if (!read_bytes(process, base, image.data(), image.size())) return 4;

    const auto first = find_metadata(image, base, "keen::ecs::DynamicActiveNpcState");
    const auto current = find_metadata(image, base, "keen::ecs::CurrentTransform");
    if (!first || !current) return 5;

    std::vector<std::uint8_t> bytes(16 * 1024 * 1024);
    std::uintptr_t cursor{};
    MEMORY_BASIC_INFORMATION memory{};
    while (VirtualQueryEx(process, reinterpret_cast<const void*>(cursor), &memory, sizeof(memory)) == sizeof(memory)) {
        const auto region = reinterpret_cast<std::uintptr_t>(memory.BaseAddress);
        const auto region_end = region + memory.RegionSize;
        const bool writable = memory.State == MEM_COMMIT && !(memory.Protect & (PAGE_GUARD | PAGE_NOACCESS)) &&
            (memory.Protect & (PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY));
        if (writable && memory.RegionSize >= 1024 * sizeof(std::uintptr_t)) {
            for (std::size_t offset = 0; offset < memory.RegionSize; offset += bytes.size()) {
                const auto wanted = (std::min)(bytes.size(), memory.RegionSize - offset);
                SIZE_T received{};
                if (!ReadProcessMemory(process, reinterpret_cast<const void*>(region + offset),
                    bytes.data(), wanted, &received)) continue;
                for (std::size_t local = 0; local + sizeof(std::uintptr_t) <= received; local += 8) {
                    std::uintptr_t value{};
                    std::memcpy(&value, bytes.data() + local, sizeof(value));
                    if (value != first) continue;
                    const auto table = region + offset + local;
                    bool contains_current{};
                    for (std::size_t index = 0; index < 1024; ++index) {
                        std::uintptr_t entry{};
                        if (!read(process, table + index * 8, entry)) break;
                        contains_current |= entry == current;
                    }
                    if (!contains_current) continue;

                    std::vector<std::string> rows;
                    for (std::size_t index = 0; index < 1024; ++index) {
                        std::uintptr_t entry{};
                        std::string name;
                        std::uint32_t size{};
                        if (!read(process, table + index * 8, entry) || !entry ||
                            !metadata(process, entry, name, size) || !name.starts_with("keen::ecs::")) continue;
                        rows.push_back(std::to_string(index) + "\t" + std::to_string(size) + "\t" + name);
                    }
                    if (rows.size() <= 400) continue;
                    std::cout << "table=0x" << std::hex << table
                              << " region=0x" << region
                              << " region_size=0x" << memory.RegionSize
                              << " type=0x" << memory.Type
                              << " protect=0x" << memory.Protect << std::dec << '\n';
                    for (std::size_t index = 0; index < 6; ++index) {
                        std::uintptr_t entry{};
                        std::array<std::uint8_t, 0x80> raw{};
                        if (!read(process, table + index * 8, entry) || !entry ||
                            !read_bytes(process, entry, raw.data(), raw.size())) continue;
                        std::cout << "metadata[" << index << "]=0x" << std::hex << entry
                                  << " rva=0x" << entry - base << " bytes=";
                        for (const auto byte : raw) {
                            const char digits[] = "0123456789abcdef";
                            std::cout << digits[byte >> 4] << digits[byte & 15];
                        }
                        std::cout << std::dec << '\n';
                    }
                    constexpr std::size_t inspected = 256;
                    std::array<std::array<std::uint8_t, 0x80>, inspected> raw_metadata{};
                    std::size_t loaded{};
                    for (; loaded < inspected; ++loaded) {
                        std::uintptr_t entry{};
                        if (!read(process, table + loaded * 8, entry) || !entry ||
                            !read_bytes(process, entry, raw_metadata[loaded].data(), 0x80)) break;
                    }
                    std::cout << "metadata_index_samples=" << loaded << '\n';
                    for (std::size_t field = 0; field + 2 <= 0x80; field += 2) {
                        std::size_t matches{};
                        for (std::size_t index = 0; index < loaded; ++index) {
                            std::uint16_t value{};
                            std::memcpy(&value, raw_metadata[index].data() + field, sizeof(value));
                            matches += value == index;
                        }
                        if (loaded && matches >= loaded * 3 / 4)
                            std::cout << "metadata_index_u16=0x" << std::hex << field << std::dec
                                      << " matches=" << matches << '/' << loaded << '\n';
                    }
                    std::cout << "component_id\tsize\tqualified_name\n";
                    for (const auto& row : rows) std::cout << row << '\n';
                    CloseHandle(process);
                    return 0;
                }
            }
        }
        if (region_end <= cursor) break;
        cursor = region_end;
    }
    CloseHandle(process);
    return 6;
}
