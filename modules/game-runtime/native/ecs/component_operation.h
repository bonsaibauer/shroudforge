#pragma once
#include "abi/runtime_api.h"
#include "engine-context/engine_owners.h"
#include <windows.h>
#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>
#include "ecs/entity_storage.h"
#include "engine-context/lock_scene_access.h"
#include "game-thread/run_game_thread_operations.h"
#include <atomic>

namespace EcsImplementation {
struct ReadOperation {
    std::uint32_t handle{};
    const char* name{};
    void* value{};
    std::size_t size{};
    bool result{};
    std::string owned_name;
    std::vector<std::uint8_t> output;
};

void read_on_game_thread(void* opaque);

struct ReadManyOperation {
    std::uint32_t handle{};
    std::vector<std::string> names;
    std::vector<std::uint32_t> sizes;
    std::vector<ComponentType> components;
    std::vector<std::uintptr_t> addresses;
    std::vector<std::uint8_t> output, present, verification;
    bool result{};
};

void read_many_on_game_thread(void* opaque);

struct WriteOperation {
    std::uint32_t handle{};
    const char* name{};
    const void* mask{};
    const void* value{};
    std::size_t size{};
    bool result{}, conflict{}, uncertain{}, direct{}, mutation_attempted{};
    std::string owned_name;
    std::vector<std::uint8_t> mask_bytes, destination, expected, before, merged, verified;
    ComponentType component{};
    ResolvedLayout layout{};
    HandleRecord record{};
    EngineContext::EcsView owner;
    std::uint64_t epoch{};
    bool blocked_by_write_fault{};
};

extern std::atomic<const char*> last_write_reason;

bool prepare_write(WriteOperation& operation);

void write_payload(WriteOperation& operation, const EngineContext::EcsWriteGuard* guard = nullptr);

void write_on_game_thread(void* opaque);

bool invoke_write_operation(const std::shared_ptr<WriteOperation>& operation);

}
