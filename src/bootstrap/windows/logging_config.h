#pragma once
#include <chrono>
#include <filesystem>
#include <fstream>
#include <mutex>
#include <nlohmann/json.hpp>

namespace ShroudforgeConfig {
inline nlohmann::json Read(const std::filesystem::path& root) {
    static std::mutex mutex;
    static auto next = std::chrono::steady_clock::time_point{};
    static nlohmann::json cached = {{"logging", {{"minimumLevel","INFO"}}}};
    std::scoped_lock lock(mutex);
    const auto now = std::chrono::steady_clock::now();
    if (now >= next) {
        next = now + std::chrono::milliseconds(500);
        try {
            auto path = root / "shroudforge/config/loader.json";
            std::ifstream input(path);
            if (input) {
                auto value = nlohmann::json::parse(input);
                const auto& logging = value.at("logging");
                const auto level = logging.at("minimumLevel").get<std::string>();
                if (level == "TRACE" || level == "DEBUG" || level == "INFO" ||
                    level == "WARN" || level == "ERROR") {
                    cached = std::move(value);
                }
            }
        } catch (...) { /* Keep the last valid recording configuration. */ }
    }
    return cached;
}
inline bool Allows(const std::filesystem::path& root, char level) {
    const auto value = Read(root);
    const auto minimum = value.at("logging").value("minimumLevel",std::string("INFO"));
    const auto rank = [](char item) { switch(item) { case 'T':return 0; case 'D':return 1; case 'W':return 3; case 'E':return 4; default:return 2; } };
    return rank(level) >= rank(minimum.front());
}
inline bool ModuleEnabled(const std::filesystem::path& root, const char* name) {
    try { return Read(root).at("modules").at(name).value("enabled",true); }
    catch (...) { return true; }
}
}
