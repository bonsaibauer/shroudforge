#pragma once
#include "ecs_runtime.h"
#include <cstddef>
#include <cstdint>
#include <string>

namespace WorldRuntime {
void ResetContext();
void OnPropUpdate(void* execution_view, void* actor_frame);
void OnActorPlacement(void* execution_view, void* actor_frame);
void OnBuildingDispatch(void* placement_context);
void OnCursorUpdate(void* cursor, void* execution_view);
void SetCursorHookReady(bool ready);
bool ReadCursorSnapshot(std::uint8_t* bytes, std::size_t capacity, std::uint64_t* sequence);
bool OperationAvailable(const char* name);
bool ActiveContextAvailable();
bool EntityContextReady();
std::string EntityHookStatus();
bool ReadVoxels(const std::int32_t origin[3], const std::uint32_t dimensions[3],
                std::uint16_t* values, std::size_t capacity, std::size_t* actual);
bool GetGridSpec(const char* grid_id, KfcRuntimeGridSpec* spec);
bool WriteVoxels(const std::int32_t origin[3], const std::uint32_t dimensions[3],
                 const std::uint16_t* values, std::size_t count, std::uint32_t* outcome);
bool SpawnEntity(const std::uint64_t template_uuid[2], const double position[3],
                 const double rotation[4], std::uint32_t tracking, std::uint32_t flags,
                 std::uint32_t* entity_handle, std::uint32_t* outcome);
bool PlaceEntity(const double position[3], const double rotation[4], const float bounds[6],
                 std::uint32_t tracking, std::uint32_t feedback, std::uint32_t* outcome);
bool DestroyEntity(const double position[3], const double rotation[4], const float bounds[6],
                   std::uint32_t tracking, std::uint32_t feedback, std::uint32_t* outcome);
bool DestroyEntityHandle(std::uint32_t entity_handle, std::uint32_t* outcome);
bool FinishBuilding(bool complete, std::uint32_t* outcome);
}
