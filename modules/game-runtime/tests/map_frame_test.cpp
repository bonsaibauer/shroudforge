#include "map/read_map_frame.h"
#include "map/hud_visibility.cpp"
#include <nlohmann/json.hpp>
#include <fstream>
#include "profiles/read_map_capture_profile.h"
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <source_location>
#include <vector>

namespace {
KfcRuntimeCompatibility::HudCaptureSelection hud_selection;
KfcRuntimeCompatibility::MapCaptureSelection map_selection;
std::vector<std::uint8_t> hud_memory;
bool retained_call_owned{};
}
namespace EngineContext {
KfcRuntimeCompatibility::HudCaptureSelection ConfiguredHudCaptureProfile() { return hud_selection; }
KfcRuntimeCompatibility::MapCaptureSelection ConfiguredMapCaptureProfile() { return map_selection; }
}
namespace MapRuntime {
bool SharedCallOwned(std::size_t, std::size_t, void*) { return retained_call_owned; }
bool InstallSharedCall(std::size_t, std::size_t, void*) { return false; }
}

namespace {
unsigned checks{};
void check(bool ok, std::source_location at = std::source_location::current()) {
    ++checks;
    if (!ok) { std::fprintf(stderr, "map frame check %u failed at %u\n", checks, at.line()); std::exit(1); }
}
template<class T> void put(std::vector<std::uint8_t>& bytes, std::size_t at, const T& value) {
    std::memcpy(bytes.data() + at, &value, sizeof(value));
}
}

int main() {
    // Native renderer bytes remain mandatory even when our retained CALL is
    // independently normalized. A foreign detour cannot inherit admission.
    for (const char* build : {"26430625", "26082781"}) {
        const auto path = std::string(SHROUDFORGE_TEST_PROFILE_DIRECTORY) + "/enshrouded/client/" + build + ".revision-1.json";
        const auto profile = nlohmann::json::parse(std::ifstream(path));
        map_selection = KfcRuntimeCompatibility::ReadMapCaptureProfile(profile);
        if (!map_selection.profile) std::fprintf(stderr, "map profile error: %s\n", map_selection.status.c_str());
        check(map_selection.profile && map_selection.status == "ready");
        const auto selected_map_profile = map_selection.profile;
        const auto& model = *selected_map_profile;
        if (std::string(build)=="26430625") {
            check(model.execution && model.execution_status=="ready");
            check(model.execution->iterator.query_bytes==0x840 && model.execution->hooks[0].original.size()==18);
            check(model.execution->discovery_unavailable_reason=="discovery-position-layout-unproved");
            auto malformed=profile;
            malformed["mapCapture"]["execution"]["hooks"]["camera"]["original"]="9090909090";
            auto rejected=KfcRuntimeCompatibility::ReadMapCaptureProfile(malformed);
            check(rejected.profile && !rejected.profile->execution && rejected.profile->execution_status.starts_with("invalid-map-capture-execution:"));
            malformed=profile;
            malformed["mapCapture"]["execution"]["iterator"]["rowsOffset"]=0x840;
            rejected=KfcRuntimeCompatibility::ReadMapCaptureProfile(malformed);
            check(rejected.profile && !rejected.profile->execution);
            malformed=profile;
            malformed["mapCapture"]["execution"]["hooks"]["weather"]["targetRva"]=1;
            rejected=KfcRuntimeCompatibility::ReadMapCaptureProfile(malformed);
            check(rejected.profile && !rejected.profile->execution);
        } else {
            check(!model.execution && model.execution_status=="legacy-iterator-extent-unproved");
        }
        std::vector<std::uint8_t> map_image(model.image_size);
        const auto image_base = reinterpret_cast<std::uintptr_t>(map_image.data());
        IMAGE_DOS_HEADER dos{}; dos.e_magic = IMAGE_DOS_SIGNATURE; dos.e_lfanew = 0x80;
        IMAGE_NT_HEADERS64 nt{}; nt.Signature = IMAGE_NT_SIGNATURE; nt.FileHeader.Machine = IMAGE_FILE_MACHINE_AMD64;
        nt.FileHeader.TimeDateStamp = model.timestamp; nt.OptionalHeader.Magic = IMAGE_NT_OPTIONAL_HDR64_MAGIC;
        nt.OptionalHeader.SizeOfImage = model.image_size;
        put(map_image, 0, dos); put(map_image, 0x80, nt);
        for (const auto& view : model.views) for (const auto& guard : view.guards)
            std::memcpy(map_image.data() + guard.rva, guard.bytes.data(), guard.bytes.size());
        std::size_t strings = model.image_size - 8192;
        const auto string_pointer = [&](const std::string& text) {
            const auto at = strings;
            strings += text.size() + 1;
            check(strings <= map_image.size());
            bool clear = true;
            for (const auto& view : model.views) for (const auto& guard : view.guards)
                clear &= strings <= guard.rva || at >= guard.rva + guard.bytes.size();
            check(clear);
            std::memcpy(map_image.data() + at, text.c_str(), text.size() + 1);
            return image_base + at;
        };
        const auto named = [&](const KfcRuntimeCompatibility::MapCaptureProfile::System& system) {
            put(map_image, system.descriptor, string_pointer(system.name));
            put(map_image, system.descriptor + 8, std::uint64_t(system.name.size()));
            put(map_image, system.descriptor + 16, image_base + system.callback);
        };
        for (const auto& system : model.required_systems) named(system);
        for (const auto& view : model.views) {
            for (const auto& system : view.systems) named(system);
            for (const auto& pointer : view.pointers) put(map_image, pointer.address, image_base + pointer.target);
            if (view.enumeration) {
                const auto& table = *view.enumeration;
                for (std::size_t i = 0; i < table.names.size(); ++i) {
                    put(map_image, table.rva + i * table.stride + table.name_offset, string_pointer(table.names[i]));
                    put(map_image, table.rva + i * table.stride + table.value_offset, std::uint64_t(i));
                }
            }
        }
        auto admitted = MapRuntime::InspectMapImage(map_image, model.image_sha256, true);
        if (admitted.status != SF_MAP_READY) std::fprintf(stderr, "image admission error: %s at %zu\n", admitted.failure_reason, admitted.failure_rva);
        check(admitted.status == SF_MAP_READY && admitted.player && admitted.renderer && admitted.markers &&
              admitted.clock && admitted.weather && admitted.fog && admitted.world && admitted.discovery && admitted.deaths);
        check(MapRuntime::InspectMapImage(map_image, model.image_sha256, false).status == SF_MAP_CAPABILITY_DENIED);
        check(MapRuntime::InspectMapImage(map_image, std::string(64, '0'), true).status == SF_MAP_UNSUPPORTED_BUILD);
        check(MapRuntime::InspectMapImage({map_image.data(), 10}, model.image_sha256, true).status == SF_MAP_UNSUPPORTED_BUILD);
        check(MapRuntime::InspectRendererImage(map_image, model.image_sha256).renderer);
        const auto renderer_rva = model.views[1].guards[0].rva;
        map_image[renderer_rva] ^= 1;
        check(!MapRuntime::InspectRendererImage(map_image, model.image_sha256).renderer);
        check(MapRuntime::InspectMapImage(map_image, model.image_sha256, true).status ==
            (model.all_views ? SF_MAP_UNSUPPORTED_BUILD : SF_MAP_READY));
        map_image[renderer_rva] ^= 1;
        const auto descriptor = model.required_systems.empty() ? model.views[0].systems[0].descriptor : model.required_systems[0].descriptor;
        std::uintptr_t callback{};
        std::memcpy(&callback, map_image.data() + descriptor + 16, sizeof(callback));
        put(map_image, descriptor + 16, std::uintptr_t{});
        check(MapRuntime::InspectMapImage(map_image, model.image_sha256, true).status == SF_MAP_UNSUPPORTED_BUILD);
        check(MapRuntime::InspectRendererImage(map_image, model.image_sha256).renderer);
        put(map_image, descriptor + 16, callback);
        for (const auto& view : model.views) if (!view.pointers.empty()) {
            const auto pointer = view.pointers[0];
            put(map_image, pointer.address, std::uintptr_t{});
            check(MapRuntime::InspectMapImage(map_image, model.image_sha256, true).status == SF_MAP_UNSUPPORTED_BUILD);
            put(map_image, pointer.address, image_base + pointer.target);
            break;
        }
        if (model.views[4].enumeration) {
            const auto& table = *model.views[4].enumeration;
            put(map_image, table.rva + table.value_offset, std::uint64_t(99));
            const auto wrong_enum = MapRuntime::InspectMapImage(map_image, model.image_sha256, true);
            check(!wrong_enum.weather && wrong_enum.status == SF_MAP_READY);
            check(MapRuntime::InspectRendererImage(map_image, model.image_sha256).renderer);
            put(map_image, table.rva + table.value_offset, std::uint64_t{});
            for (unsigned kind = 0; kind < 6; ++kind) {
                auto malformed = profile;
                auto& value = malformed["mapCapture"]["views"]["weather"]["enumTable"];
                switch (kind) {
                case 0: value["entryStride"] = 7; break;
                case 1: value["namePointerOffset"] = 33; break;
                case 2: value["valueOffset"] = -1; break;
                case 3: value["tableRva"] = model.image_size - 1; break;
                case 4: value["names"] = nlohmann::json::array(); break;
                case 5: value["names"][0] = std::string("Clear\0Hidden", 12); break;
                }
                check(!KfcRuntimeCompatibility::ReadMapCaptureProfile(malformed).profile);
            }
        }
        for (unsigned kind = 0; kind < 19; ++kind) {
            auto malformed = profile;
            auto& value = malformed["mapCapture"];
            auto& player = value["views"]["player"];
            switch (kind) {
            case 0: value["id"] = "unknown"; break;
            case 1: value["guardChunkBytes"] = 512; break;
            case 2: value["readiness"] = "trust-mod"; break;
            case 3: value["deathsRequireWorld"] = 1; break;
            case 4: value["views"].erase("renderer"); break;
            case 5: player["guards"][0]["bytes"] = "zz"; break;
            case 6: player["guards"][0]["bytes"] = "0"; break;
            case 7: player["guards"][0]["bytes"] = std::string(131074, '0'); break;
            case 8: player["guards"][0]["guardRva"] = model.image_size; break;
            case 9: player["guards"][0]["guardRva"] = -1; break;
            case 10: player["guards"][0]["guardRva"] = 1.25; break;
            case 11: player["guards"] = nlohmann::json::array(); break;
            case 12: player["pointers"] = {{{"addressRva", model.image_size - 7}, {"targetRva", 1}}}; break;
            case 13: player["pointers"] = {{{"addressRva", 0x1000}, {"targetRva", model.image_size}}}; break;
            case 14: player["systems"] = {{{"descriptorRva", 0x1000}, {"callbackRva", 0x2000}, {"name", std::string(96, 'x')}}}; break;
            case 15: player["systems"] = {{{"descriptorRva", model.image_size - 23}, {"callbackRva", 0x2000}, {"name", "x"}}}; break;
            case 16: value["unknown"] = true; break;
            case 17: malformed["target"] = "enshrouded_server.exe"; break;
            case 18: { auto conflicting = player["guards"][0]; conflicting["bytes"] = "00"; player["guards"].push_back(conflicting); break; }
            }
            const auto rejected = KfcRuntimeCompatibility::ReadMapCaptureProfile(malformed);
            check(!rejected.profile && rejected.status.starts_with("invalid-map-capture-profile:"));
        }
        map_selection = {};
        check(!MapRuntime::InspectRendererImage(map_image, model.image_sha256).renderer);
        hud_selection = KfcRuntimeCompatibility::ReadHudCaptureProfile(profile);
        check(hud_selection.profile && hud_selection.status == "ready");
        const auto selected = hud_selection.profile;
        const auto& pins = selected->guards;
        hud_memory.assign(profile.at("image").at("size").get<std::size_t>(), 0);
        for (const auto& pin : pins) std::memcpy(hud_memory.data() + pin.rva, pin.bytes.data(), pin.bytes.size());
        MapRuntime::hud_prepared = false;
        MapRuntime::hud_preflighted = false;
        check(MapRuntime::PreflightHudPreparation(reinterpret_cast<std::uintptr_t>(hud_memory.data()), hud_memory.size()));
        check(MapRuntime::NativeOwnership(*selected, false));
        hud_memory[selected->renderer_rva] = 0xe9;
        check(!MapRuntime::NativeOwnership(*selected, false));
        hud_memory[selected->renderer_rva] = pins[0].bytes[0];
        hud_memory[selected->call_rva + 1] ^= 1;
        check(!MapRuntime::NativeOwnership(*selected, false));
        retained_call_owned = false;
        check(!MapRuntime::NativeOwnership(*selected, true));
        retained_call_owned = true;
        check(MapRuntime::NativeOwnership(*selected, true));
        hud_memory[selected->renderer_rva] = 0xe9;
        check(!MapRuntime::NativeOwnership(*selected, true));
        hud_memory[selected->renderer_rva] = pins[0].bytes[0];
        hud_memory[selected->call_rva + 6] ^= 1;
        check(!MapRuntime::NativeOwnership(*selected, true));
        hud_memory[selected->call_rva + 6] ^= 1;
        MapRuntime::hud_size = selected->call_rva + 4;
        check(!MapRuntime::NativeOwnership(*selected, true));
        MapRuntime::hud_size = hud_memory.size();
        MapRuntime::hud_active = true;
        MapRuntime::observation = (1000ULL << 8) | selected->gameplay_screen;
        check(MapRuntime::HudGameplay(1500) && !MapRuntime::HudGameplay(1501));
        check(!MapRuntime::HudKnown(999));
        MapRuntime::DisableHudVisibility();
        check(!MapRuntime::HudKnown(1000));
        hud_selection = {};
        check(MapRuntime::hud_profile.load() == selected); // Forwarding keeps its original contract.
        check(!MapRuntime::PreflightHudPreparation(reinterpret_cast<std::uintptr_t>(hud_memory.data()), hud_memory.size()));
    }
    SFMapMarkerV1 identity{};const std::uint64_t identity_words[5]{UINT64_C(0x2233445566778899),UINT64_C(0x0102030405060708),3,4,5};
    MapRuntime::SetMarkerIdentity(identity,identity_words);
    check(identity.id==identity_words[0] && identity.icon_candidate_count==10);
    check(identity.icon_candidates[0]==0x66778899 && identity.icon_candidates[1]==0x22334455 && identity.icon_candidates[2]==0x05060708 && identity.icon_candidates[3]==0x01020304 && identity.icon_candidates[9]==0);
    MapRuntime::SnapshotStore bounds;bounds.SetAdmission(SF_MAP_FEATURE_DISCOVERY);bounds.SetSession(1,true,false,true,100);
    identity.position[0]=identity.position[2]=100;identity.icon_candidate_count=11;
    check(!bounds.PublishMarkers(MapRuntime::Collection::Discovery,1,{identity},100));

    using namespace MapRuntime;
    SnapshotStore store;
    auto unavailable = store.Acquire(0, 100);
    check(unavailable.frame.status == SF_MAP_DISABLED);
    store.SetAdmission(SF_MAP_FEATURE_PLAYER | SF_MAP_FEATURE_FOG | SF_MAP_FEATURE_MARKERS |
        SF_MAP_FEATURE_ENVIRONMENT | SF_MAP_FEATURE_CLOCK | SF_MAP_FEATURE_WEATHER |
        SF_MAP_FEATURE_WORLD | SF_MAP_FEATURE_DEATHS | SF_MAP_FEATURE_DISCOVERY);
    check(store.Acquire(0, 100).frame.status == SF_MAP_NO_SESSION);
    store.SetSession(9, true, false, true, 100);
    check(store.Acquire(8, 100).frame.status == SF_MAP_WRONG_GENERATION);
    const auto generation = store.Generation();
    SFMapPoseV1 pose{};
    pose.position[0] = 12.5f; pose.position[1] = 42; pose.position[2] = 14;
    pose.heading_radians = .5f; pose.flags = SF_MAP_POSE_VALID | SF_MAP_POSE_HEADING;
    check(store.PublishPlayer(generation, pose, pose, 100));
    SFMapMarkerV1 marker{}; marker.id = 17; marker.position[0] = 22; marker.position[2] = 32;
    marker.flags = SF_MAP_MARKER_VISIBLE;
    check(store.PublishMarkers(Collection::RemotePlayers, generation, {marker}, 100));
    check(store.PublishMarkers(Collection::Waypoints, generation, {}, 100));
    auto first = store.Acquire(9, 101);
    check(first.frame.status == SF_MAP_READY);
    check(first.frame.player.meta.status == SF_MAP_READY && first.frame.player.pose.position[0] == 12.5f);
    check(first.frame.remote_players.meta.status == SF_MAP_READY && first.frame.remote_players.count == 1);
    check(first.frame.waypoints.meta.status == SF_MAP_READY && first.frame.waypoints.count == 0);
    check(first.frame.pings.meta.status == SF_MAP_NOT_CAPTURED);
    auto stale = store.Acquire(9, 2101);
    check(stale.frame.remote_players.meta.status == SF_MAP_STALE && stale.frame.remote_players.count == 0);
    check(stale.frame.player.meta.status == SF_MAP_READY);
    check(store.Acquire(9, 15101).frame.player.meta.status == SF_MAP_STALE);
    check(store.Acquire(9, 99).frame.player.meta.status == SF_MAP_STALE);
    auto fog = std::make_shared<FogSnapshot>();
    fog->world_width = fog->world_height = 256; fog->columns = fog->rows = 1;
    fog->tiles.assign(1024, 255);
    check(store.PublishFog(generation, fog, 101));
    auto fogLease = store.Acquire(9, 102);
    check(fogLease.frame.fog.byte_count == 1024 && fogLease.frame.fog.tiles[17] == 255);
    const auto* borrowedFog = fogLease.frame.fog.tiles;
    check(store.Acquire(9, 103).frame.fog.tiles == borrowedFog);
    // Replacing a published snapshot must not invalidate an earlier lease.
    auto nextFog = std::make_shared<FogSnapshot>();
    nextFog->world_width = nextFog->world_height = 256; nextFog->columns = nextFog->rows = 1;
    nextFog->tiles.assign(1024, 42);
    check(store.PublishFog(generation, nextFog, 104));
    check(store.Acquire(9, 105).frame.fog.tiles != borrowedFog);
    check(borrowedFog[17] == 255);
    auto invalidFog=std::make_shared<FogSnapshot>(*nextFog);
    invalidFog->world_width=-1;
    check(!store.PublishFog(generation,invalidFog,106));
    invalidFog->world_width=512;
    check(!store.PublishFog(generation,invalidFog,106));
    store.SetSession(10, false, true, true, 110);
    check(!store.PublishPlayer(generation, pose, pose, 111));
    check(store.Acquire(9, 111).frame.status == SF_MAP_WRONG_GENERATION);
    check(store.Acquire(10, 111).frame.player.meta.status == SF_MAP_NOT_CAPTURED);
    check(first.frame.remote_players.data[0].id == 17); // Old leases remain valid without entering the new session.
    SFMapMarkerV1 invalid{}; invalid.position[0] = std::numeric_limits<float>::quiet_NaN();
    check(!store.PublishMarkers(Collection::RemotePlayers, store.Generation(), {invalid}, 112));
    check(!store.PublishMarkers(Collection::RemotePlayers, store.Generation(), std::vector<SFMapMarkerV1>(4097), 112));
    store.Disable();
    check(store.Acquire(0, 112).frame.status == SF_MAP_DISABLED);

    // Summary/group bits must not admit siblings whose independent guards failed.
    store.SetAdmission(SF_MAP_FEATURE_PLAYER | SF_MAP_FEATURE_ENVIRONMENT | SF_MAP_FEATURE_WORLD);
    store.SetSession(11,true,false,true,200);
    auto grouped=store.Acquire(11,201).frame;
    check(grouped.environment.clock_meta.status==SF_MAP_UNSUPPORTED_BUILD);
    check(grouped.environment.weather_meta.status==SF_MAP_UNSUPPORTED_BUILD);
    check(grouped.deaths.meta.status==SF_MAP_UNSUPPORTED_BUILD);
    check(grouped.world_markers.meta.status==SF_MAP_NOT_CAPTURED);
    SFMapEnvironmentV1 groupedEnvironment{};
    check(!store.PublishEnvironment(11,groupedEnvironment,true,200));
    check(!store.PublishEnvironment(11,groupedEnvironment,false,200));
    check(!store.PublishMarkers(Collection::Deaths,11,{},200));
    check(store.PublishMarkers(Collection::World,11,{},200));
    grouped=store.Acquire(11,201).frame;
    check(grouped.world_markers.meta.status==SF_MAP_READY && grouped.world_markers.count==0);

    NativeTransform player{{std::int64_t(12.5 * 4294967296.0), 42LL << 32, 14LL << 32}, {0, 0, 0, 1}};
    NativeTransform camera = player; camera.xyz[2] += 10LL << 32;
    SFMapPoseV1 decoded{}, decodedCamera{};
    check(DecodeLocalTransforms(player, camera, decoded, decodedCamera));
    check(decoded.position[0] == 12.5f && decoded.position[2] == 14 && decoded.heading_radians == 0);
    camera.quaternion[3] = 4;
    check(!DecodeLocalTransforms(player, camera, decoded, decodedCamera));
    camera = player; camera.xyz[0] += 257LL << 32;
    check(!DecodeLocalTransforms(player, camera, decoded, decodedCamera));
    std::int64_t clock[5]{6LL*3600000000000, 18LL*3600000000000, 0, 0, 12LL*3600000000000};
    SFMapEnvironmentV1 environment{};
    check(DecodeClock(clock, environment) && environment.minute_of_day == 720 && environment.daytime == 1);
    clock[4] = 24LL*3600000000000; check(!DecodeClock(clock, environment));
    float weather[4]{.1f,.2f,.6f,.1f};
    check(DecodeWeather(weather, environment) && environment.weather_kind == 2);
    weather[3] = .5f; check(!DecodeWeather(weather, environment));
    std::vector<std::uint8_t> entries(0xa0);
    put(entries, 0x10, player.xyz); put(entries, 0x30, std::uint32_t(0xA447CBA3));
    std::vector<SFMapMarkerV1> decodedMarkers;
    check(DecodeWorldMarkers(entries, 1, true, decodedMarkers));
    check(decodedMarkers.size() == 1 && decodedMarkers[0].position[0] == 12.5f &&
        decodedMarkers[0].kind == 0xA447CBA3 && (decodedMarkers[0].flags & SF_MAP_MARKER_CUSTOM));
    check(!DecodeWorldMarkers(entries, 2, false, decodedMarkers));
    check(!DecodeWorldMarkers(entries, 2049, false, decodedMarkers));
    check(DecodeWorldMarkers({}, 0, false, decodedMarkers) && decodedMarkers.empty());
    NativeFogHeader header{256,256,1,1,0,0,1};
    std::vector<std::uint8_t> tiles(1024, 127);
    header.data = reinterpret_cast<std::uintptr_t>(tiles.data());
    FogSnapshot readFog;
    check(ReadFog(reinterpret_cast<std::uintptr_t>(&header), readFog) && readFog.tiles[4] == 127);
    header.tile_count = 2; check(!ReadFog(reinterpret_cast<std::uintptr_t>(&header), readFog));
    check(!ReadFog(1, readFog));
    std::printf("PASS: %u native map capture/snapshot checks\n", checks);
}
