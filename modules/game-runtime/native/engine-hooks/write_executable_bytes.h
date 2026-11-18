#pragma once
#include <cstddef>
#include <cstdint>
namespace EngineHooks {
// False guarantees no publication, or verified original-byte/protection rollback.
// An indeterminate mutation/cleanup fails fast rather than returning to a caller
// that could free a potentially reachable relay. Writes are process-serialized.
bool WriteExecutableBytes(std::uintptr_t address, const void* bytes, std::size_t size,
                          const void* expected = nullptr);
bool ReplaceExecutableBytes(std::uintptr_t address, const void* expected,
                            const void* replacement, std::size_t size);
unsigned LastExecutableWriteFailure() noexcept;
}
