cmake_minimum_required(VERSION 3.24)
if(NOT WIN32 OR NOT CMAKE_SIZEOF_VOID_P EQUAL 8)
    message(STATUS "Native game runtime is only included in Windows x64 builds")
    return()
endif()

enable_language(RC)
enable_language(C)

add_executable(shroudforge-test-native-plugin-abi EXCLUDE_FROM_ALL
    ../../mod-runtime/tests/native_plugin_abi.c)
target_compile_features(shroudforge-test-native-plugin-abi PRIVATE c_std_11)

set(SHROUDFORGE_NATIVE_COMPILE_JOBS 2 CACHE STRING
    "Maximum compiler processes for the native runtime")
if(NOT SHROUDFORGE_NATIVE_COMPILE_JOBS MATCHES "^[1-9][0-9]*$" OR
   SHROUDFORGE_NATIVE_COMPILE_JOBS GREATER 65536)
    message(FATAL_ERROR "SHROUDFORGE_NATIVE_COMPILE_JOBS must be an integer from 1 to 65536")
endif()

# Two typed original-operation adapters inspect directed call edges. Decode
# actual instructions rather than scanning E8 bytes or storing function RVAs.
include(FetchContent)
set(ZYDIS_BUILD_TOOLS OFF CACHE BOOL "" FORCE)
set(ZYDIS_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(ZYDIS_BUILD_DOXYGEN OFF CACHE BOOL "" FORCE)
set(ZYDIS_FEATURE_ENCODER OFF CACHE BOOL "" FORCE)
set(ZYDIS_BUILD_SHARED_LIB OFF CACHE BOOL "" FORCE)
FetchContent_Declare(zydis
    GIT_REPOSITORY https://github.com/zyantific/zydis.git
    GIT_TAG a2278f1d254e492f6a6b39f6cb5d1f5d515659dc
    GIT_SHALLOW FALSE)
FetchContent_MakeAvailable(zydis)

# Validate programmable fragments before handing them to a driver. The Tools
# release's DEPS file pins this matching Headers revision. No optimizer, CLI,
# tests or shared validator are part of the runtime dependency graph.
FetchContent_Declare(sf_spirv_headers
    URL https://codeload.github.com/KhronosGroup/SPIRV-Headers/tar.gz/b824a462d4256d720bebb40e78b9eb8f78bbb305
    URL_HASH SHA256=c693867f10a7760ef1bcf85419d51783586768cc2c601d03841bc6a8b2554b9c
    SOURCE_SUBDIR download-only)
FetchContent_Declare(sf_spirv_tools
    URL https://codeload.github.com/KhronosGroup/SPIRV-Tools/tar.gz/262bdab48146c937467f826699a40da0fdfc0f1a
    URL_HASH SHA256=8f830936b057003be77e90d37068ff6066eb87a1bab0033f8f2f5feaa966ce56
    SOURCE_SUBDIR download-only)
FetchContent_MakeAvailable(sf_spirv_headers sf_spirv_tools)
set(SPIRV-Headers_SOURCE_DIR "${sf_spirv_headers_SOURCE_DIR}")
set(SPIRV_SKIP_TESTS ON CACHE BOOL "" FORCE)
set(SPIRV_SKIP_EXECUTABLES ON CACHE BOOL "" FORCE)
set(SKIP_SPIRV_TOOLS_INSTALL ON CACHE BOOL "" FORCE)
set(SPIRV_TOOLS_BUILD_STATIC ON CACHE BOOL "" FORCE)
set(SPIRV_TOOLS_USE_MIMALLOC OFF CACHE BOOL "" FORCE)
add_subdirectory("${sf_spirv_tools_SOURCE_DIR}" "${sf_spirv_tools_BINARY_DIR}" EXCLUDE_FROM_ALL)
set_property(TARGET SPIRV-Tools-static PROPERTY MSVC_RUNTIME_LIBRARY
    "MultiThreaded$<$<CONFIG:Debug>:Debug>")
add_library(shroudforge-overlay-shader STATIC graphics/validate_overlay_shader.cpp)
target_include_directories(shroudforge-overlay-shader PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}"
    "${sf_spirv_headers_SOURCE_DIR}/include")
target_compile_features(shroudforge-overlay-shader PRIVATE cxx_std_20)
target_link_libraries(shroudforge-overlay-shader PRIVATE SPIRV-Tools-static)
set_property(TARGET shroudforge-overlay-shader PROPERTY MSVC_RUNTIME_LIBRARY
    "MultiThreaded$<$<CONFIG:Debug>:Debug>")
add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../../game-profile-discovery"
    "${CMAKE_BINARY_DIR}/modules/game-profile-discovery")
set_property(TARGET Zydis Zycore PROPERTY MSVC_RUNTIME_LIBRARY
    "MultiThreaded$<$<CONFIG:Debug>:Debug>")

# Profiles are package data under paths.runtime/profiles. The provider embeds
# licenses only, so a profile update never requires recompiling this DLL.
set(generated_dir "${CMAKE_CURRENT_BINARY_DIR}/generated")
file(MAKE_DIRECTORY "${generated_dir}")
set(runtime_license_rc "${generated_dir}/runtime_licenses.rc")
file(TO_CMAKE_PATH "${zydis_SOURCE_DIR}/LICENSE" zydis_license_path)
file(TO_CMAKE_PATH "${zydis_SOURCE_DIR}/dependencies/zycore/LICENSE" zycore_license_path)
file(TO_CMAKE_PATH "${sf_spirv_tools_SOURCE_DIR}/LICENSE" spirv_tools_license_path)
file(TO_CMAKE_PATH "${sf_spirv_headers_SOURCE_DIR}/LICENSE" spirv_headers_license_path)
set(runtime_license_content "#include <windows.h>\n")
string(APPEND runtime_license_content
    "\"ZYDIS_LICENSE\" RCDATA \"${zydis_license_path}\"\n"
    "\"ZYCORE_LICENSE\" RCDATA \"${zycore_license_path}\"\n"
    "\"SPIRV_TOOLS_LICENSE\" RCDATA \"${spirv_tools_license_path}\"\n"
    "\"SPIRV_HEADERS_LICENSE\" RCDATA \"${spirv_headers_license_path}\"\n")
file(CONFIGURE OUTPUT "${runtime_license_rc}" CONTENT "@runtime_license_content@" @ONLY)

add_library(kfc-runtime SHARED
    map/read_map_frame.cpp
    map/capture_game_map.cpp
    map/hud_visibility.cpp
    map/serve_map_overlay.cpp
    graphics/render_overlay.cpp
    graphics/vulkan_overlay.cpp
    graphics/notification_overlay.cpp
    ecs/bind_world_entities.cpp
    ecs/resolve_entity_handle.cpp
    ecs/resolve_entity_command_queue.cpp
    ecs/read_entity_membership.cpp
    ecs/read_entity_components.cpp
    ecs/write_entity_components.cpp
    ecs/resolve_component_storage.cpp
    ecs/query_building_props.cpp
    ecs/read_building_prop.cpp
    building/register_prop_recipes.cpp
    ecs/set_building_prop_scale.cpp
    ecs/query_entities.cpp
    ecs/query_entities_in_bounds.cpp
    ecs/registry/refresh_component_registry.cpp
    ecs/registry/sample_component_layouts.cpp
    ecs/describe_ecs_runtime.cpp
    engine-context/resolve_engine_owners.cpp
    engine-context/resolve_registered_scene.cpp
    engine-context/configure_engine_layout.cpp
    engine-context/lock_scene_access.cpp
    engine-context/hold_resource_lifetime.cpp
    loaded-resources/initialize_resource_access.cpp
    loaded-resources/parse_resource_layout.cpp
    loaded-resources/resolve_loaded_resource.cpp
    loaded-resources/find_loaded_resources_by_type.cpp
    loaded-resources/read_loaded_resource.cpp
    loaded-resources/read_resource_scalars.cpp
    loaded-resources/resolve_item_resource.cpp
    loaded-resources/compare_exchange_resource_scalar.cpp
    engine-context/track_world_context.cpp
    player-input/sample_world_cursor.cpp
    player-input/read_local_cursor_input.cpp
    player-input/read_local_camera.cpp
    voxels/read_and_write_voxels.cpp
    voxels/read_registered_voxel_grid.cpp
    building/remove_building_prop.cpp
    building/place_building_prop.cpp
    building/run_building_requests.cpp
    building/finish_building.cpp
    building/spawn_building_prop.cpp
    voxels/read_voxel_grid.cpp
    named-services/resolve_named_service.cpp
    named-services/read_and_update_named_service.cpp
    impact/resolve_registered_operation.cpp
    impact/compare_entity_ids.cpp
    impact/submit_entity_destruction.cpp
    impact/submit_building_prop_spawn.cpp
    voxels/write_registered_voxel_grid.cpp
    main-menu/resolve_main_menu.cpp
    main-menu/resolve_character_selection.cpp
    main-menu/resolve_game_flow.cpp
    main-menu/manage_native_main_menu.cpp
    host/manage_runtime_lifecycle.cpp
    host/prepare_selected_startup.cpp
    code-inventory/export_function_inspection.cpp
    code-inventory/resolve_original_call_graph.cpp
    host/describe_runtime.cpp
    game-thread/run_game_thread_operations.cpp
    engine-hooks/scan_executable_signature.cpp
    engine-hooks/allocate_near_code.cpp
    engine-hooks/install_engine_hooks.cpp
    engine-hooks/write_executable_bytes.cpp
    engine-context/observe_entity_manager.cpp
    patches/set_runtime_patch.cpp
    profiles/select_game_profile.cpp
    profiles/read_installed_profiles.cpp
    loaded-resources/select_resource_profile.cpp
    profiles/identify_game_image.cpp
    profiles/read_game_profile.cpp
    player-input/apply_building_input.cpp
    ../../transport/native/steam_network.cpp
    "${runtime_license_rc}")
target_include_directories(kfc-runtime PRIVATE
    "${generated_dir}"
    "${CMAKE_CURRENT_SOURCE_DIR}"
    "${CMAKE_CURRENT_SOURCE_DIR}/ecs"
    "${CMAKE_CURRENT_SOURCE_DIR}/game-thread"
    "${CMAKE_CURRENT_SOURCE_DIR}/player-input"
    "${CMAKE_CURRENT_SOURCE_DIR}/patches"
    "${CMAKE_CURRENT_SOURCE_DIR}/engine-context"
    "${CMAKE_CURRENT_SOURCE_DIR}/loaded-resources"
    "${CMAKE_CURRENT_SOURCE_DIR}/json/single_include")
target_compile_features(kfc-runtime PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime PRIVATE
    KFC_RUNTIME_BUILD WIN32_LEAN_AND_MEAN NOMINMAX)
target_link_libraries(kfc-runtime PRIVATE bcrypt Zydis shroudforge-game-profile-discovery gdi32 user32)
set_property(TARGET kfc-runtime PROPERTY MSVC_RUNTIME_LIBRARY
    "MultiThreaded$<$<CONFIG:Debug>:Debug>")
foreach(configuration DEBUG RELEASE RELWITHDEBINFO MINSIZEREL)
    set_target_properties(kfc-runtime PROPERTIES
        RUNTIME_OUTPUT_DIRECTORY_${configuration} "${CMAKE_BINARY_DIR}/bin")
endforeach()
if(MSVC)
    target_compile_options(kfc-runtime PRIVATE /W4 /EHsc
        "$<$<COMPILE_LANGUAGE:CXX>:/MP${SHROUDFORGE_NATIVE_COMPILE_JOBS}>")
endif()

# Development tools never run from the game or from the modloader.
add_executable(kfc-runtime-test-resource EXCLUDE_FROM_ALL
    ../tests/loaded_resource_access_test.cpp engine-context/resolve_engine_owners.cpp engine-context/resolve_registered_scene.cpp
    engine-context/configure_engine_layout.cpp engine-context/hold_resource_lifetime.cpp engine-context/lock_scene_access.cpp
    profiles/read_installed_profiles.cpp loaded-resources/select_resource_profile.cpp)
target_include_directories(kfc-runtime-test-resource PRIVATE
    "${generated_dir}" "${CMAKE_CURRENT_SOURCE_DIR}"
    "${CMAKE_CURRENT_SOURCE_DIR}/loaded-resources" "${CMAKE_CURRENT_SOURCE_DIR}/engine-context"
    "${CMAKE_CURRENT_SOURCE_DIR}/json/single_include")
target_compile_features(kfc-runtime-test-resource PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime-test-resource PRIVATE KFC_RUNTIME_BUILD WIN32_LEAN_AND_MEAN NOMINMAX)
set_property(TARGET kfc-runtime-test-resource PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
target_link_libraries(kfc-runtime-test-resource PRIVATE shroudforge-game-profile-discovery)
get_target_property(prop_test_sources kfc-runtime SOURCES)
list(REMOVE_ITEM prop_test_sources
    ecs/bind_world_entities.cpp
    ecs/resolve_entity_handle.cpp
    ecs/read_entity_membership.cpp
    ecs/read_entity_components.cpp
    ecs/write_entity_components.cpp
    ecs/resolve_component_storage.cpp
    ecs/query_building_props.cpp
    ecs/read_building_prop.cpp
    building/register_prop_recipes.cpp
    ecs/set_building_prop_scale.cpp
    ecs/query_entities.cpp
    ecs/query_entities_in_bounds.cpp
    ecs/registry/refresh_component_registry.cpp
    ecs/registry/sample_component_layouts.cpp
    ecs/describe_ecs_runtime.cpp
)
add_executable(kfc-runtime-test-prop-query EXCLUDE_FROM_ALL
    ../tests/building_prop_query_test.cpp ${prop_test_sources})
add_executable(kfc-runtime-test-ecs-read EXCLUDE_FROM_ALL
    ../tests/ecs_access_test.cpp ${prop_test_sources})
add_executable(kfc-runtime-test-service EXCLUDE_FROM_ALL
    ../tests/named_service_access_test.cpp ${prop_test_sources})
get_target_property(prop_test_includes kfc-runtime INCLUDE_DIRECTORIES)
foreach(test_target kfc-runtime-test-prop-query kfc-runtime-test-ecs-read kfc-runtime-test-service)
    target_include_directories(${test_target} PRIVATE ${prop_test_includes})
    target_compile_features(${test_target} PRIVATE cxx_std_20)
    target_compile_definitions(${test_target} PRIVATE KFC_RUNTIME_BUILD WIN32_LEAN_AND_MEAN NOMINMAX)
    target_link_libraries(${test_target} PRIVATE bcrypt Zydis shroudforge-game-profile-discovery gdi32 user32)
    set_property(TARGET ${test_target} PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endforeach()

set(profile_development_directory "${CMAKE_CURRENT_SOURCE_DIR}/../../../dev/profiles")
add_executable(kfc-runtime-dev EXCLUDE_FROM_ALL
    "${profile_development_directory}/cli/kfc_runtime_dev.cpp"
    "${profile_development_directory}/cli/read_cli_option.cpp"
    "${profile_development_directory}/cli/write_json_report.cpp"
    "${profile_development_directory}/cli/read_pe_image.cpp"
    "${profile_development_directory}/cli/match_function_signatures.cpp"
    "${profile_development_directory}/cli/inspect_executable.cpp"
    "${profile_development_directory}/cli/select_game_build.cpp"
    "${profile_development_directory}/cli/discover_function_candidates.cpp"
    "${profile_development_directory}/cli/scan_function_signatures.cpp"
    "${profile_development_directory}/cli/export_profile_function_catalog.cpp"
    "${profile_development_directory}/cli/capture_process_modules.cpp"
    "${profile_development_directory}/cli/import_legacy_ecs_capture.cpp"
    "${profile_development_directory}/cli/inspect_provider_abi.cpp"
    "${profile_development_directory}/cli/validate_runtime_profile.cpp"
    "${profile_development_directory}/cli/create_runtime_profile_draft.cpp"
    "${profile_development_directory}/cli/approve_runtime_profile.cpp")
add_executable(kfc-runtime-test-building-input EXCLUDE_FROM_ALL
    ../tests/building_input_test.cpp player-input/apply_building_input.cpp)

add_executable(kfc-runtime-test-entity-command-queue EXCLUDE_FROM_ALL
    ../tests/entity_command_queue_test.cpp)
add_executable(kfc-runtime-test-entity-destruction EXCLUDE_FROM_ALL
    ../tests/submit_entity_destruction_test.cpp)
add_executable(kfc-runtime-test-original-code EXCLUDE_FROM_ALL
    ../tests/original_call_graph_test.cpp)
add_executable(kfc-runtime-test-native-mutation-contract EXCLUDE_FROM_ALL
    ../tests/native_world_mutation_contract_test.cpp)
foreach(test_target kfc-runtime-test-entity-command-queue kfc-runtime-test-entity-destruction kfc-runtime-test-original-code kfc-runtime-test-native-mutation-contract)
    target_compile_features(${test_target} PRIVATE cxx_std_20)
    target_include_directories(${test_target} PRIVATE
        "${CMAKE_CURRENT_SOURCE_DIR}" "${CMAKE_CURRENT_SOURCE_DIR}/json/single_include")
    target_compile_definitions(${test_target} PRIVATE KFC_RUNTIME_BUILD WIN32_LEAN_AND_MEAN NOMINMAX)
    set_property(TARGET ${test_target} PROPERTY MSVC_RUNTIME_LIBRARY
        "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endforeach()
target_link_libraries(kfc-runtime-test-original-code PRIVATE Zydis)
target_link_libraries(kfc-runtime-test-native-mutation-contract PRIVATE Zydis)
target_compile_features(kfc-runtime-test-building-input PRIVATE cxx_std_20)
target_include_directories(kfc-runtime-test-building-input PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}" "${CMAKE_CURRENT_SOURCE_DIR}/player-input"
    "${CMAKE_CURRENT_SOURCE_DIR}/ecs" "${CMAKE_CURRENT_SOURCE_DIR}/engine-context"
    "${CMAKE_CURRENT_SOURCE_DIR}/json/single_include")
target_compile_definitions(kfc-runtime-test-building-input PRIVATE KFC_RUNTIME_BUILD NOMINMAX WIN32_LEAN_AND_MEAN)
target_compile_features(kfc-runtime-dev PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime-dev PRIVATE WIN32_LEAN_AND_MEAN NOMINMAX
    SHROUDFORGE_PROFILE_DIRECTORY="${profile_development_directory}")
target_include_directories(kfc-runtime-dev PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/json/single_include"
    "${profile_development_directory}/cli")
target_link_libraries(kfc-runtime-dev PRIVATE bcrypt)
if(MSVC)
    target_compile_options(kfc-runtime-dev PRIVATE /W4 /EHsc)
endif()

add_executable(kfc-runtime-capture-ecs EXCLUDE_FROM_ALL
    "${profile_development_directory}/capture/capture_legacy_component_registry.cpp")
target_compile_features(kfc-runtime-capture-ecs PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime-capture-ecs PRIVATE NOMINMAX)
if(MSVC)
    target_compile_options(kfc-runtime-capture-ecs PRIVATE /W4 /EHsc)
endif()

# Game-specific profile investigation tools. These ship only in the development package.
add_executable(kfc-runtime-inspect-component-metadata EXCLUDE_FROM_ALL
    "${profile_development_directory}/inspect/inspect_component_metadata.cpp")
add_executable(kfc-runtime-live-entity-manager-sample EXCLUDE_FROM_ALL
    "${profile_development_directory}/checks/sample_entity_manager_registers.cpp")
add_executable(kfc-runtime-live-entity-managers EXCLUDE_FROM_ALL
    "${profile_development_directory}/capture/find_entity_managers.cpp")
add_executable(kfc-runtime-live-type-references EXCLUDE_FROM_ALL
    "${profile_development_directory}/capture/find_live_type_references.cpp")
foreach(tool_target
    kfc-runtime-inspect-component-metadata kfc-runtime-live-entity-manager-sample
    kfc-runtime-live-entity-managers kfc-runtime-live-type-references)
    target_compile_features(${tool_target} PRIVATE cxx_std_20)
    set_property(TARGET ${tool_target} PROPERTY MSVC_RUNTIME_LIBRARY
        "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endforeach()
add_executable(kfc-runtime-audit-live-component-registry EXCLUDE_FROM_ALL
    "${profile_development_directory}/verify/audit_live_component_registry.cpp")
target_compile_features(kfc-runtime-audit-live-component-registry PRIVATE cxx_std_20)
target_include_directories(kfc-runtime-audit-live-component-registry PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/json/single_include")
set_property(TARGET kfc-runtime-audit-live-component-registry PROPERTY MSVC_RUNTIME_LIBRARY
    "MultiThreaded$<$<CONFIG:Debug>:Debug>")

add_executable(kfc-runtime-verify-component-registry EXCLUDE_FROM_ALL
    "${profile_development_directory}/checks/verify_component_registry.cpp")
target_compile_features(kfc-runtime-verify-component-registry PRIVATE cxx_std_20)
target_include_directories(kfc-runtime-verify-component-registry PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/json/single_include")

foreach(tool_target
    kfc-runtime-dev
    kfc-runtime-capture-ecs
    kfc-runtime-inspect-component-metadata
    kfc-runtime-live-entity-manager-sample
    kfc-runtime-live-entity-managers
    kfc-runtime-live-type-references
    kfc-runtime-audit-live-component-registry
    kfc-runtime-verify-component-registry)
    foreach(configuration DEBUG RELEASE RELWITHDEBINFO MINSIZEREL)
        set_target_properties(${tool_target} PROPERTIES
            RUNTIME_OUTPUT_DIRECTORY_${configuration} "${CMAKE_BINARY_DIR}/tools")
    endforeach()
endforeach()

get_target_property(world_context_test_sources kfc-runtime SOURCES)
list(REMOVE_ITEM world_context_test_sources
    engine-context/track_world_context.cpp
    player-input/sample_world_cursor.cpp
    voxels/read_and_write_voxels.cpp
    building/remove_building_prop.cpp
    building/place_building_prop.cpp
    building/run_building_requests.cpp
    building/finish_building.cpp
    building/spawn_building_prop.cpp
    voxels/read_voxel_grid.cpp
)
add_executable(kfc-runtime-test-world-context EXCLUDE_FROM_ALL
    ../tests/world_context_session_test.cpp ${world_context_test_sources})
target_include_directories(kfc-runtime-test-world-context PRIVATE ${prop_test_includes})
target_compile_features(kfc-runtime-test-world-context PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime-test-world-context PRIVATE KFC_RUNTIME_BUILD WIN32_LEAN_AND_MEAN NOMINMAX)
target_link_libraries(kfc-runtime-test-world-context PRIVATE bcrypt Zydis shroudforge-game-profile-discovery gdi32 user32)
set_property(TARGET kfc-runtime-test-world-context PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")

# Inert fixtures exercise the same owners and renderer as the native provider.
add_executable(kfc-runtime-test-map-frame EXCLUDE_FROM_ALL
    ../tests/map_frame_test.cpp map/read_map_frame.cpp)
add_executable(kfc-runtime-test-map-service-lifetime EXCLUDE_FROM_ALL
    ../tests/map_service_lifetime_test.cpp map/read_map_frame.cpp
    graphics/render_overlay.cpp graphics/notification_overlay.cpp)
add_executable(kfc-runtime-test-overlay-gpu EXCLUDE_FROM_ALL
    ../tests/overlay_gpu_test.cpp map/read_map_frame.cpp map/capture_game_map.cpp
    map/hud_visibility.cpp graphics/render_overlay.cpp graphics/notification_overlay.cpp
    engine-hooks/write_executable_bytes.cpp engine-hooks/allocate_near_code.cpp)
foreach(test_target kfc-runtime-test-map-frame kfc-runtime-test-map-service-lifetime kfc-runtime-test-overlay-gpu)
    target_include_directories(${test_target} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
    target_compile_features(${test_target} PRIVATE cxx_std_20)
    target_compile_definitions(${test_target} PRIVATE KFC_RUNTIME_BUILD WIN32_LEAN_AND_MEAN NOMINMAX)
    target_link_libraries(${test_target} PRIVATE bcrypt gdi32 user32)
    set_property(TARGET ${test_target} PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endforeach()

add_executable(kfc-runtime-test-executable-write EXCLUDE_FROM_ALL ../tests/executable_write_test.cpp)
target_include_directories(kfc-runtime-test-executable-write PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_features(kfc-runtime-test-executable-write PRIVATE cxx_std_20)
set_property(TARGET kfc-runtime-test-executable-write PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
add_executable(kfc-runtime-test-service-lifecycle EXCLUDE_FROM_ALL ../tests/runtime_service_lifecycle_test.cpp)
target_include_directories(kfc-runtime-test-service-lifecycle PRIVATE ${prop_test_includes})
target_compile_features(kfc-runtime-test-service-lifecycle PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime-test-service-lifecycle PRIVATE KFC_RUNTIME_BUILD WIN32_LEAN_AND_MEAN NOMINMAX)
set_property(TARGET kfc-runtime-test-service-lifecycle PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
add_executable(kfc-runtime-test-metadata-startup EXCLUDE_FROM_ALL ../tests/metadata_startup_test.cpp)
target_include_directories(kfc-runtime-test-metadata-startup PRIVATE ${prop_test_includes})
target_compile_features(kfc-runtime-test-metadata-startup PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime-test-metadata-startup PRIVATE KFC_RUNTIME_BUILD WIN32_LEAN_AND_MEAN NOMINMAX)
target_link_libraries(kfc-runtime-test-metadata-startup PRIVATE bcrypt)
set_property(TARGET kfc-runtime-test-metadata-startup PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")

# These existing fixtures compile the real renderer as part of their source set.
foreach(shader_consumer kfc-runtime kfc-runtime-test-overlay-gpu
    kfc-runtime-test-prop-query kfc-runtime-test-ecs-read kfc-runtime-test-service
    kfc-runtime-test-world-context)
    target_link_libraries(${shader_consumer} PRIVATE shroudforge-overlay-shader)
endforeach()
