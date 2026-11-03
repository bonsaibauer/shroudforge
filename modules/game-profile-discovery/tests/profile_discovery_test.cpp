// Isolated PE/cache fixtures. No game process, saved world, or engine call.
#include "game-profile-discovery/discover_game_profile.h"
#include "game-profile-discovery/verify_image_equivalence.h"
#include <windows.h>
#include <cstring>
#include <fstream>
#include <iostream>
#include <stdexcept>

namespace {
namespace fs = std::filesystem;
using json = nlohmann::json;
using namespace GameProfileDiscovery;
std::uint64_t fixture_number{};

void check(bool value, const char* description) {
    if (!value) throw std::runtime_error(description);
}

template <typename T> void put(std::vector<std::uint8_t>& bytes, std::size_t offset, const T& value) {
    check(offset + sizeof(T) <= bytes.size(), "fixture extent");
    std::memcpy(bytes.data() + offset, &value, sizeof(value));
}

void write(const fs::path& path, const std::vector<std::uint8_t>& bytes) {
    fs::create_directories(path.parent_path());
    std::ofstream stream(path, std::ios::binary | std::ios::trunc);
    stream.write(reinterpret_cast<const char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
    check(static_cast<bool>(stream), "fixture file write");
}

std::vector<std::uint8_t> pe(std::uint32_t timestamp) {
    std::vector<std::uint8_t> bytes(0x800);
    IMAGE_DOS_HEADER dos{};
    dos.e_magic = IMAGE_DOS_SIGNATURE;
    dos.e_lfanew = 0x80;
    put(bytes, 0, dos);
    IMAGE_NT_HEADERS64 nt{};
    nt.Signature = IMAGE_NT_SIGNATURE;
    nt.FileHeader.Machine = IMAGE_FILE_MACHINE_AMD64;
    nt.FileHeader.NumberOfSections = 2;
    nt.FileHeader.TimeDateStamp = timestamp;
    nt.FileHeader.SizeOfOptionalHeader = sizeof(IMAGE_OPTIONAL_HEADER64);
    nt.OptionalHeader.Magic = IMAGE_NT_OPTIONAL_HDR64_MAGIC;
    nt.OptionalHeader.ImageBase = 0x140000000;
    nt.OptionalHeader.AddressOfEntryPoint = 0x1000;
    nt.OptionalHeader.SectionAlignment = 0x1000;
    nt.OptionalHeader.FileAlignment = 0x200;
    nt.OptionalHeader.SizeOfImage = 0x3000;
    nt.OptionalHeader.SizeOfHeaders = 0x400;
    nt.OptionalHeader.NumberOfRvaAndSizes = IMAGE_NUMBEROF_DIRECTORY_ENTRIES;
    put(bytes, 0x80, nt);
    IMAGE_SECTION_HEADER code{};
    std::memcpy(code.Name, ".text", 5);
    code.Misc.VirtualSize = code.SizeOfRawData = 0x200;
    code.VirtualAddress = 0x1000;
    code.PointerToRawData = 0x400;
    code.Characteristics = IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_EXECUTE;
    put(bytes, 0x80 + sizeof(nt), code);
    IMAGE_SECTION_HEADER data{};
    std::memcpy(data.Name, ".rdata", 6);
    data.Misc.VirtualSize = data.SizeOfRawData = 0x200;
    data.VirtualAddress = 0x2000;
    data.PointerToRawData = 0x600;
    data.Characteristics = IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ;
    put(bytes, 0x80 + sizeof(nt) + sizeof(code), data);
    std::memset(bytes.data() + 0x400, 0xcc, 0x200);
    bytes[0x400] = 0xc3;
    nt.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION] = {0x2000, sizeof(RUNTIME_FUNCTION)};
    put(bytes, 0x80, nt);
    const RUNTIME_FUNCTION function{0x1000, 0x1001, 0x2040};
    put(bytes, 0x600, function);
    bytes[0x640] = 1;
    return bytes;
}

struct Fixture {
    fs::path root = fs::temp_directory_path() / (L"shroudforge-discovery-test-" +
        std::to_wstring(GetCurrentProcessId()) + L"-" + std::to_wstring(GetTickCount64()) +
        L"-" + std::to_wstring(++fixture_number));
    fs::path source_path = root / L"source" / L"enshrouded.exe";
    fs::path target_path = root / L"target" / L"enshrouded.exe";
    fs::path profiles = root / L"profiles";
    fs::path cache = root / L"cache";
    ImageIdentity source, target;
    json baseline;

    Fixture() {
        check(!fs::exists(root), "fixture directory is unique");
        set_source(pe(100));
        set_target(pe(200));
        baseline = {{"schemaVersion", 2}, {"id", "enshrouded-client-10"},
            {"target", source.target}, {"kfcFormat", 4},
            {"image", {{"sha256", source.sha256}, {"timestamp", source.timestamp}, {"size", source.size}}},
            {"ownerLayout", {{"rootObjectRva", 0x2100}, {"rootVtableRva", 0x2000}, {"ownerOffset", 24}}},
            {"resourceLayout", {{"recordOffset", 32}}},
            {"ecsWriteLayout", {{"enabled", true}, {"threadOffset", 48}}},
            {"provenance", {{"status", "read-validated"}, {"resourceReadValidated", true},
                {"ecsWriteGateValidated", true}, {"source", "baseline-evidence-remains-verbatim"}}}};
    }
    ~Fixture() {
        // Delete only the unique directory created by this fixture, never an
        // installation path or arbitrary caller-provided directory.
        if (root.parent_path() == fs::temp_directory_path()) {
            std::error_code ignored;
            fs::remove_all(root, ignored);
        }
    }
    void set_source(const std::vector<std::uint8_t>& bytes) {
        write(source_path, bytes);
        write(source_path.parent_path() / L"enshrouded.kfc", {'K', 'F', 'C', '4'});
        source = Detail::ReadImage(source_path).identity;
    }
    void set_target(const std::vector<std::uint8_t>& bytes) {
        write(target_path, bytes);
        auto kfc = target_path;
        kfc.replace_extension(L".kfc");
        write(kfc, {'K', 'F', 'C', '4'});
        target = Detail::ReadImage(target_path).identity;
    }
    Result run(const std::vector<json>& catalog = {}) {
        return Prepare(target_path, profiles, cache, catalog.empty() ? std::vector<json>{baseline} : catalog, target);
    }
    void cache_source() {
        const auto result = Prepare(source_path, profiles, cache, {baseline}, source);
        check(result.diagnostics.at("state") == "baseline-cached", "known exact image must cache baseline");
        check(!result.profile, "normal exact profile must win over beta generation");
    }
};

void normal_profile_precedence_and_cache() {
    Fixture fixture;
    auto beta = fixture.baseline;
    beta["id"] = "beta_enshrouded-client-10";
    const auto result = Prepare(fixture.source_path, fixture.profiles, fixture.cache,
        {beta, fixture.baseline}, fixture.source);
    check(result.diagnostics.at("state") == "baseline-cached", "normal exact beats beta");
    check(!result.profile, "normal exact must not generate beta");
    const auto cached = Detail::ReadImage(fixture.cache / L"game-profile-discovery" / (fixture.source.sha256 + ".exe"), false);
    check(cached.identity.sha256 == fixture.source.sha256, "cache bytes retain exact source hash");
    check(!fs::exists(fixture.profiles), "normal cache does not write profiles");
}

void equivalent_image_generates_complete_beta() {
    Fixture fixture;
    fixture.cache_source();
    auto target = pe(200);
    put<DWORD>(target, 0x80 + offsetof(IMAGE_NT_HEADERS64, OptionalHeader) +
        offsetof(IMAGE_OPTIONAL_HEADER64, CheckSum), 123456);
    target.resize(0x810);
    IMAGE_DATA_DIRECTORY certificate{0x800, 16};
    put(target, 0x80 + offsetof(IMAGE_NT_HEADERS64, OptionalHeader) +
        offsetof(IMAGE_OPTIONAL_HEADER64, DataDirectory) + IMAGE_DIRECTORY_ENTRY_SECURITY * sizeof(IMAGE_DATA_DIRECTORY), certificate);
    put<DWORD>(target, 0x800, 16);
    put<WORD>(target, 0x804, 0x200);
    put<WORD>(target, 0x806, 2);
    fixture.set_target(target);
    const auto result = fixture.run();
    check(result.profile.has_value(), "identical mapped image generates beta");
    const auto progress_path = fixture.cache / L"game-profile-discovery" /
        ("status-" + std::to_string(GetCurrentProcessId()) + ".json");
    const auto progress = json::parse(std::ifstream(progress_path));
    check(progress.at("state") == "finished", "completed analysis leaves no active searching status");
    check(progress.at("pid") == GetCurrentProcessId() && progress.at("image").at("sha256") == fixture.target.sha256,
        "search progress belongs to the actual process and executable");
    check(result.diagnostics.at("analysisMemoHit") == false, "first analysis is not memoized");
    check(result.diagnostics.at("state") == "beta-generated", "beta is newly published");
    auto expected = fixture.baseline;
    expected["id"] = "beta_enshrouded-client-" + fixture.target.sha256;
    expected["image"]["sha256"] = fixture.target.sha256;
    expected["image"]["timestamp"] = fixture.target.timestamp;
    check(*result.profile == expected, "every feature, numeric contract and provenance is preserved");
    check(result.profile_path.filename().string().starts_with("beta_"), "beta filename is explicit");
    check(fs::exists(result.profile_path), "beta profile is published");
    const auto proof_path = fixture.cache / L"game-profile-discovery" / (fixture.target.sha256 + ".v2.proof.json");
    check(fs::exists(proof_path), "proof is retained");
    const auto proof = json::parse(std::ifstream(proof_path));
    const auto repeated = fixture.run();
    check(repeated.profile == result.profile && repeated.diagnostics.at("state") == "beta-present", "repeated generation is idempotent");
    check(repeated.diagnostics.at("analysisMemoHit") == true, "repeated analysis uses the process success memo");
    check(json::parse(std::ifstream(proof_path)) == proof, "memo reuse leaves the deterministic proof unchanged");
    const auto normalized = Prepare(fixture.target_path.parent_path() / L"." / fixture.target_path.filename(),
        fixture.profiles / L".", fixture.cache / L".", {fixture.baseline}, fixture.target);
    check(normalized.profile == result.profile && normalized.diagnostics.at("analysisMemoHit") == true,
        "equivalent absolute normalized locations share their successful analysis");
    check(Detail::ReadImage(fixture.source_path).identity.sha256 == fixture.source.sha256, "source executable is unchanged");
}

void unreferenced_compiler_padding_change_generates_beta() {
    Fixture fixture;
    fixture.cache_source();
    auto target = pe(200);
    target[0x410] = 0x90;
    fixture.set_target(target);
    const auto result = fixture.run();
    check(result.profile.has_value(), "unreferenced compiler padding change generates beta");
    check(result.diagnostics.at("state") == "beta-generated", "padding-only target is published as beta");
    const auto proof_path = fixture.cache / L"game-profile-discovery" / (fixture.target.sha256 + ".v2.proof.json");
    const auto proof = json::parse(std::ifstream(proof_path));
    const auto& padding = proof.at("comparison").at("paddingEquivalence");
    check(padding.at("equivalent") == true && padding.at("changedBytes") == 1 &&
        padding.at("verifiedProcedures") == 1, "padding proof identifies exact bytes and code inventory");
    auto expected = fixture.baseline;
    expected["id"] = "beta_enshrouded-client-" + fixture.target.sha256;
    expected["image"]["sha256"] = fixture.target.sha256;
    expected["image"]["timestamp"] = fixture.target.timestamp;
    check(*result.profile == expected, "padding-only beta preserves every profile field except identity");
}

void memo_keys_cover_the_complete_baseline_and_locations() {
    Fixture fixture;
    fixture.cache_source();
    const auto first = fixture.run();
    check(first.profile.has_value(), "prepare successful memo key fixture");
    auto changed_baseline = fixture.baseline;
    changed_baseline["ownerLayout"]["ownerOffset"] = 25;
    const auto changed = fixture.run({changed_baseline});
    check(!changed.profile && changed.diagnostics.at("analysisMemoHit") == false &&
        changed.diagnostics.at("reason") == "existing-artifact-content-conflict",
        "different baseline contracts are reanalyzed and cannot reuse an earlier proof");

    const auto alternate_target = fixture.root / L"alternate-target" / L"enshrouded.exe";
    write(alternate_target, pe(200));
    write(alternate_target.parent_path() / L"enshrouded.kfc", {'K', 'F', 'C', '4'});
    const auto target = Prepare(alternate_target, fixture.profiles, fixture.cache, {fixture.baseline}, fixture.target);
    check(target.profile == first.profile && target.diagnostics.at("analysisMemoHit") == false,
        "equal executable identities at different target paths have separate memo entries");
    const auto profiles = Prepare(fixture.target_path, fixture.root / L"alternate-profiles",
        fixture.cache, {fixture.baseline}, fixture.target);
    check(profiles.profile == first.profile && profiles.diagnostics.at("analysisMemoHit") == false,
        "different profile locations have separate memo entries");
    const auto alternate_cache = fixture.root / L"alternate-cache";
    write(alternate_cache / L"game-profile-discovery" / (fixture.source.sha256 + ".exe"), pe(100));
    const auto cache = Prepare(fixture.target_path, fixture.profiles, alternate_cache, {fixture.baseline}, fixture.target);
    check(cache.profile == first.profile && cache.diagnostics.at("analysisMemoHit") == false,
        "different source/cache locations have separate memo entries");
    check(fixture.run().diagnostics.at("analysisMemoHit") == true, "separate entries preserve the original successful analysis");
}

void memo_never_trusts_changed_files_or_failed_publication() {
    Fixture fixture;
    fixture.cache_source();
    const auto proof_path = fixture.cache / L"game-profile-discovery" / (fixture.target.sha256 + ".v2.proof.json");
    write(proof_path, {'{', '}'});
    const auto conflict = fixture.run();
    check(!conflict.profile && conflict.diagnostics.at("reason") == "existing-artifact-content-conflict",
        "pre-existing proof conflicts remain errors");
    fs::remove(proof_path);
    const auto first = fixture.run();
    check(first.profile.has_value() && first.diagnostics.at("analysisMemoHit") == false,
        "failed publication never populates the success memo");

    auto damaged = pe(100);
    damaged[0x420] ^= 1;
    const auto cached_source = fixture.cache / L"game-profile-discovery" / (fixture.source.sha256 + ".exe");
    write(cached_source, damaged);
    check(fixture.run().diagnostics.at("reason") == "cached-baseline-image-identity-mismatch",
        "memo hits still read and hash the actual source file");
    write(cached_source, pe(100));
    damaged = pe(200);
    damaged[0x420] ^= 1;
    write(fixture.target_path, damaged);
    check(fixture.run().diagnostics.at("reason") == "current-executable-identity-changed",
        "memo hits still read and hash the actual target file");
    write(fixture.target_path, pe(200));
    write(proof_path, {'{', '}'});
    const auto reused = fixture.run();
    check(!reused.profile && reused.diagnostics.at("analysisMemoHit") == true &&
        reused.diagnostics.at("reason") == "existing-artifact-content-conflict",
        "successful analysis reuse still verifies the on-disk proof before publication");
    check(json::parse(std::ifstream(proof_path)).empty(), "conflicting proof content is preserved");
}

void memo_is_bounded_and_unicode_paths_are_preserved() {
    Fixture fixture;
    const auto unicode = fixture.root / fs::path(u8"paths-\u00e4-\u4e16\u754c");
    fixture.source_path = unicode / L"source" / L"enshrouded.exe";
    fixture.target_path = unicode / L"target" / L"enshrouded.exe";
    fixture.profiles = unicode / L"profiles";
    fixture.cache = unicode / L"cache";
    fixture.set_source(pe(100));
    fixture.set_target(pe(200));
    fixture.cache_source();
    const auto first = fixture.run();
    check(first.profile.has_value(), "Unicode executable, profile and cache paths are supported");
    const auto encoded = first.profile_path.generic_u8string();
    check(first.diagnostics.at("profilePath") == std::string(encoded.begin(), encoded.end()),
        "diagnostic paths retain their complete UTF-8 spelling");
    check(fixture.run().diagnostics.at("analysisMemoHit") == true, "Unicode paths participate in memo hits");
    for (unsigned index = 0; index != 8; ++index) {
        const auto profiles = unicode / (L"other-profiles-" + std::to_wstring(index));
        const auto other = Prepare(fixture.target_path, profiles, fixture.cache, {fixture.baseline}, fixture.target);
        check(other.profile == first.profile && other.diagnostics.at("analysisMemoHit") == false,
            "new locations have distinct successful analyses");
    }
    const auto evicted = fixture.run();
    check(evicted.profile == first.profile && evicted.diagnostics.at("analysisMemoHit") == false,
        "the process memo retains at most eight successful analyses");
}

void changed_code_and_data_are_unresolved() {
    for (const auto offset : {0x410u, 0x610u}) {
        Fixture fixture;
        fixture.cache_source();
        auto target = pe(200);
        target[offset] ^= 1;
        fixture.set_target(target);
        const auto result = fixture.run();
        check(!result.profile, "changed section must not produce beta");
        check(result.diagnostics.at("reason") == "image-equivalence-not-established", "changed image has explicit reason");
        check(result.diagnostics.at("comparison").at("reason") ==
            (offset == 0x410 ? "executable-section-changed" : "data-section-changed"), "code and data changes are distinguished");
        const auto report_text = result.diagnostics.at("evidencePath").get<std::string>();
        const auto report_path = fs::path(std::u8string(report_text.begin(), report_text.end()));
        check(json::parse(std::ifstream(report_path)).at("rebase").at("complete") == false,
            "complete unresolved evidence is retained outside the profile catalog");
        check(!fs::exists(fixture.profiles), "unresolved image writes no profile");
    }
}

void changed_loader_contract_is_unresolved() {
    Fixture fixture;
    fixture.cache_source();
    auto target = pe(200);
    put<DWORD>(target, 0x80 + offsetof(IMAGE_NT_HEADERS64, OptionalHeader) +
        offsetof(IMAGE_OPTIONAL_HEADER64, AddressOfEntryPoint), 0x1010);
    fixture.set_target(target);
    const auto result = fixture.run();
    check(!result.profile && result.diagnostics.at("comparison").at("reason") == "mapped-pe-header-layout-changed",
        "loader contract changes are not normalized away");
}

void absent_and_corrupt_cache_are_not_trusted() {
    Fixture fixture;
    auto result = fixture.run();
    check(!result.profile && result.diagnostics.at("reason") == "exact-baseline-executable-not-cached", "missing source requires evidence");
    fixture.cache_source();
    auto damaged = pe(100);
    damaged[0x420] ^= 1;
    write(fixture.cache / L"game-profile-discovery" / (fixture.source.sha256 + ".exe"), damaged);
    result = fixture.run();
    check(!result.profile && result.diagnostics.at("reason") == "cached-baseline-image-identity-mismatch", "corrupt cached source is rejected");
}

void process_targets_and_beta_sources_are_separate() {
    Fixture fixture;
    fixture.cache_source();
    fixture.target_path = fixture.root / L"server" / L"enshrouded_server.exe";
    fixture.set_target(pe(200));
    auto result = fixture.run();
    check(!result.profile && result.diagnostics.at("reason") == "no-normal-same-target-kfc-baseline", "client cannot authorize server");
    fixture.target_path = fixture.root / L"target" / L"enshrouded.exe";
    fixture.set_target(pe(200));
    auto beta = fixture.baseline;
    beta["id"] = "beta_enshrouded-client-10";
    result = fixture.run({beta});
    check(!result.profile && result.diagnostics.at("reason") == "no-normal-same-target-kfc-baseline", "beta is never a normal source");
}

void ambiguity_does_not_select_an_arbitrary_baseline() {
    Fixture fixture;
    auto duplicate = fixture.baseline;
    duplicate["id"] = "enshrouded-client-11";
    auto result = Prepare(fixture.source_path, fixture.profiles, fixture.cache,
        {fixture.baseline, duplicate}, fixture.source);
    check(!result.profile && result.diagnostics.at("reason") == "normal-exact-profile-ambiguous", "multiple exact sources are rejected");
    duplicate = fixture.baseline;
    result = fixture.run({fixture.baseline, duplicate});
    check(!result.profile && result.diagnostics.at("reason") == "latest-normal-baseline-ambiguous", "duplicate latest builds are rejected");
}

void execution_approval_is_not_fabricated() {
    Fixture fixture;
    fixture.baseline["provenance"]["status"] = "approved";
    fixture.cache_source();
    const auto result = fixture.run();
    check(!result.profile && result.diagnostics.at("reason") == "automatic-execution-profile-approval-not-supported",
        "approved execution attestations are never fabricated");
}

void existing_beta_is_never_overwritten_with_different_content() {
    Fixture fixture;
    fixture.cache_source();
    const auto generated = fixture.run();
    check(generated.profile.has_value(), "prepare beta conflict fixture");
    std::ofstream(generated.profile_path, std::ios::trunc) << "{\"user-owned\":true}\n";
    const auto result = fixture.run();
    check(!result.profile && result.diagnostics.at("reason") == "existing-artifact-content-conflict", "existing beta content conflict is preserved");
    const auto document = json::parse(std::ifstream(generated.profile_path));
    check(document.at("user-owned") == true, "existing beta was not overwritten");
}

void changed_current_identity_is_not_cached() {
    Fixture fixture;
    auto wrong = fixture.source;
    wrong.sha256[0] = wrong.sha256[0] == 'a' ? 'b' : 'a';
    const auto result = Prepare(fixture.source_path, fixture.profiles, fixture.cache, {fixture.baseline}, wrong);
    check(!result.profile && result.diagnostics.at("reason") == "current-executable-identity-changed", "caller identity is checked against real file");
    check(!fs::exists(fixture.cache), "mismatched current image writes no cache");
}

void malformed_certificate_extent_is_rejected() {
    Fixture fixture;
    fixture.cache_source();
    auto target = pe(200);
    IMAGE_DATA_DIRECTORY certificate{0x400, 16};
    put(target, 0x80 + offsetof(IMAGE_NT_HEADERS64, OptionalHeader) +
        offsetof(IMAGE_OPTIONAL_HEADER64, DataDirectory) + IMAGE_DIRECTORY_ENTRY_SECURITY * sizeof(IMAGE_DATA_DIRECTORY), certificate);
    write(fixture.target_path, target);
    const auto result = fixture.run();
    check(!result.profile && result.diagnostics.at("reason") == "authenticode-overlaps-mapped-section",
        "security-directory normalization must not hide mapped content");
}
}

int main() {
    try {
        normal_profile_precedence_and_cache();
        equivalent_image_generates_complete_beta();
        unreferenced_compiler_padding_change_generates_beta();
        memo_keys_cover_the_complete_baseline_and_locations();
        memo_never_trusts_changed_files_or_failed_publication();
        memo_is_bounded_and_unicode_paths_are_preserved();
        changed_code_and_data_are_unresolved();
        changed_loader_contract_is_unresolved();
        absent_and_corrupt_cache_are_not_trusted();
        process_targets_and_beta_sources_are_separate();
        ambiguity_does_not_select_an_arbitrary_baseline();
        execution_approval_is_not_fabricated();
        existing_beta_is_never_overwritten_with_different_content();
        changed_current_identity_is_not_cached();
        malformed_certificate_extent_is_rejected();
        std::cout << "Game profile discovery fixture checks passed\n";
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "Game profile discovery fixture failed: " << error.what() << '\n';
        return 1;
    }
}
