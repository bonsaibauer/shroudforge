#pragma once
#include <array>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <vector>
#include <nlohmann/json_fwd.hpp>
#include "map/native_iterator_snapshot.h"

namespace KfcRuntimeCompatibility {
inline constexpr std::array<const char*, 9> MapViewNames{
    "player", "renderer", "markers", "clock", "weather", "fog", "world", "deaths", "discovery"};
struct MapCaptureProfile {
    struct Hook {std::uint32_t rva{}, target{}; std::vector<std::uint8_t> original;};
    struct Execution {
        // camera, waypoints, remote players, clock, weather CALL, fog CALL, world.
        std::array<Hook, 7> hooks;
        std::uint32_t iter_init{}, iter_next{}, get_world{}, lookup{};
        std::uint32_t entity_array{}, entity_stride{}, remote_stride{}, local_id{}, waypoint_position{}, clock_values{};
        std::uint32_t custom_list{}, world_list{}, secondary_list{}, death_list{}, marker_stride{};
        NativeIteratorSnapshot::Layout iterator;
        std::string discovery_unavailable_reason;
    };
    struct Guard { std::uint32_t rva{}; std::vector<std::uint8_t> bytes; };
    struct System { std::uint32_t descriptor{}, callback{}; std::string name; };
    struct Pointer { std::uint32_t address{}, target{}; };
    struct EnumTable {
        std::uint32_t rva{}, stride{}, name_offset{}, value_offset{};
        std::vector<std::string> names;
    };
    struct View {
        std::vector<Guard> guards;
        std::vector<System> systems;
        std::vector<Pointer> pointers;
        std::optional<EnumTable> enumeration;
    };
    std::string profile_id, image_sha256;
    std::uint32_t revision{}, image_size{}, timestamp{}, guard_chunk_bytes{};
    bool all_views{}, deaths_require_world{};
    std::vector<System> required_systems;
    std::array<View, 9> views;
    std::optional<Execution> execution;
    std::string execution_status{"map-capture-execution-absent"};
};
struct MapCaptureSelection {
    std::shared_ptr<const MapCaptureProfile> profile;
    std::string status{"map-capture-profile-absent"};
};
// Data only. EngineContext admits exact identity before publishing a snapshot.
MapCaptureSelection ReadMapCaptureProfile(const nlohmann::json& exact_profile);
}
