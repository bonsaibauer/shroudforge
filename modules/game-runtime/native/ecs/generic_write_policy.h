#pragma once
#include <atomic>
namespace EcsImplementation {
class GenericWritePolicy final {
    // One atomic publication keeps the legacy/selected/request tuple coherent.
    std::atomic<unsigned> mode_{0}; // 0 legacy, 1 selected read-only, 2 selected hooks
public:
    void Legacy() noexcept { mode_.store(0); }
    void Selected(bool requested) noexcept { mode_.store(requested ? 2u : 1u); }
    bool Allows(bool actual_hooks_ready, bool original_admission = true) const noexcept {
        const auto mode = mode_.load();
        return original_admission && (mode == 0 || (mode == 2 && actual_hooks_ready));
    }
};
}
