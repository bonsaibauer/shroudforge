// Engine HUD visibility capture adapted from Minimap, see SOURCE-NOTICE.txt.
#include "map/hud_visibility.h"
#include "map/capture_game_map.h"
#include "engine-context/engine_owners.h"
#include "host/executable_span_validation.h"
#include <array>
#include <atomic>
#include <cstring>
#include <windows.h>

namespace MapRuntime {
namespace {
using HudProfile = KfcRuntimeCompatibility::HudCaptureProfile;
std::atomic<std::shared_ptr<const HudProfile>> hud_profile;
std::atomic<std::uintptr_t> hud_image{};
std::atomic<std::size_t> hud_size{};
std::atomic<bool> hud_active{}, hud_prepared{}, hud_preflighted{};
std::atomic<std::uint64_t> hud_epoch{}, observation{};
std::atomic<const char*> hud_preparation_reason{"not-started"};

bool RendererValidated(const HudProfile& profile, std::uint32_t phase,
                       std::uint8_t identity[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES]) {
    if (identity) std::memset(identity, 0, SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES);
    if (!profile.runtime_owned_prefix) return true;
    for (const auto& guard : profile.guards) {
        if (guard.rva != profile.renderer_rva) continue;
        std::array<std::uint8_t, 64> actual{};
        if (!ReadMemory(hud_image.load() + guard.rva, actual.data(), guard.bytes.size())) return false;
        return ExecutableSpanValidation::Services().ValidateGuard(guard.rva, guard.bytes.data(),
            actual.data(), guard.bytes.size(), phase, identity);
    }
    return false;
}

void HudRender();
bool NativeOwnership(const HudProfile& profile, bool retained) {
    const auto base = hud_image.load(), size = hud_size.load();
    if (!base || base > UINTPTR_MAX - size) return false;
    if (retained && !SharedCallOwned(profile.call_rva, profile.renderer_rva,
                                    reinterpret_cast<void*>(&HudRender))) return false;
    for (const auto& guard : profile.guards) {
        if (guard.rva > size || guard.bytes.size() > size - guard.rva) return false;
        std::array<std::uint8_t, 64> actual{};
        if (!ReadMemory(base + guard.rva, actual.data(), guard.bytes.size())) return false;
        // Only the independently proved framework CALL may be overlaid. Every
        // other difference needs the existing runtime-owned prefix proof.
        if (retained && guard.rva == profile.call_rva) std::memcpy(actual.data(), guard.bytes.data(), 5);
        if (std::memcmp(actual.data(), guard.bytes.data(), guard.bytes.size()) &&
            (!profile.runtime_owned_prefix || !ExecutableSpanValidation::Services().ValidateGuard(
                guard.rva, guard.bytes.data(), actual.data(), guard.bytes.size(),
                SF_EXECUTABLE_SPAN_PHASE_NORMALIZE))) return false;
    }
    return true;
}

bool Screen(const HudProfile& profile, std::uint8_t& screen, std::array<std::uintptr_t, 4>& chain) {
    const auto pointer = [](std::uintptr_t base, std::size_t offset, std::uintptr_t& output) {
        return base >= 0x10000 && base <= UINTPTR_MAX - offset - sizeof(output) &&
            ReadMemory(base + offset, &output, sizeof(output));
    };
    std::uintptr_t context{}, holder{}, state{}, play{};
    if (!pointer(hud_image.load(), profile.context_rva, context) ||
        !pointer(context, profile.holder_offset, holder) || !pointer(holder, 0, state) ||
        !pointer(state, profile.play_state_offset, play) || play < 0x10000 ||
        play > UINTPTR_MAX - profile.screen_offset - 1 ||
        !ReadMemory(play + profile.screen_offset, &screen, 1) ||
        screen < profile.minimum_screen || screen > profile.maximum_screen) return false;
    chain = {context, holder, state, play};
    return true;
}

void HudRender() {
    // Keep the exact immutable contract across forwarding. Reconfiguration or
    // shutdown cannot switch an in-flight callback to another original target.
    const auto profile = hud_profile.load();
    if (!profile) return; // No CALL is published before this snapshot exists.
    const auto epoch = hud_epoch.load();
    std::array<std::uintptr_t, 4> first{};
    std::uint8_t screen{};
    std::array<std::uint8_t, SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES> identity_before{}, identity_after{};
    const bool owner_before = NativeOwnership(*profile, true);
    const bool entry_before = RendererValidated(*profile, SF_EXECUTABLE_SPAN_PHASE_BEFORE_ORIGINAL, identity_before.data());
    const bool staged = hud_active && owner_before && entry_before && Screen(*profile, screen, first);
    if (!staged || screen != profile->gameplay_screen || !HudGameplay(GetTickCount64())) observation = 0;
    reinterpret_cast<void(*)()>(hud_image.load() + profile->renderer_rva)();
    const bool owner_after = NativeOwnership(*profile, true);
    const bool entry_after = RendererValidated(*profile, SF_EXECUTABLE_SPAN_PHASE_AFTER_ORIGINAL, identity_after.data());
    if (!staged || !hud_active || epoch != hud_epoch.load()) { observation = 0; return; }
    std::array<std::uintptr_t, 4> second{};
    if (!owner_after || !entry_after || identity_before != identity_after || !Screen(*profile, screen, second) ||
        first != second || !hud_active || epoch != hud_epoch.load()) { observation = 0; return; }
    const auto now = GetTickCount64();
    observation = now <= (UINT64_MAX >> 8) ? (now << 8) | screen : 0;
    if (!hud_active || epoch != hud_epoch.load()) observation = 0;
}

bool Preflight(std::uintptr_t image, std::size_t size) {
    const auto selected = EngineContext::ConfiguredHudCaptureProfile();
    if (!selected.profile) { hud_preparation_reason = "hud-capture-profile-unavailable"; return false; }
    if (!image || image > UINTPTR_MAX - size || size != selected.profile->image_size) return false;
    hud_profile.store(selected.profile);
    hud_image = image;
    hud_size = size;
    observation = 0;
    ++hud_epoch;
    hud_active = false;
    hud_preflighted = false;
    hud_preparation_reason = "engine-guards";
    if (!NativeOwnership(*selected.profile, false)) return false;
    hud_preflighted = true;
    return true;
}
}

bool PreflightHudPreparation(std::uintptr_t image, std::size_t size) {
    hud_preparation_reason = "preconditions";
    return !hud_prepared.load() && Preflight(image, size);
}
bool InstallPreparedHudHooks() {
    const auto profile = hud_profile.load();
    hud_preparation_reason = "preconditions";
    if (!profile || !hud_preflighted.load() || hud_prepared.load()) return false;
    hud_preparation_reason = "shared-call";
    if (!InstallSharedCall(profile->call_rva, profile->renderer_rva, reinterpret_cast<void*>(&HudRender))) return false;
    hud_prepared = true;
    hud_preflighted = false;
    hud_preparation_reason = "ready";
    return true;
}
std::string HudPreparationStatus() {
    const auto profile = hud_profile.load();
    const auto* reason = hud_preparation_reason.load();
    std::string status = reason ? reason : "unknown";
    if (status == "hud-capture-profile-unavailable") return EngineContext::ConfiguredHudCaptureProfile().status;
    if (profile) status += ":" + profile->profile_id + ":revision-" + std::to_string(profile->revision);
    return status;
}
bool ValidatePreparedHudActivation() {
    const auto profile = hud_profile.load();
    return profile && hud_prepared.load() && NativeOwnership(*profile, true);
}
bool ActivatePreparedHudVisibility() {
    if (!hud_profile.load() || !hud_prepared.load()) return false;
    observation = 0;
    ++hud_epoch;
    hud_active = true;
    return true;
}
void DisableHudVisibility() {
    hud_active = false;
    hud_preflighted = false;
    ++hud_epoch;
    observation = 0;
    // Retain the profile while published relays may still forward originals.
}
bool HudKnown(std::uint64_t now) {
    const auto profile = hud_profile.load();
    const auto value = observation.load(), at = value >> 8;
    return profile && hud_active && (value & 255) && now >= at && now - at <= profile->maximum_age_ms;
}
bool HudInputAllowed(std::uint64_t now) { return !hud_active || HudGameplay(now); }
bool HudGameplay(std::uint64_t now) {
    const auto profile = hud_profile.load();
    return profile && HudKnown(now) && (observation.load() & 255) == profile->gameplay_screen;
}
}
