#include "first-person/first_person_control.h"
#include <cstdio>
#include <cstdlib>
#include <initializer_list>
#include <limits>

namespace {
unsigned checks{};
void check(bool value, const char* message) {
    ++checks;
    if (!value) { std::fprintf(stderr, "%s\n", message); std::exit(1); }
}
void test_owner_lifecycle() {
    using namespace FirstPersonControl;
    OwnerState owner;

    check(!owner.Prepared(), "camera and gauge hooks start unprepared");
    check(owner.Configure(10, -0.15) == Status::Failed,
        "configuration cannot install or claim an owner before held-entry preparation");
    check(owner.SetMode(10, true) == Status::Failed,
        "mode cannot be enabled before held-entry preparation");
    check(owner.SetMode(10, false) == Status::Ready && !owner.Enabled(),
        "disable remains safe and idempotent before startup preparation");
    check(owner.ReleaseOwner(10) == Status::Ready,
        "release remains safe and idempotent before startup preparation");
    check(owner.MarkPrepared() && owner.Prepared(),
        "validated held-entry preparation marks hooks ready");
    check(owner.SetMode(10, true) == Status::OwnerConflict,
        "prepared mode still requires configuration to claim an owner");

    check(owner.Configure(0, -0.15) == Status::InvalidArgument,
        "zero owner tokens are refused");
    check(owner.Configure(10, std::numeric_limits<double>::quiet_NaN()) == Status::InvalidArgument,
        "NaN camera distances are refused");
    check(owner.Configure(10, std::numeric_limits<double>::infinity()) == Status::InvalidArgument,
        "infinite camera distances are refused");
    check(owner.Configure(10, -0.500001) == Status::InvalidArgument,
        "camera distances below the admitted range are refused");
    check(owner.Configure(10, 0.500001) == Status::InvalidArgument,
        "camera distances above the admitted range are refused");

    for (const double distance : {-0.5, -0.15, 0.0, 0.5}) {
        check(owner.Configure(10, distance) == Status::Ready,
            "finite distances at or inside the supported range are accepted");
        check(owner.CameraDistance() == distance,
            "configuration retains the last valid camera distance");
    }
    check(owner.Configure(20, 0.0) == Status::OwnerConflict,
        "another runtime owner cannot replace the active owner");
    check(owner.SetMode(20, true) == Status::OwnerConflict,
        "another runtime owner cannot enable the mode");
    check(owner.SetMode(10, true) == Status::Ready && owner.Enabled(),
        "the configured owner can enable the shared mode");
    check(owner.ReleaseOwner(20) == Status::OwnerConflict && owner.Enabled(),
        "a stale owner cannot release a newer active owner");
    check(owner.SetMode(10, false) == Status::Ready && !owner.Enabled(),
        "disabling restores original camera and gauge behavior");
    check(owner.SetMode(10, true) == Status::Ready && owner.Enabled(),
        "the same owner can re-enable after disabling");
    check(owner.ReleaseOwner(10) == Status::Ready && !owner.Enabled() && owner.OwnerToken() == 0,
        "owner release disables both controls and clears the generation");
    check(owner.CameraDistance() == DefaultCameraDistance,
        "owner release restores the configured default distance");
    check(owner.ReleaseOwner(10) == Status::Ready,
        "owner release is idempotent after the owner has been cleared");
    owner.Configure(30, 0.25);
    owner.SetMode(30, true);
    owner.Shutdown();
    check(owner.OwnerToken() == 0 && !owner.Enabled() &&
        owner.CameraDistance() == DefaultCameraDistance,
        "process shutdown disables and clears the current owner");
    check(owner.Prepared(),
        "shutdown retains process-resident disabled hooks for safe forwarding");
    check(owner.ShutdownRequested() && owner.Configure(31, 0.0) == Status::Failed &&
        owner.SetMode(31, true) == Status::Failed,
        "process shutdown refuses new owners and mode activation while retaining stubs");
}

}
int main() {
    test_owner_lifecycle();
    std::printf("PASS: %u First Person ownership checks\n", checks);
}
