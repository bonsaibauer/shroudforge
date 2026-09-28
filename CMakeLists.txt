cmake_minimum_required(VERSION 3.24)
project(shroudforge LANGUAGES CXX)

# The native KFC runtime is a component of the ShroudForge loader build.
add_subdirectory(src/loader/runtime/native)
