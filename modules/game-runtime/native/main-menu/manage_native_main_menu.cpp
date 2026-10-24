#include "main-menu/native_main_menu.h"
#include "main-menu/resolve_main_menu.h"
#include "main-menu/resolve_game_flow.h"
#include "engine-hooks/allocate_near_code.h"
#include "engine-hooks/encode_relative_jump.h"
#include "engine-hooks/write_executable_bytes.h"
#include "named-services/named_service_contract.h"
#include <windows.h>
#include <algorithm>
#include <array>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <mutex>
#include <vector>
#include <nlohmann/json.hpp>

namespace NativeMainMenu {
namespace {
// Enshrouded's immediate menu-entry argument: UTF-8 span, its engine identity,
// then eight option bytes. Preserve every option from the Settings entry.
struct MenuDescriptor {
    const char* text;
    std::uint64_t length;
    std::uint64_t identity;
    std::array<std::uint8_t, 8> options;
};
static_assert(sizeof(MenuDescriptor) == 32);
static_assert(offsetof(MenuDescriptor, options) == 24);
using DrawEntry = bool(__fastcall*)(void*, const MenuDescriptor*);

constexpr std::uint64_t label_identity(const char* text) {
    // FNV-1a is the engine's widget-label identity, never an executable gate.
    std::uint64_t value = 14695981039346656037ull;
    while (*text) { value ^= static_cast<unsigned char>(*text++); value *= 1099511628211ull; }
    return value;
}

struct InstalledCall {
    MenuCall call;
    std::array<std::uint8_t, 5> replacement;
};
std::mutex lifecycle_mutex;
std::atomic<DrawEntry> original_entry{};
std::atomic<bool> accepting{};
std::atomic<std::uint64_t> draws{}, clicks{};
std::vector<InstalledCall> installed;
std::size_t menu_calls{};
std::string state{"not-initialized"}, reason;
GameFlowBinding game_flow;
std::atomic<std::uint32_t> main_menu_state{};

// The existing menu callback owns capture. ABI/Lua/status consumers only copy
// this cache. No worker, timer, process handle or second menu hook is involved.
std::mutex character_mutex;
CharacterSelectionBinding character_binding;
std::uintptr_t observed_owner{};
std::array<std::uint8_t, 2> observed_selection{};
bool selection_observed{};
std::string character_name, character_reason;
std::uint64_t name_reads{};
struct SourceHeader {
    std::uintptr_t entries{};
    std::uint64_t count{};
    bool operator==(const SourceHeader&) const = default;
};
struct NameDescriptor {
    std::uintptr_t text{};
    std::uint64_t length{}, identity{};
    bool operator==(const NameDescriptor&) const = default;
};
static_assert(sizeof(NameDescriptor) == 24);
std::array<SourceHeader, 64> observed_sources{};
std::size_t selected_source{}, selected_index{};
NameDescriptor observed_name;
bool entry_located{};
bool name_attempted{};

bool character_owner(std::uintptr_t& owner) {
    using namespace NamedServiceImplementation;
    if (!character_binding.context_slot || !read(character_binding.context_slot, owner) || !owner) return false;
    for (const auto offset : character_binding.owner_path) {
        if (!field(owner, offset, owner) || !owner) return false;
    }
    return true;
}

bool read_sources(std::uintptr_t owner, std::array<SourceHeader, 64>& headers) {
    using namespace NamedServiceImplementation;
    const auto& c = character_binding;
    if (!c.source_count || c.source_count > headers.size()) return false;
    for (std::size_t source_index = 0; source_index < c.source_count; ++source_index) {
        std::uintptr_t source{};
        if (!add(owner, c.sources_offset + source_index * c.source_stride, source)) return false;
        auto& header = headers[source_index];
        if (!field(source, c.entries_pointer_offset, header.entries) ||
            !field(source, c.entries_count_offset, header.count) || header.count > 256 ||
            (header.count && (!header.entries || header.entries % alignof(std::uintptr_t)))) return false;
    }
    return true;
}

bool locate_character(const std::array<SourceHeader, 64>& headers,
        const std::array<std::uint8_t, 2>& selected) {
    using namespace NamedServiceImplementation;
    const auto& c = character_binding;
    bool found{};
    for (std::size_t source = 0; source < c.source_count; ++source) {
        for (std::size_t index = 0; index < headers[source].count; ++index) {
            std::uintptr_t entry{};
            std::array<std::uint8_t, 2> id{};
            if (!add(headers[source].entries, index * c.entry_stride, entry) ||
                !field(entry, c.entry_id_offset, id)) return false;
            if (id != selected) continue;
            if (found) return false;
            found = true;
            selected_source = source;
            selected_index = index;
        }
    }
    return found;
}

bool read_name_descriptor(const std::array<SourceHeader, 64>& headers,
        const std::array<std::uint8_t, 2>& selected, NameDescriptor& descriptor) {
    using namespace NamedServiceImplementation;
    const auto& c = character_binding;
    std::uintptr_t entry{};
    std::array<std::uint8_t, 2> id{};
    // Compute the entry from the CURRENT list. Never dereference a retained
    // entry pointer after a list replacement, resize, or menu transition.
    return selected_source < c.source_count && selected_index < headers[selected_source].count &&
        c.name_span_offset <= c.entry_stride && sizeof(descriptor) <= c.entry_stride - c.name_span_offset &&
        c.entry_id_offset == c.name_span_offset + sizeof(descriptor) &&
        add(headers[selected_source].entries, selected_index * c.entry_stride, entry) &&
        field(entry, c.entry_id_offset, id) && id == selected && field(entry, c.name_span_offset, descriptor);
}

bool read_character_name(std::uintptr_t owner, const std::array<std::uint8_t, 2>& selected,
        const std::array<SourceHeader, 64>& headers, const NameDescriptor& span, std::string& name) {
    using namespace NamedServiceImplementation;
    if (!span.text || !span.length || span.length > 320) return false;
    std::array<char, 320> text{};
    if (!copy(text.data(), reinterpret_cast<const void*>(span.text), static_cast<std::size_t>(span.length))) return false;
    // The original producer stores the engine's signed-byte FNV identity next
    // to its 16-byte name view. Admit that extra field only when the copied
    // text proves its identity. This is name data, never an executable gate.
    std::uint64_t identity = 14695981039346656037ull;
    for (std::size_t index = 0; index < span.length; ++index) {
        identity ^= static_cast<std::uint64_t>(static_cast<std::int64_t>(static_cast<std::int8_t>(text[index])));
        identity *= 1099511628211ull;
    }
    if (identity != span.identity) return false;
    // Validate complete UTF-8 and the same bounded, control-free name contract
    // consumed by Rust. No locale conversion or replacement characters.
    std::array<wchar_t, 320> wide{};
    const auto wide_size = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
        static_cast<int>(span.length), wide.data(), static_cast<int>(wide.size()));
    if (!wide_size) return false;
    std::size_t characters{};
    for (std::size_t index = 0; index < span.length; ++index)
        if ((static_cast<unsigned char>(text[index]) & 0xc0) != 0x80) ++characters;
    if (characters > 80 || std::any_of(wide.begin(), wide.begin() + wide_size,
            [](wchar_t value) { return value < 0x20 || (value >= 0x7f && value <= 0x9f); })) return false;
    std::array<std::uint8_t, 2> retained_selection{};
    std::array<SourceHeader, 64> retained_sources{};
    NameDescriptor retained;
    std::uintptr_t retained_owner{};
    if (!read_sources(owner, retained_sources) || retained_sources != headers ||
        !read_name_descriptor(retained_sources, selected, retained) || retained != span ||
        !field(owner, character_binding.selected_id_offset, retained_selection) || retained_selection != selected ||
        !character_owner(retained_owner) || retained_owner != owner) return false;
    name.assign(text.data(), static_cast<std::size_t>(span.length));
    return name.find_first_not_of(' ') != std::string::npos;
}

void observe_character_selection() noexcept {
    try {
        std::lock_guard lock(character_mutex);
        if (!accepting.load(std::memory_order_acquire)) return;
        std::uintptr_t owner{};
        std::array<std::uint8_t, 2> selected{};
        // Observe selection/descriptor changes on the already-running original
        // menu callback. Unchanged metadata never reads the name's text again.
        if (!character_owner(owner) ||
            !NamedServiceImplementation::field(owner, character_binding.selected_id_offset, selected)) return;
        const bool same = selection_observed && observed_owner == owner && observed_selection == selected;
        if (!same) name_attempted = false;
        selection_observed = true;
        observed_owner = owner;
        observed_selection = selected;
        std::array<SourceHeader, 64> sources{};
        if (!selected[0] || !read_sources(owner, sources)) {
            entry_located = false;
            character_name.clear();
            return;
        }
        const bool locate = !same || sources != observed_sources;
        if (same && !entry_located && !locate) return;
        observed_sources = sources;
        if (locate && !locate_character(sources, selected)) {
            entry_located = false;
            character_name.clear();
            character_reason = "selected-character-entry-unavailable";
            return;
        }
        NameDescriptor descriptor;
        // An in-place reorder can move the selected ID without reallocating
        // the list. Relocate once, then keep default if that attempt fails.
        bool descriptor_read = read_name_descriptor(sources, selected, descriptor);
        if (!descriptor_read && !locate && locate_character(sources, selected))
            descriptor_read = read_name_descriptor(sources, selected, descriptor);
        if (!descriptor_read) {
            entry_located = false;
            character_name.clear();
            character_reason = "selected-character-descriptor-unavailable";
            return;
        }
        entry_located = true;
        if (same && name_attempted && descriptor == observed_name) return;
        name_attempted = true;
        observed_name = descriptor;
        character_name.clear();
        character_reason.clear();
        ++name_reads;
        if (!read_character_name(owner, selected, sources, descriptor, character_name)) {
            character_name.clear();
            character_reason = "selected-character-name-unavailable";
        }
    } catch (...) {
        // Rendering must still forward to the original menu entry.
    }
}

// Published callbacks, thunks and event handles stay alive until process exit.
// A UI thread can retain a return address even after its CALL is restored.
HANDLE enabled_event{}, open_event{};

bool enabled() noexcept {
    return enabled_event && WaitForSingleObject(enabled_event, 0) == WAIT_OBJECT_0;
}

bool __fastcall draw_with_mods(void* selection, const MenuDescriptor* descriptor) {
    const auto draw = original_entry.load(std::memory_order_acquire);
    if (!draw) return false;
    observe_character_selection();
    if (accepting.load(std::memory_order_acquire) && enabled()) {
        auto mods = *descriptor;
        mods.text = "Mods";
        mods.length = 4;
        mods.identity = label_identity("Mods");
        draws.fetch_add(1, std::memory_order_relaxed);
        // Stay on the engine's UI thread for rendering. The external Modloader
        // owns its event loop, just as it does for the configured shortcut and
        // shroudforge.ui.toggle_modloader(). Never enter Lua from this thread.
        if (draw(selection, &mods) && accepting.load(std::memory_order_acquire)) {
            if (SetEvent(open_event)) clicks.fetch_add(1, std::memory_order_relaxed);
        }
    }
    return draw(selection, descriptor);
}

bool ensure_events() {
    const auto name = [](const wchar_t* kind) {
        return std::wstring(L"Local\\ShroudForge.ModMenu.") + kind + L"." +
            std::to_wstring(GetCurrentProcessId());
    };
    if (!enabled_event) enabled_event = CreateEventW(nullptr, TRUE, FALSE, name(L"Enabled").c_str());
    if (!open_event) open_event = CreateEventW(nullptr, FALSE, FALSE, name(L"Open").c_str());
    return enabled_event && open_event;
}

bool restore_calls() {
    bool restored = true;
    for (auto iterator = installed.rbegin(); iterator != installed.rend(); ++iterator) {
        if (!iterator->call.address) continue;
        if (EngineHooks::ReplaceExecutableBytes(iterator->call.address,
                iterator->replacement.data(), iterator->call.original.data(), iterator->call.original.size()))
            iterator->call.address = 0;
        else restored = false;
    }
    std::erase_if(installed, [](const InstalledCall& call) { return !call.call.address; });
    return restored;
}
}

void Initialize() noexcept {
    try {
        std::lock_guard lock(lifecycle_mutex);
        if (accepting.load(std::memory_order_acquire)) return;
        main_menu_state.store(0, std::memory_order_release);
        // Screen classification is independent of ECS readiness and whether
        // the optional Mods row can be installed or is currently visible.
        ResolveGameFlow(game_flow);
        main_menu_state.store(ReadGameFlowState(game_flow), std::memory_order_release);
        if (!installed.empty()) {
            state = "unavailable"; reason = "previous-menu-calls-could-not-be-restored"; return;
        }
        state = "unavailable";
        reason.clear();
        MenuBinding binding;
        if (!ResolveMainMenu(binding)) { reason = binding.error; return; }
        if (!binding.original_entry || binding.insertion_calls.empty()) {
            reason = "menu-binding-incomplete"; return;
        }
        menu_calls = binding.menu_calls;
        if (!ensure_events()) { reason = "menu-event-creation-failed"; return; }
        HMODULE pinned{};
        if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                reinterpret_cast<LPCWSTR>(&draw_with_mods), &pinned)) {
            reason = "menu-provider-pin-failed"; return;
        }
        // One absolute indirect jump is reachable from all selected native CALLs.
        std::array<std::uint8_t, 14> thunk{0xff, 0x25, 0, 0, 0, 0};
        const auto callback = reinterpret_cast<std::uintptr_t>(&draw_with_mods);
        std::memcpy(thunk.data() + 6, &callback, sizeof(callback));
        auto allocation = EngineHooks::AllocateNearCode(binding.insertion_calls.front().address, thunk.size());
        if (!allocation) { reason = "menu-near-allocation-failed"; return; }
        std::memcpy(allocation, thunk.data(), thunk.size());
        DWORD previous{};
        if (!VirtualProtect(allocation, thunk.size(), PAGE_EXECUTE_READ, &previous)) {
            VirtualFree(allocation, 0, MEM_RELEASE); reason = "menu-thunk-protection-failed"; return;
        }
        FlushInstructionCache(GetCurrentProcess(), allocation, thunk.size());
        std::vector<InstalledCall> prepared;
        for (const auto& call : binding.insertion_calls) {
            InstalledCall next{call, {0xe8}};
            std::int32_t relative{};
            if (!EngineHooks::EncodeRelativeJump(call.address,
                    reinterpret_cast<std::uintptr_t>(allocation), relative)) {
                VirtualFree(allocation, 0, MEM_RELEASE); reason = "menu-call-out-of-range"; return;
            }
            std::memcpy(next.replacement.data() + 1, &relative, sizeof(relative));
            prepared.push_back(next);
        }
        if (!OriginalCode::EvidenceUnchanged(binding.evidence)) {
            VirtualFree(allocation, 0, MEM_RELEASE); reason = "menu-code-changed-before-install"; return;
        }
        {
            std::lock_guard character_lock(character_mutex);
            character_binding = std::move(binding.character);
            if (character_binding.context_slot && !OriginalCode::EvidenceUnchanged(character_binding.evidence)) {
                character_binding = {};
                character_binding.error = "character-selection-code-changed-before-install";
            }
            selection_observed = false;
            entry_located = false;
            name_attempted = false;
            observed_owner = 0;
            character_name.clear();
            character_reason = character_binding.error;
            name_reads = 0;
        }
        installed.reserve(prepared.size());
        original_entry.store(reinterpret_cast<DrawEntry>(binding.original_entry), std::memory_order_release);
        bool published = false;
        for (const auto& call : prepared) {
            if (!EngineHooks::ReplaceExecutableBytes(call.call.address, call.call.original.data(),
                    call.replacement.data(), call.replacement.size())) {
                restore_calls();
                if (!published) VirtualFree(allocation, 0, MEM_RELEASE);
                reason = "menu-call-installation-rejected"; return;
            }
            published = true;
            installed.push_back(call);
        }
        state = "installed";
        accepting.store(true, std::memory_order_release);
    } catch (...) {
        accepting.store(false, std::memory_order_release);
        // If bookkeeping allocation failed, keep the published callback mapped
        // and forwarding to the original engine function.
        try { std::lock_guard lock(lifecycle_mutex); restore_calls(); state = "unavailable"; reason = "menu-initialization-exception"; }
        catch (...) {}
    }
}

void Tick() noexcept {
    try {
        std::lock_guard lock(lifecycle_mutex);
        main_menu_state.store(ReadGameFlowState(game_flow), std::memory_order_release);
    } catch (...) { main_menu_state.store(0, std::memory_order_release); }
}

std::uint32_t MainMenuState() noexcept { return main_menu_state.load(std::memory_order_acquire); }

void Shutdown() noexcept {
    accepting.store(false, std::memory_order_release);
    main_menu_state.store(0, std::memory_order_release);
    try {
        std::lock_guard lock(lifecycle_mutex);
        game_flow = {};
        main_menu_state.store(0, std::memory_order_release);
        const bool restored = restore_calls();
        {
            std::lock_guard character_lock(character_mutex);
            character_binding = {};
            selection_observed = false;
            entry_located = false;
            name_attempted = false;
            character_name.clear();
        }
        state = restored ? "stopped" : "stopped-with-forwarding-callback";
        if (!restored) reason = "menu-call-restoration-rejected";
    } catch (...) {}
}

std::string Status() {
    std::lock_guard lock(lifecycle_mutex);
    std::lock_guard character_lock(character_mutex);
    return nlohmann::json({{"state", state}, {"reason", reason}, {"enabled", enabled()},
        {"menuCalls", menu_calls}, {"insertionCalls", installed.size()},
        {"draws", draws.load(std::memory_order_relaxed)},
        {"clicks", clicks.load(std::memory_order_relaxed)},
        {"mainMenuState", MainMenuState()}, {"mainMenuReason", game_flow.error},
        {"characterNameReads", name_reads}, {"characterNameReason", character_reason}}).dump();
}
}

extern "C" KFC_RUNTIME_API std::uint32_t __cdecl KfcRuntimeMainMenuState() {
    return NativeMainMenu::MainMenuState();
}

extern "C" KFC_RUNTIME_API std::size_t __cdecl KfcRuntimeSelectedCharacterName(char* buffer, std::size_t capacity) {
    try {
        std::lock_guard lock(NativeMainMenu::character_mutex);
        const auto& name = NativeMainMenu::character_name;
        if (name.empty()) return 0;
        const auto required = name.size() + 1;
        if (buffer && capacity >= required) std::memcpy(buffer, name.c_str(), required);
        return required;
    } catch (...) { return 0; }
}
