#include "first-person/first_person_control.h"

#include "abi/runtime_api.h"
#include "engine-hooks/write_executable_bytes.h"
#include "first-person/first_person_profile.h"
#include "first-person/stamina_anchor_stub.h"
#include "map/capture_game_map.h"
#include "profiles/game_profile.h"

#include <windows.h>

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <initializer_list>
#include <mutex>
#include <string>
#include <vector>

namespace FirstPersonControl {
namespace {

constexpr char UnsupportedDetail[] =
    "first-person camera and stamina controls require the exact supported client image and all profile guards";
constexpr char InvalidArgumentDetail[] =
    "owner token must be nonzero and cameraDistance must be finite in [-0.5, 0.5]";
constexpr char OwnerConflictDetail[] =
    "another first-person runtime owner is active or this owner token is stale";
constexpr char FailedDetail[] =
    "first-person startup hooks could not be prepared or no longer match the owned profile";
constexpr char RestartRequiredDetail[] =
    "first-person hooks were not prepared at startup; enable the package and restart the client";
constexpr char ShutdownDetail[] =
    "first-person runtime has shut down; restart the client before enabling the package";
constexpr char ReadyDetail[] =
    "exact client image and camera/stamina profile guards matched";

struct Hook {
    std::uintptr_t site{};
    std::uintptr_t stub{};
    std::array<std::uint8_t, 5> original{};
    std::array<std::uint8_t, 5> replacement{};
    bool installed{};
};

struct NativeState {
    std::mutex mutex;
    OwnerState owner;
    std::uintptr_t image_base{};
    std::uint8_t* allocation{};
    Hook camera;
    Hook stamina;
    std::array<std::uint8_t, CameraSystemEntry.size()> camera_system_entry =
        CameraSystemEntry;
    bool unwind_registered{};
    bool module_pinned{};
    std::string detail{UnsupportedDetail};
};

struct WritableAllocationGuard {
    std::uint8_t* address{};
    bool retained{};

    ~WritableAllocationGuard() {
        if (address && !retained) VirtualFree(address, 0, MEM_RELEASE);
    }
};

NativeState& state() {
    static NativeState value;
    return value;
}

struct SharedControlData {
    alignas(4) volatile LONG enabled{};
    alignas(4) volatile LONG distance_bits{};
};

static_assert(offsetof(SharedControlData, enabled) == 0);
static_assert(offsetof(SharedControlData, distance_bits) == sizeof(LONG));

SharedControlData* shared_data(NativeState& value) noexcept {
    if (!value.allocation) return nullptr;
    return reinterpret_cast<SharedControlData*>(value.allocation + SharedStateOffset);
}

void set_enabled(NativeState& value, bool enabled) noexcept {
    if (auto* shared = shared_data(value)) {
        InterlockedExchange(&shared->enabled, enabled ? 1 : 0);
    }
}

void set_distance(NativeState& value, double distance) noexcept {
    if (auto* shared = shared_data(value)) {
        const float as_float = static_cast<float>(distance);
        LONG bits{};
        static_assert(sizeof(bits) == sizeof(as_float));
        std::memcpy(&bits, &as_float, sizeof(bits));
        InterlockedExchange(&shared->distance_bits, bits);
    }
}

bool read_bytes(std::uintptr_t address, void* bytes, std::size_t size) noexcept {
    SIZE_T read{};
    return address && bytes && size &&
        ReadProcessMemory(GetCurrentProcess(), reinterpret_cast<const void*>(address),
            bytes, size, &read) && read == size;
}

bool call_matches(std::uintptr_t address,
                  const std::array<std::uint8_t, 5>& expected) noexcept {
    std::array<std::uint8_t, 5> actual{};
    return read_bytes(address, actual.data(), actual.size()) && actual == expected;
}

bool encode_call(std::uintptr_t site,
                 std::uintptr_t target,
                 std::array<std::uint8_t, 5>& call) noexcept {
    std::int32_t displacement{};
    if (!RelativeDisplacement(site + call.size(), target, displacement)) return false;
    call[0] = 0xE8;
    std::memcpy(call.data() + 1, &displacement, sizeof(displacement));
    return true;
}

bool allocate_writable_near(std::uintptr_t target,
                            std::size_t size,
                            std::uint8_t*& output) noexcept {
    output = nullptr;
    SYSTEM_INFO system{};
    GetSystemInfo(&system);
    const auto granularity = static_cast<std::uintptr_t>(system.dwAllocationGranularity);
    constexpr std::uintptr_t range = 0x70000000;
    const auto minimum = (std::max)(
        reinterpret_cast<std::uintptr_t>(system.lpMinimumApplicationAddress),
        target > range ? target - range : std::uintptr_t{});
    const auto maximum = (std::min)(
        reinterpret_cast<std::uintptr_t>(system.lpMaximumApplicationAddress),
        target <= UINTPTR_MAX - range ? target + range : target);

    for (auto cursor = minimum; cursor < maximum;) {
        MEMORY_BASIC_INFORMATION memory{};
        if (!VirtualQuery(reinterpret_cast<const void*>(cursor), &memory, sizeof(memory))) break;
        const auto region = reinterpret_cast<std::uintptr_t>(memory.BaseAddress);
        if (memory.RegionSize > UINTPTR_MAX - region) break;
        const auto end = region + memory.RegionSize;
        const auto alignment_base = (std::max)(region, cursor);
        const auto candidate = !granularity || alignment_base > UINTPTR_MAX - (granularity - 1)
            ? UINTPTR_MAX
            : (alignment_base + granularity - 1) & ~(granularity - 1);
        if (memory.State == MEM_FREE && candidate >= minimum && candidate < maximum && candidate <= end &&
            size <= end - candidate && size <= maximum - candidate) {
            auto* allocation = static_cast<std::uint8_t*>(VirtualAlloc(
                reinterpret_cast<void*>(candidate), size,
                MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
            if (allocation) {
                output = allocation;
                return true;
            }
        }
        if (end <= cursor) break;
        cursor = end;
    }
    return false;
}

std::uintptr_t loaded_image_base() noexcept {
    return reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
}

bool supported_image_locked(NativeState& value) {
    const auto base = loaded_image_base();
    if (!base || KfcRuntimeCompatibility::EnshroudedClient::image_timestamp != ClientTimestamp ||
        KfcRuntimeCompatibility::EnshroudedClient::image_size != ClientImageSize ||
        KfcRuntimeCompatibility::EnshroudedClient::image_sha256 != ClientSha256) {
        return false;
    }

    const auto* image = reinterpret_cast<const std::uint8_t*>(base);
    std::array<std::uint8_t, CameraSystemEntry.size()> camera_system_entry =
        CameraSystemEntry;
    std::array<std::uint8_t, CameraSystemEntry.size()> actual_camera_entry{};
    if (!read_bytes(base + CameraSystemFunction, actual_camera_entry.data(),
            actual_camera_entry.size())) {
        return false;
    }
    if (actual_camera_entry != CameraSystemEntry) {
        if (!MapRuntime::ReadOwnedCameraEntryReplacement(
                CameraSystemEntry, camera_system_entry) ||
            actual_camera_entry != camera_system_entry) {
            return false;
        }
    }

    const auto& camera_call = value.camera.installed ? value.camera.replacement : OriginalCameraCall;
    const auto& stamina_call = value.stamina.installed ? value.stamina.replacement : OriginalStaminaCall;
    if (!MatchesSupportedImage(image, ClientImageSize, base,
            KfcRuntimeCompatibility::EnshroudedClient::image_sha256,
            camera_call, stamina_call, camera_system_entry)) {
        return false;
    }

    if (value.image_base && value.image_base != base) return false;
    value.image_base = base;
    value.camera_system_entry = camera_system_entry;
    return true;
}

bool pin_provider_module(NativeState& value) noexcept {
    if (value.module_pinned) return true;
    HMODULE pinned{};
    if (!GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            reinterpret_cast<LPCWSTR>(&Available), &pinned)) {
        return false;
    }
    value.module_pinned = true;
    return true;
}

std::vector<std::uint8_t> make_camera_stub(
    std::uintptr_t codeBase,
    std::uintptr_t sharedEnabled,
    std::uintptr_t sharedDistance,
    std::uintptr_t originalTransform) {
    std::vector<std::uint8_t> code = {
        0x83, 0x3D, 0, 0, 0, 0, 0,                 // cmp dword ptr [rip+enabled], 0
        0x74, 0x16,                                // disabled: jump to native tail jump
        0x8B, 0x05, 0, 0, 0, 0,                    // mov eax, dword ptr [rip+distance]
        0x89, 0x82, 0xA4, 0x14, 0x00, 0x00,        // store camera distance
        0xC7, 0x82, 0x8C, 0x14, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,                    // zero shoulder offset
        0xE9, 0, 0, 0, 0,                          // tail jump to native transform
    };
    std::int32_t enabled_delta{}, distance_delta{}, transform_delta{};
    if (!RelativeDisplacement(codeBase + 7, sharedEnabled, enabled_delta) ||
        !RelativeDisplacement(codeBase + 15, sharedDistance, distance_delta) ||
        !RelativeDisplacement(codeBase + 36, originalTransform, transform_delta)) {
        return {};
    }
    std::memcpy(code.data() + 2, &enabled_delta, sizeof(enabled_delta));
    std::memcpy(code.data() + 11, &distance_delta, sizeof(distance_delta));
    std::memcpy(code.data() + 32, &transform_delta, sizeof(transform_delta));
    return code;
}

bool create_resident_stubs(NativeState& value) {
    if (value.allocation) return value.unwind_registered;
    if (!pin_provider_module(value)) return false;

    const auto base = value.image_base;
    if (!base) return false;
    const auto camera_site = base + CameraTransformCall;
    const auto stamina_site = base + StaminaLayoutSite;
    const auto midpoint = camera_site + (stamina_site - camera_site) / 2;

    std::uint8_t* allocation{};
    if (!allocate_writable_near(midpoint, AllocationSize, allocation)) return false;
    WritableAllocationGuard allocation_guard{allocation};
    std::memset(allocation, 0, AllocationSize);

    auto* shared = reinterpret_cast<SharedControlData*>(allocation + SharedStateOffset);
    shared->enabled = 0;
    LONG initial_distance{};
    const float default_distance = DefaultCameraDistance;
    std::memcpy(&initial_distance, &default_distance, sizeof(initial_distance));
    shared->distance_bits = initial_distance;

    const auto camera_stub_address = reinterpret_cast<std::uintptr_t>(allocation);
    const auto shared_address = reinterpret_cast<std::uintptr_t>(shared);
    const auto shared_enabled = shared_address + offsetof(SharedControlData, enabled);
    const auto shared_distance = shared_address + offsetof(SharedControlData, distance_bits);
    auto camera_code = make_camera_stub(camera_stub_address, shared_enabled,
        shared_distance, base + CameraTransformFunction);
    if (camera_code.empty() || camera_code.size() > CodePageSize) {
        return false;
    }
    std::memcpy(allocation, camera_code.data(), camera_code.size());

    GaugeCode gauge_code;
    if (!BuildGaugeCode(base + UiContextPointer, shared_enabled,
            reinterpret_cast<std::uintptr_t>(allocation), gauge_code) ||
        gauge_code.bytes.size() > AllocationSize - GaugeCodeOffset) {
        return false;
    }
    std::memcpy(allocation + GaugeCodeOffset, gauge_code.bytes.data(), gauge_code.bytes.size());

    const auto native_setter = base + NativePositionSetter;
    std::memcpy(allocation + GaugeCodeOffset + gauge_code.tail,
        &native_setter, sizeof(native_setter));
    auto* function_table = reinterpret_cast<RUNTIME_FUNCTION*>(
        allocation + SharedStateOffset + GaugeFunctionTableOffset);
    *function_table = gauge_code.function;

    DWORD previous{};
    if (!VirtualProtect(allocation, CodePageSize * 2, PAGE_EXECUTE_READ, &previous) ||
        !FlushInstructionCache(GetCurrentProcess(), allocation, CodePageSize * 2)) {
        return false;
    }

    Hook camera{camera_site, camera_stub_address, OriginalCameraCall, {}, false};
    Hook stamina{stamina_site, camera_stub_address + GaugeCodeOffset,
        OriginalStaminaCall, {}, false};
    if (!encode_call(camera.site, camera.stub, camera.replacement) ||
        !encode_call(stamina.site, stamina.stub, stamina.replacement)) {
        return false;
    }

    if (!RtlAddFunctionTable(function_table, 1,
            reinterpret_cast<DWORD64>(allocation + GaugeCodeOffset))) {
        return false;
    }

    allocation_guard.retained = true;
    value.allocation = allocation;
    value.unwind_registered = true;
    value.camera = camera;
    value.stamina = stamina;
    return true;
}

bool install_one(Hook& hook) {
    if (hook.installed) return call_matches(hook.site, hook.replacement);
    if (!call_matches(hook.site, hook.original)) return false;
    if (!EngineHooks::ReplaceExecutableBytes(hook.site, hook.original.data(),
            hook.replacement.data(), hook.replacement.size())) {
        return false;
    }
    hook.installed = true;
    return true;
}

bool restore_one(Hook& hook) {
    if (!hook.installed) return true;
    if (!EngineHooks::ReplaceExecutableBytes(hook.site, hook.replacement.data(),
            hook.original.data(), hook.original.size())) {
        return false;
    }
    hook.installed = false;
    return true;
}

bool hooks_match_locked(const NativeState& value) noexcept {
    return value.owner.Prepared() && value.allocation && value.unwind_registered &&
        value.camera.installed && value.stamina.installed &&
        call_matches(value.camera.site, value.camera.replacement) &&
        call_matches(value.stamina.site, value.stamina.replacement);
}

bool revalidate_camera_entry_quiescent(const NativeState& value) {
    if (!value.image_base) return false;
    if (value.camera_system_entry != CameraSystemEntry) {
        std::array<std::uint8_t, CameraSystemEntry.size()> owned_replacement{};
        if (!MapRuntime::ReadOwnedCameraEntryReplacement(
                CameraSystemEntry, owned_replacement) ||
            owned_replacement != value.camera_system_entry) {
            return false;
        }
    }
    const auto address = value.image_base + CameraSystemFunction;
    // This identical-byte transaction suspends other process threads, rejects
    // a thread whose RIP is inside the entry, and rechecks the captured entry
    // bytes immediately before first-person call-site writes.
    return EngineHooks::ReplaceExecutableBytes(address,
        value.camera_system_entry.data(), value.camera_system_entry.data(),
        value.camera_system_entry.size());
}

Status finish(NativeState& value, Status status, const char* detail) noexcept {
    try {
        value.detail = detail ? detail : FailedDetail;
    } catch (...) {
    }
    return status;
}

} // namespace

bool Available() noexcept {
    try {
        auto& value = state();
        std::lock_guard lock(value.mutex);
        if (!supported_image_locked(value)) {
            finish(value, Status::UnsupportedBuild, UnsupportedDetail);
            return false;
        }
        if (value.owner.ShutdownRequested()) {
            finish(value, Status::Failed, ShutdownDetail);
            return false;
        }
        if (!value.owner.Prepared()) {
            finish(value, Status::Failed, RestartRequiredDetail);
            return false;
        }
        if (!hooks_match_locked(value)) {
            finish(value, Status::Failed, FailedDetail);
            return false;
        }
        finish(value, Status::Ready, ReadyDetail);
        return true;
    } catch (...) {
        return false;
    }
}

std::size_t Detail(char* buffer, std::size_t capacity) noexcept {
    constexpr char fallback[] = "first-person provider detail is unavailable";
    try {
        auto& value = state();
        std::lock_guard lock(value.mutex);
        const char* detail = value.detail.empty() ? fallback : value.detail.c_str();
        const auto required = std::strlen(detail) + 1;
        if (buffer && capacity >= required) std::memcpy(buffer, detail, required);
        return required;
    } catch (...) {
        const auto required = sizeof(fallback);
        if (buffer && capacity >= required) std::memcpy(buffer, fallback, required);
        return required;
    }
}

bool Prepare(const EngineHooks::HeldEntryToken& token) noexcept {
    if (!token.ValidForCurrentThread()) return false;
    try {
        auto& value = state();
        std::lock_guard lock(value.mutex);
        if (!token.ValidForCurrentThread()) {
            finish(value, Status::Failed, RestartRequiredDetail);
            return false;
        }
        if (!supported_image_locked(value)) {
            finish(value, Status::UnsupportedBuild, UnsupportedDetail);
            return false;
        }
        if (value.owner.ShutdownRequested()) {
            finish(value, Status::Failed, ShutdownDetail);
            return false;
        }
        if (value.owner.Prepared()) {
            const bool intact = hooks_match_locked(value);
            finish(value, intact ? Status::Ready : Status::Failed,
                intact ? ReadyDetail : FailedDetail);
            return intact;
        }
        if (!create_resident_stubs(value)) {
            finish(value, Status::Failed, FailedDetail);
            return false;
        }

        // Both retained stubs are inert until the single shared mode word is
        // enabled. This preparation runs while the game entry thread is held.
        // The executable writer proves each individual patch against executing
        // peers while the loader's held-entry gate prevents normal startup.
        const bool camera_was_installed = value.camera.installed;
        if (!token.ValidForCurrentThread() || !revalidate_camera_entry_quiescent(value) ||
            !token.ValidForCurrentThread() || !install_one(value.camera)) {
            finish(value, Status::Failed, FailedDetail);
            return false;
        }
        if (!token.ValidForCurrentThread() || !revalidate_camera_entry_quiescent(value) ||
            !token.ValidForCurrentThread() || !install_one(value.stamina)) {
            if (!camera_was_installed && token.ValidForCurrentThread()) {
                // A failed restore leaves a valid disabled forwarding stub and
                // is safe to retry on a later startup preparation attempt.
                restore_one(value.camera);
            }
            finish(value, Status::Failed, FailedDetail);
            return false;
        }
        const bool profile_matches = supported_image_locked(value);
        const bool hooks_verified = profile_matches && value.camera.installed &&
            value.stamina.installed &&
            call_matches(value.camera.site, value.camera.replacement) &&
            call_matches(value.stamina.site, value.stamina.replacement);
        if (!hooks_verified || !token.ValidForCurrentThread()) {
            finish(value, Status::Failed, FailedDetail);
            return false;
        }

        if (!value.owner.MarkPrepared()) {
            finish(value, Status::Failed, ShutdownDetail);
            return false;
        }
        set_enabled(value, false);
        finish(value, Status::Ready, ReadyDetail);
        return true;
    } catch (...) {
        return false;
    }
}

Status Configure(std::uint64_t ownerToken, double cameraDistance) noexcept {
    if (!ownerToken || !std::isfinite(cameraDistance) ||
        cameraDistance < -0.5 || cameraDistance > 0.5) {
        try {
            auto& value = state();
            std::lock_guard lock(value.mutex);
            return finish(value, Status::InvalidArgument, InvalidArgumentDetail);
        } catch (...) {
            return Status::InvalidArgument;
        }
    }

    try {
        auto& value = state();
        std::lock_guard lock(value.mutex);
        if (!supported_image_locked(value)) {
            set_enabled(value, false);
            if (value.owner.OwnerToken()) value.owner.SetMode(value.owner.OwnerToken(), false);
            return finish(value, Status::UnsupportedBuild, UnsupportedDetail);
        }
        if (value.owner.ShutdownRequested()) {
            return finish(value, Status::Failed, ShutdownDetail);
        }
        if (!value.owner.Prepared()) {
            return finish(value, Status::Failed, RestartRequiredDetail);
        }
        if (!hooks_match_locked(value)) {
            set_enabled(value, false);
            if (value.owner.OwnerToken()) value.owner.SetMode(value.owner.OwnerToken(), false);
            return finish(value, Status::Failed, FailedDetail);
        }

        const auto configured = value.owner.Configure(ownerToken, cameraDistance);
        if (configured != Status::Ready) {
            return finish(value, configured,
                configured == Status::OwnerConflict ? OwnerConflictDetail : InvalidArgumentDetail);
        }

        set_distance(value, cameraDistance);

        return finish(value, Status::Ready, ReadyDetail);
    } catch (...) {
        return Status::Failed;
    }
}

Status SetMode(std::uint64_t ownerToken, bool enabled) noexcept {
    try {
        auto& value = state();
        std::lock_guard lock(value.mutex);

        const auto mode = value.owner.SetMode(ownerToken, enabled);
        if (mode != Status::Ready) {
            return finish(value, mode,
                mode == Status::OwnerConflict ? OwnerConflictDetail :
                    (value.owner.ShutdownRequested() ? ShutdownDetail :
                        (enabled && !value.owner.Prepared() ? RestartRequiredDetail : InvalidArgumentDetail)));
        }
        if (!enabled) {
            set_enabled(value, false);
            return finish(value, Status::Ready, ReadyDetail);
        }

        if (!supported_image_locked(value)) {
            value.owner.SetMode(ownerToken, false);
            set_enabled(value, false);
            return finish(value, Status::UnsupportedBuild, UnsupportedDetail);
        }
        if (!hooks_match_locked(value)) {
            value.owner.SetMode(ownerToken, false);
            set_enabled(value, false);
            return finish(value, Status::Failed, FailedDetail);
        }

        set_distance(value, value.owner.CameraDistance());
        // Both retained stubs read this one aligned word, so mode changes take
        // effect for camera and stamina together after both call sites are owned.
        set_enabled(value, true);
        return finish(value, Status::Ready, ReadyDetail);
    } catch (...) {
        return Status::Failed;
    }
}

Status ReleaseOwner(std::uint64_t ownerToken) noexcept {
    try {
        auto& value = state();
        std::lock_guard lock(value.mutex);
        if (!ownerToken) return finish(value, Status::InvalidArgument, InvalidArgumentDetail);
        if (value.owner.OwnerToken() && value.owner.OwnerToken() != ownerToken) {
            return finish(value, Status::OwnerConflict, OwnerConflictDetail);
        }

        const auto released = value.owner.ReleaseOwner(ownerToken);
        if (released == Status::Ready) set_enabled(value, false);
        return finish(value, released,
            released == Status::Ready ? ReadyDetail : OwnerConflictDetail);
    } catch (...) {
        return Status::Failed;
    }
}

void Shutdown() noexcept {
    try {
        auto& value = state();
        std::lock_guard lock(value.mutex);
        set_enabled(value, false);
        value.owner.Shutdown();
        finish(value, Status::Ready, ReadyDetail);
        // Call replacements and their unwind metadata remain valid for any
        // delayed engine call until process exit. The disabled stubs forward
        // to the exact original native functions.
    } catch (...) {
    }
}

} // namespace FirstPersonControl

extern "C" {
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeFirstPersonAvailable() {
    return FirstPersonControl::Available();
}

KFC_RUNTIME_API std::size_t KFC_RUNTIME_CALL KfcRuntimeFirstPersonDetail(
    char* buffer, std::size_t capacity) {
    return FirstPersonControl::Detail(buffer, capacity);
}

KFC_RUNTIME_API std::uint32_t KFC_RUNTIME_CALL KfcRuntimeFirstPersonConfigure(
    std::uint64_t ownerToken, double cameraDistance) {
    return static_cast<std::uint32_t>(FirstPersonControl::Configure(ownerToken, cameraDistance));
}

KFC_RUNTIME_API std::uint32_t KFC_RUNTIME_CALL KfcRuntimeFirstPersonSetMode(
    std::uint64_t ownerToken, bool enabled) {
    return static_cast<std::uint32_t>(FirstPersonControl::SetMode(ownerToken, enabled));
}

KFC_RUNTIME_API std::uint32_t KFC_RUNTIME_CALL KfcRuntimeFirstPersonReleaseOwner(
    std::uint64_t ownerToken) {
    return static_cast<std::uint32_t>(FirstPersonControl::ReleaseOwner(ownerToken));
}
}
