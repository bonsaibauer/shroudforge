#pragma once
#include "code-inventory/resolve_original_call_graph.h"
#include <array>
#include <cstddef>
#include <cstdint>
#include <string>

namespace NativeMainMenu {
// A read-only contract for the menu's current frame. It grants no engine-call ABI.
struct CharacterSelectionBinding {
    std::uintptr_t context_slot{};
    std::array<std::size_t, 3> owner_path{};
    std::size_t sources_offset{}, source_stride{}, source_count{};
    std::size_t entries_pointer_offset{}, entries_count_offset{}, entry_stride{};
    std::size_t selected_id_offset{}, entry_id_offset{}, name_span_offset{};
    // The builder already belongs to MenuBinding's evidence. These are its
    // unchanged original getter and formatter dependencies only.
    OriginalCode::CodeEvidence evidence;
    std::string error;
};

// Reuses the already proven original main-menu builder before hook installation.
// No executable write, engine invocation, process scan or version lookup occurs.
bool ResolveCharacterSelection(const OriginalCode::Procedure& builder,
    CharacterSelectionBinding& output);
}
