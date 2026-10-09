cmake_minimum_required(VERSION 3.24)
if(NOT WIN32 OR NOT CMAKE_SIZEOF_VOID_P EQUAL 8)
    message(STATUS "Native game runtime is only included in Windows x64 builds")
    return()
endif()

enable_language(RC)

# Each approved build is a complete, single-file compatibility profile.
file(GLOB_RECURSE profile_json_files CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/../profiles/*.json")
set(runtime_profiles)
foreach(profile_file IN LISTS profile_json_files)
    file(READ "${profile_file}" profile_contents)
    string(JSON profile_status ERROR_VARIABLE profile_status_error
        GET "${profile_contents}" provenance status)
    if(profile_status_error STREQUAL "NOTFOUND" AND NOT profile_status STREQUAL "approved")
        message(FATAL_ERROR "Only approved profiles may be embedded: ${profile_file}")
    endif()
    list(APPEND runtime_profiles "${profile_file}")
endforeach()
if(NOT runtime_profiles)
    message(FATAL_ERROR "No production profiles found under profiles/")
endif()

set(generated_dir "${CMAKE_CURRENT_BINARY_DIR}/generated")
file(MAKE_DIRECTORY "${generated_dir}")
set(profile_resource_rc "${generated_dir}/embedded_profiles.rc")
set(profile_resource_header "${generated_dir}/embedded_profile_ids.h")
set(profile_rc_content "#include <windows.h>\n")
set(profile_header_content "#pragma once\nnamespace KfcRuntimeEmbedded {\ninline constexpr int profile_resource_ids[] = {")

set(profile_resource_id 101)
foreach(profile IN LISTS runtime_profiles)
    file(TO_CMAKE_PATH "${profile}" profile_rc_path)
    string(APPEND profile_rc_content "${profile_resource_id} RCDATA \"${profile_rc_path}\"\n")
    string(APPEND profile_header_content "${profile_resource_id},")
    math(EXPR profile_resource_id "${profile_resource_id} + 1")
endforeach()
string(APPEND profile_header_content "};\n}")

file(WRITE "${profile_resource_rc}" "${profile_rc_content}")
file(WRITE "${profile_resource_header}" "${profile_header_content}")
set_source_files_properties("${profile_resource_rc}" PROPERTIES
    GENERATED TRUE
    OBJECT_DEPENDS "${runtime_profiles}")

add_library(kfc-runtime SHARED
    ecs/ecs_runtime.cpp
    game-thread/dispatcher.cpp
    provider/profile.cpp
    provider/provider.cpp
    provider/steam_network.cpp
    world/world_runtime.cpp
    world/building_input.cpp
    patch/patch_runtime.cpp
    "${profile_resource_rc}")
target_include_directories(kfc-runtime PRIVATE
    "${generated_dir}"
    "${CMAKE_CURRENT_SOURCE_DIR}"
    "${CMAKE_CURRENT_SOURCE_DIR}/ecs"
    "${CMAKE_CURRENT_SOURCE_DIR}/game-thread"
    "${CMAKE_CURRENT_SOURCE_DIR}/world"
    "${CMAKE_CURRENT_SOURCE_DIR}/patch"
    "${CMAKE_CURRENT_SOURCE_DIR}/provider"
    "${CMAKE_CURRENT_SOURCE_DIR}/json/single_include")
target_compile_features(kfc-runtime PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime PRIVATE
    KFC_RUNTIME_BUILD WIN32_LEAN_AND_MEAN NOMINMAX)
target_link_libraries(kfc-runtime PRIVATE bcrypt)
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
get_target_property(prop_test_sources kfc-runtime SOURCES)
list(REMOVE_ITEM prop_test_sources ecs/ecs_runtime.cpp)
add_executable(kfc-runtime-test-prop-query EXCLUDE_FROM_ALL
    ../profile-tools/dev/tools/tests/prop_query_test.cpp ${prop_test_sources})
get_target_property(prop_test_includes kfc-runtime INCLUDE_DIRECTORIES)
target_include_directories(kfc-runtime-test-prop-query PRIVATE ${prop_test_includes})
target_compile_features(kfc-runtime-test-prop-query PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime-test-prop-query PRIVATE KFC_RUNTIME_BUILD WIN32_LEAN_AND_MEAN NOMINMAX)
target_link_libraries(kfc-runtime-test-prop-query PRIVATE bcrypt)
set_property(TARGET kfc-runtime-test-prop-query PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
add_executable(kfc-runtime-dev EXCLUDE_FROM_ALL ../profile-tools/dev/tools/dev-console/main.cpp)
add_executable(kfc-runtime-test-building-input EXCLUDE_FROM_ALL
    ../profile-tools/dev/tools/tests/building_input_test.cpp world/building_input.cpp)
target_compile_features(kfc-runtime-test-building-input PRIVATE cxx_std_20)
target_include_directories(kfc-runtime-test-building-input PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}" "${CMAKE_CURRENT_SOURCE_DIR}/world"
    "${CMAKE_CURRENT_SOURCE_DIR}/ecs" "${CMAKE_CURRENT_SOURCE_DIR}/provider")
target_compile_definitions(kfc-runtime-test-building-input PRIVATE KFC_RUNTIME_BUILD NOMINMAX WIN32_LEAN_AND_MEAN)
target_compile_features(kfc-runtime-dev PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime-dev PRIVATE WIN32_LEAN_AND_MEAN NOMINMAX)
target_include_directories(kfc-runtime-dev PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/json/single_include")
target_link_libraries(kfc-runtime-dev PRIVATE bcrypt)
if(MSVC)
    target_compile_options(kfc-runtime-dev PRIVATE /W4 /EHsc)
endif()

add_executable(kfc-runtime-capture-ecs EXCLUDE_FROM_ALL ../profile-tools/dev/tools/dev-console/live-ecs-capture.cpp)
target_compile_features(kfc-runtime-capture-ecs PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime-capture-ecs PRIVATE NOMINMAX)
if(MSVC)
    target_compile_options(kfc-runtime-capture-ecs PRIVATE /W4 /EHsc)
endif()

# Game-specific profile investigation tools. These ship only in the development package.
foreach(tool inspect-component-metadata live-entity-manager-sample live-entity-managers live-type-references)
    add_executable(kfc-runtime-${tool} EXCLUDE_FROM_ALL ../profile-tools/dev/tools/enshrouded/native/${tool}.cpp)
    target_compile_features(kfc-runtime-${tool} PRIVATE cxx_std_20)
    set_property(TARGET kfc-runtime-${tool} PROPERTY MSVC_RUNTIME_LIBRARY
        "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endforeach()
add_executable(kfc-runtime-audit-live-component-registry EXCLUDE_FROM_ALL
    ../profile-tools/dev/tools/enshrouded/native/audit-live-component-registry.cpp)
target_compile_features(kfc-runtime-audit-live-component-registry PRIVATE cxx_std_20)
target_include_directories(kfc-runtime-audit-live-component-registry PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/json/single_include")
set_property(TARGET kfc-runtime-audit-live-component-registry PROPERTY MSVC_RUNTIME_LIBRARY
    "MultiThreaded$<$<CONFIG:Debug>:Debug>")

add_executable(kfc-runtime-verify-component-registry EXCLUDE_FROM_ALL
    ../profile-tools/dev/tools/enshrouded/native/verify-component-registry.cpp)
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
list(REMOVE_ITEM world_context_test_sources world/world_runtime.cpp)
add_executable(kfc-runtime-test-world-context EXCLUDE_FROM_ALL
    ../profile-tools/dev/tools/tests/world_context_test.cpp ${world_context_test_sources})
target_include_directories(kfc-runtime-test-world-context PRIVATE ${prop_test_includes})
target_compile_features(kfc-runtime-test-world-context PRIVATE cxx_std_20)
target_compile_definitions(kfc-runtime-test-world-context PRIVATE KFC_RUNTIME_BUILD WIN32_LEAN_AND_MEAN NOMINMAX)
target_link_libraries(kfc-runtime-test-world-context PRIVATE bcrypt)
set_property(TARGET kfc-runtime-test-world-context PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
