#include "discover_game_profile.h"
#include "verify_image_equivalence.h"
#include "rebase_profile_contract.h"
#include "live_profile_layout.h"
#include "profile_discovery_log.h"
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

fs::path path_from_utf8(const std::string& value) {
    std::u8string encoded;
    encoded.reserve(value.size());
    for (const auto byte : value)
        encoded.push_back(static_cast<char8_t>(static_cast<unsigned char>(byte)));
    return fs::path(std::move(encoded));
}

bool valid_sha(const std::string& value) {
    return value.size() == 64 && std::all_of(value.begin(), value.end(), [](char c) {
        return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f');
    });
}

std::string profile_id(const json& profile) {
    if (!profile.is_object()) return "<missing-or-invalid>";
    const auto id = profile.find("id");
    return id != profile.end() && id->is_string() ? id->get<std::string>() : "<missing-or-invalid>";
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

struct UnresolvedAnalysis {
    json generated;
    json comparison;
    json rebase;
    fs::path evidence_path;
};

struct UnresolvedMemoEntry {
    std::string key;
    UnresolvedAnalysis analysis;
};

std::mutex analysis_memo_mutex;
std::vector<MemoEntry> analysis_memo;
std::vector<UnresolvedMemoEntry> unresolved_analysis_memo;
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

std::optional<UnresolvedAnalysis> recalled_unresolved_analysis(const std::string& key) {
    std::scoped_lock lock(analysis_memo_mutex);
    for (const auto& entry : unresolved_analysis_memo)
        if (entry.key == key) return entry.analysis;
    return std::nullopt;
}

void remember_unresolved_analysis(std::string key, UnresolvedAnalysis analysis) {
    std::scoped_lock lock(analysis_memo_mutex);
    for (const auto& entry : unresolved_analysis_memo)
        if (entry.key == key) return;
    if (unresolved_analysis_memo.size() == maximum_memo_entries)
        unresolved_analysis_memo.erase(unresolved_analysis_memo.begin());
    unresolved_analysis_memo.push_back({std::move(key), std::move(analysis)});
}

void remember_unresolved_evidence_path(const std::string& key, const fs::path& evidence_path) {
    std::scoped_lock lock(analysis_memo_mutex);
    for (auto& entry : unresolved_analysis_memo) {
        if (entry.key == key) {
            entry.analysis.evidence_path = evidence_path;
            return;
        }
    }
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

void log_evidence(const fs::path& game_root, const char* stage, const json& evidence,
                  bool trace_rows) {
    if (!evidence.is_object()) return;
    const auto count = [&evidence](const char* key) {
        const auto found = evidence.find(key);
        return found != evidence.end() && found->is_array() ? found->size() : std::size_t{};
    };
    const auto complete = evidence.contains("complete") ? evidence["complete"].dump() :
        evidence.contains("equivalent") ? evidence["equivalent"].dump() : std::string("unknown");
    const auto reason = evidence.value("reason", std::string("<none>"));
    const auto method = evidence.value("method", std::string("<unknown>"));
    Log(game_root, 'D', std::string(stage) + " evidence summary. method=" + method +
        " complete=" + complete + " reason=" + reason + " fields=" +
        std::to_string(count("fields")) + " unresolved=" + std::to_string(count("unresolved")) +
        " sections=" + std::to_string(count("sections")));
    if (!trace_rows) return;
    for (const auto key : {"fields", "unresolved"}) {
        const auto found = evidence.find(key);
        if (found == evidence.end() || !found->is_array()) continue;
        for (std::size_t index = 0; index < found->size(); ++index) {
            Log(game_root, 'T', std::string(stage) + " evidence " + key + "[" +
                std::to_string(index) + "]=" + (*found)[index].dump());
        }
    }
}

void write_diagnostics_report(Result& result, const fs::path& game_root,
                              const fs::path& cache_directory,
                              const ImageIdentity& identity) noexcept {
    if (!valid_sha(identity.sha256) || cache_directory.empty()) {
        Log(game_root, 'W', "No diagnostics report can be written because the image SHA-256 or cache directory is unavailable");
        return;
    }
    fs::path report_path;
    try {
        const auto cache_root = cache_directory / L"game-profile-discovery";
        const auto failed = result.diagnostics.value("state", std::string{}) == "error";
        const auto extension = failed ? ".v2.error.json" : ".v2.unresolved.json";
        report_path = cache_root / (identity.sha256 + extension);
        result.diagnostics["evidencePath"] = to_utf8(report_path);
        auto bytes = json_bytes(result.diagnostics);
        if (fs::exists(report_path) && file_bytes(report_path) != bytes) {
            report_path = cache_root / (identity.sha256 + (failed ? ".v2.error-" : ".v2.unresolved-") +
                std::to_string(GetCurrentProcessId()) + "-" + std::to_string(GetTickCount64()) + ".json");
            result.diagnostics["evidencePath"] = to_utf8(report_path);
            bytes = json_bytes(result.diagnostics);
        }
        const bool created = publish(report_path, bytes);
        Log(game_root, 'I', std::string(created ? "Diagnostics report written. path=" :
            "Existing diagnostics report reused. path=") + to_utf8(report_path));
    } catch (const std::exception& error) {
        result.diagnostics.erase("evidencePath");
        result.diagnostics["evidenceWriteError"] = error.what();
        Log(game_root, 'E', "Could not publish discovery diagnostics report. path=" +
            (report_path.empty() ? std::string("<not-resolved>") : to_utf8(report_path)) +
            " reason=" + error.what());
    } catch (...) {
        result.diagnostics.erase("evidencePath");
        result.diagnostics["evidenceWriteError"] = "unknown-diagnostics-publication-failure";
        Log(game_root, 'E', "Could not publish discovery diagnostics report because of an unknown error. path=" +
            (report_path.empty() ? std::string("<not-resolved>") : to_utf8(report_path)));
    }
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
    const auto game_root = executable_path.parent_path();
    Log(game_root, 'I', "Discovery preparation entered. target=" + identity.target +
        " sha256=" + identity.sha256 + " timestamp=" + std::to_string(identity.timestamp) +
        " imageSize=" + std::to_string(identity.size) + " kfcFormat=" +
        std::to_string(identity.kfc_format) + " executable=" + to_utf8(executable_path) +
        " profiles=" + to_utf8(profile_directory) + " cache=" + to_utf8(cache_directory) +
        " catalogEntries=" + std::to_string(catalog.size()));
    try {
        if ((identity.target != "enshrouded.exe" && identity.target != "enshrouded_server.exe") ||
            !valid_sha(identity.sha256) || (identity.kfc_format != 3 && identity.kfc_format != 4) ||
            profile_directory.empty() || cache_directory.empty())
            throw std::runtime_error("invalid-discovery-input");
        Log(game_root, 'D', "Discovery inputs passed target, SHA-256, KFC format, and output-directory validation");
        Log(game_root, 'D', "Reading and hashing the current executable image. path=" + to_utf8(executable_path));
        const auto current = Detail::ReadImage(executable_path);
        Log(game_root, 'T', "Current executable image read. identity=" + image_json(current.identity).dump() +
            " sections=" + std::to_string(current.sections.size()) +
            " fileBytes=" + std::to_string(current.file_bytes.size()));
        if (!same_identity(current.identity, identity)) {
            Log(game_root, 'E', "Current executable identity changed between caller identification and discovery. caller=" +
                image_json(identity).dump() + " freshlyRead=" + image_json(current.identity).dump());
            throw std::runtime_error("current-executable-identity-changed");
        }
        const auto cache_root = cache_directory / L"game-profile-discovery";
        Log(game_root, 'D', "Discovery cache root resolved. path=" + to_utf8(cache_root));
        std::vector<const json*> exact;
        std::vector<std::pair<std::uint64_t, const json*>> baselines;
        std::size_t ignored_catalog_entries{};
        for (const auto& profile : catalog) {
            const auto number = build_number(profile, identity);
            if (!number) {
                ++ignored_catalog_entries;
                Log(game_root, 'T', "Catalog entry is not a supported normal same-target and same-KFC baseline. id=" +
                    profile_id(profile));
                continue;
            }
            baselines.emplace_back(*number, &profile);
            const auto exact_match = matches(profile, identity);
            Log(game_root, 'T', "Baseline candidate inspected. id=" + profile_id(profile) +
                " build=" + std::to_string(*number) + " exactImage=" + (exact_match ? "true" : "false") +
                " sha256=" + profile.value("image", json::object()).value("sha256", std::string("<missing>")));
            if (exact_match) exact.push_back(&profile);
        }
        result.diagnostics["normalExactMatches"] = exact.size();
        Log(game_root, 'D', "Normal profile baseline inventory complete. supportedBaselines=" +
            std::to_string(baselines.size()) + " exactImageMatches=" + std::to_string(exact.size()) +
            " ignoredCatalogEntries=" + std::to_string(ignored_catalog_entries));
        if (exact.size() > 1) {
            result.diagnostics["reason"] = "normal-exact-profile-ambiguous";
            Log(game_root, 'W', "Discovery stopped because multiple normal profiles exactly match the current image. matches=" +
                std::to_string(exact.size()));
            write_diagnostics_report(result, game_root, cache_directory, identity);
            return result;
        }
        if (exact.size() == 1) {
            const auto cache_path = cache_root / (identity.sha256 + ".exe");
            result.diagnostics["baselineId"] = exact.front()->at("id");
            result.diagnostics["cachePath"] = to_utf8(cache_path);
            Log(game_root, 'I', "A normal profile already exactly matches this executable. No beta profile is needed. baselineId=" +
                exact.front()->at("id").get<std::string>() + " executableCache=" + to_utf8(cache_path));
            if (fs::exists(cache_path)) {
                Log(game_root, 'D', "Verifying the existing exact-image executable cache. path=" + to_utf8(cache_path));
                const auto cached = Detail::ReadImage(cache_path, false);
                if (cached.identity.sha256 != identity.sha256)
                    throw std::runtime_error("cached-baseline-sha256-mismatch");
                Log(game_root, 'D', "Existing exact-image executable cache verified by SHA-256. sha256=" + identity.sha256);
            } else {
                Log(game_root, 'D', "Caching the current executable as the exact baseline for the next game update. path=" +
                    to_utf8(cache_path) + " bytes=" + std::to_string(current.file_bytes.size()));
                const bool created = publish(cache_path, current.file_bytes);
                Log(game_root, created ? 'I' : 'D', std::string(created ?
                    "Exact baseline executable cached. path=" : "Matching exact baseline cache already exists. path=") +
                    to_utf8(cache_path));
            }
            result.diagnostics["state"] = "baseline-cached";
            result.diagnostics["reason"] = "normal-exact-profile-preferred";
            Log(game_root, 'I', "Discovery finished without generating a beta profile because the exact normal profile remains preferred. baselineId=" +
                exact.front()->at("id").get<std::string>() + " cachePath=" + to_utf8(cache_path));
            return result;
        }
        std::sort(baselines.begin(), baselines.end(), [](const auto& left, const auto& right) {
            return left.first > right.first;
        });
        if (baselines.empty()) {
            result.diagnostics["reason"] = "no-normal-same-target-kfc-baseline";
            Log(game_root, 'W', "No normal profile can serve as a baseline for this target and KFC format. target=" +
                identity.target + " kfcFormat=" + std::to_string(identity.kfc_format));
            write_diagnostics_report(result, game_root, cache_directory, identity);
            return result;
        }
        if (baselines.size() > 1 && baselines[0].first == baselines[1].first) {
            result.diagnostics["reason"] = "latest-normal-baseline-ambiguous";
            Log(game_root, 'W', "The latest normal baseline build number is ambiguous. build=" +
                std::to_string(baselines[0].first) + " first=" + baselines[0].second->value("id", std::string("<missing>")) +
                " second=" + baselines[1].second->value("id", std::string("<missing>")));
            write_diagnostics_report(result, game_root, cache_directory, identity);
            return result;
        }
        const auto& baseline = *baselines.front().second;
        result.diagnostics["baselineId"] = baseline.at("id");
        Log(game_root, 'I', "Selected the highest supported normal profile as the beta build baseline. id=" +
            baseline.at("id").get<std::string>() + " build=" + std::to_string(baselines.front().first) +
            " target=" + identity.target + " kfcFormat=" + std::to_string(identity.kfc_format));
        if (!baseline.contains("provenance") ||
            baseline.at("provenance").value("status", std::string{}) != "read-validated") {
            result.diagnostics["reason"] = "automatic-execution-profile-approval-not-supported";
            Log(game_root, 'W', "Selected baseline does not have read-validated provenance. Beta generation is blocked. baselineId=" +
                baseline.at("id").get<std::string>() + " provenance=" +
                (baseline.contains("provenance") ? baseline.at("provenance").dump() : std::string("<missing>")));
            write_diagnostics_report(result, game_root, cache_directory, identity);
            return result;
        }
        if (!baseline.at("provenance").value("resourceReadValidated", false) ||
            !baseline.contains("ownerLayout") || !baseline.at("ownerLayout").is_object() ||
            !baseline.contains("resourceLayout") || !baseline.at("resourceLayout").is_object()) {
            result.diagnostics["reason"] = "baseline-resource-read-contract-unavailable";
            Log(game_root, 'W', "Selected baseline is missing its resource-read, owner-layout, or resource-layout contract. baselineId=" +
                baseline.at("id").get<std::string>() + " resourceReadValidated=" +
                std::string(baseline.at("provenance").value("resourceReadValidated", false) ? "true" : "false") +
                " ownerLayout=" + std::string(baseline.contains("ownerLayout") ? "present" : "missing") +
                " resourceLayout=" + std::string(baseline.contains("resourceLayout") ? "present" : "missing"));
            write_diagnostics_report(result, game_root, cache_directory, identity);
            return result;
        }
        const auto baseline_hash = baseline.at("image").at("sha256").get<std::string>();
        const auto source_path = cache_root / (baseline_hash + ".exe");
        result.diagnostics["baselineCachePath"] = to_utf8(source_path);
        if (!fs::exists(source_path)) {
            result.diagnostics["reason"] = "exact-baseline-executable-not-cached";
            Log(game_root, 'W', "The exact executable image for the selected baseline is not cached. baselineId=" +
                baseline.at("id").get<std::string>() + " requiredPath=" + to_utf8(source_path));
            write_diagnostics_report(result, game_root, cache_directory, identity);
            return result;
        }
        Log(game_root, 'D', "Reading and hashing the cached baseline executable. path=" + to_utf8(source_path));
        const auto source = Detail::ReadImage(source_path, false);
        if (source.identity.sha256 != baseline_hash ||
            baseline.at("image").at("timestamp") != source.identity.timestamp ||
            baseline.at("image").at("size") != source.identity.size) {
            Log(game_root, 'E', "Cached baseline executable identity does not match its normal profile. profileId=" +
                baseline.at("id").get<std::string>() + " expectedSha256=" + baseline_hash +
                " actual=" + image_json(source.identity).dump() + " path=" + to_utf8(source_path));
            throw std::runtime_error("cached-baseline-image-identity-mismatch");
        }
        Log(game_root, 'D', "Cached baseline executable identity verified. image=" + image_json(source.identity).dump() +
            " sections=" + std::to_string(source.sections.size()));
        // Reuse analyses only after freshly hashing both images and binding
        // the entire baseline and paths. A static unresolved result may be
        // retried against fresh live memory without repeating the PE scan.
        // Persistent proof files are outputs, never trusted inputs.
        const auto memo_key = analysis_key(baseline, source, current, source_path,
            executable_path, profile_directory, cache_directory);
        auto memo = recalled_analysis(memo_key);
        auto unresolved_memo = memo ? std::optional<UnresolvedAnalysis>{} :
            recalled_unresolved_analysis(memo_key);
        const AnalysisProgress progress(cache_root, identity);
        result.diagnostics["analysisMemoHit"] = memo.has_value() || unresolved_memo.has_value();
        Log(game_root, memo || unresolved_memo ? 'D' : 'I', std::string("Static analysis cache lookup completed. result=") +
            (memo ? "complete-analysis-hit" : unresolved_memo ? "unresolved-analysis-hit" : "no-hit") +
            " liveProofWillRun=" + (memo ? "false" : "true"));
        if (memo) Log(game_root, 'D', "Reusing the successful in-process static analysis after freshly hashing both images");
        if (unresolved_memo) {
            Log(game_root, 'D', "Reusing unresolved static image and profile evidence. Static scanning is skipped and live validation will be retried. evidencePath=" +
                (unresolved_memo->evidence_path.empty() ? std::string("<not-yet-written>") : to_utf8(unresolved_memo->evidence_path)));
        }
        json comparison;
        json rebase;
        auto generated = baseline;
        bool static_complete = memo.has_value();
        bool static_rebase_performed{};
        bool live_layout_used{};
        if (memo) {
            generated = std::move(memo->generated);
            comparison = std::move(memo->comparison);
        } else if (unresolved_memo) {
            generated = std::move(unresolved_memo->generated);
            comparison = std::move(unresolved_memo->comparison);
            rebase = std::move(unresolved_memo->rebase);
            log_evidence(game_root, "Reused image comparison", comparison, false);
            log_evidence(game_root, "Reused static profile rebase", rebase, false);
        } else if (!Detail::Equivalent(source, current, comparison)) {
            Log(game_root, 'I', "Baseline and current mapped PE images differ. Starting complete original-image profile rebase analysis");
            Log(game_root, 'T', "Image comparison evidence=" + comparison.dump());
            log_evidence(game_root, "Mapped image comparison", comparison, false);
            static_rebase_performed = true;
            Log(game_root, 'D', "Rebasing profile addresses, reflected layouts, hooks, and resource contracts from the exact original images");
            static_complete = Detail::RebaseProfile(source, current, baseline, generated, rebase);
            log_evidence(game_root, "Static profile rebase", rebase, true);
            if (!static_complete) {
                remember_unresolved_analysis(memo_key, {generated, comparison, rebase, {}});
                Log(game_root, 'W', "Static profile rebase is incomplete. A live in-process layout proof will be attempted before beta generation is rejected");
            } else {
                Log(game_root, 'I', "Static rebase established the complete profile contract from the original images");
            }
        } else {
            static_complete = true;
            Log(game_root, 'I', "Mapped PE images are equivalent. Existing baseline addresses and layouts remain valid for this image");
            Log(game_root, 'T', "Image comparison evidence=" + comparison.dump());
            log_evidence(game_root, "Mapped image comparison", comparison, false);
        }
        if (!static_complete) {
            json live_profile = generated;
            json live_layout;
            Log(game_root, 'I', "Starting exact-process live layout verification for fields not established by static analysis. executable=" +
                to_utf8(executable_path) + " imageSha256=" + identity.sha256);
            const bool live_complete = Detail::ResolveLiveProfileLayouts(
                executable_path, identity, live_profile, rebase, live_layout);
            log_evidence(game_root, "Live process layout proof", live_layout, true);
            Log(game_root, 'T', "Live layout proof image=" + live_layout.dump());
            if (live_complete) {
                rebase["staticComplete"] = false;
                rebase["staticUnresolved"] = rebase.value("unresolved", json::array());
                rebase["unresolved"] = json::array();
                rebase["complete"] = true;
                rebase["method"] = "original-image-profile-rebase-with-exact-process-live-layout-proof";
                rebase["reason"] = "all-address-and-layout-fields-verified";
                rebase["liveLayoutProof"] = std::move(live_layout);
                for (auto& field : rebase["liveLayoutProof"]["fields"]) {
                    field["role"] = "live-layout";
                    rebase["fields"].push_back(std::move(field));
                }
                generated = std::move(live_profile);
                comparison = std::move(rebase);
                live_layout_used = true;
                Log(game_root, 'I', "Live process layout proof completed every unresolved contract field. Beta generation can continue");
            } else {
                result.diagnostics["reason"] = "image-equivalence-not-established";
                result.diagnostics["comparison"] = std::move(comparison);
                result.diagnostics["rebase"] = std::move(rebase);
                result.diagnostics["liveLayoutProof"] = std::move(live_layout);
                // Keep the first complete failure report. Subsequent startup
                // polls reuse the static evidence and retry only live memory.
                std::error_code evidence_check_error;
                const bool memo_report_exists = unresolved_memo && !unresolved_memo->evidence_path.empty() &&
                    fs::is_regular_file(unresolved_memo->evidence_path, evidence_check_error) &&
                    !evidence_check_error;
                if (memo_report_exists) {
                    result.diagnostics["evidencePath"] = to_utf8(unresolved_memo->evidence_path);
                    Log(game_root, 'D', "Reusing the original unresolved report for this live proof retry. path=" +
                        to_utf8(unresolved_memo->evidence_path));
                } else {
                    write_diagnostics_report(result, game_root, cache_directory, identity);
                    const auto evidence_path = result.diagnostics.value("evidencePath", std::string{});
                    if (!evidence_path.empty() && !memo)
                        remember_unresolved_evidence_path(memo_key, path_from_utf8(evidence_path));
                }
                Log(game_root, 'W', "Beta profile was not generated because static image evidence and current live layout evidence did not resolve every required contract. reason=" +
                    result.diagnostics.value("reason", std::string("unknown")) + " unresolved=" +
                    std::to_string(result.diagnostics["rebase"].value("unresolved", json::array()).size()) +
                    " report=" + result.diagnostics.value("evidencePath", std::string("<report-not-written>")));
                return result;
            }
        }
        if (static_complete && static_rebase_performed && !memo && !unresolved_memo)
            comparison = std::move(rebase);
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
        result.diagnostics["profileId"] = id;
        result.diagnostics["profilePath"] = to_utf8(profile_path);
        result.diagnostics["proofPath"] = to_utf8(proof_path);
        Log(game_root, 'D', "Preparing final beta profile and proof publication. profileId=" + id +
            " profilePath=" + to_utf8(profile_path) + " proofPath=" + to_utf8(proof_path) +
            " baselineId=" + baseline.at("id").get<std::string>() +
            " liveLayoutProof=" + (live_layout_used ? "true" : "false"));
        const json proof = {{"schemaVersion", 1}, {"kind", "automatic-beta-profile-evidence"},
            {"analysisVersion", 3},
            {"baselineId", baseline.at("id")}, {"sourceSha256", baseline_hash},
            {"target", image_json(identity)}, {"comparison", comparison},
            {"differences", json::diff(baseline, generated)}, {"baselineFlagsPreserved", true},
            {"gameplayEffectsTested", false}, {"liveLayoutProof", live_layout_used}, {"beta", true}};
        const bool proof_created = publish(proof_path, json_bytes(proof));
        Log(game_root, proof_created ? 'I' : 'D', std::string(proof_created ?
            "Complete beta proof report written. path=" : "Matching complete beta proof report already exists. path=") +
            to_utf8(proof_path));
        const bool created = publish(profile_path, json_bytes(generated));
        if (!memo && !unresolved_memo && static_complete && !live_layout_used)
            remember_analysis(memo_key, {analyzed_profile, comparison});
        result.profile = std::move(generated);
        result.profile_path = profile_path;
        result.diagnostics["state"] = created ? "beta-generated" : "beta-present";
        result.diagnostics["reason"] = live_layout_used ?
            "complete-original-image-and-live-layout-contract-verified" :
            "complete-original-image-profile-contract-verified";
        result.diagnostics["liveLayoutProof"] = live_layout_used;
        result.diagnostics["beta"] = true;
        result.diagnostics["profileId"] = id;
        result.diagnostics["profilePath"] = to_utf8(profile_path);
        result.diagnostics["proofPath"] = to_utf8(proof_path);
        result.diagnostics["evidencePath"] = to_utf8(proof_path);
        result.diagnostics["comparison"] = std::move(comparison);
        Log(game_root, created ? 'I' : 'D', std::string(created ?
            "Beta profile generated and saved. id=" : "Previously generated matching beta profile is present. id=") +
            id + " savedPath=" + to_utf8(profile_path) + " proofPath=" + to_utf8(proof_path) +
            " baselineId=" + baseline.at("id").get<std::string>());
        log_evidence(game_root, "Final beta profile contract", result.diagnostics["comparison"], true);
        return result;
    } catch (const std::exception& error) {
        result.profile.reset();
        result.diagnostics["state"] = "error";
        result.diagnostics["reason"] = error.what();
        Log(game_root, 'E', "Discovery failed with an exception. reason=" + std::string(error.what()) +
            " target=" + identity.target + " sha256=" + identity.sha256 +
            " executable=" + to_utf8(executable_path) + " profiles=" + to_utf8(profile_directory) +
            " cache=" + to_utf8(cache_directory) + " profilePath=" +
            result.diagnostics.value("profilePath", std::string("<not-resolved>")) + " proofPath=" +
            result.diagnostics.value("proofPath", std::string("<not-resolved>")) + " baselineCachePath=" +
            result.diagnostics.value("baselineCachePath", std::string("<not-resolved>")) + " cachePath=" +
            result.diagnostics.value("cachePath", std::string("<not-resolved>")));
        write_diagnostics_report(result, game_root, cache_directory, identity);
        return result;
    } catch (...) {
        result.profile.reset();
        result.diagnostics["state"] = "error";
        result.diagnostics["reason"] = "unknown-discovery-exception";
        Log(game_root, 'E', "Discovery failed with an unknown exception. target=" + identity.target +
            " sha256=" + identity.sha256 + " executable=" + to_utf8(executable_path));
        write_diagnostics_report(result, game_root, cache_directory, identity);
        return result;
    }
}
}
