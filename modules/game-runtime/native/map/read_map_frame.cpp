// Capture decoding adapted from ESM Minimap 0.5.16-gpu-test.4, MIT,
// Copyright (c) 2024 Miguel Oppermann. See map/SOURCE-NOTICE.txt.
#include "map/read_map_frame.h"
#include <windows.h>
#include <algorithm>
#include <cmath>
#include <cstring>

namespace MapRuntime {
namespace {
bool valid_position(const float* xyz) {
    return std::isfinite(xyz[0]) && std::isfinite(xyz[1]) && std::isfinite(xyz[2]) &&
        xyz[0] > 1 && xyz[2] > 1 && xyz[0] < 10304 && xyz[2] < 10304 &&
        xyz[1] > -2000 && xyz[1] < 5000;
}
bool pose(const NativeTransform& source, SFMapPoseV1& result) {
    SFMapPoseV1 candidate{};
    for (unsigned i = 0; i < 3; ++i) candidate.position[i] = float(double(source.xyz[i]) / 4294967296.0);
    if (!valid_position(candidate.position)) return false;
    const auto* q = source.quaternion;
    const float n = q[0]*q[0]+q[1]*q[1]+q[2]*q[2]+q[3]*q[3];
    if (!std::isfinite(n) || n < .9f || n > 1.1f) return false;
    candidate.heading_radians = std::atan2(2*(q[3]*q[1]+q[0]*q[2])/n, 1-2*(q[1]*q[1]+q[2]*q[2])/n);
    candidate.flags = SF_MAP_POSE_VALID | SF_MAP_POSE_HEADING;
    result = candidate;
    return true;
}
SFMapSnapshotMetaV1 unavailable(std::uint32_t status, std::uint64_t generation) {
    SFMapSnapshotMetaV1 result{}; result.status = status; result.session_generation = generation;
    result.clock_domain = SF_MAP_CLOCK_DOMAIN_WINDOWS_TICK64; return result;
}
bool fresh(SFMapSnapshotMetaV1& meta, std::uint64_t generation, std::uint64_t now) {
    if (meta.status != SF_MAP_READY) return false;
    if (meta.session_generation != generation) { meta.status = SF_MAP_WRONG_GENERATION; return false; }
    if (meta.clock_domain != SF_MAP_CLOCK_DOMAIN_WINDOWS_TICK64 || now < meta.published_at_ms ||
        now - meta.published_at_ms > meta.freshness_limit_ms) { meta.status = SF_MAP_STALE; return false; }
    return true;
}
std::uint32_t collection_feature(Collection collection) {
    return collection == Collection::World ? SF_MAP_FEATURE_WORLD :
        collection == Collection::Deaths ? SF_MAP_FEATURE_DEATHS :
        collection == Collection::Discovery ? SF_MAP_FEATURE_DISCOVERY : SF_MAP_FEATURE_MARKERS;
}
}
void SetMarkerIdentity(SFMapMarkerV1& marker,const std::uint64_t (&words)[5]) {
    marker.id=words[0];marker.icon_candidate_count=10;
    for(unsigned i=0;i<5;++i){marker.icon_candidates[i*2]=std::uint32_t(words[i]);marker.icon_candidates[i*2+1]=std::uint32_t(words[i]>>32);}
}
bool ReadMemory(std::uintptr_t address, void* output, std::size_t size) noexcept {
    if (!address || !output || address > UINTPTR_MAX - size) return false;
    __try { std::memcpy(output, reinterpret_cast<const void*>(address), size); return true; }
    __except (EXCEPTION_EXECUTE_HANDLER) { return false; }
}
bool DecodeLocalTransforms(const NativeTransform& player, const NativeTransform& camera,
    SFMapPoseV1& out_player, SFMapPoseV1& out_camera) noexcept {
    SFMapPoseV1 p{}, c{};
    if (!pose(player, p) || !pose(camera, c)) return false;
    const auto dx = p.position[0]-c.position[0], dz = p.position[2]-c.position[2];
    if (dx*dx+dz*dz > 256*256) return false;
    p.heading_radians = c.heading_radians;
    out_player = p; out_camera = c; return true;
}
bool DecodeClock(const std::int64_t (&fields)[5], SFMapEnvironmentV1& result) noexcept {
    constexpr std::int64_t minute = 60000000000, day = 86400000000000;
    if (fields[0] < 0 || fields[1] <= fields[0] || fields[1] > day || fields[4] < 0 || fields[4] >= day) return false;
    result.minute_of_day = std::int32_t(fields[4] / minute);
    result.daytime = fields[4] >= fields[0] && fields[4] < fields[1]; return true;
}
bool DecodeWeather(const float (&weights)[4], SFMapEnvironmentV1& result) noexcept {
    float sum{}; unsigned strongest{};
    for (unsigned i=0;i<4;++i) {
        if (!std::isfinite(weights[i]) || weights[i] < -.0001f || weights[i] > 1.0001f) return false;
        sum += weights[i]; if (weights[i] > weights[strongest]) strongest=i;
    }
    if (std::abs(sum-1) > .01f) return false;
    result.weather_kind = strongest; return true;
}
bool DecodeWorldMarkers(std::span<const std::uint8_t> bytes, std::size_t count, bool custom,
    std::vector<SFMapMarkerV1>& result) {
    result.clear();
    if (count > 2048 || bytes.size() < count * 0xa0) return false;
    std::vector<SFMapMarkerV1> pending; pending.reserve(count);
    for (std::size_t i=0;i<count;++i) {
        const auto* entry = bytes.data()+i*0xa0;
        SFMapMarkerV1 marker{}; std::int64_t xyz[3]{};std::uint64_t words[5]{};
        std::memcpy(words,entry,sizeof(words));SetMarkerIdentity(marker,words);
        std::memcpy(xyz, entry+0x10, sizeof(xyz)); std::memcpy(&marker.kind, entry+0x30, 4);
        for (unsigned axis=0;axis<3;++axis) marker.position[axis]=float(double(xyz[axis])/4294967296.0);
        if (!valid_position(marker.position)) continue;
        marker.flags = SF_MAP_MARKER_VISIBLE | (custom ? SF_MAP_MARKER_CUSTOM : 0);
        pending.push_back(marker);
    }
    result = std::move(pending); return true;
}
bool ReadFog(std::uintptr_t address, FogSnapshot& result) {
    result = {}; NativeFogHeader header{}, after{};
    if (!ReadMemory(address,&header,sizeof(header)) || !std::isfinite(header.world_width) ||
        !std::isfinite(header.world_height) || header.world_width < 256 || header.world_height < 256 ||
        header.world_width > 32768 || header.world_height > 32768 || !header.columns || !header.rows ||
        header.columns > 128 || header.rows > 128 ||
        header.columns != (unsigned(header.world_width * .125f)+31)/32 ||
        header.rows != (unsigned(header.world_height * .125f)+31)/32 ||
        header.tile_count != std::uint64_t(header.columns)*header.rows || !header.data) return false;
    FogSnapshot pending; pending.tiles.resize(std::size_t(header.tile_count)*1024);
    if (!ReadMemory(header.data,pending.tiles.data(),pending.tiles.size()) ||
        !ReadMemory(address,&after,sizeof(after)) || std::memcmp(&header,&after,sizeof(header))) return false;
    pending.world_width=header.world_width; pending.world_height=header.world_height;
    pending.columns=header.columns; pending.rows=header.rows; result=std::move(pending); return true;
}

void SnapshotStore::Clear() { player_.reset(); markers_={}; fog_.reset(); environment_={}; }
SFMapSnapshotMetaV1 SnapshotStore::Meta(std::uint64_t now, std::uint32_t age) {
    return {SF_MAP_READY,age,generation_,++revision_,now,SF_MAP_CLOCK_DOMAIN_WINDOWS_TICK64};
}
void SnapshotStore::SetAdmission(std::uint32_t features) {
    std::lock_guard lock(mutex_); features_=features; flags_=0; generation_=0; Clear();
}
void SnapshotStore::Disable() {
    std::lock_guard lock(mutex_); features_=0; flags_=0; generation_=0; Clear();
}
void SnapshotStore::SetSession(std::uint64_t generation, bool gameplay, bool menu, bool focused, std::uint64_t now) {
    std::lock_guard lock(mutex_);
    if (generation_ != generation) { Clear(); generation_=generation; }
    flags_ = (generation ? SF_MAP_STATE_ACTIVE : 0) | (gameplay ? SF_MAP_STATE_GAMEPLAY : 0) |
        (menu ? SF_MAP_STATE_MENU : 0) | (focused ? SF_MAP_STATE_FOCUSED : 0);
    state_at_=now;
}
std::uint64_t SnapshotStore::Generation() const { std::lock_guard lock(mutex_); return generation_; }
bool SnapshotStore::PublishPlayer(std::uint64_t generation, const SFMapPoseV1& p, const SFMapPoseV1& c, std::uint64_t now) {
    if (!(p.flags & SF_MAP_POSE_VALID) || !valid_position(p.position) || !valid_position(c.position) ||
        !std::isfinite(p.heading_radians) || !std::isfinite(c.heading_radians)) return false;
    auto pending=std::make_shared<PlayerSnapshot>(); pending->value={p,c};
    std::lock_guard lock(mutex_);
    if (!generation || generation != generation_ || !(features_ & SF_MAP_FEATURE_PLAYER)) return false;
    pending->meta=Meta(now,15000); player_=std::move(pending); return true;
}
bool SnapshotStore::PublishMarkers(Collection collection, std::uint64_t generation,
    std::vector<SFMapMarkerV1> values, std::uint64_t now) {
    const auto index=std::size_t(collection);
    if (index >= markers_.size() || values.size() > 4096) return false;
    for (const auto& marker:values) if (marker.icon_candidate_count>10 || !valid_position(marker.position) || !std::isfinite(marker.heading_radians)) return false;
    auto pending=std::make_shared<MarkerSnapshot>(); pending->value=std::move(values);
    std::lock_guard lock(mutex_);
    if (!generation || generation != generation_ || !(features_ & collection_feature(collection))) return false;
    pending->meta=Meta(now,collection == Collection::Pings ? 8000 : 2000);
    markers_[index]=std::move(pending); return true;
}
bool SnapshotStore::PublishFog(std::uint64_t generation, std::shared_ptr<const FogSnapshot> value, std::uint64_t now) {
    if (!value || !value->columns || !value->rows || value->columns > 128 || value->rows > 128 ||
        value->tiles.size() != std::size_t(value->columns)*value->rows*1024 ||
        !std::isfinite(value->world_width) || !std::isfinite(value->world_height) ||
        value->world_width < 256 || value->world_height < 256 ||
        value->world_width > 32768 || value->world_height > 32768 ||
        value->columns != (unsigned(value->world_width * .125f)+31)/32 ||
        value->rows != (unsigned(value->world_height * .125f)+31)/32) return false;
    auto pending=std::make_shared<FogPublication>(); pending->value=std::move(value);
    std::lock_guard lock(mutex_);
    if (!generation || generation != generation_ || !(features_ & SF_MAP_FEATURE_FOG)) return false;
    pending->meta=Meta(now,2000); fog_=std::move(pending); return true;
}
bool SnapshotStore::PublishEnvironment(std::uint64_t generation, SFMapEnvironmentV1 value, bool clock, std::uint64_t now) {
    if ((clock && (value.minute_of_day < 0 || value.minute_of_day >= 1440 || value.daytime > 1)) ||
        (!clock && value.weather_kind > 3)) return false;
    std::lock_guard lock(mutex_);
    if (!generation || generation != generation_ || !(features_ & (clock ? SF_MAP_FEATURE_CLOCK : SF_MAP_FEATURE_WEATHER))) return false;
    if (clock) { environment_.clock_meta=Meta(now,2000); environment_.minute_of_day=value.minute_of_day; environment_.daytime=value.daytime; }
    else { environment_.weather_meta=Meta(now,2000); environment_.weather_kind=value.weather_kind; }
    return true;
}
FrameLease SnapshotStore::Acquire(std::uint64_t expected, std::uint64_t now) const {
    FrameLease result;
    auto& frame=result.frame; frame.size=sizeof(frame); frame.version=SF_MAP_SERVICE_VERSION;
    std::lock_guard lock(mutex_);
    frame.now_ms=now; frame.clock_domain=SF_MAP_CLOCK_DOMAIN_WINDOWS_TICK64;
    frame.session_generation=generation_; frame.state_flags=flags_; frame.supported_features=features_;
    frame.status=!features_ ? SF_MAP_DISABLED : !generation_ ? SF_MAP_NO_SESSION :
        expected && expected != generation_ ? SF_MAP_WRONG_GENERATION : SF_MAP_READY;
    const auto missing=[&](std::uint32_t feature) {
        return unavailable(frame.status != SF_MAP_READY ? frame.status :
            !(features_ & feature) ? SF_MAP_UNSUPPORTED_BUILD : SF_MAP_NOT_CAPTURED,generation_);
    };
    frame.player.meta=missing(SF_MAP_FEATURE_PLAYER); frame.fog.meta=missing(SF_MAP_FEATURE_FOG);
    frame.environment.clock_meta=missing(SF_MAP_FEATURE_CLOCK);
    frame.environment.weather_meta=missing(SF_MAP_FEATURE_WEATHER);
    SFMapMarkerViewV1* views[]{&frame.world_markers,&frame.remote_players,&frame.deaths,&frame.waypoints,&frame.pings,&frame.discovery};
    for (std::size_t i=0;i<result.markers.size();++i) views[i]->meta=missing(collection_feature(Collection(i)));
    if (frame.status != SF_MAP_READY) return result;
    // A lost/stale HUD observation cannot leave the preceding gameplay state on.
    if (now < state_at_ || now-state_at_ > 500) frame.state_flags &= ~SF_MAP_STATE_GAMEPLAY;
    result.player=player_; result.markers=markers_; result.fog=fog_;
    if (result.player) {
        frame.player.meta=result.player->meta;
        if (fresh(frame.player.meta,generation_,now)) { frame.player.pose=result.player->value[0]; frame.player.camera=result.player->value[1]; }
    }
    for (std::size_t i=0;i<result.markers.size();++i) if (const auto& snapshot=result.markers[i]) {
        auto& view=*views[i]; view.meta=snapshot->meta;
        if (fresh(view.meta,generation_,now)) { view.data=snapshot->value.data(); view.count=std::uint32_t(snapshot->value.size()); }
    }
    if (result.fog) {
        frame.fog.meta=result.fog->meta;
        if (fresh(frame.fog.meta,generation_,now)) {
            const auto& fog=*result.fog->value;
            frame.fog.world_width=fog.world_width; frame.fog.world_height=fog.world_height;
            frame.fog.columns=fog.columns; frame.fog.rows=fog.rows;
            frame.fog.tiles=fog.tiles.data(); frame.fog.byte_count=fog.tiles.size();
        }
    }
    frame.environment=environment_;
    if (!frame.environment.clock_meta.revision) frame.environment.clock_meta=missing(SF_MAP_FEATURE_CLOCK);
    if (!frame.environment.weather_meta.revision) frame.environment.weather_meta=missing(SF_MAP_FEATURE_WEATHER);
    fresh(frame.environment.clock_meta,generation_,now); fresh(frame.environment.weather_meta,generation_,now);
    return result;
}
}
