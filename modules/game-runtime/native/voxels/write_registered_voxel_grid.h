#pragma once
#include <cstddef>
#include <cstdint>

namespace WorldRuntime::RegisteredVoxelGrid {
bool WriteAvailable();
// Resolves the writer on a worker once the local world accepts writes, so the
// first write does not pay for it. Called at each tick; cheap once done.
void PrepareWriter();
// Waits for a running preparation (runtime shutdown).
void StopPreparingWriter();
// outcome: 0 verified, 1 rejected before writing, 2 original restored and
// verified, 3 uncertain and the exact ECS session is fenced against writes.
bool WriteForSession(std::uint64_t expected_session, const std::int32_t origin[3],
    const std::uint32_t dimensions[3], const std::uint16_t* values,
    std::size_t count, std::uint32_t* outcome);
}
