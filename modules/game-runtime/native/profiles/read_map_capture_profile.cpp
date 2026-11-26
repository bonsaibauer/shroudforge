#include "profiles/read_map_capture_profile.h"
#include <nlohmann/json.hpp>
#include <algorithm>
#include <stdexcept>
#include <cstring>

namespace KfcRuntimeCompatibility {
namespace {
void read_execution(const nlohmann::json& model, MapCaptureProfile& profile) {
    using json = nlohmann::json;
    if (!model.contains("execution")) return;
    try {
        const auto& data = model.at("execution");
        if (data.is_object() && data.size() == 1 && data.contains("unavailableReason")) {
            const auto reason = data.at("unavailableReason").get<std::string>();
            if (reason.empty() || reason.size() > 160 || std::any_of(reason.begin(),reason.end(),[](unsigned char c){return c<32 || c==127;})) throw std::runtime_error("invalid capture unavailability reason");
            profile.execution_status = reason;
            return;
        }
        if (!data.is_object() || data.size() != 6 || data.at("id") != "map-capture-checked-v1")
            throw std::runtime_error("unsupported map capture execution ABI");
        const auto number = [](const json& source, const char* key, std::uint64_t min, std::uint64_t max) {
            const auto& value = source.at(key);
            if (!value.is_number_integer() || (!value.is_number_unsigned() && value.get<std::int64_t>() < 0) ||
                value.get<std::uint64_t>() < min || value.get<std::uint64_t>() > max)
                throw std::runtime_error(std::string("invalid capture execution value: ") + key);
            return static_cast<std::uint32_t>(value.get<std::uint64_t>());
        };
        const auto guarded = [&](std::uint32_t rva, const std::vector<std::uint8_t>& bytes) {
            for (const auto& view : profile.views) for (const auto& guard : view.guards)
                if (rva >= guard.rva && rva - guard.rva <= guard.bytes.size() && bytes.size() <= guard.bytes.size() - (rva - guard.rva) &&
                    std::equal(bytes.begin(), bytes.end(), guard.bytes.begin() + (rva - guard.rva))) return true;
            return false;
        };
        MapCaptureProfile::Execution execution;
        const auto& unavailable=data.at("unavailableViews");
        if(!unavailable.is_object() || unavailable.size()!=1 || !unavailable.contains("discovery"))
            throw std::runtime_error("unsupported capture view availability");
        execution.discovery_unavailable_reason=unavailable.at("discovery").get<std::string>();
        if(execution.discovery_unavailable_reason.empty() || execution.discovery_unavailable_reason.size()>160 ||
           std::any_of(execution.discovery_unavailable_reason.begin(),execution.discovery_unavailable_reason.end(),[](unsigned char c){return c<32 || c==127;}))
            throw std::runtime_error("invalid discovery unavailability reason");
        const char* names[]{"camera", "waypoints", "remote", "clock", "weather", "fog", "world"};
        const auto& hooks = data.at("hooks");
        if (!hooks.is_object() || hooks.size() != 7) throw std::runtime_error("incomplete capture hook catalog");
        for (std::size_t i = 0; i != 7; ++i) {
            const auto& value = hooks.at(names[i]);
            const bool call = i == 4 || i == 5;
            if (!value.is_object() || value.size() != (call ? 4 : 3) || value.at("kind") != (call ? "call-v1" : "entry-four-args-v1"))
                throw std::runtime_error("unsupported capture hook kind");
            auto& hook = execution.hooks[i];
            hook.rva = number(value, "rva", 1, profile.image_size - 32);
            const auto hex = value.at("original").get<std::string>();
            if (hex.size() < 10 || hex.size() > 64 || hex.size() % 2) throw std::runtime_error("invalid capture hook bytes");
            const auto nibble = [](char c) -> unsigned {
                if (c >= '0' && c <= '9') return c - '0';
                if (c >= 'a' && c <= 'f') return c - 'a' + 10;
                throw std::runtime_error("invalid capture hook hex");
            };
            for (std::size_t j = 0; j < hex.size(); j += 2) hook.original.push_back(static_cast<std::uint8_t>((nibble(hex[j]) << 4) | nibble(hex[j + 1])));
            if (!guarded(hook.rva, hook.original)) throw std::runtime_error("capture hook original lacks an admitted image guard");
            for (std::size_t j = 0; j < i; ++j) {
                const auto& other = execution.hooks[j];
                if (hook.rva < other.rva + other.original.size() && other.rva < hook.rva + hook.original.size())
                    throw std::runtime_error("overlapping capture hooks");
            }
            if (call) {
                hook.target = number(value, "targetRva", 1, profile.image_size - 1);
                std::int32_t displacement{};
                if (hook.original.size() != 5 || hook.original[0] != 0xe8) throw std::runtime_error("capture hook is not a relative CALL");
                std::memcpy(&displacement, hook.original.data() + 1, 4);
                if (std::int64_t(hook.rva) + 5 + displacement != hook.target) throw std::runtime_error("capture CALL target mismatch");
            }
        }
        // The independently verified camera relay contract uses an 18-byte entry.
        if (execution.hooks[0].original.size() != 18) throw std::runtime_error("unsupported camera relay extent");
        const auto entry_rva = [&](const json& value, const char* key, std::size_t minimum=1) {
            const auto rva = number(value, key, 1, profile.image_size - 1);
            bool found{};
            for (const auto& view : profile.views) for (const auto& guard : view.guards) found |= guard.rva == rva && guard.bytes.size()>=minimum;
            if (!found) throw std::runtime_error("capture function entry lacks an image guard");
            return rva;
        };
        const auto& functions = data.at("functions");
        if (!functions.is_object() || functions.size() != 4) throw std::runtime_error("invalid capture function catalog");
        execution.iter_init = entry_rva(functions, "iterInitRva"); execution.iter_next = entry_rva(functions, "iterNextRva");
        execution.get_world = entry_rva(functions, "getWorldRva"); execution.lookup = entry_rva(functions, "lookupRva");
        entry_rva(hooks.at("weather"), "targetRva"); entry_rva(hooks.at("fog"), "targetRva",15);
        const auto& layout = data.at("layout");
        if (!layout.is_object() || layout.size() != 11) throw std::runtime_error("invalid capture record layout");
        const auto offset = [&](const char* key) {return number(layout, key, 0, 0x4000000);};
        execution.entity_array=offset("entityArrayOffset");execution.entity_stride=offset("entityStride");
        execution.remote_stride=offset("remoteStride");execution.local_id=offset("localIdOffset");
        execution.waypoint_position=offset("waypointPositionOffset");execution.clock_values=offset("clockValuesOffset");
        execution.custom_list=offset("customListOffset");execution.world_list=offset("worldListOffset");
        execution.secondary_list=offset("secondaryListOffset");execution.death_list=offset("deathListOffset");execution.marker_stride=offset("markerStride");
        if (execution.entity_stride < 4 || execution.entity_stride > 4096 || execution.remote_stride < 4 || execution.remote_stride > 4096 ||
            execution.marker_stride != 160) throw std::runtime_error("unsupported capture record stride");
        const auto& iterator = data.at("iterator");
        if (!iterator.is_object() || iterator.size() != 12 || iterator.at("id") != "native-iterator-extent-v1")
            throw std::runtime_error("unsupported capture iterator ABI");
        auto& it=execution.iterator;
        it.query_bytes=number(iterator,"queryBytes",32,65536);
        const auto field = [&](const char* key, unsigned width) {return number(iterator,key,0,it.query_bytes-width);};
        it.indirect=field("indirectOffset",16);it.source_counts=field("sourceCountsOffset",24);it.list_counts=field("listCountsOffset",12);
        it.n5=field("n5Offset",1);it.n4=field("n4Offset",1);it.rows=field("rowsOffset",8);
        it.p3_count=number(iterator,"p3CountOffset",0,65535);it.p8_count=number(iterator,"p8CountOffset",0,65535);
        it.query_copy_a=field("queryCopyAOffset",1);it.query_copy_b=field("queryCopyBOffset",1);
        if (execution.entity_array > it.query_bytes - 8) throw std::runtime_error("entity array exceeds iterator control extent");
        profile.execution=std::move(execution);profile.execution_status="ready";
    } catch (const std::exception& error) {
        profile.execution_status=std::string("invalid-map-capture-execution: ")+error.what();
    }
}
}
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
            !model.is_object() || model.size() != (model.contains("execution") ? 7 : 6) || model.at("id") != "map-image-admission-v1")
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
        read_execution(model,*result);
        return {std::move(result), "ready"};
    } catch (const nlohmann::json::exception& error) {
        return {{}, std::string("invalid-map-capture-profile: ") + error.what()};
    } catch (const std::runtime_error& error) {
        return {{}, std::string("invalid-map-capture-profile: ") + error.what()};
    }
}
}
