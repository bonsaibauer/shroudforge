cmake_minimum_required(VERSION 3.24)
if(NOT WIN32 OR NOT CMAKE_SIZEOF_VOID_P EQUAL 8)
    message(STATUS "Native game runtime is only included in Windows x64 builds")
    return()
endif()

enable_language(RC)
enable_language(C)

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
set_property(TARGET Zydis Zycore PROPERTY MSVC_RUNTIME_LIBRARY
    "MultiThreaded$<$<CONFIG:Debug>:Debug>")

# Each approved build is a complete, single-file compatibility profile.
file(GLOB_RECURSE profile_json_files CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/../profiles/*.json")
set(runtime_profiles)
set(resource_read_profiles)
foreach(profile_file IN LISTS profile_json_files)
    file(READ "${profile_file}" profile_contents)
    string(JSON profile_provenance_type ERROR_VARIABLE profile_provenance_error
        TYPE "${profile_contents}" provenance)
    if(profile_provenance_error MATCHES "not found")
        # Existing execution profiles predate provenance metadata. Keep their
        # established runtime validation behavior while requiring provenance
        # for newly introduced read-only resource profiles.
        list(APPEND runtime_profiles "${profile_file}")
    elseif(NOT profile_provenance_error STREQUAL "NOTFOUND" OR
           NOT profile_provenance_type STREQUAL "OBJECT")
        message(FATAL_ERROR "Profile provenance must be a JSON object: ${profile_file}")
    else()
        string(JSON profile_status ERROR_VARIABLE profile_status_error
            GET "${profile_contents}" provenance status)
        if(NOT profile_status_error STREQUAL "NOTFOUND")
            message(FATAL_ERROR "Profile provenance status is required: ${profile_file}")
        elseif(profile_status STREQUAL "approved")
        list(APPEND runtime_profiles "${profile_file}")
        elseif(profile_status STREQUAL "read-validated")
            string(JSON resource_read_validated ERROR_VARIABLE resource_status_error
                GET "${profile_contents}" provenance resourceReadValidated)
            string(JSON resource_layout_type ERROR_VARIABLE resource_layout_error
                TYPE "${profile_contents}" resourceLayout)
            if(NOT resource_status_error STREQUAL "NOTFOUND" OR
               NOT resource_read_validated OR
               NOT resource_layout_error STREQUAL "NOTFOUND" OR
               NOT resource_layout_type STREQUAL "OBJECT")
                message(FATAL_ERROR "Read-validated profiles must contain a validated resourceLayout: ${profile_file}")
            endif()
            list(APPEND resource_read_profiles "${profile_file}")
        else()
            message(FATAL_ERROR "Unsupported profile provenance status: ${profile_file}")
        endif()
    endif()
endforeach()
if(NOT runtime_profiles)
    message(FATAL_ERROR "No production profiles found under profiles/")
endif()

set(generated_dir "${CMAKE_CURRENT_BINARY_DIR}/generated")
file(MAKE_DIRECTORY "${generated_dir}")
set(profile_resource_rc "${generated_dir}/embedded_profiles.rc")
set(profile_resource_header "${generated_dir}/embedded_profile_ids.h")
set(profile_rc_content "#include <windows.h>\n")
file(TO_CMAKE_PATH "${zydis_SOURCE_DIR}/LICENSE" zydis_license_path)
file(TO_CMAKE_PATH "${zydis_SOURCE_DIR}/dependencies/zycore/LICENSE" zycore_license_path)
string(APPEND profile_rc_content
    "\"ZYDIS_LICENSE\" RCDATA \"${zydis_license_path}\"\n"
    "\"ZYCORE_LICENSE\" RCDATA \"${zycore_license_path}\"\n")
set(profile_header_content "#pragma once\nnamespace KfcRuntimeEmbedded {\ninline constexpr int profile_resource_ids[] = {")
set(resource_read_header_content "inline constexpr int resource_read_profile_ids[] = {")

set(profile_resource_id 101)
foreach(profile IN LISTS runtime_profiles)
    file(TO_CMAKE_PATH "${profile}" profile_rc_path)
    string(APPEND profile_rc_content "${profile_resource_id} RCDATA \"${profile_rc_path}\"\n")
    string(APPEND profile_header_content "${profile_resource_id},")
    math(EXPR profile_resource_id "${profile_resource_id} + 1")
endforeach()
if(resource_read_profiles)
    foreach(profile IN LISTS resource_read_profiles)
        file(TO_CMAKE_PATH "${profile}" profile_rc_path)
        string(APPEND profile_rc_content "${profile_resource_id} RCDATA \"${profile_rc_path}\"\n")
        string(APPEND resource_read_header_content "${profile_resource_id},")
        math(EXPR profile_resource_id "${profile_resource_id} + 1")
    endforeach()
else()
    string(APPEND resource_read_header_content "0,")
endif()
string(APPEND profile_header_content "};\n${resource_read_header_content}};\n}")

file(WRITE "${profile_resource_rc}" "${profile_rc_content}")
file(WRITE "${profile_resource_header}" "${profile_header_content}")
set_source_files_properties("${profile_resource_rc}" PROPERTIES
    GENERATED TRUE
    OBJECT_DEPENDS "${runtime_profiles};${resource_read_profiles}")

add_library(kfc-runtime SHARED
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
    profiles/identify_game_image.cpp
    profiles/read_game_profile.cpp
    player-input/apply_building_input.cpp
    ../../transport/native/steam_network.cpp
    "${profile_resource_rc}")
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
target_link_libraries(kfc-runtime PRIVATE bcrypt Zydis)
set_property(TARGET kfc-runtime PROPERTY MSVC_RUNTIME_LIBRARY
    "MultiThreaded$<$<CONFIG:Debug>:Debug>")
foreach(configuration DEBUG RELEASE RELWITHDEBINFO MINSIZEREL)
    set_target_properties(kfc-runtime PROPERTIES
        RUNTIME_OUTPUT_DIRECTORY_${configuration} "${CMAKE_BINARY_DIR}/bin")
endforeach()
if(MSVC)
    target_compile_options(kfc-runtime PRIVATE /W4 /EHsc)
endif()

# Development tools never run from the game or from the modloader.
add_executable(kfc-runtime-test-resource EXCLUDE_FROM_ALL
    ../tests/loaded_resource_access_test.cpp engine-context/resolve_engine_owners.cpp engine-context/resolve_registered_scene.cpp
    engine-context/configure_engine_layout.cpp engine-context/hold_resource_lifetime.cpp engine-context/lock_scene_access.cpp)
target_include_directories(kfc-runtime-test-resource PRIVATE
    "${generated_dir}" "${CMAKE_CURRENT_SOURCE_DIR}"
    "${CMAKE_CURRENT_SOURCE_DIR}/loaded-resources" "${CMAKE_CURRENT_SOURCE_DIR}/engine-context"
    "${CMAKE_CURRENT_SOURCE_DIR}/json/single_include")
target_compile_features(kfc-runtime-test-resource PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime-test-resource PRIVATE KFC_RUNTIME_BUILD WIN32_LEAN_AND_MEAN NOMINMAX)
set_property(TARGET kfc-runtime-test-resource PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
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
    target_link_libraries(${test_target} PRIVATE bcrypt Zydis)
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
target_link_libraries(kfc-runtime-test-world-context PRIVATE bcrypt Zydis)
set_property(TARGET kfc-runtime-test-world-context PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
