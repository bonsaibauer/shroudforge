#pragma once

#include "camera_distance.h"
#include "engine-hooks/held_entry_preparation.h"

#include <cmath>
#include <cstddef>
#include <cstdint>

namespace FirstPersonControl {

enum class Status : std::uint32_t {
    Ready = 0,
    UnsupportedBuild = 1,
    InvalidArgument = 2,
    OwnerConflict = 3,
    Failed = 4,
};

class OwnerState final {
public:
    bool MarkPrepared() noexcept {
        if (shutdown_requested_) return false;
        prepared_ = true;
        return true;
    }
    bool Prepared() const noexcept { return prepared_; }
    bool ShutdownRequested() const noexcept { return shutdown_requested_; }

    Status Configure(std::uint64_t ownerToken, double cameraDistance) noexcept {
        if (!ownerToken || !std::isfinite(cameraDistance) ||
            cameraDistance < -0.5 || cameraDistance > 0.5) {
            return Status::InvalidArgument;
        }
        if (!prepared_ || shutdown_requested_) return Status::Failed;
        if (owner_token_ && owner_token_ != ownerToken) return Status::OwnerConflict;
        owner_token_ = ownerToken;
        camera_distance_ = cameraDistance;
        return Status::Ready;
    }

    Status SetMode(std::uint64_t ownerToken, bool enabled) noexcept {
        if (!ownerToken) return Status::InvalidArgument;
        if (!enabled) {
            if (!owner_token_) return Status::Ready;
            if (owner_token_ != ownerToken) return Status::OwnerConflict;
            enabled_ = false;
            return Status::Ready;
        }
        if (!prepared_ || shutdown_requested_) return Status::Failed;
        if (!owner_token_) return enabled ? Status::OwnerConflict : Status::Ready;
        if (owner_token_ != ownerToken) return Status::OwnerConflict;
        enabled_ = true;
        return Status::Ready;
    }

    Status ReleaseOwner(std::uint64_t ownerToken) noexcept {
        if (!ownerToken) return Status::InvalidArgument;
        if (!owner_token_) return Status::Ready;
        if (owner_token_ != ownerToken) return Status::OwnerConflict;
        ClearOwner();
        return Status::Ready;
    }

    void Shutdown() noexcept {
        ClearOwner();
        shutdown_requested_ = true;
    }

    std::uint64_t OwnerToken() const noexcept { return owner_token_; }
    double CameraDistance() const noexcept { return camera_distance_; }
    bool Enabled() const noexcept { return enabled_; }

private:
    void ClearOwner() noexcept {
        owner_token_ = 0;
        camera_distance_ = DefaultCameraDistance;
        enabled_ = false;
    }
    bool prepared_{};
    bool shutdown_requested_{};
    std::uint64_t owner_token_{};
    double camera_distance_{DefaultCameraDistance};
    bool enabled_{};
};

// The native provider keeps these functions independent from the public C ABI.
// ABI wrappers in the same source file delegate to this owner-scoped control.
bool Available() noexcept;
std::size_t Detail(char* buffer, std::size_t capacity) noexcept;
bool Prepare(const EngineHooks::HeldEntryToken& token) noexcept;
Status Configure(std::uint64_t ownerToken, double cameraDistance) noexcept;
Status SetMode(std::uint64_t ownerToken, bool enabled) noexcept;
Status ReleaseOwner(std::uint64_t ownerToken) noexcept;
void Shutdown() noexcept;

} // namespace FirstPersonControl
