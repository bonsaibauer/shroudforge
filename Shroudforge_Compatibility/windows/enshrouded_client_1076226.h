#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <string_view>

// Validated compatibility facts for Enshrouded game build 1076226.
// These values are private loader details. They are never exposed through the
// ShroudForge API and never belong in a mod.
namespace ShroudforgeCompatibility::EnshroudedClient {
inline constexpr std::uint32_t image_timestamp = 0x6a4236c8;
inline constexpr std::uint32_t image_size = 0x02da7000;

inline constexpr std::size_t entity_manager_count = 0x158;
inline constexpr std::size_t entity_manager_table = 0x188;
inline constexpr std::size_t component_offsets = 0x84;
inline constexpr std::size_t component_strides = 0xa84;

// Continuous prop-system update used by the validated ShroudForge runtime.
// Compatibility owns this build-specific integration point; mods do not.
inline constexpr std::string_view game_thread_signature =
    "33 D2 48 8D 4C 24 30 E8 4E AA 52 00 48 8D 54 24 30 49 8B CF";
inline constexpr std::array<std::uint8_t, 7> game_thread_original{
    0x33, 0xd2, 0x48, 0x8d, 0x4c, 0x24, 0x30
};

inline constexpr std::string_view entity_manager_signature =
    "48 81 C3 88 00 00 00 48 81 FB 80 08 00 00 0F 82 66 FE FF FF";
inline constexpr std::array<std::uint8_t, 7> entity_manager_original{
    0x48, 0x81, 0xc3, 0x88, 0x00, 0x00, 0x00
};

struct RuntimeComponent {
    std::string_view qualified_name;
    std::uint16_t index;
    std::uint32_t size;
};

// Live component catalog entries required by the bundled Lua gameplay mods.
// The schema still supplies fields and types; this profile supplies only the
// build-specific runtime index proven by the live component dump.
inline constexpr RuntimeComponent runtime_components[]{
    {"keen::ecs::DynamicActiveNpcState", 0, 136},
    {"keen::ecs::ClientPlayerInput", 85, 1392},
    {"keen::ecs::CurrentTransform", 125, 56},
    {"keen::ecs::DynamicFallDamage", 199, 16},
    {"keen::ecs::Inventory", 265, 96},
    {"keen::ecs::DynamicLocomotion", 302, 312},
    {"keen::ecs::NetworkStamina", 355, 4},
    {"keen::ecs::PlayerInput", 404, 1320},
    {"keen::ecs::ServerConsumedPlayerInput", 462, 216},
    {"keen::ecs::StaminaDepletion", 499, 4},
    {"keen::ecs::UsedItem", 566, 4},
};
}
