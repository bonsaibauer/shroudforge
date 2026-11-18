#pragma once
#include <atomic>
#include "profiles/game_profile.h"
#include "profiles/identify_game_image.h"
#include "engine-hooks/install_engine_hooks.h"

namespace EcsImplementation {
// Image identity is required by read profiles and individually admitted features,
// independently of permission to select a generic execution profile or hooks.
inline bool InitializeMetadata(bool enable_execution_hooks,
        std::uintptr_t& base_address, std::size_t& size, std::atomic<bool>& stopped) {
    bool execution_profile{};
    if (enable_execution_hooks) {
        execution_profile = KfcRuntimeCompatibility::EnshroudedClient::Load();
    } else {
        wchar_t process_path[32768]{};
        const auto length = GetModuleFileNameW(nullptr, process_path, 32768);
        if (!length || length >= 32768) return false;
        const std::uint8_t* base{};
        const IMAGE_NT_HEADERS64* nt{};
        if (!KfcRuntimeCompatibility::EnshroudedClient::IdentifyGameImage(process_path, base, nt)) return false;
        using namespace KfcRuntimeCompatibility::EnshroudedClient;
        if (image_sha256.size() != 64 || image_target.empty() || (kfc_format != 3 && kfc_format != 4)) return false;
    }
    base_address = reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
    if (!base_address) return false;
    const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base_address);
    if (dos->e_magic != IMAGE_DOS_SIGNATURE) return false;
    const auto nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(base_address + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE) return false;
    size = nt->OptionalHeader.SizeOfImage;
    stopped.store(false, std::memory_order_release);
    if (execution_profile) EngineHooks::Install();
    return true;
}
}
