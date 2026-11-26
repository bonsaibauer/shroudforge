#pragma once
#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

namespace EngineHooks {
// Runtime domains retain their existing, separately controlled lifetimes.
// This is a publication receipt, not another registry or a public mod API.
// The domain's control lock covers both mutation and witness readback.
struct PublishedEngineHook {
    std::uintptr_t target{}, allocation{};
    std::vector<std::uint8_t> original, replacement, relay;
    bool installed{};
    // Only the qualified held-entry caller establishes this evidence.
    bool publicationQualified{};
    void* residentOwner{};
};
struct HookPublicationResult {
    const char* reason{"none"};
    unsigned writer_failure{}, windows_error{};
};

// A jump relay ends in E9/rel32, filled to resume after the original span.
// A call relay returns through the original CALL site's return address.
// All receipt storage is owned before publication. Unpublished allocations
// are freed on failure; published allocations remain resident through exit.
bool PublishEngineHook(std::vector<PublishedEngineHook>& owned,
    std::uintptr_t target, std::span<const std::uint8_t> original,
    std::vector<std::uint8_t> relay, bool call, HookPublicationResult& result);
bool PublishEntryHook(std::vector<PublishedEngineHook>& owned,
    std::uintptr_t target, std::span<const std::uint8_t> original,
    void* callback, HookPublicationResult& result);
bool PublishCallHook(std::vector<PublishedEngineHook>& owned,
    std::uintptr_t target, std::uintptr_t original_target,
    void* callback, HookPublicationResult& result);
// Restore in reverse order only while our exact replacement is still present.
// A refused record remains installed/owned for the caller's cleanup policy.
bool RestoreEngineHooks(std::span<PublishedEngineHook> owned);
}
