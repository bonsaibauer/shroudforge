#include "profiles/read_map_capture_profile.h"
#include <nlohmann/json.hpp>
#include <algorithm>
#include <stdexcept>

namespace KfcRuntimeCompatibility {
MapCaptureSelection ReadMapCaptureProfile(const nlohmann::json& profile) {
    if (!profile.contains("mapCapture")) return {};
    try {
        using json = nlohmann::json;
        const auto number = [](const json& object, const char* key, std::uint64_t min, std::uint64_t max) {
            const auto& value = object.at(key);
            if (!value.is_number_integer() ||
                (!value.is_number_unsigned() && value.get<std::int64_t>() < 0) ||
                value.get<std::uint64_t>() < min || value.get<std::uint64_t>() > max)
                throw std::runtime_error(std::string("invalid map capture value: ") + key);
            return static_cast<std::uint32_t>(value.get<std::uint64_t>());
        };
        const auto bounded_array = [](const json& value, std::size_t maximum) {
            if (!value.is_array() || value.size() > maximum) throw std::runtime_error("invalid map capture array");
        };
        const auto name = [](const json& value) {
            auto text = value.get<std::string>();
            if (text.empty() || text.size() > 95 || text.find('\0') != std::string::npos)
                throw std::runtime_error("invalid map capture name");
            return text;
        };
        const auto& model = profile.at("mapCapture");
        if (profile.at("target") != "enshrouded.exe" || !profile.at("kfcFormat").is_number_integer() || profile.at("kfcFormat") != 4 ||
            !model.is_object() || model.size() != 6 || model.at("id") != "map-image-admission-v1")
            throw std::runtime_error("unsupported map capture image contract");
        auto result = std::make_shared<MapCaptureProfile>();
        result->profile_id = name(profile.at("id"));
        result->image_sha256 = profile.at("image").at("sha256").get<std::string>();
        if (result->image_sha256.size() != 64 || result->image_sha256.find_first_not_of("0123456789abcdef") != std::string::npos)
            throw std::runtime_error("invalid map capture image hash");
        result->image_size = number(profile.at("image"), "size", 4096, UINT32_MAX);
        result->timestamp = number(profile.at("image"), "timestamp", 0, UINT32_MAX);
        if (profile.contains("profileRevision")) result->revision = number(profile, "profileRevision", 1, UINT32_MAX);
        result->guard_chunk_bytes = number(model, "guardChunkBytes", 256, 1024);
        if (result->guard_chunk_bytes != 256 && result->guard_chunk_bytes != 1024)
            throw std::runtime_error("unsupported map guard chunk size");
        const auto readiness = model.at("readiness").get<std::string>();
        if (readiness != "all-views" && readiness != "player-only") throw std::runtime_error("invalid map readiness contract");
        result->all_views = readiness == "all-views";
        result->deaths_require_world = model.at("deathsRequireWorld").get<bool>();
        const auto rva = [&](const json& value, const char* key, std::uint32_t extent = 1) {
            if (extent > result->image_size) throw std::runtime_error("map capture extent exceeds image");
            return number(value, key, 1, result->image_size - extent);
        };
        const auto systems = [&](const json& values) {
            bounded_array(values, 32);
            std::vector<MapCaptureProfile::System> output;
            for (const auto& value : values) {
                if (!value.is_object() || value.size() != 3) throw std::runtime_error("invalid map system shape");
                MapCaptureProfile::System system{rva(value, "descriptorRva", 24), rva(value, "callbackRva"), name(value.at("name"))};
                for (const auto& other : output) if (system.descriptor == other.descriptor || system.name == other.name)
                    throw std::runtime_error("duplicate map system");
                output.push_back(std::move(system));
            }
            return output;
        };
        result->required_systems = systems(model.at("requiredSystems"));
        const auto& views = model.at("views");
        if (!views.is_object() || views.size() != MapViewNames.size()) throw std::runtime_error("incomplete map view catalog");
        std::size_t total_bytes{};
        for (std::size_t index = 0; index < MapViewNames.size(); ++index) {
            const auto& value = views.at(MapViewNames[index]);
            if (!value.is_object() || value.size() != (value.contains("enumTable") ? 4 : 3))
                throw std::runtime_error("invalid map view shape");
            auto& view = result->views[index];
            view.systems = systems(value.at("systems"));
            const auto& guards = value.at("guards");
            bounded_array(guards, 4096);
            if (guards.empty()) throw std::runtime_error("map view has no guard bytes");
            for (const auto& item : guards) {
                if (!item.is_object() || item.size() != 2) throw std::runtime_error("invalid map guard shape");
                MapCaptureProfile::Guard guard;
                guard.rva = rva(item, "guardRva");
                const auto& hex = item.at("bytes").get_ref<const std::string&>();
                const auto bytes = hex.size() / 2;
                if (hex.empty() || hex.size() % 2 || bytes > 65536 || bytes > result->image_size - guard.rva ||
                    total_bytes > 4 * 1024 * 1024 - bytes || (result->guard_chunk_bytes == 1024 && bytes > 1024))
                    throw std::runtime_error("map guard byte budget or image range exceeded");
                total_bytes += bytes;
                const auto nibble = [](char c) -> unsigned {
                    if (c >= '0' && c <= '9') return c - '0';
                    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
                    throw std::runtime_error("invalid map guard hex");
                };
                guard.bytes.reserve(bytes);
                for (std::size_t i = 0; i < hex.size(); i += 2)
                    guard.bytes.push_back(static_cast<std::uint8_t>((nibble(hex[i]) << 4) | nibble(hex[i + 1])));
                for (const auto& other : view.guards) {
                    const auto begin = (std::max)(std::size_t(guard.rva), std::size_t(other.rva));
                    const auto end = (std::min)(guard.rva + guard.bytes.size(), other.rva + other.bytes.size());
                    if (begin < end && !std::equal(guard.bytes.begin() + (begin - guard.rva),
                        guard.bytes.begin() + (end - guard.rva), other.bytes.begin() + (begin - other.rva)))
                        throw std::runtime_error("contradictory overlapping map guards");
                }
                view.guards.push_back(std::move(guard));
            }
            bounded_array(value.at("pointers"), 4096);
            for (const auto& item : value.at("pointers")) {
                if (!item.is_object() || item.size() != 2) throw std::runtime_error("invalid map pointer shape");
                view.pointers.push_back({rva(item, "addressRva", 8), rva(item, "targetRva")});
            }
            if (value.contains("enumTable")) {
                const auto& table = value.at("enumTable");
                if (!table.is_object() || table.size() != 5 || index != 4) throw std::runtime_error("unsupported map enum table");
                MapCaptureProfile::EnumTable enumeration;
                enumeration.stride = number(table, "entryStride", 8, 4096);
                enumeration.name_offset = number(table, "namePointerOffset", 0, enumeration.stride - 8);
                enumeration.value_offset = number(table, "valueOffset", 0, enumeration.stride - 8);
                bounded_array(table.at("names"), 32);
                if (table.at("names").empty()) throw std::runtime_error("empty map enum table");
                for (const auto& entry : table.at("names")) enumeration.names.push_back(name(entry));
                enumeration.rva = rva(table, "tableRva", enumeration.stride * static_cast<std::uint32_t>(enumeration.names.size()));
                view.enumeration = std::move(enumeration);
            }
        }
        return {std::move(result), "ready"};
    } catch (const nlohmann::json::exception& error) {
        return {{}, std::string("invalid-map-capture-profile: ") + error.what()};
    } catch (const std::runtime_error& error) {
        return {{}, std::string("invalid-map-capture-profile: ") + error.what()};
    }
}
}
