#pragma once
#include <cstdint>
#include <cstddef>
#include <string>
#include <vector>

namespace KfcRuntimeCompatibility::EnshroudedClient {
struct RuntimeComponent { std::string qualified_name; std::uint16_t index; std::uint32_t size; };
struct RuntimeOperation {
    std::string name;
    std::uintptr_t function_rva{}, global_rva{}, guard_rva{};
    std::uintptr_t validation_offset{};
    std::uint32_t mode{};
    std::ptrdiff_t context_pointer_offset{}, world_offset{};
    std::vector<std::uint8_t> guard_bytes;
    std::string abi, thread, context, status;
    bool available{};
};
struct RuntimeWorldContextLayout {
    std::size_t actor_frame_service_view{}, service_view_world{}, placement_context{};
    std::size_t place_queue{}, remove_queue{}, publish_state{}, publish_commands{}, owner{};
};
struct RuntimeWorldGrid {
    std::string id;
    double origin[3]{}, cell_size[3]{};
    std::uint64_t maximum[3]{};
};
struct RuntimePatch {
    std::string name, signature, kind;
    std::string function_id;
    std::vector<std::uint8_t> payload;
    std::uintptr_t function_begin_rva{}, function_end_rva{};
    std::size_t overwrite{}, return_rel32_offset{}, target_offset{};
};
inline std::uint32_t image_timestamp{}, image_size{};
inline std::string image_sha256, image_target;
inline bool exact_build_match{};
inline std::size_t entity_manager_count{}, entity_manager_table{}, component_offsets{}, component_strides{};
inline std::size_t entity_id{}, entity_generation{}, entity_layout{}, entity_storage{}, entity_definition{}, entity_row{}, component_bits{}, lookup_manager{};
inline std::size_t definition_uuid{}, definition_name{}, definition_name_size{};
inline std::string game_thread_signature, entity_manager_signature, status{"not-initialized"};
inline std::string world_prop_update_signature, world_actor_placement_signature;
inline std::vector<std::uint8_t> game_thread_original, entity_manager_original;
inline std::vector<std::uint8_t> world_prop_update_original, world_actor_placement_original;
inline std::string world_building_dispatch_signature;
inline std::vector<std::uint8_t> world_building_dispatch_original;
inline std::string world_cursor_signature;
inline std::vector<std::uint8_t> world_cursor_original;
inline std::size_t world_cursor_capture_offset{};
inline std::uintptr_t world_finish_event_id_rva{};
inline RuntimeWorldContextLayout world_context_layout{};
inline std::vector<RuntimeWorldGrid> runtime_world_grids;
inline std::vector<RuntimeComponent> runtime_components;
inline std::vector<RuntimeOperation> runtime_operations;
inline std::vector<RuntimePatch> runtime_patches;
bool Load();
}
