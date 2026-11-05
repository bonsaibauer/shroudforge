#include "game-profile-discovery/rebase_profile_contract.h"
#include <windows.h>
#include <bcrypt.h>
#include <algorithm>
#include <fstream>
#include <iostream>
#include <map>
#include <stdexcept>
#include <cwctype>
#include <vector>

namespace {
namespace fs = std::filesystem;
using json = nlohmann::json;
using namespace GameProfileDiscovery::Detail;

json identity_json(const GameProfileDiscovery::ImageIdentity& identity) {
    return {{"target", identity.target}, {"sha256", identity.sha256},
        {"timestamp", identity.timestamp}, {"size", identity.size}, {"kfcFormat", identity.kfc_format}};
}

json read_json(const fs::path& path) {
    std::ifstream stream(path, std::ios::binary);
    if (!stream) throw std::runtime_error("cannot-open-profile");
    return json::parse(stream);
}

std::string sha256_file(const fs::path& path) {
    std::ifstream stream(path, std::ios::binary | std::ios::ate);
    if (!stream) throw std::runtime_error("live-evidence-file-unreadable");
    const auto length = stream.tellg();
    if (length < 0 || length > (1ll << 30)) throw std::runtime_error("live-evidence-file-size-invalid");
    std::vector<std::uint8_t> bytes(static_cast<std::size_t>(length));
    stream.seekg(0);
    if (length && !stream.read(reinterpret_cast<char*>(bytes.data()), length))
        throw std::runtime_error("live-evidence-file-read-incomplete");

    BCRYPT_ALG_HANDLE algorithm{};
    BCRYPT_HASH_HANDLE hash{};
    DWORD object_size{}, digest_size{}, received{};
    if (BCryptOpenAlgorithmProvider(&algorithm, BCRYPT_SHA256_ALGORITHM, nullptr, 0) < 0)
        throw std::runtime_error("sha256-provider-unavailable");
    const auto close = [&] {
        if (hash) BCryptDestroyHash(hash);
        if (algorithm) BCryptCloseAlgorithmProvider(algorithm, 0);
    };
    if (BCryptGetProperty(algorithm, BCRYPT_OBJECT_LENGTH, reinterpret_cast<PUCHAR>(&object_size),
            sizeof(object_size), &received, 0) < 0 ||
        BCryptGetProperty(algorithm, BCRYPT_HASH_LENGTH, reinterpret_cast<PUCHAR>(&digest_size),
            sizeof(digest_size), &received, 0) < 0 || digest_size != 32) {
        close();
        throw std::runtime_error("sha256-provider-properties-invalid");
    }
    std::vector<std::uint8_t> object(object_size), digest(digest_size);
    if (BCryptCreateHash(algorithm, &hash, object.data(), object_size, nullptr, 0, 0) < 0 ||
        BCryptHashData(hash, bytes.data(), static_cast<ULONG>(bytes.size()), 0) < 0 ||
        BCryptFinishHash(hash, digest.data(), digest_size, 0) < 0) {
        close();
        throw std::runtime_error("sha256-failed");
    }
    close();
    constexpr char digits[] = "0123456789abcdef";
    std::string result;
    for (const auto byte : digest) {
        result.push_back(digits[byte >> 4]);
        result.push_back(digits[byte & 15]);
    }
    return result;
}

std::wstring normalized_path(const fs::path& value) {
    auto path = fs::absolute(value).lexically_normal().wstring();
    std::replace(path.begin(), path.end(), L'/', L'\\');
    if (path.starts_with(L"\\\\?\\")) path.erase(0, 4);
    std::transform(path.begin(), path.end(), path.begin(), towlower);
    return path;
}

void apply_live_evidence(const fs::path& executable_path, const fs::path& baseline_path,
                         const json& baseline, const GameProfileDiscovery::ImageIdentity& source,
                         const GameProfileDiscovery::ImageIdentity& target,
                         const fs::path& evidence_path, json& profile, json& evidence) {
    const auto live = read_json(evidence_path);
    if (live.value("schemaVersion", 0) != 1 || live.value("complete", false) != true ||
        live.value("method", std::string{}) != "read-only-exact-image-live-profile-layout-validation")
        throw std::runtime_error("live-layout-evidence-incomplete-or-unsupported");
    if (!live.contains("pid") || !live.at("pid").is_number_unsigned() || !live.at("pid").get<std::uint32_t>() ||
        !live.contains("target") || live.at("target") != identity_json(target) ||
        !live.contains("baseline") || live.at("baseline").value("id", std::string{}) != baseline.at("id").get<std::string>() ||
        live.at("baseline").value("sha256", std::string{}) != sha256_file(baseline_path))
        throw std::runtime_error("live-layout-evidence-image-or-baseline-mismatch");
    if (!live.contains("executablePath") || !live.at("executablePath").is_string() ||
        normalized_path(live.at("executablePath").get<std::string>()) != normalized_path(executable_path))
        throw std::runtime_error("live-layout-evidence-executable-path-mismatch");
    if (!live.contains("staticReport") || !live.at("staticReport").is_object())
        throw std::runtime_error("live-layout-evidence-static-report-reference-missing");
    const fs::path static_path = live.at("staticReport").at("path").get<std::string>();
    if (sha256_file(static_path) != live.at("staticReport").at("sha256").get<std::string>())
        throw std::runtime_error("live-layout-evidence-static-report-hash-mismatch");
    const auto static_report = read_json(static_path);
    if (static_report.value("complete", false) || static_report.at("target") != identity_json(target) ||
        static_report.at("source") != identity_json(source))
        throw std::runtime_error("live-layout-evidence-static-report-identity-mismatch");

    std::map<std::string, json> unresolved;
    for (const auto& field : evidence.at("unresolved")) {
        if (!field.is_object() || !field.contains("path") || !field.at("path").is_string() || !field.contains("after"))
            throw std::runtime_error("static-unresolved-field-has-no-concrete-candidate");
        const auto path = field.at("path").get<std::string>();
        if (!unresolved.emplace(path, field).second)
            throw std::runtime_error("static-unresolved-field-duplicated");
    }
    std::map<std::string, json> live_fields;
    for (const auto& field : live.at("fields")) {
        if (!field.is_object() || !field.contains("path") || !field.at("path").is_string() ||
            !field.contains("value") || field.value("verified", false) != true ||
            !field.contains("observations") || !field.at("observations").is_object())
            throw std::runtime_error("live-layout-field-record-invalid");
        const auto path = field.at("path").get<std::string>();
        if (!live_fields.emplace(path, field).second)
            throw std::runtime_error("live-layout-field-duplicated");
    }
    if (unresolved.empty() || unresolved.size() != live_fields.size())
        throw std::runtime_error("live-layout-field-coverage-mismatch");

    for (const auto& [path, static_field] : unresolved) {
        const auto found = live_fields.find(path);
        if (found == live_fields.end() || found->second.at("value") != static_field.at("after"))
            throw std::runtime_error("live-layout-field-value-or-path-mismatch");
        profile[json::json_pointer(path)] = found->second.at("value");
        auto field = found->second;
        field["before"] = static_field.value("before", static_field.at("after"));
        field["after"] = found->second.at("value");
        field["role"] = "live-layout";
        field["source"] = "exact-process-live-layout-validation";
        evidence["fields"].push_back(std::move(field));
    }
    evidence["staticComplete"] = false;
    evidence["liveLayoutProof"] = {{"method", live.at("method")}, {"pid", live.at("pid")},
        {"capturedAt", live.at("capturedAt")}, {"executablePath", live.at("executablePath")},
        {"staticReport", live.at("staticReport")}, {"fieldCount", live_fields.size()},
        {"limitations", live.value("limitations", json::array())}};
    evidence["unresolved"] = json::array();
    evidence["complete"] = true;
    evidence["reason"] = "static-address-contract-and-exact-process-live-layouts-verified";
}

void write_new_report(const fs::path& path, const json& report) {
    if (fs::exists(path)) throw std::runtime_error("report-path-already-exists");
    if (!path.parent_path().empty()) fs::create_directories(path.parent_path());
    auto temporary = path;
    temporary += L".tmp";
    if (fs::exists(temporary)) throw std::runtime_error("temporary-report-path-already-exists");
    {
        std::ofstream stream(temporary, std::ios::binary | std::ios::trunc);
        if (!stream) throw std::runtime_error("cannot-create-report");
        const auto bytes = report.dump(2) + "\n";
        stream.write(bytes.data(), static_cast<std::streamsize>(bytes.size()));
        stream.flush();
        if (!stream) throw std::runtime_error("cannot-write-report");
    }
    fs::rename(temporary, path);
}
}

int main(int argc, char** argv) try {
    if (argc != 6 && argc != 8) {
        std::cerr << "usage: kfc-runtime-profile-rebase-report <cached-source.exe> <target.exe> "
                     "<baseline-profile.json> <candidate-id> <report.json> [--live-fields live-fields.json]\n";
        return 64;
    }
    const fs::path live_fields_path = argc == 8 ? fs::path(argv[7]) : fs::path{};
    if (argc == 8 && std::string(argv[6]) != "--live-fields")
        throw std::runtime_error("unknown-profile-rebase-report-option");

    const fs::path source_path = argv[1];
    const fs::path target_path = argv[2];
    const fs::path profile_path = argv[3];
    const std::string candidate_id = argv[4];
    const fs::path report_path = argv[5];
    const auto baseline = read_json(profile_path);
    if (!baseline.contains("image") || !baseline.contains("target") || !baseline.contains("kfcFormat"))
        throw std::runtime_error("baseline-profile-identity-incomplete");

    auto source = ReadImage(source_path, false);
    const auto target = ReadImage(target_path, true);
    const auto& baseline_image = baseline.at("image");
    if (source.identity.sha256 != baseline_image.at("sha256").get<std::string>() ||
        source.identity.timestamp != baseline_image.at("timestamp").get<std::uint32_t>() ||
        source.identity.size != baseline_image.at("size").get<std::uint32_t>())
        throw std::runtime_error("cached-source-does-not-match-baseline-profile");
    source.identity.target = baseline.at("target").get<std::string>();
    if (target.identity.target != source.identity.target || target.identity.kfc_format == 0 ||
        target.identity.kfc_format != baseline.at("kfcFormat").get<std::uint8_t>())
        throw std::runtime_error("target-executable-or-kfc-format-does-not-match-baseline");

    json rebased, evidence;
    const bool static_complete = RebaseProfile(source, target, baseline, rebased, evidence);
    bool complete = static_complete;
    if (!complete && !live_fields_path.empty()) {
        const auto static_evidence = evidence;
        apply_live_evidence(target_path, profile_path, baseline, source.identity,
            target.identity, live_fields_path, rebased, evidence);
        complete = evidence.value("complete", false);
        evidence["staticEvidence"] = static_evidence;
    }
    if (complete) {
        const auto expected_prefix = baseline.at("target").get<std::string>() == "enshrouded.exe" ?
            "enshrouded-client-" : "enshrouded-server-";
        if (!candidate_id.starts_with(expected_prefix))
            throw std::runtime_error("candidate-id-does-not-match-executable-target");
        rebased["id"] = candidate_id;
        rebased["image"]["sha256"] = target.identity.sha256;
        rebased["image"]["timestamp"] = target.identity.timestamp;
        rebased["image"]["size"] = target.identity.size;
    }

    json report{{"schemaVersion", 1}, {"complete", complete}, {"staticComplete", static_complete},
        {"source", identity_json(source.identity)}, {"target", identity_json(target.identity)},
        {"evidence", std::move(evidence)}};
    if (complete) report["candidateProfile"] = std::move(rebased);
    write_new_report(report_path, report);
    std::cout << (complete ? "profile contract resolved" : "profile contract unresolved")
              << "; report: " << report_path.string() << '\n';
    return complete ? 0 : 2;
} catch (const std::exception& error) {
    std::cerr << "profile rebase report failed: " << error.what() << '\n';
    return 1;
}
