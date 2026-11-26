#include "engine-hooks/scan_executable_signature.h"
#include "engine-hooks/publish_engine_hook.h"
#include "engine-hooks/install_engine_hooks.h"
#include "game-thread/game_thread_state.h"
#include "engine-context/observe_entity_manager.h"
#include "profiles/game_profile.h"
#include "engine-context/world_context.h"
#include "player-input/sample_world_cursor.h"
#include "patches/set_runtime_patch.h"
#include <windows.h>
#include <algorithm>
#include <array>
#include <charconv>
#include <cstring>
#include <limits>
#include <sstream>
#include <string_view>
#include <vector>

using namespace GameThreadImplementation;

namespace {
std::vector<EngineHooks::PublishedEngineHook> hooks;

bool supported_image(std::uint8_t*& base, std::size_t& size) {
    base = reinterpret_cast<std::uint8_t*>(GetModuleHandleW(nullptr));
    if (!base) return false;
    const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base);
    if (dos->e_magic != IMAGE_DOS_SIGNATURE) return false;
    const auto nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(base + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE ||
        nt->FileHeader.TimeDateStamp != KfcRuntimeCompatibility::EnshroudedClient::image_timestamp ||
        nt->OptionalHeader.SizeOfImage != KfcRuntimeCompatibility::EnshroudedClient::image_size) return false;
    size = nt->OptionalHeader.SizeOfImage;
    return true;
}

std::vector<std::uint8_t> callback_code(void* callback) {
    // Exact register/XMM preservation used by the validated ShroudForge actor
    // callback. RCX/RDX are forwarded but the generic dispatcher ignores them.
    std::vector<std::uint8_t> code{0x9c,0x50,0x51,0x52,0x41,0x50,0x41,0x51,0x41,0x52,0x41,0x53,
        0x48,0x81,0xec,0x80,0,0,0};
    for (unsigned index = 0; index < 6; ++index) {
        const std::uint8_t store[]{0xf3,0x0f,0x7f,static_cast<std::uint8_t>(0x44+index*8),0x24,
            static_cast<std::uint8_t>(0x20+index*16)};
        code.insert(code.end(), std::begin(store), std::end(store));
    }
    const std::uint8_t args[]{0xfc,0x49,0x8b,0xcf,0x48,0x8b,0xd5,0x48,0xb8};
    code.insert(code.end(), std::begin(args), std::end(args));
    const auto address = reinterpret_cast<std::uintptr_t>(callback);
    for (unsigned index = 0; index < 8; ++index) code.push_back(static_cast<std::uint8_t>(address >> (index*8)));
    code.insert(code.end(), {0xff,0xd0});
    for (unsigned index = 0; index < 6; ++index) {
        const std::uint8_t restore[]{0xf3,0x0f,0x6f,static_cast<std::uint8_t>(0x44+index*8),0x24,
            static_cast<std::uint8_t>(0x20+index*16)};
        code.insert(code.end(), std::begin(restore), std::end(restore));
    }
    code.insert(code.end(), {0x48,0x81,0xc4,0x80,0,0,0,0x41,0x5b,0x41,0x5a,0x41,0x59,0x41,0x58,
        0x5a,0x59,0x58,0x9d});
    return code;
}

std::vector<std::uint8_t> cursor_callback_code(void* callback, std::size_t capture_offset) {
    // Cursor call site is profile-verified at an instruction that reads [R14+2F8].
    // The reflected cursor begins at [R14+270]; pass that address while the
    // owning object is live, then restore every volatile register before replay.
    std::vector<std::uint8_t> code{0x9c,0x50,0x51,0x52,0x41,0x50,0x41,0x51,0x41,0x52,0x41,0x53,
        0x48,0x81,0xec,0x80,0,0,0};
    for (unsigned index = 0; index < 6; ++index) {
        const std::uint8_t store[]{0xf3,0x0f,0x7f,static_cast<std::uint8_t>(0x44+index*8),0x24,
            static_cast<std::uint8_t>(0x20+index*16)};
        code.insert(code.end(), std::begin(store), std::end(store));
    }
    code.insert(code.end(), {0xfc,0x49,0x8d,0x8e}); // lea rcx,[r14+disp32]
    const auto offset = static_cast<std::uint32_t>(capture_offset);
    for (unsigned index = 0; index < sizeof(offset); ++index)
        code.push_back(static_cast<std::uint8_t>(offset >> (index * 8)));
    // Third argument is the same RBP frame used by client_cursor's own voxel
    // reads. R8 is already saved/restored above/below; stack layout is unchanged.
    code.insert(code.end(), {0x49,0x8b,0xd5,0x49,0x89,0xe8,0x48,0xb8}); // rdx=r13, r8=rbp
    const auto address = reinterpret_cast<std::uintptr_t>(callback);
    for (unsigned index = 0; index < 8; ++index) code.push_back(static_cast<std::uint8_t>(address >> (index*8)));
    code.insert(code.end(), {0xff,0xd0});
    for (unsigned index = 0; index < 6; ++index) {
        const std::uint8_t restore[]{0xf3,0x0f,0x6f,static_cast<std::uint8_t>(0x44+index*8),0x24,
            static_cast<std::uint8_t>(0x20+index*16)};
        code.insert(code.end(), std::begin(restore), std::end(restore));
    }
    code.insert(code.end(), {0x48,0x81,0xc4,0x80,0,0,0,0x41,0x5b,0x41,0x5a,0x41,0x59,0x41,0x58,
        0x5a,0x59,0x58,0x9d});
    return code;
}

std::vector<std::uint8_t> context_callback_code(void* callback) {
    // This profile hook is at the native building-dispatch entry. Preserve the
    // caller registers, then forward its original RCX context to the callback.
    std::vector<std::uint8_t> code{0x9c,0x50,0x51,0x52,0x41,0x50,0x41,0x51,0x41,0x52,0x41,0x53,
        0x48,0x81,0xec,0x88,0,0,0};
    for (unsigned index = 0; index < 6; ++index) {
        const std::uint8_t store[]{0xf3,0x0f,0x7f,static_cast<std::uint8_t>(0x44+index*8),0x24,
            static_cast<std::uint8_t>(0x20+index*16)};
        code.insert(code.end(), std::begin(store), std::end(store));
    }
    code.insert(code.end(), {0x48,0x8b,0x8c,0x24,0xb0,0,0,0,0x31,0xd2,0x48,0xb8});
    const auto address = reinterpret_cast<std::uintptr_t>(callback);
    for (unsigned index = 0; index < 8; ++index) code.push_back(static_cast<std::uint8_t>(address >> (index*8)));
    code.insert(code.end(), {0xff,0xd0});
    for (unsigned index = 0; index < 6; ++index) {
        const std::uint8_t restore[]{0xf3,0x0f,0x6f,static_cast<std::uint8_t>(0x44+index*8),0x24,
            static_cast<std::uint8_t>(0x20+index*16)};
        code.insert(code.end(), std::begin(restore), std::end(restore));
    }
    code.insert(code.end(), {0x48,0x81,0xc4,0x88,0,0,0,0x41,0x5b,0x41,0x5a,0x41,0x59,0x41,0x58,
        0x5a,0x59,0x58,0x9d});
    return code;
}

bool install_hook(std::uint8_t* base, std::string_view signature,
                  const std::uint8_t* original, std::size_t original_size, void* callback,
                  bool cursor_callback = false, bool context_callback = false) {
    const auto fail = [](const char* reason) { initialization_error = reason; return false; };
    if (!original || original_size < 5) return fail("invalid-overwrite-length");
    const auto match = EngineHooks::FindExecutableSignature(base, signature, 2);
    const auto target = match.count == 1 ? match.address : 0;
    if (!target) return fail("signature-not-unique-or-missing");
    if (std::memcmp(reinterpret_cast<void*>(target), original, original_size)) return fail("original-bytes-mismatch");
    auto payload = cursor_callback
        ? cursor_callback_code(callback, KfcRuntimeCompatibility::EnshroudedClient::world_cursor_capture_offset)
        : context_callback ? context_callback_code(callback) : callback_code(callback);
    payload.insert(payload.end(), original, original + original_size);
    payload.insert(payload.end(), {0xe9,0,0,0,0});
    EngineHooks::HookPublicationResult result;
    if (EngineHooks::PublishEngineHook(hooks,target,{original,original_size},std::move(payload),false,result)) return true;
    if (std::strcmp(result.reason,"near-allocation")==0) return fail("near-allocation-failed");
    if (std::strcmp(result.reason,"executable-write-rejected")==0) return fail("safe-code-write-rejected");
    return fail(result.reason);
}
}

namespace EngineHooks {
bool StartupReady() {return accepting.load(std::memory_order_acquire);}
bool Install() {
    if (!hooks.empty()) return accepting.load(std::memory_order_acquire);
    // Published trampolines and their callbacks remain mapped until process exit.
    HMODULE pinned{};
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            reinterpret_cast<LPCWSTR>(&Install), &pinned)) return false;
    std::uint8_t* base{};
    std::size_t image_size{};
    if (!supported_image(base, image_size)) return false;
    hooks.clear();
    // Drain on the actor-world update entry, which continues while the local
    // actor is active. The prop-system callback is event-driven and can stop
    // being called for long stretches, leaving queued ECS work permanently stale.
    if (!install_hook(base, KfcRuntimeCompatibility::EnshroudedClient::game_thread_signature,
            KfcRuntimeCompatibility::EnshroudedClient::game_thread_original.data(),
            KfcRuntimeCompatibility::EnshroudedClient::game_thread_original.size(),
            reinterpret_cast<void*>(&drain))) {
        initialization_error = "game-thread:" + initialization_error;
        Uninstall();
        return false;
    }
    if (!install_hook(base, KfcRuntimeCompatibility::EnshroudedClient::entity_manager_signature,
            KfcRuntimeCompatibility::EnshroudedClient::entity_manager_original.data(),
            KfcRuntimeCompatibility::EnshroudedClient::entity_manager_original.size(),
            reinterpret_cast<void*>(&capture_entity_manager))) {
        initialization_error = "entity-manager:" + initialization_error;
        Uninstall();
        return false;
    }
    const bool prop_hook = install_hook(base, KfcRuntimeCompatibility::EnshroudedClient::world_prop_update_signature,
        KfcRuntimeCompatibility::EnshroudedClient::world_prop_update_original.data(),
        KfcRuntimeCompatibility::EnshroudedClient::world_prop_update_original.size(),
        reinterpret_cast<void*>(&WorldRuntime::OnPropUpdate));
    const bool placement_hook = install_hook(base, KfcRuntimeCompatibility::EnshroudedClient::world_actor_placement_signature,
        KfcRuntimeCompatibility::EnshroudedClient::world_actor_placement_original.data(),
        KfcRuntimeCompatibility::EnshroudedClient::world_actor_placement_original.size(),
        reinterpret_cast<void*>(&WorldRuntime::OnActorPlacement));
    const bool building_dispatch_hook = !KfcRuntimeCompatibility::EnshroudedClient::world_building_dispatch_signature.empty() &&
        install_hook(base, KfcRuntimeCompatibility::EnshroudedClient::world_building_dispatch_signature,
            KfcRuntimeCompatibility::EnshroudedClient::world_building_dispatch_original.data(),
            KfcRuntimeCompatibility::EnshroudedClient::world_building_dispatch_original.size(),
            reinterpret_cast<void*>(&WorldRuntime::OnBuildingDispatch), false, true);
    entity_context_hooks_ready.store(prop_hook && placement_hook && building_dispatch_hook, std::memory_order_release);
    const bool cursor_hook = !KfcRuntimeCompatibility::EnshroudedClient::world_cursor_signature.empty() &&
        install_hook(base, KfcRuntimeCompatibility::EnshroudedClient::world_cursor_signature,
        KfcRuntimeCompatibility::EnshroudedClient::world_cursor_original.data(),
        KfcRuntimeCompatibility::EnshroudedClient::world_cursor_original.size(),
        reinterpret_cast<void*>(&WorldRuntime::OnCursorUpdate), true);
    WorldRuntime::SetCursorHookReady(cursor_hook);
    accepting.store(true, std::memory_order_release);
    PatchRuntime::Initialize();
    return true;
}

void Uninstall() {
    WorldRuntime::ResetContext();
    PatchRuntime::Shutdown();
    accepting.store(false, std::memory_order_release);
    entity_context_hooks_ready.store(false, std::memory_order_release);
    WorldRuntime::SetCursorHookReady(false);
    RestoreEngineHooks(hooks);
    // A return address may still reference a relay. Its allocation stays mapped
    // through process exit even after the restored receipt is discarded.
    std::erase_if(hooks, [](const auto& hook) { return !hook.installed; });
    engine_thread.store(0, std::memory_order_release);
    entity_manager.store(0, std::memory_order_release);
    command_observed.store(false, std::memory_order_release);
    CancelPendingOperations();
}
}
