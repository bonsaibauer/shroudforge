cmake_minimum_required(VERSION 3.24)
if(NOT WIN32 OR NOT CMAKE_SIZEOF_VOID_P EQUAL 8)
    return()
endif()

add_library(shroudforge-game-profile-discovery STATIC
    discover_game_profile.cpp
    verify_image_equivalence.cpp
    rebase_profile_contract.cpp
    prove_profile_layout.cpp
    live_profile_layout.cpp)
target_include_directories(shroudforge-game-profile-discovery PUBLIC
    "${CMAKE_CURRENT_SOURCE_DIR}/.."
    "${CMAKE_CURRENT_SOURCE_DIR}/../game-runtime/native/json/single_include")
target_compile_features(shroudforge-game-profile-discovery PUBLIC cxx_std_20)
target_compile_definitions(shroudforge-game-profile-discovery PRIVATE WIN32_LEAN_AND_MEAN NOMINMAX)
target_link_libraries(shroudforge-game-profile-discovery PRIVATE bcrypt Zydis)
set_property(TARGET shroudforge-game-profile-discovery PROPERTY MSVC_RUNTIME_LIBRARY
    "MultiThreaded$<$<CONFIG:Debug>:Debug>")
if(MSVC)
    target_compile_options(shroudforge-game-profile-discovery PRIVATE /W4 /EHsc)
endif()

add_executable(kfc-runtime-test-game-profile-discovery EXCLUDE_FROM_ALL
    tests/profile_discovery_test.cpp)
target_link_libraries(kfc-runtime-test-game-profile-discovery PRIVATE shroudforge-game-profile-discovery)
target_compile_definitions(kfc-runtime-test-game-profile-discovery PRIVATE WIN32_LEAN_AND_MEAN NOMINMAX)
set_property(TARGET kfc-runtime-test-game-profile-discovery PROPERTY MSVC_RUNTIME_LIBRARY
    "MultiThreaded$<$<CONFIG:Debug>:Debug>")
foreach(configuration DEBUG RELEASE RELWITHDEBINFO MINSIZEREL)
    set_target_properties(kfc-runtime-test-game-profile-discovery PROPERTIES
        RUNTIME_OUTPUT_DIRECTORY_${configuration} "${CMAKE_BINARY_DIR}/tools")
endforeach()

add_executable(kfc-runtime-test-profile-rebase EXCLUDE_FROM_ALL tests/profile_rebase_test.cpp)
target_link_libraries(kfc-runtime-test-profile-rebase PRIVATE shroudforge-game-profile-discovery)
target_compile_definitions(kfc-runtime-test-profile-rebase PRIVATE WIN32_LEAN_AND_MEAN NOMINMAX)
set_property(TARGET kfc-runtime-test-profile-rebase PROPERTY MSVC_RUNTIME_LIBRARY
    "MultiThreaded$<$<CONFIG:Debug>:Debug>")
foreach(configuration DEBUG RELEASE RELWITHDEBINFO MINSIZEREL)
    set_target_properties(kfc-runtime-test-profile-rebase PROPERTIES
        RUNTIME_OUTPUT_DIRECTORY_${configuration} "${CMAKE_BINARY_DIR}/tools")
endforeach()

add_executable(kfc-runtime-profile-rebase-report EXCLUDE_FROM_ALL tools/profile_rebase_report.cpp)
target_link_libraries(kfc-runtime-profile-rebase-report PRIVATE shroudforge-game-profile-discovery)
target_compile_definitions(kfc-runtime-profile-rebase-report PRIVATE WIN32_LEAN_AND_MEAN NOMINMAX)
set_property(TARGET kfc-runtime-profile-rebase-report PROPERTY MSVC_RUNTIME_LIBRARY
    "MultiThreaded$<$<CONFIG:Debug>:Debug>")
foreach(configuration DEBUG RELEASE RELWITHDEBINFO MINSIZEREL)
    set_target_properties(kfc-runtime-profile-rebase-report PROPERTIES
        RUNTIME_OUTPUT_DIRECTORY_${configuration} "${CMAKE_BINARY_DIR}/tools")
endforeach()
