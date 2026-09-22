#pragma once

#include <cstddef>
#include <cstdint>
#include <string>

namespace EcsRuntime {
bool Initialize();
void Tick();
std::string Status();
void Shutdown();
}

extern "C" {
__declspec(dllexport) bool __cdecl ShroudforgeEcsConfigure(
    const char* const* qualified_names, const std::uint32_t* sizes, std::size_t count);
__declspec(dllexport) bool __cdecl ShroudforgeEcsReady();
__declspec(dllexport) bool __cdecl ShroudforgeEcsCanWrite();
__declspec(dllexport) bool __cdecl ShroudforgeEcsDescribe(
    const char* qualified_name, std::uint32_t* size);
__declspec(dllexport) std::size_t __cdecl ShroudforgeEcsQuery(
    const char* const* qualified_names, std::size_t component_count,
    std::uint32_t* entities, std::size_t capacity);
__declspec(dllexport) std::uint32_t __cdecl ShroudforgeEcsResolve(std::uint32_t entity_id);
__declspec(dllexport) bool __cdecl ShroudforgeEcsRead(
    std::uint32_t entity, const char* qualified_name, void* value, std::size_t size);
__declspec(dllexport) bool __cdecl ShroudforgeEcsWrite(
    std::uint32_t entity, const char* qualified_name, const void* expected,
    const void* value, std::size_t size);
}
