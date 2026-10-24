#pragma once
#include "code-inventory/resolve_original_call_graph.h"
#include <cstddef>
#include <cstdint>
#include <string>

namespace NativeMainMenu {
// Original application FSM identity and two derived mutable fields. No engine
// function is called and no borrowed allocation is retained between samples.
struct GameFlowBinding {
    std::uintptr_t root{}, root_vtable{}, state_rows{};
    std::size_t fsm_offset{}, current_offset{};
    std::uint64_t state_count{};
    std::int32_t pregame_state{-1};
    std::string error;
};
bool ResolveGameFlow(GameFlowBinding& output);
// 0 unknown/transition, 1 original PreGame, 2 validated other original state.
std::uint32_t ReadGameFlowState(const GameFlowBinding& binding) noexcept;
}
