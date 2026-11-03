#include "discover_game_profile.h"
#include "verify_image_equivalence.h"
#include "rebase_profile_contract.h"
#include <windows.h>
#include <algorithm>
#include <charconv>
#include <chrono>
#include <condition_variable>
#include <fstream>
#include <mutex>
#include <set>
#include <stdexcept>
#include <thread>

namespace GameProfileDiscovery {
namespace {
using json = nlohmann::json;
namespace fs = std::filesystem;

std::string to_utf8(const fs::path& path) {
    const auto encoded = path.generic_u8string();
    return {encoded.begin(), encoded.end()};
}

bool valid_sha(const std::string& value) {
    return value.size() == 64 && std::all_of(value.begin(), value.end(), [](char c) {
        return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f');
    });
}

json image_json(const ImageIdentity& identity) {
    return {{"target", identity.target}, {"sha256", identity.sha256},
        {"timestamp", identity.timestamp}, {"size", identity.size}};
}

struct SuccessfulAnalysis {
    json generated;
    json comparison;
};

struct MemoEntry {
    std::string key;
    SuccessfulAnalysis analysis;
};

std::mutex analysis_memo_mutex;
std::vector<MemoEntry> analysis_memo;
constexpr std::size_t maximum_memo_entries = 8;

std::string analysis_key(const json& baseline, const Detail::Image& source,
                         const Detail::Image& target, const fs::path& source_path,
                         const fs::path& target_path, const fs::path& profile_directory,
                         const fs::path& cache_directory) {
    const auto normalized = [](const fs::path& path) {
        auto absolute = fs::absolute(path).lexically_normal();
        if (absolute.has_relative_path() && absolute.filename().empty()) absolute = absolute.parent_path();
        return to_utf8(absolute);
    };
    return json{{"baseline", baseline.dump()},
        {"source", image_json(source.identity)}, {"sourceKfcFormat", source.identity.kfc_format},
        {"target", image_json(target.identity)}, {"targetKfcFormat", target.identity.kfc_format},
        {"sourcePath", normalized(source_path)}, {"targetPath", normalized(target_path)},
        {"profileDirectory", normalized(profile_directory)},
        {"cacheDirectory", normalized(cache_directory)}}.dump();
}

std::optional<SuccessfulAnalysis> recalled_analysis(const std::string& key) {
    std::scoped_lock lock(analysis_memo_mutex);
    for (const auto& entry : analysis_memo)
        if (entry.key == key) return entry.analysis;
    return std::nullopt;
}

void remember_analysis(std::string key, SuccessfulAnalysis analysis) {
    std::scoped_lock lock(analysis_memo_mutex);
    for (const auto& entry : analysis_memo)
        if (entry.key == key) return;
    if (analysis_memo.size() == maximum_memo_entries) analysis_memo.erase(analysis_memo.begin());
    analysis_memo.push_back({std::move(key), std::move(analysis)});
}

bool matches(const json& profile, const ImageIdentity& identity) {
    try {
        const auto& image = profile.at("image");
        return profile.at("target") == identity.target && profile.at("kfcFormat") == identity.kfc_format &&
            image.at("sha256") == identity.sha256 && image.at("timestamp") == identity.timestamp &&
            image.at("size") == identity.size;
    } catch (const json::exception&) { return false; }
}

bool same_identity(const ImageIdentity& left, const ImageIdentity& right) {
    return left.target == right.target && left.sha256 == right.sha256 && left.timestamp == right.timestamp &&
        left.size == right.size && left.kfc_format == right.kfc_format;
}

std::optional<std::uint64_t> build_number(const json& profile, const ImageIdentity& identity) {
    try {
        if (profile.at("target") != identity.target || profile.at("kfcFormat") != identity.kfc_format ||
            !valid_sha(profile.at("image").at("sha256").get<std::string>())) return std::nullopt;
        const auto id = profile.at("id").get<std::string>();
        const auto prefix = identity.target == "enshrouded.exe" ? "enshrouded-client-" : "enshrouded-server-";
        if (!id.starts_with(prefix)) return std::nullopt;
        const auto suffix = std::string_view(id).substr(std::char_traits<char>::length(prefix));
        std::uint64_t number{};
        const auto parsed = std::from_chars(suffix.data(), suffix.data() + suffix.size(), number);
        if (!number || parsed.ec != std::errc{} || parsed.ptr != suffix.data() + suffix.size()) return std::nullopt;
        const auto version = profile.at("schemaVersion").get<unsigned>();
        if (version != 1 && version != 2) return std::nullopt;
        if (profile.contains("provenance")) {
            const auto status = profile.at("provenance").value("status", std::string{});
            if (status != "approved" && status != "read-validated") return std::nullopt;
        }
        return number;
    } catch (const json::exception&) { return std::nullopt; }
}

std::vector<std::uint8_t> file_bytes(const fs::path& path) {
    std::ifstream input(path, std::ios::binary | std::ios::ate);
    if (!input) throw std::runtime_error("existing-artifact-unreadable");
    const auto size = input.tellg();
    if (size < 0 || size > (1ll << 30)) throw std::runtime_error("existing-artifact-size-out-of-bounds");
    std::vector<std::uint8_t> bytes(static_cast<std::size_t>(size));
    input.seekg(0);
    if (size && !input.read(reinterpret_cast<char*>(bytes.data()), size))
        throw std::runtime_error("existing-artifact-read-incomplete");
    return bytes;
}

// Publish only our uniquely named temporary file. Never replace an existing
// profile, executable cache or diagnostic artifact with different bytes.
bool publish(const fs::path& path, const std::vector<std::uint8_t>& bytes) {
    if (fs::exists(path)) {
        if (file_bytes(path) != bytes) throw std::runtime_error("existing-artifact-content-conflict");
        return false;
    }
    fs::create_directories(path.parent_path());
    auto temporary = path;
    temporary += L".tmp." + std::to_wstring(GetCurrentProcessId()) + L"-" +
        std::to_wstring(GetCurrentThreadId()) + L"-" + std::to_wstring(GetTickCount64());
    const auto handle = CreateFileW(temporary.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_NEW,
        FILE_ATTRIBUTE_NORMAL, nullptr);
    if (handle == INVALID_HANDLE_VALUE) throw std::runtime_error("temporary-artifact-create-failed");
    bool written = true;
    for (std::size_t offset = 0; written && offset < bytes.size();) {
        const auto count = static_cast<DWORD>(std::min<std::size_t>(1u << 20, bytes.size() - offset));
        DWORD actual{};
        written = WriteFile(handle, bytes.data() + offset, count, &actual, nullptr) && actual == count;
        offset += actual;
    }
    written = written && FlushFileBuffers(handle);
    CloseHandle(handle);
    if (!written) {
        fs::remove(temporary);
        throw std::runtime_error("temporary-artifact-write-failed");
    }
    if (MoveFileExW(temporary.c_str(), path.c_str(), MOVEFILE_WRITE_THROUGH)) return true;
    fs::remove(temporary);
    if (fs::exists(path) && file_bytes(path) == bytes) return false;
    throw std::runtime_error("artifact-publication-failed-or-conflicting");
}

std::vector<std::uint8_t> json_bytes(const json& document) {
    const auto text = document.dump(2) + '\n';
    return {text.begin(), text.end()};
}

// The independent F9 window starts before the native provider and Lua status
// loop. Publish only this module's actual analysis phase in its existing cache.
// A failed progress write must never prevent profile verification.
class AnalysisProgress {
    fs::path directory;
    json document;
    std::mutex mutex;
    std::condition_variable stop;
    bool finished{};
    std::thread heartbeat;
    void write(const char* state) noexcept {
        fs::path temporary;
        try {
            document["state"] = state;
            document["updatedAt"] = std::chrono::duration_cast<std::chrono::seconds>(
                std::chrono::system_clock::now().time_since_epoch()).count();
            temporary = directory / ("status.json.tmp." + std::to_string(GetCurrentProcessId()) +
                "-" + std::to_string(GetCurrentThreadId()));
            std::ofstream stream(temporary, std::ios::binary | std::ios::trunc);
            stream << document.dump();
            stream.close();
            const auto status_path = directory / ("status-" + std::to_string(GetCurrentProcessId()) + ".json");
            if (stream) MoveFileExW(temporary.c_str(), status_path.c_str(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH);
        } catch (...) {}
        std::error_code ignored;
        if (!temporary.empty()) fs::remove(temporary, ignored);
    }
public:
    AnalysisProgress(const fs::path& cache_root, const ImageIdentity& identity)
        : directory(cache_root), document{{"schemaVersion", 1}, {"pid", GetCurrentProcessId()},
            {"image", image_json(identity)}, {"enabled", true}} {
        write("searching");
        try {
            heartbeat = std::thread([this] {
                std::unique_lock lock(mutex);
                while (!stop.wait_for(lock, std::chrono::seconds(2), [this] { return finished; })) {
                    lock.unlock();
                    write("searching");
                    lock.lock();
                }
            });
        } catch (...) {}
    }
    ~AnalysisProgress() {
        { std::scoped_lock lock(mutex); finished = true; }
        stop.notify_one();
        if (heartbeat.joinable()) heartbeat.join();
        write("finished");
    }
};

// Steam metadata supplies a convenient label only. Executable identity and
// address evidence never depend on a mutable appmanifest or its build number.
std::optional<std::string> installed_build_number(const fs::path& executable) {
    const auto game_directory = fs::absolute(executable).lexically_normal().parent_path();
    const auto steamapps = game_directory.parent_path().parent_path();
    if (!fs::is_directory(steamapps)) return std::nullopt;
    std::set<std::string> matches;
    for (const auto& entry : fs::directory_iterator(steamapps)) {
        const auto filename = to_utf8(entry.path().filename());
        if (!entry.is_regular_file() || !filename.starts_with("appmanifest_") || entry.path().extension() != ".acf") continue;
        std::ifstream input(entry.path());
        if (!input) continue;
        std::vector<std::string> tokens;
        std::string token; bool quoted{};
        char c{};
        while (input.get(c)) {
            if (c == '"') { if (quoted) tokens.push_back(token); token.clear(); quoted = !quoted; }
            else if (quoted) token.push_back(c);
        }
        std::string directory, build;
        for (std::size_t i = 0; i + 1 < tokens.size(); ++i) {
            if (tokens[i] == "installdir") directory = tokens[i + 1];
            if (tokens[i] == "buildid") build = tokens[i + 1];
        }
        if (directory == to_utf8(game_directory.filename()) && !build.empty() &&
            std::all_of(build.begin(), build.end(), [](char byte) { return byte >= '0' && byte <= '9'; })) matches.insert(build);
    }
    if (matches.size() != 1) return std::nullopt;
    return *matches.begin();
}
}

Result Prepare(const fs::path& executable_path, const fs::path& profile_directory,
               const fs::path& cache_directory, const std::vector<json>& catalog,
               const ImageIdentity& identity) {
    Result result;
    result.diagnostics = {{"enabled", true}, {"state", "unresolved"},
        {"image", image_json(identity)}, {"kfcFormat", identity.kfc_format},
        {"method", "identical-mapped-pe-image"}, {"beta", false}};
    try {
        if ((identity.target != "enshrouded.exe" && identity.target != "enshrouded_server.exe") ||
            !valid_sha(identity.sha256) || (identity.kfc_format != 3 && identity.kfc_format != 4) ||
            profile_directory.empty() || cache_directory.empty())
            throw std::runtime_error("invalid-discovery-input");
        const auto current = Detail::ReadImage(executable_path);
        if (!same_identity(current.identity, identity)) throw std::runtime_error("current-executable-identity-changed");
        const auto cache_root = cache_directory / L"game-profile-discovery";
        std::vector<const json*> exact;
        std::vector<std::pair<std::uint64_t, const json*>> baselines;
        for (const auto& profile : catalog) {
            const auto number = build_number(profile, identity);
            if (!number) continue;
            baselines.emplace_back(*number, &profile);
            if (matches(profile, identity)) exact.push_back(&profile);
        }
        result.diagnostics["normalExactMatches"] = exact.size();
        if (exact.size() > 1) {
            result.diagnostics["reason"] = "normal-exact-profile-ambiguous";
            return result;
        }
        if (exact.size() == 1) {
            const auto cache_path = cache_root / (identity.sha256 + ".exe");
            result.diagnostics["baselineId"] = exact.front()->at("id");
            result.diagnostics["cachePath"] = to_utf8(cache_path);
            if (fs::exists(cache_path)) {
                const auto cached = Detail::ReadImage(cache_path, false);
                if (cached.identity.sha256 != identity.sha256)
                    throw std::runtime_error("cached-baseline-sha256-mismatch");
            } else publish(cache_path, current.file_bytes);
            result.diagnostics["state"] = "baseline-cached";
            result.diagnostics["reason"] = "normal-exact-profile-preferred";
            return result;
        }
        std::sort(baselines.begin(), baselines.end(), [](const auto& left, const auto& right) {
            return left.first > right.first;
        });
        if (baselines.empty()) {
            result.diagnostics["reason"] = "no-normal-same-target-kfc-baseline";
            return result;
        }
        if (baselines.size() > 1 && baselines[0].first == baselines[1].first) {
            result.diagnostics["reason"] = "latest-normal-baseline-ambiguous";
            return result;
        }
        const auto& baseline = *baselines.front().second;
        result.diagnostics["baselineId"] = baseline.at("id");
        if (!baseline.contains("provenance") ||
            baseline.at("provenance").value("status", std::string{}) != "read-validated") {
            result.diagnostics["reason"] = "automatic-execution-profile-approval-not-supported";
            return result;
        }
        if (!baseline.at("provenance").value("resourceReadValidated", false) ||
            !baseline.contains("ownerLayout") || !baseline.at("ownerLayout").is_object() ||
            !baseline.contains("resourceLayout") || !baseline.at("resourceLayout").is_object()) {
            result.diagnostics["reason"] = "baseline-resource-read-contract-unavailable";
            return result;
        }
        const auto baseline_hash = baseline.at("image").at("sha256").get<std::string>();
        const auto source_path = cache_root / (baseline_hash + ".exe");
        result.diagnostics["baselineCachePath"] = to_utf8(source_path);
        if (!fs::exists(source_path)) {
            result.diagnostics["reason"] = "exact-baseline-executable-not-cached";
            return result;
        }
        const auto source = Detail::ReadImage(source_path, false);
        if (source.identity.sha256 != baseline_hash ||
            baseline.at("image").at("timestamp") != source.identity.timestamp ||
            baseline.at("image").at("size") != source.identity.size)
            throw std::runtime_error("cached-baseline-image-identity-mismatch");
        // Reuse only a complete successful analysis of these freshly hashed
        // files and this entire baseline. Persistent proof files are outputs,
        // never trusted inputs. No memo mutex is held during either scan.
        const auto memo_key = analysis_key(baseline, source, current, source_path,
            executable_path, profile_directory, cache_directory);
        auto memo = recalled_analysis(memo_key);
        const AnalysisProgress progress(cache_root, identity);
        result.diagnostics["analysisMemoHit"] = memo.has_value();
        json comparison;
        auto generated = baseline;
        if (memo) {
            generated = std::move(memo->generated);
            comparison = std::move(memo->comparison);
        } else if (!Detail::Equivalent(source, current, comparison)) {
            json rebase;
            if (!Detail::RebaseProfile(source, current, baseline, generated, rebase)) {
                result.diagnostics["reason"] = "image-equivalence-not-established";
                result.diagnostics["comparison"] = std::move(comparison);
                result.diagnostics["rebase"] = std::move(rebase);
                // Keep full failed evidence outside the production catalog.
                // Runtime snapshots carry only its bounded summary and path.
                try {
                    const auto bytes = json_bytes(result.diagnostics);
                    auto report_path = cache_root / (identity.sha256 + ".v2.unresolved.json");
                    if (fs::exists(report_path) && file_bytes(report_path) != bytes)
                        report_path = cache_root / (identity.sha256 + ".v2.unresolved-" +
                            std::to_string(GetCurrentProcessId()) + "-" + std::to_string(GetTickCount64()) + ".json");
                    publish(report_path, bytes);
                    result.diagnostics["evidencePath"] = to_utf8(report_path);
                } catch (const std::exception& error) {
                    result.diagnostics["evidenceWriteError"] = error.what();
                }
                return result;
            }
            comparison = std::move(rebase);
        }
        const auto analyzed_profile = generated;
        const auto kind = identity.target == "enshrouded.exe" ? "client" : "server";
        const auto build_label = installed_build_number(executable_path).value_or(identity.sha256);
        const auto id = std::string("beta_enshrouded-") + kind + "-" + build_label;
        generated["id"] = id;
        generated["image"]["sha256"] = identity.sha256;
        generated["image"]["timestamp"] = identity.timestamp;
        generated["image"]["size"] = identity.size;
        const auto profile_path = profile_directory / L"enshrouded" / kind / (id + ".json");
        const auto proof_path = cache_root / (identity.sha256 + ".v2.proof.json");
        const json proof = {{"schemaVersion", 1}, {"kind", "automatic-beta-profile-evidence"},
            {"analysisVersion", 2},
            {"baselineId", baseline.at("id")}, {"sourceSha256", baseline_hash},
            {"target", image_json(identity)}, {"comparison", comparison},
            {"differences", json::diff(baseline, generated)}, {"baselineFlagsPreserved", true},
            {"gameplayEffectsTested", false}, {"beta", true}};
        publish(proof_path, json_bytes(proof));
        const bool created = publish(profile_path, json_bytes(generated));
        if (!memo) remember_analysis(memo_key, {analyzed_profile, comparison});
        result.profile = std::move(generated);
        result.profile_path = profile_path;
        result.diagnostics["state"] = created ? "beta-generated" : "beta-present";
        result.diagnostics["reason"] = "complete-original-image-profile-contract-verified";
        result.diagnostics["beta"] = true;
        result.diagnostics["profileId"] = id;
        result.diagnostics["profilePath"] = to_utf8(profile_path);
        result.diagnostics["proofPath"] = to_utf8(proof_path);
        result.diagnostics["comparison"] = std::move(comparison);
        return result;
    } catch (const std::exception& error) {
        result.profile.reset();
        result.diagnostics["state"] = "error";
        result.diagnostics["reason"] = error.what();
        return result;
    }
}
}
