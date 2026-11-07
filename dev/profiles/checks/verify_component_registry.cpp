#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <tlhelp32.h>
#include "../../../modules/game-runtime/native/ecs/registry/read_component_registry.h"
#include "../../../modules/game-runtime/native/ecs/registry/find_component_registry.h"
#include "../../../modules/game-runtime/native/code-inventory/inspect_game_functions.h"
#include <nlohmann/json.hpp>
#include <fstream>
#include <iostream>
#include <unordered_map>

int main(int argc, char** argv) try {
    const bool discover = argc > 1 && std::string(argv[1]) == "--discover";
    const bool owner_only = argc > 1 && std::string(argv[1]) == "--owner";
    if ((!discover && !owner_only && argc != 4 && argc != 5) || ((discover || owner_only) && argc != 5)) {
        std::cerr << "usage: verify-component-registry PID manager-hex output.json [matching-profile.json]\n"
                     "       verify-component-registry --discover PID reflection.json output.json\n"
                     "       verify-component-registry --owner PID owner-hex output.json\n";
        return 2;
    }
    const auto pid = static_cast<DWORD>(std::stoul(argv[discover || owner_only ? 2 : 1]));
    const auto manager = discover || owner_only ? std::uintptr_t{} : static_cast<std::uintptr_t>(std::stoull(argv[2], nullptr, 16));
    const auto owner = owner_only ? static_cast<std::uintptr_t>(std::stoull(argv[3], nullptr, 16)) : std::uintptr_t{};
    const auto output_path = argv[discover || owner_only ? 4 : 3];
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
    nlohmann::json discovery_report;
    bool registry_valid{};
    if (discover) {
        const auto reflection = nlohmann::json::parse(std::ifstream(argv[3]));
        std::uint8_t header[4096]{};
        if (!read(reinterpret_cast<std::uintptr_t>(module.modBaseAddr), header, sizeof(header))) return 7;
        const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(header);
        if (dos->e_magic != IMAGE_DOS_SIGNATURE || dos->e_lfanew <= 0 ||
            static_cast<std::size_t>(dos->e_lfanew) > sizeof(header) - sizeof(IMAGE_NT_HEADERS64)) return 7;
        const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(header + dos->e_lfanew);
        if (nt->Signature != IMAGE_NT_SIGNATURE || reflection.at("image").at("size") != module.modBaseSize ||
            reflection.at("image").at("timestamp") != nt->FileHeader.TimeDateStamp) return 10;
        std::unordered_map<std::string, std::uint32_t> contract;
        for (const auto& type : reflection.at("types")) {
            const auto name = type.at("qualifiedName").get<std::string>();
            const auto size = type.at("size").get<std::uint32_t>();
            if (name.starts_with("keen::ecs::") && size) contract.emplace(name, size);
        }
        const ComponentRegistry::Discovery::Validator validate = [&](const ComponentRegistry::Snapshot& registry, std::string& reason) {
            for (const auto& entry : registry.entries) for (const auto* type : {&entry.storage, &entry.configuration}) {
                if (type->name.empty()) continue;
                const auto found = contract.find(type->name);
                if (found == contract.end() || found->second != type->size) {
                    reason = "engine registration disagrees with parser contract: " + type->name;
                    return false;
                }
            }
            return true;
        };
        ComponentRegistry::Discovery discovery;
        const auto started = GetTickCount64();
        auto last_progress = started;
        // The scan observes resident private pages. Its catalog agreement is
        // scoped to observed owners and never authorizes entity access.
        while (GetTickCount64() - started < 600000) {
            registry_valid = discovery.Tick(reinterpret_cast<std::uintptr_t>(module.modBaseAddr), module.modBaseSize,
                read, validate, snapshot, [&](std::uintptr_t address, MEMORY_BASIC_INFORMATION& memory) {
                    return VirtualQueryEx(process, reinterpret_cast<const void*>(address), &memory, sizeof(memory)) == sizeof(memory);
                }, std::chrono::milliseconds(250), [&](PSAPI_WORKING_SET_EX_INFORMATION* pages, std::size_t count) {
                    return count <= MAXDWORD / sizeof(*pages) &&
                        K32QueryWorkingSetEx(process, pages, static_cast<DWORD>(count * sizeof(*pages)));
                });
            const auto& progress = discovery.Status();
            if (registry_valid || (!progress.scanning && !progress.available)) break;
            if (GetTickCount64() - last_progress >= 10000) {
                std::cerr << "registry discovery: " << progress.scanned_bytes / (1024 * 1024)
                    << " MiB scanned, " << progress.completed_passes << " complete passes, "
                    << progress.reason << '\n';
                last_progress = GetTickCount64();
            }
            Sleep(1);
        }
        const auto& progress = discovery.Status();
        const bool timed_out = !registry_valid && progress.scanning && GetTickCount64() - started >= 600000;
        nlohmann::json candidates = nlohmann::json::array();
        const auto type_json = [](const ComponentRegistry::Type& type) {
            return nlohmann::json{{"qualified_name",type.name},{"qualified_hash",type.hash},
                {"size",type.size},{"metadata_rva",type.metadata_rva}};
        };
        for (const auto& candidate : discovery.CandidateSnapshots()) {
            nlohmann::json entries = nlohmann::json::array();
            for (const auto& entry : candidate.entries) {
                nlohmann::json callbacks = nlohmann::json::array();
                for (const auto& callback : entry.callbacks)
                    callbacks.push_back({{"slot_offset",callback.slot_offset},{"function_rva",callback.function_rva},
                        {"in_record",callback.in_record}});
                entries.push_back({{"index",entry.index},{"qualified_name",entry.name},{"qualified_hash",entry.hash},
                    {"runtime_size",entry.size},{"flags",entry.flags},{"storage_flags",entry.storage_flags},
                    {"runtime_type",type_json(entry.storage)},{"template_type",type_json(entry.configuration)},
                    {"callbacks",callbacks}});
            }
            candidates.push_back({{"owner",candidate.owner},{"records",candidate.records},{"entries",entries}});
        }
        discovery_report = {{"source","production-resident-private-memory-registry-discovery"},
            {"scope","resident-private-memory"},{"skippedNonresidentBytes",progress.skipped_nonresident_bytes},
            {"scannedBytes",progress.scanned_bytes},{"validationBytes",progress.validation_bytes},
            {"completedPasses",progress.completed_passes},{"distinctRegistries",progress.distinct_registries},
            {"distinctCatalogs",progress.distinct_catalogs},{"catalogCandidates",candidates},
            {"timedOut",timed_out},
            {"budgetLimitedSteps",progress.budget_limited_steps},{"durationMs",GetTickCount64() - started},
            {"reason",progress.reason},{"parserContractTypes",contract.size()},
            {"imageIdentityCheck","timestamp-and-image-size-only"},
            {"entityAccessAuthorized",false}};
        error = timed_out ? "external inspection time limit reached before a complete registry scan" : progress.reason;
    } else if (owner_only) {
        registry_valid = ComponentRegistry::ReadOwner(owner, reinterpret_cast<std::uintptr_t>(module.modBaseAddr),
            module.modBaseSize, read, snapshot, error);
    } else {
        registry_valid = ComponentRegistry::Read(manager, reinterpret_cast<std::uintptr_t>(module.modBaseAddr),
            module.modBaseSize, read, snapshot, error);
    }
    if (!registry_valid) {
        if (discover) std::ofstream(output_path) << nlohmann::json({{"available",false},{"discovery",discovery_report}}).dump(2) << '\n';
        std::cerr << error << '\n'; CloseHandle(process); return 5;
    }
    nlohmann::json rows = nlohmann::json::array();
    std::size_t runtime{}, template_only{}, callback_count{};
    const auto type_json = [](const ComponentRegistry::Type& type) -> nlohmann::json {
        if (type.name.empty()) return nullptr;
        return {{"qualified_name",type.name},{"qualified_hash",type.hash},
            {"size",type.size},{"metadata_rva",type.metadata_rva}};
    };
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
            {"runtime_type",type_json(entry.storage)},{"template_type",type_json(entry.configuration)},
            {"runtime_size",entry.size},
            {"size",entry.size}});
    }
    std::ofstream output(output_path);
    output << nlohmann::json({{"available",true},{"pid",pid},{"runtime",runtime},{"templateOnly",template_only},
        {"callbackPointers",callback_count},{"entries",rows},{"discovery",discovery_report},
        {"entityAccessAuthorized",false}}).dump(2) << '\n';
    std::cout << "Validated with the production registry reader: " << rows.size() << " registrations, "
        << runtime << " storage layouts, " << template_only << " template-only, " << callback_count << " callback pointers\n";
    // Fault injection affects local copies only. The game is opened VM_READ.
    if (!discover && !owner_only) for (unsigned mode = 0; mode < 3; ++mode) {
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
    if (!discover && !owner_only) std::cout << "Production reader rejected oversized counts, mismatched hashes, and changing readbacks\n";
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
    if (!discover && !owner_only && argc == 5) {
        using namespace KfcRuntimeCompatibility::EnshroudedClient;
        const auto profile = nlohmann::json::parse(std::ifstream(argv[4]));
        if (profile.at("image").at("size") != image_size || profile.at("image").at("timestamp") != image_timestamp) return 10;
        // The caller additionally verifies disk SHA256. This executable never loads
        // the profile into the game and never allocates or executes trampolines.
        exact_build_match = true;
        image_sha256 = profile.at("image").at("sha256");
        // Read-validated owner/resource profiles omit execution interventions.
        // Present entries retain the execution profile's required-field checks.
        if (const auto patches = profile.find("runtimePatches"); patches != profile.end()) {
            for (const auto& [name, entry] : patches->items()) {
                RuntimePatch patch{};
                patch.name = name;
                patch.modifier_json = entry.at("modifier").dump();
                patch.function_begin_rva = entry.at("function").at("beginRva");
                patch.target_offset = entry.at("function").at("targetOffset");
                runtime_patches.push_back(std::move(patch));
            }
        }
        if (const auto operations = profile.find("worldOperations"); operations != profile.end()) {
            for (const auto& [name, entry] : operations->items()) {
                RuntimeOperation operation{};
                operation.name = name;
                operation.function_rva = entry.value("functionRva", std::uintptr_t{});
                operation.abi = entry.at("abi");
                operation.thread = entry.at("thread");
                operation.context = entry.at("context");
                const auto guard = entry.value("guardBytes", std::vector<std::uint8_t>{});
                const auto rva = entry.value("guardRva", std::size_t{});
                operation.available = entry.value("validated", true) && !guard.empty() && rva < image.size() &&
                    guard.size() <= image.size() - rva && !std::memcmp(image.data() + rva, guard.data(), guard.size());
                operation.status = entry.value("validated", true) ? "instruction-guard-checked" :
                    "unresolved:" + entry.at("unresolvedReason").get<std::string>();
                runtime_operations.push_back(std::move(operation));
            }
        }
    }
    const auto functions = nlohmann::json::parse(FunctionInventory::Build(image.data(), base));
    std::ofstream function_output(std::string(output_path) + ".functions.json");
    function_output << functions.dump() << '\n';
    std::cout << "Production function reader: " << functions.at("count") << " candidates, " << functions.at("unwind_count")
        << " unwind groups, " << functions.at("pointer_target_count") << " pointer targets, " << functions.at("range_count") << " ranges\n";
    CloseHandle(process);
    return output ? 0 : 6;
} catch (const std::exception& error) {
    std::cerr << error.what() << '\n';
    return 11;
}
