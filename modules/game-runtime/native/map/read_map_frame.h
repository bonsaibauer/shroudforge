#pragma once
#include "abi/map_overlay_api.h"
#include <array>
#include <cstdint>
#include <memory>
#include <mutex>
#include <span>
#include <vector>

namespace MapRuntime {
enum class Collection : std::uint32_t { World, RemotePlayers, Deaths, Waypoints, Pings, Discovery, Count };
struct NativeTransform { std::int64_t xyz[3]; float quaternion[4]; };
struct NativeFogHeader {
    float world_width, world_height;
    std::uint32_t columns, rows;
    std::uintptr_t allocator, data;
    std::uint64_t tile_count;
};
static_assert(sizeof(NativeFogHeader) == 0x28);
struct FogSnapshot {
    float world_width{}, world_height{};
    std::uint32_t columns{}, rows{};
    std::vector<std::uint8_t> tiles;
};
template<class T> struct PublishedSnapshot { SFMapSnapshotMetaV1 meta{}; T value{}; };
using PlayerSnapshot = PublishedSnapshot<std::array<SFMapPoseV1, 2>>;
using MarkerSnapshot = PublishedSnapshot<std::vector<SFMapMarkerV1>>;
struct FogPublication { SFMapSnapshotMetaV1 meta{}; std::shared_ptr<const FogSnapshot> value; };
struct FrameLease {
    SFMapFrameV1 frame{};
    std::shared_ptr<const PlayerSnapshot> player;
    std::array<std::shared_ptr<const MarkerSnapshot>, static_cast<std::size_t>(Collection::Count)> markers;
    std::shared_ptr<const FogPublication> fog;
};

// Native capture owner. Publication is generation checked under the same lock
// as session invalidation. Acquire only shares immutable storage, never a grid.
class SnapshotStore {
public:
    void SetAdmission(std::uint32_t supported_features);
    void Disable();
    void SetSession(std::uint64_t generation, bool gameplay, bool menu, bool focused, std::uint64_t now);
    std::uint64_t Generation() const;
    bool PublishPlayer(std::uint64_t generation, const SFMapPoseV1& player, const SFMapPoseV1& camera, std::uint64_t now);
    bool PublishMarkers(Collection collection, std::uint64_t generation, std::vector<SFMapMarkerV1> values, std::uint64_t now);
    bool PublishFog(std::uint64_t generation, std::shared_ptr<const FogSnapshot> value, std::uint64_t now);
    bool PublishEnvironment(std::uint64_t generation, SFMapEnvironmentV1 value, bool clock, std::uint64_t now);
    FrameLease Acquire(std::uint64_t expected_generation, std::uint64_t now) const;
private:
    mutable std::mutex mutex_;
    std::uint64_t generation_{}, revision_{}, state_at_{};
    std::uint32_t features_{}, flags_{};
    std::shared_ptr<const PlayerSnapshot> player_;
    std::array<std::shared_ptr<const MarkerSnapshot>, static_cast<std::size_t>(Collection::Count)> markers_;
    std::shared_ptr<const FogPublication> fog_;
    SFMapEnvironmentV1 environment_{};
    SFMapSnapshotMetaV1 Meta(std::uint64_t now, std::uint32_t age);
    void Clear();
};

void SetMarkerIdentity(SFMapMarkerV1& marker,const std::uint64_t (&words)[5]);
bool ReadMemory(std::uintptr_t address, void* output, std::size_t size) noexcept;
bool DecodeLocalTransforms(const NativeTransform& player, const NativeTransform& camera,
    SFMapPoseV1& out_player, SFMapPoseV1& out_camera) noexcept;
bool DecodeClock(const std::int64_t (&fields)[5], SFMapEnvironmentV1& result) noexcept;
bool DecodeWeather(const float (&weights)[4], SFMapEnvironmentV1& result) noexcept;
bool DecodeWorldMarkers(std::span<const std::uint8_t> bytes, std::size_t count, bool custom,
    std::vector<SFMapMarkerV1>& result);
bool ReadFog(std::uintptr_t address, FogSnapshot& result);
}
