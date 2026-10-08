#include <windows.h>
#include <bcrypt.h>
#include <tlhelp32.h>
#include <nlohmann/json.hpp>
#include "../../../../native/runtime.h"

#include <algorithm>
#include <array>
#include <charconv>
#include <cctype>
#include <cmath>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>
#include <sstream>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <unordered_set>
#include <vector>

namespace fs = std::filesystem;
using json = nlohmann::json;

namespace {
struct PeImage {
    std::vector<std::uint8_t> bytes;
    std::uint32_t timestamp{};
    std::uint32_t image_size{};
    std::uint32_t checksum{};
    std::uint16_t machine{};
    struct Section { std::string name; std::uint32_t rva{}, virtual_size{}, raw_offset{}, raw_size{}, characteristics{}; };
    std::vector<Section> sections;
    struct RuntimeFunction { std::uint32_t begin{}, end{}, unwind{}; };
    std::vector<RuntimeFunction> runtime_functions;
};

template<class T> const T* at(const std::vector<std::uint8_t>& bytes, std::size_t offset) {
    if (offset > bytes.size() || sizeof(T) > bytes.size() - offset) return nullptr;
    return reinterpret_cast<const T*>(bytes.data() + offset);
}

std::optional<PeImage> read_pe(const fs::path& path, std::string& error) {
    PeImage image;
    std::ifstream input(path, std::ios::binary);
    if (!input) { error = "cannot-open-executable"; return std::nullopt; }
    input.seekg(0, std::ios::end);
    const auto length = input.tellg();
    if (length < 4096 || length > static_cast<std::streamoff>(2ull << 30)) {
        error = "invalid-executable-size"; return std::nullopt;
    }
    image.bytes.resize(static_cast<std::size_t>(length));
    input.seekg(0);
    if (!input.read(reinterpret_cast<char*>(image.bytes.data()), static_cast<std::streamsize>(image.bytes.size()))) {
        error = "cannot-read-executable"; return std::nullopt;
    }
    const auto dos = at<IMAGE_DOS_HEADER>(image.bytes, 0);
    if (!dos || dos->e_magic != IMAGE_DOS_SIGNATURE || dos->e_lfanew < 0) {
        error = "invalid-dos-header"; return std::nullopt;
    }
    const auto nt_offset = static_cast<std::size_t>(dos->e_lfanew);
    const auto signature = at<DWORD>(image.bytes, nt_offset);
    const auto file = at<IMAGE_FILE_HEADER>(image.bytes, nt_offset + sizeof(DWORD));
    if (!signature || *signature != IMAGE_NT_SIGNATURE || !file || file->SizeOfOptionalHeader < sizeof(IMAGE_OPTIONAL_HEADER64)) {
        error = "invalid-pe-header"; return std::nullopt;
    }
    const auto optional = at<IMAGE_OPTIONAL_HEADER64>(image.bytes, nt_offset + sizeof(DWORD) + sizeof(IMAGE_FILE_HEADER));
    if (!optional || optional->Magic != IMAGE_NT_OPTIONAL_HDR64_MAGIC) {
        error = "not-pe32-plus"; return std::nullopt;
    }
    image.timestamp = file->TimeDateStamp;
    image.image_size = optional->SizeOfImage;
    image.checksum = optional->CheckSum;
    image.machine = file->Machine;
    const auto section_offset = nt_offset + sizeof(DWORD) + sizeof(IMAGE_FILE_HEADER) + file->SizeOfOptionalHeader;
    for (unsigned i = 0; i < file->NumberOfSections; ++i) {
        const auto section = at<IMAGE_SECTION_HEADER>(image.bytes, section_offset + i * sizeof(IMAGE_SECTION_HEADER));
        if (!section) { error = "truncated-section-table"; return std::nullopt; }
        std::size_t name_size = 0;
        while (name_size < IMAGE_SIZEOF_SHORT_NAME && section->Name[name_size]) ++name_size;
        image.sections.push_back({std::string(reinterpret_cast<const char*>(section->Name), name_size),
            section->VirtualAddress, section->Misc.VirtualSize, section->PointerToRawData, section->SizeOfRawData,
            section->Characteristics});
    }
    const auto exception_directory = optional->DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION];
    auto rva_to_raw = [&](std::uint32_t rva, std::size_t required) -> std::optional<std::size_t> {
        for (const auto& section : image.sections) {
            if (rva < section.rva) continue;
            const auto delta = static_cast<std::size_t>(rva - section.rva);
            if (delta > section.raw_size || required > section.raw_size - delta) continue;
            const auto raw = static_cast<std::size_t>(section.raw_offset) + delta;
            if (raw <= image.bytes.size() && required <= image.bytes.size() - raw) return raw;
        }
        return std::nullopt;
    };
    if (exception_directory.VirtualAddress && exception_directory.Size >= 12) {
        const auto count = exception_directory.Size / 12;
        const auto raw = rva_to_raw(exception_directory.VirtualAddress, static_cast<std::size_t>(count) * 12);
        if (raw) {
            for (std::uint32_t i = 0; i < count; ++i) {
                struct RuntimeFunctionRow { std::uint32_t begin, end, unwind; } row{};
                std::memcpy(&row, image.bytes.data() + *raw + static_cast<std::size_t>(i) * 12, sizeof(row));
                if (row.begin < row.end && row.end <= image.image_size)
                    image.runtime_functions.push_back({row.begin, row.end, row.unwind});
            }
        }
    }
    return image;
}

std::string sha256(const fs::path& path) {
    BCRYPT_ALG_HANDLE algorithm{};
    BCRYPT_HASH_HANDLE hash{};
    DWORD object_size{}, digest_size{}, received{};
    if (BCryptOpenAlgorithmProvider(&algorithm, BCRYPT_SHA256_ALGORITHM, nullptr, 0) < 0) return {};
    if (BCryptGetProperty(algorithm, BCRYPT_OBJECT_LENGTH, reinterpret_cast<PUCHAR>(&object_size), sizeof(object_size), &received, 0) < 0 ||
        BCryptGetProperty(algorithm, BCRYPT_HASH_LENGTH, reinterpret_cast<PUCHAR>(&digest_size), sizeof(digest_size), &received, 0) < 0) {
        BCryptCloseAlgorithmProvider(algorithm, 0); return {};
    }
    std::vector<UCHAR> object(object_size), digest(digest_size), chunk(1u << 20);
    if (BCryptCreateHash(algorithm, &hash, object.data(), object_size, nullptr, 0, 0) < 0) {
        BCryptCloseAlgorithmProvider(algorithm, 0); return {};
    }
    std::ifstream input(path, std::ios::binary);
    bool ok = static_cast<bool>(input);
    while (ok && input) {
        input.read(reinterpret_cast<char*>(chunk.data()), static_cast<std::streamsize>(chunk.size()));
        const auto count = input.gcount();
        if (count > 0 && BCryptHashData(hash, chunk.data(), static_cast<ULONG>(count), 0) < 0) ok = false;
    }
    ok = ok && input.eof() && BCryptFinishHash(hash, digest.data(), digest_size, 0) >= 0;
    BCryptDestroyHash(hash);
    BCryptCloseAlgorithmProvider(algorithm, 0);
    if (!ok) return {};
    constexpr char digits[] = "0123456789abcdef";
    std::string result;
    result.reserve(digest.size() * 2);
    for (const auto byte : digest) { result.push_back(digits[byte >> 4]); result.push_back(digits[byte & 15]); }
    return result;
}

json image_report(const fs::path& path, const PeImage& pe) {
    json sections = json::array();
    for (const auto& section : pe.sections) {
        sections.push_back({{"name", section.name}, {"rva", section.rva}, {"virtualSize", section.virtual_size},
            {"rawOffset", section.raw_offset}, {"rawSize", section.raw_size},
            {"executable", (section.characteristics & IMAGE_SCN_MEM_EXECUTE) != 0}});
    }
    return {{"schemaVersion", 1}, {"file", fs::absolute(path).string()}, {"fileSize", pe.bytes.size()},
        {"sha256", sha256(path)}, {"machine", pe.machine}, {"timestamp", pe.timestamp},
        {"imageSize", pe.image_size}, {"checksum", pe.checksum}, {"sections", std::move(sections)},
        {"runtimeFunctionCount", pe.runtime_functions.size()}};
}

void usage() {
    std::cout <<
        "kfc-runtime-dev - build-time Enshrouded runtime investigation tools\n\n"
        "Commands:\n"
        "  select-build <game-directory-or-exe> [--out-dir <capture-directory>]\n"
        "  inspect <enshrouded.exe> [--out <image-report.json>]\n"
        "  discover-functions <enshrouded.exe> [--out <function-candidates.json>]\n"
        "  extract-profile-functions <profile.json> [--out <function-catalog.json>]\n"
        "  scan-functions <enshrouded.exe> <function-catalog.json> [--out <scan-report.json>]\n"
        "  capture-process <pid> [--out <process-capture.json>]\n"
        "  import-ecs-capture <capture.log> --image-report <image.json> --reflection <kfc-parser-reflection.json> [--id <profile-id>] [--out <component-catalog.json>]\n"
        "  inspect-provider <kfc-runtime.dll> [--out <provider-report.json>]\n"
        "  validate-profile <profile.json>\n"
        "  approve-profile <draft.json> --function-scan <scan.json> --components <catalog.json>\n"
        "  generate-profile <base-profile.json> <image-report.json> <scan-report.json> --components <live-capture.json> --out <draft.json>\n\n"
        "This console is developer-only. It is not run by the game or modloader.\n";
}

std::optional<fs::path> option(int argc, char** argv, std::string_view key) {
    for (int i = 0; i + 1 < argc; ++i) if (std::string_view(argv[i]) == key) return fs::path(argv[i + 1]);
    return std::nullopt;
}

bool write_json(const fs::path& path, const json& value) {
    std::error_code ec;
    if (!path.parent_path().empty()) fs::create_directories(path.parent_path(), ec);
    if (ec) { std::cerr << "cannot-create-output-directory: " << ec.message() << '\n'; return false; }
    std::ofstream out(path, std::ios::binary | std::ios::trunc);
    if (!out) { std::cerr << "cannot-write: " << path.string() << '\n'; return false; }
    out << value.dump(2) << '\n';
    return static_cast<bool>(out);
}

std::vector<int> parse_pattern(std::string_view text) {
    std::vector<int> result;
    std::size_t cursor{};
    while (cursor < text.size()) {
        while (cursor < text.size() && std::isspace(static_cast<unsigned char>(text[cursor]))) ++cursor;
        if (cursor == text.size()) break;
        const auto end = text.find_first_of(" \t\r\n", cursor);
        const auto token = text.substr(cursor, end == std::string_view::npos ? text.size() - cursor : end - cursor);
        if (token == "?" || token == "??") result.push_back(-1);
        else {
            unsigned byte{};
            const auto parsed = std::from_chars(token.data(), token.data() + token.size(), byte, 16);
            if (parsed.ec != std::errc{} || parsed.ptr != token.data() + token.size() || byte > 255) return {};
            result.push_back(static_cast<int>(byte));
        }
        cursor = end == std::string_view::npos ? text.size() : end;
    }
    return result;
}

std::vector<std::uint32_t> find_pattern(const PeImage& pe, const std::vector<int>& pattern) {
    std::vector<std::uint32_t> matches;
    if (pattern.empty()) return matches;
    for (const auto& section : pe.sections) {
        if (!(section.characteristics & IMAGE_SCN_MEM_EXECUTE) || section.raw_offset >= pe.bytes.size()) continue;
        const auto available = pe.bytes.size() - section.raw_offset;
        const auto size = (std::min)(static_cast<std::size_t>(section.raw_size), available);
        if (size < pattern.size()) continue;
        for (std::size_t offset = 0; offset <= size - pattern.size(); ++offset) {
            bool match = true;
            for (std::size_t i = 0; i < pattern.size(); ++i)
                if (pattern[i] >= 0 && pe.bytes[section.raw_offset + offset + i] != pattern[i]) { match = false; break; }
            if (match) matches.push_back(section.rva + static_cast<std::uint32_t>(offset));
        }
    }
    return matches;
}

bool rva_bytes_equal(const PeImage& pe, std::uint32_t rva, const std::vector<std::uint8_t>& expected) {
    for (const auto& section : pe.sections) {
        if (rva < section.rva) continue;
        const auto delta = static_cast<std::size_t>(rva - section.rva);
        if (delta > section.raw_size || expected.size() > section.raw_size - delta) continue;
        const auto raw = static_cast<std::size_t>(section.raw_offset) + delta;
        return raw <= pe.bytes.size() && expected.size() <= pe.bytes.size() - raw &&
            std::equal(expected.begin(), expected.end(), pe.bytes.begin() + raw);
    }
    return false;
}

json function_candidates(const PeImage& pe) {
    json candidates = json::array();
    // Conservative x64 entry-point heuristics. These are leads for a developer
    // to inspect, never approved function bindings or safe hook locations.
    for (const auto& section : pe.sections) {
        if (!(section.characteristics & IMAGE_SCN_MEM_EXECUTE) || section.raw_offset >= pe.bytes.size()) continue;
        const auto size = (std::min)(static_cast<std::size_t>(section.raw_size), pe.bytes.size() - section.raw_offset);
        std::uint32_t last_rva{};
        for (std::size_t offset = 0; offset + 8 <= size; ++offset) {
            const auto* code = pe.bytes.data() + section.raw_offset + offset;
            const bool push_nonvolatile = (code[0] == 0x53 || code[0] == 0x55 || code[0] == 0x56 || code[0] == 0x57 ||
                ((code[0] & 0xf8) == 0x40 && (code[1] == 0x53 || code[1] == 0x55 || code[1] == 0x56 || code[1] == 0x57)));
            const bool stack_frame = code[0] == 0x48 && code[1] == 0x83 && code[2] == 0xec && code[3] >= 0x20;
            const bool large_stack_frame = code[0] == 0x48 && code[1] == 0x81 && code[2] == 0xec;
            if (!push_nonvolatile && !stack_frame && !large_stack_frame) continue;
            const auto rva = section.rva + static_cast<std::uint32_t>(offset);
            if (rva - last_rva < 8) continue;
            last_rva = rva;
            std::string prefix;
            constexpr char digits[] = "0123456789ABCDEF";
            for (std::size_t i = 0; i < 16 && offset + i < size; ++i) {
                if (!prefix.empty()) prefix.push_back(' ');
                const auto byte = code[i];
                prefix.push_back(digits[byte >> 4]); prefix.push_back(digits[byte & 15]);
            }
            candidates.push_back({{"rva", rva}, {"section", section.name}, {"heuristic",
                stack_frame ? "sub-rsp-imm8" : large_stack_frame ? "sub-rsp-imm32" : "push-nonvolatile-register"},
                {"prefix", std::move(prefix)}, {"status", "unverified-candidate"}});
        }
    }
    return candidates;
}

bool validate_profile(const json& profile, std::string& error, const json* external_components = nullptr) {
    try {
        if (profile.at("schemaVersion") != 1) throw std::runtime_error("unsupported schemaVersion");
        const auto target = profile.at("target").get<std::string>();
        if (target != "enshrouded.exe" && target != "enshrouded_server.exe") throw std::runtime_error("unsupported target");
        const auto& image = profile.at("image");
        if (image.at("size").get<std::uint64_t>() < 4096) throw std::runtime_error("image size is too small");
        const auto& layout = profile.at("ecsLayout");
        for (const char* key : {"entityManagerCount", "entityManagerTable", "componentOffsets", "componentStrides",
             "entityId", "entityGeneration", "entityLayout", "entityStorage", "entityDefinitionPointer", "entityRow", "componentBits", "lookupManager"})
            if (layout.at(key).get<std::uint64_t>() > 0x10000) throw std::runtime_error(std::string("layout offset out of range: ") + key);
        const auto& definition = profile.at("entityDefinitionLayout");
        for (const char* key : {"uuid", "name", "nameSize"})
            if (definition.at(key).get<std::uint64_t>() > 0x10000) throw std::runtime_error(std::string("entity definition offset out of range: ") + key);
        const auto& hooks = profile.at("hooks");
        for (const char* key : {"game_thread", "entity_manager", "world_prop_update", "world_actor_placement", "world_cursor"}) {
            if (std::string_view(key) == "world_cursor" && !hooks.contains(key)) continue;
            const auto& hook = hooks.at(key);
            if (parse_pattern(hook.at("signature").get<std::string>()).size() < 7) throw std::runtime_error(std::string("invalid hook signature: ") + key);
            const auto bytes = hook.at("original").get<std::vector<unsigned>>();
            if (bytes.size() < 5 || bytes.size() > 32) throw std::runtime_error(std::string("invalid overwritten bytes: ") + key);
            for (const auto byte : bytes) if (byte > 255) throw std::runtime_error(std::string("invalid original byte: ") + key);
        }
        if (hooks.contains("world_building_dispatch")) {
            const auto& hook = hooks.at("world_building_dispatch");
            if (parse_pattern(hook.at("signature").get<std::string>()).size() < 7)
                throw std::runtime_error("invalid hook signature: world_building_dispatch");
            const auto bytes = hook.at("original").get<std::vector<unsigned>>();
            if (bytes.size() < 5 || bytes.size() > 32)
                throw std::runtime_error("invalid overwritten bytes: world_building_dispatch");
            for (const auto byte : bytes) if (byte > 255)
                throw std::runtime_error("invalid original byte: world_building_dispatch");
        }
        if (hooks.contains("world_cursor") && hooks.at("world_cursor").at("captureOffset").get<std::uint64_t>() > 0x10000)
            throw std::runtime_error("native cursor capture offset out of range");
        const auto& placement = profile.at("worldContexts").at("entityPlacement");
        for (const char* key : {"actorFrameServiceViewOffset", "serviceViewWorldOffset", "placementContextOffset",
             "placeQueueOffset", "removeQueueOffset", "publishStateOffset", "publishCommandsOffset", "ownerOffset"})
            if (placement.at(key).get<std::uint64_t>() > 0x10000) throw std::runtime_error(std::string("world context offset out of range: ") + key);
        const auto& grids = profile.at("worldGrids");
        if (!grids.is_object() || grids.empty()) throw std::runtime_error("world grid catalog is missing or empty");
        for (auto item = grids.begin(); item != grids.end(); ++item) {
            const auto origin = item.value().at("origin").get<std::vector<double>>();
            const auto cell_size = item.value().at("cellSize").get<std::vector<double>>();
            const auto maximum = item.value().at("maximum").get<std::vector<std::uint64_t>>();
            if (item.key().empty() || item.key().size() >= 16 || origin.size() != 3 || cell_size.size() != 3 || maximum.size() != 3)
                throw std::runtime_error("invalid world grid specification: " + item.key());
            for (std::size_t axis = 0; axis < 3; ++axis)
                if (!std::isfinite(origin[axis]) || !std::isfinite(cell_size[axis]) || cell_size[axis] <= 0 || !maximum[axis])
                    throw std::runtime_error("invalid world grid specification: " + item.key());
        }
        const auto& operations = profile.at("worldOperations");
        if (operations.empty()) throw std::runtime_error("world operation catalog is empty");
        for (auto item = operations.begin(); item != operations.end(); ++item) {
            const auto& operation = item.value();
            if (!item.key().starts_with("runtime.world.") || operation.at("abi").get<std::string>().empty() ||
                operation.at("thread") != "game" || operation.at("context").get<std::string>().empty() ||
                (operation.value("validated", true) && operation.at("abi") != "actor-world-context" && operation.contains("functionRva") == operation.contains("globalRva")) ||
                (operation.contains("functionRva") && operation.contains("globalRva")))
                throw std::runtime_error("invalid world operation entry: " + item.key());
            for (const auto byte : operation.value("guardBytes", std::vector<unsigned>{}))
                if (byte > 255) throw std::runtime_error("invalid world operation guard bytes: " + item.key());
        }
        const auto& patches = profile.at("runtimePatches");
        if (patches.empty()) throw std::runtime_error("runtime patch catalog is empty");
        for (auto item = patches.begin(); item != patches.end(); ++item) {
            const auto& patch = item.value();
            if (!item.key().starts_with("runtime.patch.") || parse_pattern(patch.at("signature").get<std::string>()).empty() ||
                (patch.at("kind") != "bytes" && patch.at("kind") != "detour") ||
                patch.at("overwriteBytes").get<unsigned>() < 3 || patch.at("overwriteBytes").get<unsigned>() > 32 ||
                patch.at("payload").empty()) throw std::runtime_error("invalid runtime patch entry: " + item.key());
        }
        std::unordered_set<std::string> names;
        std::unordered_set<unsigned> indices;
        const bool live_components = profile.value("componentResolution", std::string{}) == "live-registration";
        if (live_components && profile.contains("components")) throw std::runtime_error("live registration profile must not duplicate component mappings");
        const auto empty_components = json::array();
        const json* components = profile.contains("components") ? &profile.at("components") : external_components;
        if (live_components) components = &empty_components;
        if (!components && profile.contains("componentCatalog")) throw std::runtime_error("external component catalog must be supplied for validation");
        if (!components || !components->is_array()) throw std::runtime_error("component catalog is missing or invalid");
        for (const auto& component : *components) {
            const auto name = component.at("name").get<std::string>();
            const auto index = component.at("index").get<unsigned>();
            const auto size = component.at("size").get<unsigned>();
            if (!name.starts_with("keen::ecs::") || index >= 1024 || !size || size > 65535 ||
                !names.insert(name).second || !indices.insert(index).second) throw std::runtime_error("invalid or duplicate component mapping");
        }
        if (names.empty() && !live_components) throw std::runtime_error("component catalog is empty");
        return true;
    } catch (const std::exception& exception) { error = exception.what(); return false; }
}

int inspect_command(int argc, char** argv) {
    const fs::path exe = argv[2];
    std::string error;
    auto pe = read_pe(exe, error);
    if (!pe) { std::cerr << error << '\n'; return 2; }
    const auto report = image_report(exe, *pe);
    if (const auto out = option(argc, argv, "--out")) {
        if (!write_json(*out, report)) return 2;
        std::cout << "image report: " << fs::absolute(*out).string() << '\n';
    } else std::cout << report.dump(2) << '\n';
    return 0;
}

int select_build_command(int argc, char** argv) {
    fs::path selected = fs::absolute(argv[2]);
    if (fs::is_directory(selected)) {
        const auto client = selected / "enshrouded.exe";
        const auto server = selected / "enshrouded_server.exe";
        if (fs::exists(client)) selected = client;
        else if (fs::exists(server)) selected = server;
        else { std::cerr << "no enshrouded.exe or enshrouded_server.exe in selected directory\n"; return 2; }
    }
    std::string error;
    const auto pe = read_pe(selected, error);
    if (!pe) { std::cerr << error << '\n'; return 2; }
    const auto hash = sha256(selected);
    if (hash.empty()) { std::cerr << "cannot-hash-selected-executable\n"; return 2; }
    const auto build_id = selected.stem().string() + "-" + std::to_string(pe->timestamp) + "-" + hash.substr(0, 12);
    const auto output = option(argc, argv, "--out-dir").value_or(fs::current_path() / "devdata" / build_id);
    if (!write_json(output / "image-report.json", image_report(selected, *pe))) return 2;
    auto function_report = image_report(selected, *pe);
    function_report["method"] = "x64-exception-table-and-prologue-heuristics";
    function_report["warning"] = "Function ranges are recovered from PE unwind metadata; names and semantics require investigation.";
    json ranges = json::array();
    for (const auto& function : pe->runtime_functions)
        ranges.push_back({{"beginRva", function.begin}, {"endRva", function.end},
            {"size", function.end - function.begin}, {"unwindInfoRva", function.unwind}});
    function_report["runtimeFunctions"] = std::move(ranges);
    function_report["candidates"] = function_candidates(*pe);
    if (!write_json(output / "function-candidates.json", function_report)) return 2;

    const auto catalog_directory = fs::current_path() / "dev" / "function-catalogs";
    std::optional<fs::path> catalog_path;
    if (fs::exists(catalog_directory)) for (const auto& entry : fs::recursive_directory_iterator(catalog_directory)) {
        if (entry.path().extension() != ".json" || entry.path().filename() == "schema.json") continue;
        try {
            const auto candidate = json::parse(std::ifstream(entry.path()));
            if (candidate.at("image").at("timestamp") == pe->timestamp && candidate.at("image").at("size") == pe->image_size) {
                if (catalog_path) { std::cerr << "multiple function catalogs match this PE identity\n"; return 3; }
                catalog_path = entry.path();
            }
        } catch (...) {}
    }
    std::cout << "selected build: " << selected.string() << '\n'
              << "build id: " << build_id << '\n'
              << "capture directory: " << fs::absolute(output).string() << '\n'
              << "runtime function ranges: " << pe->runtime_functions.size() << '\n'
              << "heuristic candidates: " << function_report["candidates"].size() << '\n';
    if (catalog_path) {
        const auto report_path = output / "function-scan.json";
        auto report = image_report(selected, *pe);
        json runtime_functions = json::array();
        for (const auto& function : pe->runtime_functions)
            runtime_functions.push_back({{"beginRva", function.begin}, {"endRva", function.end}});
        report["runtimeFunctions"] = std::move(runtime_functions);
        const auto catalog = json::parse(std::ifstream(*catalog_path));
        json rows = json::array();
        bool all_unique = true;
        for (const auto& entry : catalog.at("functions")) {
            const auto matches = find_pattern(*pe, parse_pattern(entry.at("signature").get<std::string>()));
            json addresses = json::array();
            for (const auto rva : matches) addresses.push_back(rva);
            const auto one_match = matches.size() == 1;
            const auto expected_rva_matches = !entry.contains("expectedRva") ||
                (one_match && matches.front() == entry.at("expectedRva").get<std::uint32_t>());
            const auto original = entry.value("original", std::vector<std::uint8_t>{});
            const auto original_matches = original.empty() || (one_match && rva_bytes_equal(*pe, matches.front(), original));
            const auto unique = one_match && expected_rva_matches && original_matches;
            all_unique = all_unique && unique;
            rows.push_back({{"id", entry.at("id")}, {"signature", entry.at("signature")},
                {"matches", std::move(addresses)}, {"matchCount", matches.size()},
                {"original", original},
                {"expectedRvaMatches", expected_rva_matches}, {"originalBytesMatch", original_matches},
                {"status", unique ? "unique-candidate" : matches.empty() ? "not-found" : matches.size() > 1 ? "ambiguous" : !expected_rva_matches ? "wrong-rva" : "original-bytes-mismatch"},
                {"callingConvention", entry.value("callingConvention", "unknown")},
                {"semanticStatus", entry.value("semanticStatus", "unverified")}});
        }
        auto scan = report;
        scan["functions"] = std::move(rows);
        scan["allFunctionCandidatesUnique"] = all_unique;
        if (!write_json(report_path, scan)) return 2;
        std::cout << "function signature scan: " << report_path.string() << '\n';
    } else {
        std::cout << "no curated catalog for this build yet; inspect candidates and create a reviewed catalog before profile generation\n";
    }
    return 0;
}

int scan_command(int argc, char** argv) {
    const fs::path exe = argv[2], catalog_path = argv[3];
    std::string error;
    auto pe = read_pe(exe, error);
    if (!pe) { std::cerr << error << '\n'; return 2; }
    json catalog;
    try { catalog = json::parse(std::ifstream(catalog_path)); }
    catch (const std::exception& exception) { std::cerr << "invalid function catalog: " << exception.what() << '\n'; return 2; }
    json functions = json::array();
    bool all_unique = true;
    for (const auto& entry : catalog.at("functions")) {
        const auto id = entry.at("id").get<std::string>();
        const auto signature = entry.at("signature").get<std::string>();
        const auto pattern = parse_pattern(signature);
        const auto matches = find_pattern(*pe, pattern);
        json rvas = json::array();
        for (const auto rva : matches) rvas.push_back(rva);
        const bool one_match = matches.size() == 1;
        const bool expected_rva_matches = !entry.contains("expectedRva") ||
            (one_match && matches.front() == entry.at("expectedRva").get<std::uint32_t>());
        const auto original = entry.value("original", std::vector<std::uint8_t>{});
        const bool original_matches = original.empty() || (one_match && rva_bytes_equal(*pe, matches.front(), original));
        const bool unique = one_match && expected_rva_matches && original_matches;
        all_unique = all_unique && unique;
        functions.push_back({{"id", id}, {"signature", signature}, {"matches", std::move(rvas)},
            {"matchCount", matches.size()}, {"expectedRva", entry.value("expectedRva", 0u)},
            {"original", original},
            {"expectedRvaMatches", expected_rva_matches}, {"originalBytesMatch", original_matches},
            {"status", unique ? "unique-candidate" : matches.empty() ? "not-found" : matches.size() > 1 ? "ambiguous" : !expected_rva_matches ? "wrong-rva" : "original-bytes-mismatch"},
            {"callingConvention", entry.value("callingConvention", "unknown")},
            {"semanticStatus", entry.value("semanticStatus", "unverified")}});
    }
    auto report = image_report(exe, *pe);
    report["functions"] = std::move(functions);
    report["allFunctionCandidatesUnique"] = all_unique;
    json runtime_functions = json::array();
    for (const auto& function : pe->runtime_functions)
        runtime_functions.push_back({{"beginRva", function.begin}, {"endRva", function.end}});
    report["runtimeFunctions"] = std::move(runtime_functions);
    if (const auto out = option(argc, argv, "--out")) {
        if (!write_json(*out, report)) return 2;
        std::cout << "function scan: " << fs::absolute(*out).string() << '\n';
    } else std::cout << report.dump(2) << '\n';
    return all_unique ? 0 : 3;
}

int discover_command(int argc, char** argv) {
    const fs::path exe = argv[2];
    std::string error;
    auto pe = read_pe(exe, error);
    if (!pe) { std::cerr << error << '\n'; return 2; }
    auto report = image_report(exe, *pe);
    report["method"] = "x64-prologue-heuristics";
    report["warning"] = "The PE exception directory enumerates compiler-described function ranges. Prologue candidates are heuristic and neither source names nor semantics are inferred.";
    json ranges = json::array();
    for (const auto& function : pe->runtime_functions)
        ranges.push_back({{"beginRva", function.begin}, {"endRva", function.end},
            {"size", function.end - function.begin}, {"unwindInfoRva", function.unwind}});
    report["runtimeFunctions"] = std::move(ranges);
    report["candidates"] = function_candidates(*pe);
    if (const auto out = option(argc, argv, "--out")) {
        if (!write_json(*out, report)) return 2;
        std::cout << "function candidates: " << report["candidates"].size() << " -> " << fs::absolute(*out).string() << '\n';
    } else std::cout << report.dump(2) << '\n';
    return 0;
}

std::string bytes_signature(const std::vector<std::uint8_t>& bytes) {
    constexpr char digits[] = "0123456789ABCDEF";
    std::string result;
    for (const auto byte : bytes) {
        if (!result.empty()) result.push_back(' ');
        result.push_back(digits[byte >> 4]); result.push_back(digits[byte & 15]);
    }
    return result;
}

int extract_profile_functions_command(int argc, char** argv) {
    const fs::path profile_path = argv[2];
    try {
        const auto profile = json::parse(std::ifstream(profile_path));
        json functions = json::array(), globals = json::array();
        const auto& hooks = profile.at("hooks");
        for (auto item = hooks.begin(); item != hooks.end(); ++item) {
            const auto id = "hook." + item.key();
            functions.push_back({{"id", id}, {"purpose", "Profile hook: " + item.key()},
                {"signature", item.value().at("signature")}, {"original", item.value().at("original")},
                {"callingConvention", "x64-windows; review from hook implementation"},
                {"thread", item.key() == "game_thread" ? "game-update-thread" : "engine-thread"},
                {"semanticStatus", "candidate"}, {"sideEffects", json::array({"hook-installation"})},
                {"evidence", json::array({"source profile hook entry"})}});
        }
        const auto& operations = profile.at("worldOperations");
        for (auto item = operations.begin(); item != operations.end(); ++item) {
            const auto& operation = item.value();
            if (operation.contains("functionRva") && operation.contains("guardRva") && !operation.at("guardBytes").empty()) {
                const auto raw = operation.at("guardBytes").get<std::vector<std::uint8_t>>();
                functions.push_back({{"id", item.key()}, {"purpose", operation.at("context")},
                    {"signature", bytes_signature(raw)}, {"original", raw}, {"expectedRva", operation.at("guardRva")},
                    {"callingConvention", operation.at("abi")}, {"thread", operation.at("thread")},
                    {"semanticStatus", "candidate"}, {"operationKind", "world-operation"},
                    {"sideEffects", json::array({"game-thread-call"})},
                    {"evidence", json::array({"source profile guard bytes and RVA"})}});
            } else if (operation.contains("globalRva")) {
                globals.push_back({{"id", item.key()}, {"rva", operation.at("globalRva")},
                    {"abi", operation.at("abi")}, {"thread", operation.at("thread")},
                    {"context", operation.at("context")}, {"semanticStatus", "candidate"}});
            }
        }
        const auto& patches = profile.at("runtimePatches");
        for (auto item = patches.begin(); item != patches.end(); ++item) {
            const auto& patch = item.value();
            const auto& function = patch.at("function");
            functions.push_back({{"id", item.key()}, {"purpose", "Guarded runtime patch: " + item.key()},
                {"signature", patch.at("signature")}, {"expectedRva", function.at("beginRva").get<std::uint32_t>() + function.at("targetOffset").get<std::uint32_t>()},
                {"callingConvention", "patch-site; review owning function ABI"}, {"thread", "game-thread"},
                {"semanticStatus", "candidate"}, {"operationKind", patch.at("kind")},
                {"sideEffects", json::array({"code-patch"})}, {"evidence", json::array({"source profile patch signature and function range"})}});
        }
        const auto& image = profile.at("image");
        const json catalog = {{"schemaVersion", 1}, {"id", profile.at("id")}, {"target", profile.at("target")},
            {"image", {{"timestamp", image.at("timestamp")}, {"size", image.at("size")}}},
            {"functions", std::move(functions)}, {"globals", std::move(globals)}};
        if (const auto out = option(argc, argv, "--out")) {
            if (!write_json(*out, catalog)) return 2;
            std::cout << "function catalog extracted from profile: " << fs::absolute(*out).string() << '\n';
        } else std::cout << catalog.dump(2) << '\n';
        return 0;
    } catch (const std::exception& exception) {
        std::cerr << "cannot extract function catalog: " << exception.what() << '\n';
        return 2;
    }
}

int capture_process_command(int argc, char** argv) {
    DWORD pid{};
    const auto text = std::string_view(argv[2]);
    const auto parsed = std::from_chars(text.data(), text.data() + text.size(), pid);
    if (parsed.ec != std::errc{} || parsed.ptr != text.data() + text.size() || !pid) {
        std::cerr << "invalid-pid\n"; return 2;
    }
    HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
    if (!process) { std::cerr << "cannot-open-process: " << GetLastError() << '\n'; return 2; }
    std::wstring path(32768, L'\0');
    DWORD path_size = static_cast<DWORD>(path.size());
    FILETIME now{};
    GetSystemTimeAsFileTime(&now);
    ULARGE_INTEGER ticks{};
    ticks.LowPart = now.dwLowDateTime;
    ticks.HighPart = now.dwHighDateTime;
    json report = {{"schemaVersion", 1}, {"pid", pid}, {"capturedAtFileTimeUtc", ticks.QuadPart}};
    if (QueryFullProcessImageNameW(process, 0, path.data(), &path_size)) {
        path.resize(path_size);
        const fs::path image_path(path);
        report["processPath"] = image_path.string();
        std::string error;
        if (auto pe = read_pe(image_path, error)) report["image"] = image_report(image_path, *pe);
        else report["imageError"] = error;
    }
    CloseHandle(process);
    HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid);
    json modules = json::array();
    if (snapshot != INVALID_HANDLE_VALUE) {
        MODULEENTRY32W module{sizeof(module)};
        if (Module32FirstW(snapshot, &module)) do {
            modules.push_back({{"name", fs::path(module.szModule).string()},
                {"path", fs::path(module.szExePath).string()}, {"baseAddress", reinterpret_cast<std::uintptr_t>(module.modBaseAddr)},
                {"size", module.modBaseSize}});
        } while (Module32NextW(snapshot, &module));
        CloseHandle(snapshot);
    }
    report["modules"] = std::move(modules);
    if (const auto out = option(argc, argv, "--out")) {
        if (!write_json(*out, report)) return 2;
        std::cout << "process capture: " << fs::absolute(*out).string() << '\n';
    } else std::cout << report.dump(2) << '\n';
    return 0;
}

int import_ecs_capture_command(int argc, char** argv) {
    std::ifstream input(argv[2]);
    if (!input) { std::cerr << "cannot-open-capture\n"; return 2; }
    const auto image_path = option(argc, argv, "--image-report");
    const auto reflection_path = option(argc, argv, "--reflection");
    if (!image_path || !reflection_path) { std::cerr << "import-ecs-capture requires --image-report and --reflection\n"; return 2; }
    json image, reflection;
    try {
        image = json::parse(std::ifstream(*image_path));
        reflection = json::parse(std::ifstream(*reflection_path));
    } catch (const std::exception& exception) { std::cerr << "invalid image or reflection report: " << exception.what() << '\n'; return 2; }
    std::unordered_map<std::string, std::vector<std::uint32_t>> reflection_sizes;
    const bool parser_registry = reflection.contains("types") && reflection.at("types").is_array();
    const auto& reflection_entries = parser_registry ? reflection.at("types") : reflection.at("entries");
    for (const auto& entry : reflection_entries) {
        if (!entry.contains("size")) continue;
        const auto name_key = parser_registry ? "qualifiedName" : "qualified_name";
        if (!entry.contains(name_key)) continue;
        if (!parser_registry && entry.value("kind", "") != "component") continue;
        const auto name = entry.at(name_key).get<std::string>();
        if (!name.starts_with("keen::ecs::")) continue;
        reflection_sizes[name].push_back(entry.at("size").get<std::uint32_t>());
    }
    if (reflection_sizes.empty()) {
        std::cerr << "reflection input has no keen::ecs types; expected kfc-parser reflection_data.json or normalized entries catalog\n";
        return 3;
    }
    json components = json::array(), unresolved = json::array();
    std::unordered_set<std::string> names;
    std::unordered_set<unsigned> indices;
    std::string line;
    while (std::getline(input, line)) {
        std::istringstream row(line);
        std::string index_text, size_text, name;
        if (!std::getline(row, index_text, '\t') || !std::getline(row, size_text, '\t') || !std::getline(row, name)) continue;
        unsigned index{}, size{};
        const auto parsed_index = std::from_chars(index_text.data(), index_text.data() + index_text.size(), index);
        const auto parsed_size = std::from_chars(size_text.data(), size_text.data() + size_text.size(), size);
        if (parsed_index.ec != std::errc{} || parsed_size.ec != std::errc{} ||
            !name.starts_with("keen::ecs::") || index >= 1024 || !size || size > 65535) continue;
        if (!names.insert(name).second || !indices.insert(index).second) {
            unresolved.push_back({{"name", name}, {"index", index}, {"size", size}, {"reason", "duplicate-live-name-or-index"}});
            continue;
        }
        const auto found = reflection_sizes.find(name);
        if (found == reflection_sizes.end() || found->second.size() != 1 || found->second.front() != size) {
            unresolved.push_back({{"name", name}, {"index", index}, {"liveSize", size},
                {"reason", found == reflection_sizes.end() ? "missing-reflection-component" : found->second.size() != 1 ? "ambiguous-reflection-name" : "reflection-size-mismatch"}});
            continue;
        }
        components.push_back({{"name", name}, {"index", index}, {"size", size}});
    }
    if (components.empty()) { std::cerr << "no component rows found; provide output from kfc-runtime-capture-ecs.exe\n"; return 3; }
    const auto exe_name = fs::path(image.at("file").get<std::string>()).filename().string();
    const std::string target = exe_name == "enshrouded_server.exe" ? "enshrouded_server.exe" : "enshrouded.exe";
    const auto version_text = reflection.value("game_version", std::string{});
    const auto version_separator = version_text.find('|');
    const auto game_version = version_text.substr(0, version_separator);
    const std::string id_prefix = target == "enshrouded.exe" ? "enshrouded-client-" : "enshrouded-server-";
    const auto requested_id = option(argc, argv, "--id");
    const auto id = requested_id ? requested_id->string() :
        id_prefix + (!game_version.empty() ? game_version : std::to_string(image.at("timestamp").get<std::uint32_t>()));
    const json catalog = {{"schemaVersion", 1}, {"id", id}, {"target", target},
        {"image", {{"timestamp", image.at("timestamp")}, {"size", image.at("imageSize")}, {"sha256", image.at("sha256")}}},
        {"components", std::move(components)}, {"unresolved", std::move(unresolved)},
        {"source", {{"liveCapture", fs::absolute(argv[2]).string()}, {"reflection", fs::absolute(*reflection_path).string()}}}};
    if (const auto out = option(argc, argv, "--out")) {
        if (!write_json(*out, catalog)) return 2;
        std::cout << "live ECS catalog: " << catalog["components"].size() << " rows -> " << fs::absolute(*out).string() << '\n';
    } else std::cout << catalog.dump(2) << '\n';
    return 0;
}

int inspect_provider_command(int argc, char** argv) {
    const fs::path path = fs::absolute(argv[2]);
    HMODULE module = LoadLibraryExW(path.c_str(), nullptr, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
    if (!module) { std::cerr << "cannot-load-provider: " << GetLastError() << '\n'; return 2; }
    using AbiFunction = unsigned (__cdecl*)();
    const auto abi_function = reinterpret_cast<AbiFunction>(GetProcAddress(module, "KfcRuntimeAbi"));
    json exports = json::array();
    constexpr std::string_view names[]{
        "KfcRuntimeAbi", "KfcRuntimeInitialize", "KfcRuntimeTick", "KfcRuntimeShutdown",
        "KfcRuntimeStatus", "KfcRuntimeDiagnostics", "KfcRuntimeEcsConfigure",
        "KfcRuntimeEcsReady", "KfcRuntimeEcsCanWrite", "KfcRuntimeEcsDescribe",
        "KfcRuntimeEcsQuery", "KfcRuntimeEcsResolve", "KfcRuntimeEcsRead", "KfcRuntimeEcsWrite",
        "KfcRuntimeEcsCompareExchange", "KfcRuntimeEcsRegistry", "KfcRuntimeEcsResolveType",
        "KfcRuntimeFunctions", "KfcRuntimeFunctionCode",
        "KfcRuntimePatchAvailable", "KfcRuntimePatchSetEnabled",
        "KfcRuntimeWorldOperationAvailable", "KfcRuntimeWorldContextActive",
        "KfcRuntimeWorldEntityContextReady", "KfcRuntimeWorldCursorRead",
        "KfcRuntimeWorldVoxelRead", "KfcRuntimeWorldVoxelWrite",
        "KfcRuntimeWorldEntitySpawn", "KfcRuntimeWorldEntityPlace", "KfcRuntimeWorldEntityDestroy",
        "KfcRuntimeWorldEntityFinishBuilding"
    };
    bool complete = true;
    for (const auto name : names) {
        const bool present = GetProcAddress(module, std::string(name).c_str()) != nullptr;
        exports.push_back({{"name", name}, {"present", present}});
        complete = complete && present;
    }
    const auto abi = abi_function ? abi_function() : 0;
    FreeLibrary(module);
    const json report = {{"schemaVersion", 1}, {"provider", path.string()}, {"providerAbi", abi},
        {"expectedAbi", KFC_RUNTIME_ABI_VERSION}, {"expectedExportsPresent", complete}, {"exports", std::move(exports)}};
    if (const auto out = option(argc, argv, "--out")) {
        if (!write_json(*out, report)) return 2;
        std::cout << "provider inventory: " << fs::absolute(*out).string() << '\n';
    } else std::cout << report.dump(2) << '\n';
    return complete && abi == KFC_RUNTIME_ABI_VERSION ? 0 : 3;
}

int validate_command(const fs::path& path, const std::optional<fs::path>& component_path = std::nullopt) {
    try {
        const auto profile = json::parse(std::ifstream(path));
        std::optional<json> component_catalog;
        if (component_path) component_catalog = json::parse(std::ifstream(*component_path)).at("components");
        else if (profile.contains("componentCatalog")) {
            auto catalog_path = path.parent_path() / profile.at("componentCatalog").get<std::string>();
            if (!fs::exists(catalog_path)) throw std::runtime_error("referenced component catalog not found: " + catalog_path.string());
            component_catalog = json::parse(std::ifstream(catalog_path)).at("components");
        }
        std::string error;
        if (!validate_profile(profile, error, component_catalog ? &*component_catalog : nullptr)) { std::cerr << "invalid profile: " << error << '\n'; return 2; }
        const auto is_draft = profile.contains("provenance") && profile.at("provenance").value("status", "") != "approved";
        std::cout << "profile structure valid: " << profile.at("id").get<std::string>() << '\n';
        if (is_draft) {
            std::cout << "runtime readiness: draft only; complete live layout, ECS mapping, and function-semantic review first\n";
            return 3;
        }
        std::cout << "runtime readiness: approved status\n";
        return 0;
    } catch (const std::exception& exception) { std::cerr << "cannot validate profile: " << exception.what() << '\n'; return 2; }
}

int approve_profile_command(int argc, char** argv) {
    const auto scan_path = option(argc, argv, "--function-scan");
    const auto components_path = option(argc, argv, "--components");
    if (!scan_path || !components_path) { std::cerr << "approve-profile requires --function-scan and --components\n"; return 2; }
    try {
        auto profile = json::parse(std::ifstream(argv[2]));
        const auto scan = json::parse(std::ifstream(*scan_path));
        const auto catalog = json::parse(std::ifstream(*components_path));
        if (!profile.contains("provenance") || profile.at("provenance").value("status", "") != "draft-requires-live-review") {
            std::cerr << "input profile is not a generated draft\n"; return 2;
        }
        const auto capture_path = fs::path(profile.at("provenance").value("componentCapture", std::string{}));
        if (capture_path.empty() || !fs::exists(capture_path)) {
            std::cerr << "original live ECS capture report is missing; regenerate the profile draft\n"; return 2;
        }
        const auto live_capture = json::parse(std::ifstream(capture_path));
        if (profile.at("image").at("sha256") != scan.at("sha256") ||
            profile.at("image").at("timestamp") != scan.at("timestamp") ||
            profile.at("image").at("size") != scan.at("imageSize") ||
            profile.at("image").at("sha256") != catalog.at("image").at("sha256") ||
            profile.at("image").at("timestamp") != catalog.at("image").at("timestamp") ||
            profile.at("image").at("size") != catalog.at("image").at("size") ||
            profile.at("id") != catalog.at("id") || profile.at("target") != catalog.at("target")) {
            std::cerr << "profile, function scan, and component catalog must match by filename and executable SHA-256\n"; return 2;
        }
        if (!scan.value("allFunctionCandidatesUnique", false)) {
            std::cerr << "function scan contains missing, ambiguous, wrong-RVA, or byte-mismatched candidates\n"; return 3;
        }
        if (catalog.contains("unresolved") && !catalog.at("unresolved").empty()) {
            std::cerr << "component capture contains unresolved reflection joins\n"; return 3;
        }
        if (live_capture.at("image").at("sha256") != profile.at("image").at("sha256") ||
            live_capture.at("id") != profile.at("id") || live_capture.at("target") != profile.at("target") ||
            live_capture.at("components") != catalog.at("components") ||
            (live_capture.contains("unresolved") && !live_capture.at("unresolved").empty())) {
            std::cerr << "source live capture differs from the normalized component catalog or selected executable\n"; return 3;
        }
        if (profile.at("provenance").value("unresolvedComponentCount", std::size_t(-1)) != 0) {
            std::cerr << "profile provenance records unresolved component mappings\n"; return 3;
        }
        std::string validation_error;
        const auto& components = catalog.at("components");
        if (!validate_profile(profile, validation_error, &components)) {
            std::cerr << "profile structure invalid: " << validation_error << '\n'; return 2;
        }
        std::cout << "Profile: " << profile.at("id").get<std::string>() << '\n'
                  << "Image SHA-256: " << profile.at("image").at("sha256").get<std::string>() << '\n'
                  << "Function scan: " << fs::absolute(*scan_path).string() << '\n'
                  << "Component catalog: " << fs::absolute(*components_path).string() << '\n';
        std::string answer;
        std::cout << "Have you verified all layout offsets against a live session of this exact build? Type VERIFY: ";
        std::getline(std::cin, answer);
        if (answer != "VERIFY") { std::cerr << "profile remains a draft\n"; return 3; }
        std::cout << "Have you reviewed hook/function semantics, calling conventions, and side effects? Type VERIFY: ";
        std::getline(std::cin, answer);
        if (answer != "VERIFY") { std::cerr << "profile remains a draft\n"; return 3; }
        FILETIME now{};
        GetSystemTimeAsFileTime(&now);
        ULARGE_INTEGER ticks{};
        ticks.LowPart = now.dwLowDateTime;
        ticks.HighPart = now.dwHighDateTime;
        auto& provenance = profile["provenance"];
        provenance["status"] = "approved";
        provenance["runtimeLayoutValidated"] = true;
        provenance["componentMappingsValidated"] = true;
        provenance["functionSemanticsValidated"] = true;
        std::array<char, 256> username{};
        const auto username_size = GetEnvironmentVariableA("USERNAME", username.data(), static_cast<DWORD>(username.size()));
        provenance["reviewer"] = username_size > 0 && username_size < username.size()
            ? std::string(username.data(), username_size) : "local-developer";
        provenance["reviewedAtFileTimeUtc"] = ticks.QuadPart;
        provenance["reviewEvidence"] = json::array({fs::absolute(*scan_path).string(), fs::absolute(capture_path).string(), fs::absolute(*components_path).string()});
        if (!write_json(argv[2], profile)) return 2;
        std::cout << "profile approved and saved: " << fs::absolute(argv[2]).string() << '\n';
        return 0;
    } catch (const std::exception& exception) { std::cerr << "profile approval failed: " << exception.what() << '\n'; return 2; }
}

int generate_command(int argc, char** argv) {
    const fs::path base_path = argv[2], image_path = argv[3], scan_path = argv[4];
    const auto out_path = option(argc, argv, "--out");
    const auto components_path = option(argc, argv, "--components");
    const auto catalog_out = option(argc, argv, "--catalog-out");
    if (!out_path || !components_path) { std::cerr << "generate-profile requires --components and --out\n"; return 2; }
    try {
        auto profile = json::parse(std::ifstream(base_path));
        // A relocated signature does not prove calculation semantics in a new build.
        profile.erase("attributeCalculationModel");
        const auto image = json::parse(std::ifstream(image_path));
        const auto scan = json::parse(std::ifstream(scan_path));
        const auto observed_components = json::parse(std::ifstream(*components_path));
        if (image.value("sha256", std::string{}).empty() || image.value("sha256", std::string{}) != scan.value("sha256", std::string{})) {
            std::cerr << "image and function scan must refer to the same executable SHA-256\n"; return 2;
        }
        if (observed_components.at("image").value("sha256", std::string{}) != image.at("sha256").get<std::string>() ||
            observed_components.at("image").at("timestamp") != image.at("timestamp") ||
            observed_components.at("image").at("size") != image.at("imageSize")) {
            std::cerr << "component capture and image report do not refer to the same executable identity\n"; return 2;
        }
        profile["image"]["timestamp"] = image.at("timestamp");
        profile["image"]["size"] = image.at("imageSize");
        profile["image"]["sha256"] = image.at("sha256");
        profile["id"] = observed_components.at("id");
        profile["target"] = observed_components.at("target");
        for (const auto& row : scan.value("functions", json::array())) {
            if (row.value("matchCount", 0u) != 1 || row.at("matches").empty()) continue;
            const auto id = row.at("id").get<std::string>();
            const auto rva = row.at("matches").at(0).get<std::uint32_t>();
            if (id.starts_with("hook.")) {
                const auto hook_name = id.substr(5);
                if (profile.at("hooks").contains(hook_name)) {
                    profile["hooks"][hook_name]["signature"] = row.at("signature");
                    if (row.contains("original") && !row.at("original").empty())
                        profile["hooks"][hook_name]["original"] = row.at("original");
                }
            } else if (profile.at("worldOperations").contains(id)) {
                auto& operation = profile["worldOperations"][id];
                if (operation.contains("functionRva") && operation.contains("guardRva")) {
                    const auto old_function = operation.at("functionRva").get<std::int64_t>();
                    const auto old_guard = operation.at("guardRva").get<std::int64_t>();
                    operation["functionRva"] = static_cast<std::uint32_t>(static_cast<std::int64_t>(rva) - (old_guard - old_function));
                    operation["guardRva"] = rva;
                    if (row.contains("original") && !row.at("original").empty()) operation["guardBytes"] = row.at("original");
                }
            } else if (profile.at("runtimePatches").contains(id)) {
                auto& patch = profile["runtimePatches"][id];
                patch["signature"] = row.at("signature");
                const auto containing = std::find_if(scan.at("runtimeFunctions").begin(), scan.at("runtimeFunctions").end(),
                    [&](const json& function) { return function.at("beginRva").get<std::uint32_t>() <= rva &&
                        rva < function.at("endRva").get<std::uint32_t>(); });
                if (containing == scan.at("runtimeFunctions").end()) {
                    std::cerr << "patch signature is not inside a PE runtime function: " << id << '\n'; return 3;
                }
                auto& function = patch["function"];
                function["beginRva"] = containing->at("beginRva");
                function["endRva"] = containing->at("endRva");
                function["targetOffset"] = rva - containing->at("beginRva").get<std::uint32_t>();
            }
        }
        if (profile.value("componentResolution", std::string{}) != "live-registration")
            profile["components"] = observed_components.at("components");
        const json component_catalog = {{"schemaVersion", 1}, {"id", profile.at("id")}, {"target", profile.at("target")},
            {"image", profile.at("image")}, {"components", observed_components.at("components")}};
        profile["allowStructuralRevalidation"] = false;
        json provenance = {{"status", "draft-requires-live-review"}, {"generator", "kfc-runtime-dev"},
            {"executableSha256", image.at("sha256")}, {"imageReport", fs::absolute(image_path).string()},
            {"functionScan", fs::absolute(scan_path).string()}, {"componentCapture", fs::absolute(*components_path).string()},
            {"reflectionCatalog", observed_components.value("source", json::object()).value("reflection", std::string{})},
            {"allFunctionCandidatesUnique", scan.value("allFunctionCandidatesUnique", scan.value("allCandidatesUnique", false))},
            {"runtimeLayoutValidated", false}, {"componentMappingsValidated", false},
            {"functionSemanticsValidated", false},
            {"unresolvedComponentCount", observed_components.value("unresolved", json::array()).size()}};
        profile["provenance"] = std::move(provenance);
        if (catalog_out && !write_json(*catalog_out, component_catalog)) return 2;
        if (!write_json(*out_path, profile)) return 2;
        std::cout << "profile draft written (not installable until live layout, component mappings, and function semantics are reviewed): "
                  << fs::absolute(*out_path).string() << '\n';
        return 0;
    } catch (const std::exception& exception) { std::cerr << "profile generation failed: " << exception.what() << '\n'; return 2; }
}
}

int main(int argc, char** argv) {
    if (argc < 2 || std::string_view(argv[1]) == "--help" || std::string_view(argv[1]) == "help") { usage(); return argc < 2 ? 1 : 0; }
    const std::string_view command(argv[1]);
    if (command == "select-build" && argc >= 3) return select_build_command(argc, argv);
    if (command == "inspect" && argc >= 3) return inspect_command(argc, argv);
    if (command == "discover-functions" && argc >= 3) return discover_command(argc, argv);
    if (command == "extract-profile-functions" && argc >= 3) return extract_profile_functions_command(argc, argv);
    if (command == "scan-functions" && argc >= 4) return scan_command(argc, argv);
    if (command == "capture-process" && argc >= 3) return capture_process_command(argc, argv);
    if (command == "import-ecs-capture" && argc >= 3) return import_ecs_capture_command(argc, argv);
    if (command == "inspect-provider" && argc >= 3) return inspect_provider_command(argc, argv);
    if (command == "validate-profile" && argc >= 3) return validate_command(argv[2], option(argc, argv, "--components"));
    if (command == "approve-profile" && argc >= 3) return approve_profile_command(argc, argv);
    if (command == "generate-profile" && argc >= 5) return generate_command(argc, argv);
    usage();
    return 1;
}
