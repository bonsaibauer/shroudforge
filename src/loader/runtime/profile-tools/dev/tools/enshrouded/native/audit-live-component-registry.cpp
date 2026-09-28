#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <tlhelp32.h>

#include <algorithm>
#include <cctype>
#include <charconv>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iostream>
#include <string>
#include <string_view>
#include <sstream>
#include <unordered_map>
#include <unordered_set>
#include <vector>

#include <nlohmann/json.hpp>

namespace {
struct TypeRecord {
    std::string name;
    std::uint32_t size{};
};
struct Hit {
    std::uintptr_t address{};
    std::uintptr_t metadata{};
    std::string name;
    std::uint32_t size{};
};

std::vector<int> parse_signature(std::string_view text) {
    std::vector<int> result;
    std::istringstream input{std::string(text)};
    std::string token;
    while (input >> token) {
        if (token == "?" || token == "??") { result.push_back(-1); continue; }
        unsigned value{};
        const auto parsed = std::from_chars(token.data(), token.data() + token.size(), value, 16);
        if (parsed.ec != std::errc{} || parsed.ptr != token.data() + token.size() || value > 255) return {};
        result.push_back(static_cast<int>(value));
    }
    return result;
}

bool find_function_range(const std::vector<std::uint8_t>& image, std::uint32_t rva,
                         std::uint32_t& begin, std::uint32_t& end) {
    if (image.size() < sizeof(IMAGE_DOS_HEADER)) return false;
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(image.data());
    if (dos->e_magic != IMAGE_DOS_SIGNATURE || dos->e_lfanew <= 0 ||
        static_cast<std::size_t>(dos->e_lfanew) > image.size() - sizeof(IMAGE_NT_HEADERS64)) return false;
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(image.data() + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE || nt->OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR64_MAGIC) return false;
    const auto directory = nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION];
    if (!directory.VirtualAddress || directory.Size < 12 ||
        directory.VirtualAddress > image.size() || directory.Size > image.size() - directory.VirtualAddress) return false;
    struct FunctionEntry { std::uint32_t begin, end, unwind; };
    const auto* entries = reinterpret_cast<const FunctionEntry*>(image.data() + directory.VirtualAddress);
    const auto count = directory.Size / sizeof(FunctionEntry);
    for (std::size_t index = 0; index < count; ++index) {
        if (entries[index].begin <= rva && rva < entries[index].end) {
            begin = entries[index].begin;
            end = entries[index].end;
            return true;
        }
    }
    return false;
}

void audit_code_operations(const std::vector<std::uint8_t>& image, const nlohmann::json& profile) {
    if (image.size() < sizeof(IMAGE_DOS_HEADER)) return;
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(image.data());
    if (dos->e_magic != IMAGE_DOS_SIGNATURE || dos->e_lfanew <= 0 ||
        static_cast<std::size_t>(dos->e_lfanew) > image.size() - sizeof(IMAGE_NT_HEADERS64)) return;
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(image.data() + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE || nt->OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR64_MAGIC) return;
    const auto expected_image = profile.value("image", nlohmann::json::object());
    const auto expected_timestamp = expected_image.value("timestamp", std::uint32_t{});
    const auto expected_size = expected_image.value("size", std::uint32_t{});
    const bool exact_image = expected_timestamp == nt->FileHeader.TimeDateStamp &&
        expected_size == nt->OptionalHeader.SizeOfImage;
    std::cout << "code_operation_profile\tid=" << profile.value("id", std::string{})
              << "\texact_image=" << (exact_image ? 1 : 0)
              << "\timage_timestamp=" << nt->FileHeader.TimeDateStamp
              << "\timage_size=" << nt->OptionalHeader.SizeOfImage << '\n';
    const auto sections = IMAGE_FIRST_SECTION(nt);

    const auto world_operations = profile.value("worldOperations", nlohmann::json::object());
    std::size_t world_count{};
    for (auto item = world_operations.begin(); item != world_operations.end(); ++item) {
        const auto& operation = item.value();
        if (!operation.contains("functionRva")) continue;
        ++world_count;
        const auto function_rva = operation.at("functionRva").get<std::uint32_t>();
        const auto guard_rva = operation.value("guardRva", function_rva);
        const auto bytes = operation.value("guardBytes", std::vector<std::uint8_t>{});
        bool guard_matches = guard_rva <= image.size() && bytes.size() <= image.size() - guard_rva;
        for (std::size_t index = 0; guard_matches && index < bytes.size(); ++index)
            guard_matches = image[guard_rva + index] == bytes[index];
        std::uint32_t begin{}, end{};
        const bool has_function = find_function_range(image, function_rva, begin, end);
        std::cout << "code_operation\tkind=world\tname=" << item.key()
                  << "\tfunction_rva=0x" << std::hex << function_rva
                  << "\tguard_rva=0x" << guard_rva << std::dec
                  << "\tguard_matches=" << (guard_matches ? 1 : 0);
        if (has_function) std::cout << "\tcontaining_function=0x" << std::hex << begin << "-0x" << end << std::dec;
        std::cout << '\n';
    }

    std::size_t patch_count{};
    const auto patches = profile.value("runtimePatches", nlohmann::json::object());
    for (auto item = patches.begin(); item != patches.end(); ++item) {
        const auto signature = parse_signature(item.value().value("signature", std::string{}));
        std::size_t matches{};
        std::uint32_t target{};
        if (!signature.empty()) {
            for (WORD section_index = 0; section_index < nt->FileHeader.NumberOfSections; ++section_index) {
                const auto& section = sections[section_index];
                if (!(section.Characteristics & IMAGE_SCN_MEM_EXECUTE) ||
                    section.VirtualAddress > image.size()) continue;
                const auto length = (std::min)(static_cast<std::size_t>(section.Misc.VirtualSize),
                    image.size() - section.VirtualAddress);
                if (length < signature.size()) continue;
                const auto* begin = image.data() + section.VirtualAddress;
                for (std::size_t offset = 0; offset <= length - signature.size(); ++offset) {
                    bool equal = true;
                    for (std::size_t byte = 0; byte < signature.size(); ++byte)
                        if (signature[byte] >= 0 && begin[offset + byte] != signature[byte]) { equal = false; break; }
                    if (!equal) continue;
                    if (!matches) target = section.VirtualAddress + static_cast<std::uint32_t>(offset);
                    ++matches;
                }
            }
        }
        std::uint32_t begin{}, end{};
        const bool has_function = matches == 1 && find_function_range(image, target, begin, end);
        std::cout << "code_operation\tkind=patch\tname=" << item.key()
                  << "\tsignature_matches=" << matches;
        if (matches == 1) std::cout << "\ttarget_rva=0x" << std::hex << target << std::dec;
        if (has_function) std::cout << "\tcontaining_function=0x" << std::hex << begin << "-0x" << end << std::dec;
        std::cout << '\n';
        ++patch_count;
    }
    std::cout << "code_operation_totals\tworld=" << world_count
              << "\tpatches=" << patch_count << '\n';
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

bool read_bytes(HANDLE process, std::uintptr_t address, void* value, std::size_t size) {
    SIZE_T received{};
    return address && value && size && ReadProcessMemory(process,
        reinterpret_cast<const void*>(address), value, size, &received) && received == size;
}

bool readable_protection(DWORD protect) {
    if (protect & (PAGE_GUARD | PAGE_NOACCESS)) return false;
    switch (protect & 0xff) {
    case PAGE_READONLY: case PAGE_READWRITE: case PAGE_WRITECOPY:
    case PAGE_EXECUTE_READ: case PAGE_EXECUTE_READWRITE: case PAGE_EXECUTE_WRITECOPY:
        return true;
    default: return false;
    }
}

template<class T> bool read(HANDLE process, std::uintptr_t address, T& value) {
    return read_bytes(process, address, &value, sizeof(value));
}

int inspect_range(int argc, char** argv) {
    if (argc != 6) return 2;
    DWORD pid{};
    std::uintptr_t table{};
    std::size_t count{};
    const auto pid_end = argv[2] + std::strlen(argv[2]);
    const auto table_end = argv[3] + std::strlen(argv[3]);
    const auto count_end = argv[4] + std::strlen(argv[4]);
    if (std::from_chars(argv[2], pid_end, pid).ec != std::errc{} ||
        std::from_chars(argv[3], table_end, table, 16).ec != std::errc{} ||
        std::from_chars(argv[4], count_end, count).ec != std::errc{} || !pid || !table || !count || count > 4096)
        return 2;
    nlohmann::json profile;
    try { std::ifstream file(argv[5]); if (!file) return 3; file >> profile; }
    catch (...) { return 3; }
    std::unordered_map<std::string, std::size_t> known;
    for (const auto& entry : profile.at("components"))
        known.emplace(entry.at("name").get<std::string>(), entry.at("index").get<std::size_t>());
    const auto process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, pid);
    if (!process) return 4;
    std::size_t loaded{}, direct_matches{};
    for (std::size_t index = 0; index < count; ++index) {
        std::uintptr_t descriptor{}, text{};
        std::uint64_t length{};
        std::uint32_t size{};
        if (!read(process, table + index * sizeof(descriptor), descriptor) || !descriptor ||
            !read(process, descriptor + 0x20, text) || !read(process, descriptor + 0x28, length) ||
            !read(process, descriptor + 0x40, size) || !text || !length || length > 512) continue;
        std::string name(static_cast<std::size_t>(length), '\0');
        if (!read_bytes(process, text, name.data(), name.size())) continue;
        ++loaded;
        const auto found = known.find(name);
        if (found != known.end()) direct_matches += found->second == index;
        if (index < 12 || (found != known.end() && found->second != index)) {
            std::cout << "slot=" << index << " size=" << size << " name=" << name
                      << " profileIndex=";
            if (found == known.end()) std::cout << "absent";
            else std::cout << found->second;
            std::cout << '\n';
        }
    }
    std::cout << "loaded=" << loaded << " direct_profile_index_matches=" << direct_matches
              << " profile_entries=" << known.size() << '\n';
    CloseHandle(process);
    return 0;
}
}

int main(int argc, char** argv) {
    if (argc >= 2 && std::string_view(argv[1]) == "--inspect-range") return inspect_range(argc, argv);
    if (argc != 4) {
        std::cerr << "usage: audit-live-component-registry <pid> <type-catalog.json> <profile.json>\n";
        return 2;
    }
    DWORD pid{};
    const auto pid_end = argv[1] + std::strlen(argv[1]);
    if (std::from_chars(argv[1], pid_end, pid).ec != std::errc{} || !pid) return 2;

    nlohmann::json catalog, profile;
    try {
        std::ifstream catalog_file(argv[2]), profile_file(argv[3]);
        if (!catalog_file || !profile_file) return 3;
        catalog_file >> catalog;
        profile_file >> profile;
    } catch (const std::exception& error) {
        std::cerr << "input JSON: " << error.what() << '\n';
        return 3;
    }

    std::unordered_map<std::string, TypeRecord> targets;
    std::unordered_set<std::string> component_targets;
    for (const auto& entry : catalog.at("entries")) {
        const auto name = entry.at("qualified_name").get<std::string>();
        if (!name.starts_with("keen::ecs::")) continue;
        targets.insert_or_assign(name, TypeRecord{name, entry.at("size").get<std::uint32_t>()});
        if (entry.at("kind") == "component") component_targets.insert(name);
    }
    std::unordered_map<std::string, std::size_t> known_indices;
    for (const auto& entry : profile.at("components"))
        known_indices.emplace(entry.at("name").get<std::string>(),
            entry.at("index").get<std::size_t>());

    std::size_t image_size{};
    const auto base = image_base(pid, image_size);
    const auto process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, pid);
    if (!base || !process || image_size < 0x1000) return 4;
    std::vector<std::uint8_t> image(image_size);
    if (!read_bytes(process, base, image.data(), image.size())) {
        CloseHandle(process);
        return 5;
    }

    audit_code_operations(image, profile);

    std::unordered_map<std::uintptr_t, std::string> text_addresses;
    for (const auto& [name, record] : targets) {
        (void)record;
        std::string needle = name;
        needle.push_back('\0');
        auto offset = std::search(image.begin(), image.end(), needle.begin(), needle.end());
        while (offset != image.end()) {
            const auto position = static_cast<std::size_t>(offset - image.begin());
            text_addresses.emplace(base + position, name);
            offset = std::search(offset + 1, image.end(), needle.begin(), needle.end());
        }
    }

    std::unordered_map<std::uintptr_t, TypeRecord> metadata_records;
    for (std::size_t offset = 0; offset + 0x50 <= image.size(); offset += 8) {
        std::uintptr_t text{};
        std::uint64_t length{};
        std::memcpy(&text, image.data() + offset + 0x20, sizeof(text));
        std::memcpy(&length, image.data() + offset + 0x28, sizeof(length));
        const auto found = text_addresses.find(text);
        if (found == text_addresses.end() || length != found->second.size()) continue;
        std::uint32_t size{};
        std::memcpy(&size, image.data() + offset + 0x40, sizeof(size));
        const auto target = targets.find(found->second);
        if (target == targets.end() || size != target->second.size) continue;
        metadata_records.insert_or_assign(base + offset, target->second);
    }

    std::vector<Hit> hits;
    std::unordered_map<std::uintptr_t, std::size_t> base_votes;
    std::vector<std::uint8_t> buffer(16 * 1024 * 1024);
    std::uintptr_t cursor{};
    MEMORY_BASIC_INFORMATION memory{};
    while (VirtualQueryEx(process, reinterpret_cast<const void*>(cursor), &memory, sizeof(memory)) == sizeof(memory)) {
        const auto region = reinterpret_cast<std::uintptr_t>(memory.BaseAddress);
        const auto region_end = region + memory.RegionSize;
        if (memory.State == MEM_COMMIT && readable_protection(memory.Protect)) {
            for (std::size_t offset = 0; offset < memory.RegionSize; offset += buffer.size()) {
                const auto wanted = (std::min)(buffer.size(), memory.RegionSize - offset);
                SIZE_T received{};
                if (!ReadProcessMemory(process, reinterpret_cast<const void*>(region + offset),
                    buffer.data(), wanted, &received)) continue;
                for (std::size_t local = 0; local + sizeof(std::uintptr_t) <= received; local += 8) {
                    std::uintptr_t value{};
                    std::memcpy(&value, buffer.data() + local, sizeof(value));
                    const auto descriptor = metadata_records.find(value);
                    if (descriptor == metadata_records.end()) continue;
                    const auto address = region + offset + local;
                    const auto& name = descriptor->second.name;
                    hits.push_back({address, value, name, descriptor->second.size});
                    const auto known = known_indices.find(name);
                    if (known != known_indices.end() && address >= known->second * 8) {
                        const auto candidate = address - known->second * 8;
                        if ((candidate & 7) == 0) ++base_votes[candidate];
                    }
                }
            }
        }
        if (region_end <= cursor) break;
        cursor = region_end;
    }

    std::vector<std::pair<std::uintptr_t, std::size_t>> candidates;
    for (const auto& item : base_votes) if (item.second >= 100) candidates.push_back(item);
    std::sort(candidates.begin(), candidates.end(), [](const auto& left, const auto& right) {
        return left.second > right.second;
    });

    std::cout << "catalog_types=" << targets.size()
              << " metadata_descriptors_found=" << metadata_records.size()
              << " profile_mappings=" << known_indices.size()
              << " descriptor_pointer_references=" << hits.size() << '\n';
    std::unordered_set<std::string> printed;
    for (const auto& [table, votes] : candidates) {
        std::unordered_map<std::size_t, TypeRecord> rows;
        std::size_t known{};
        for (const auto& hit : hits) {
            if (hit.address < table) continue;
            const auto delta = hit.address - table;
            if (delta >= 1024 * sizeof(std::uintptr_t) || (delta & 7)) continue;
            const auto index = static_cast<std::size_t>(delta / sizeof(std::uintptr_t));
            rows.insert_or_assign(index, TypeRecord{hit.name, hit.size});
            if (const auto expected = known_indices.find(hit.name);
                expected != known_indices.end() && expected->second == index) ++known;
        }
        if (known < 100) continue;
        std::cout << "candidate_table=0x" << std::hex << table << std::dec
                  << " votes=" << votes << " confirmed_known_slots=" << known
                  << " rows=" << rows.size() << '\n';
        for (const auto& [index, record] : rows) {
            if (known_indices.contains(record.name) || !printed.insert(record.name).second) continue;
            std::cout << "new_mapping\t" << index << '\t' << record.size << '\t' << record.name << '\n';
        }
    }

    std::sort(hits.begin(), hits.end(), [](const Hit& left, const Hit& right) {
        return left.address < right.address;
    });
    for (std::size_t begin = 0; begin < hits.size();) {
        std::size_t end = begin + 1;
        while (end < hits.size() && hits[end].address - hits[end - 1].address <= 80 &&
               hits[end].address - hits[begin].address < 64 * 1024) ++end;
        if (end - begin >= 50) {
            std::unordered_set<std::string> names;
            std::unordered_set<std::string> mapped_names;
            std::unordered_set<std::string> unmapped_components;
            for (std::size_t index = begin; index < end; ++index) {
                const auto& name = hits[index].name;
                names.insert(name);
                if (known_indices.contains(name)) mapped_names.insert(name);
                if (!known_indices.contains(name) && component_targets.contains(name))
                    unmapped_components.insert(name);
            }
            if (names.size() >= 40) {
                std::cout << "type_pointer_run=0x" << std::hex << hits[begin].address
                          << "..0x" << hits[end - 1].address << std::dec
                          << " refs=" << end - begin << " distinctTypes=" << names.size()
                          << " profileTypes=" << mapped_names.size()
                          << " unmappedComponentTypes=" << unmapped_components.size() << '\n';
                std::size_t direct_index_matches{};
                std::size_t known_in_run{};
                for (std::size_t index = begin; index < end; ++index) {
                    const auto known = known_indices.find(hits[index].name);
                    if (known == known_indices.end()) continue;
                    ++known_in_run;
                    const auto slot = static_cast<std::size_t>(
                        (hits[index].address - hits[begin].address) / sizeof(std::uintptr_t));
                    direct_index_matches += known->second == slot;
                }
                if (known_in_run)
                    std::cout << "run_slot_vs_profile_index=" << direct_index_matches << '/'
                              << known_in_run << '\n';
                std::size_t shown{};
                for (std::size_t index = begin; index < end && shown < 10; ++index) {
                    const auto known = known_indices.find(hits[index].name);
                    if (known == known_indices.end() && unmapped_components.empty()) continue;
                    const auto slot = static_cast<std::size_t>(
                        (hits[index].address - hits[begin].address) / sizeof(std::uintptr_t));
                    std::cout << "run_slot_sample\t" << slot << '\t' << hits[index].name
                              << "\tprofileIndex=";
                    if (known == known_indices.end()) std::cout << "absent";
                    else std::cout << known->second;
                    std::cout << '\n';
                    ++shown;
                }
                for (const auto& name : unmapped_components)
                    std::cout << "run_unmapped_type\t" << name << '\n';
            }
        }
        begin = end;
    }
    CloseHandle(process);
    return 0;
}
