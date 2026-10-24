#pragma once
#include "code-inventory/resolve_original_call_graph.h"
#include "main-menu/resolve_character_selection.h"
#include <array>
#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

namespace NativeMainMenu {
struct MenuCall {
    std::uintptr_t address{};
    std::array<std::uint8_t, 5> original{};
};
struct MenuBinding {
    std::uintptr_t original_entry{};
    std::vector<MenuCall> insertion_calls;
    std::size_t menu_calls{};
    OriginalCode::CodeEvidence evidence;
    CharacterSelectionBinding character;
    std::string error;
};

// Reads the loaded image and validates the original menu call contract.
// This operation never changes executable memory or calls the selected entry.
bool ResolveMainMenu(MenuBinding& output);
}
