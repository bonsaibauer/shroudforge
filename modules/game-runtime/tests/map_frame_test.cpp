#include "map/read_map_frame.h"
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <source_location>
#include <vector>

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
