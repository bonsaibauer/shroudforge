#pragma once
#include "map/read_map_frame.h"
#include <string_view>
#include <string>
// Exact definition is supplied by Main's shared held_entry_preparation.h.
namespace EngineHooks {class HeldEntryToken;}

namespace MapRuntime {
inline constexpr char CurrentClientSha256[] = "d64966e23b1a458ee6f147bcbceac0b577fa7e39a212f371287d8cc119817882";
struct MapImageAdmission {
    std::uint32_t status{SF_MAP_UNSUPPORTED_BUILD};
    bool player{}, markers{}, clock{}, weather{}, fog{}, world{}, deaths{}, discovery{}, renderer{};
    const char* failure_reason{"image-identity"};
    std::size_t failure_rva{};
    std::uint8_t expected_byte{}, observed_byte{};
    std::uint32_t Features() const;
};
MapImageAdmission InspectMapImage(std::span<const std::uint8_t> image,
    std::string_view exact_file_sha256, bool capability_allowed);

struct NativeCameraRecord {
    std::uintptr_t unknown0{}, unknown1{}, player_transform{}, local_player{}, unknown4{}, unknown5{},
        camera_transform{}, unknown7{}, unknown8{}, unknown9{};
};
static_assert(sizeof(NativeCameraRecord)==0x50);
bool ReadLocalCamera(const NativeCameraRecord& record, std::uint32_t entity_id,
    SFMapPoseV1& player, SFMapPoseV1& camera);
// Internal cache discriminator only. Never exported, dereferenced after the
// synchronous sample, or included in a package snapshot.
struct DiscoveryIdentity {std::uintptr_t marker{},entity{};bool operator==(const DiscoveryIdentity&)const=default;};
bool ReadDiscoveryMarker(std::uintptr_t record,SFMapMarkerV1& marker,bool& positioned,DiscoveryIdentity* identity=nullptr);
bool ReadPingEvents(std::uintptr_t list, std::uint32_t excluded_sender, std::uint64_t now,
    std::vector<SFMapMarkerV1>& result);

// Internal host control-plane operations. The caller establishes package
// identity/capability first; these are not an unrestricted public mod API.
std::int32_t EnableCapture(std::uint64_t package_owner, bool capability_allowed);
void DisableCapture(std::uint64_t package_owner);
// Root metadata selection precedes this genuine held-entry boundary.
// READY(0) means guarded inactive preparation, never a live world/session.
std::int32_t PrepareCapture(const EngineHooks::HeldEntryToken&) noexcept;
// Logical retirement only. Forwarding bridges/ownership remain process resident.
void StopCapture() noexcept;
// Retained control-plane evidence; does not query/mutate feature readiness.
std::string PreparationStatus();
bool FinalizeOriginalOwners() noexcept;
bool RequireOriginalOwners() noexcept;
bool ReadOwnedCameraEntryReplacement(std::span<const std::uint8_t> original,
    std::span<std::uint8_t> replacement) noexcept;
void UpdateCaptureSession(std::uint64_t native_session, bool gameplay, bool menu, bool focused);
SnapshotStore& CapturedFrames();
bool RendererAdmitted();
bool PreparedRendererAdmitted() noexcept;
bool InstallSharedCall(std::size_t rva,std::size_t target,void* callback);
bool SharedCallOwned(std::size_t rva,std::size_t target,void* callback);
bool InstallSharedEntry(std::size_t rva,std::span<const std::uint8_t> expected,void* callback);
}
