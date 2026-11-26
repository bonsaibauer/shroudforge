#pragma once

#include <array>
#include <cstdint>
#include <memory>
#include <mutex>
#include <span>
#include <vector>

namespace EngineHooks {
// Native-only lifetime authority. No package callback or verifier runs while
// Gate() is held. Commit/revoke order is Gate -> validation registry, never
// the reverse. Publication/close may enter from framework control locks.
class OwnedSpanProofs final {
public:
    using Verifier = bool (*)(const void*) noexcept;
    struct Record {
        std::uint64_t session{}, serial{};
        std::uint32_t rva{}, count{};
        std::array<std::uint8_t, 32> original{}, replacement{};
        std::shared_ptr<const void> witness;
        Verifier verify{};
        bool live{true}; // Lifetime metadata, accessed only under Gate().
    };
    struct Limits { std::size_t sessions{4096}, records{4096}; };
    OwnedSpanProofs();
    explicit OwnedSpanProofs(Limits limits);
    std::mutex& Gate() noexcept { return gate_; }
    // These methods require Gate(). The pre-initialization session survives
    // ordinary provider initialization. Only permitted Stage acquisition can
    // open a successor after terminal close.
    std::uint64_t BeginStageLocked();
    std::uint64_t SessionLocked() const noexcept;
    bool SessionLiveLocked(std::uint64_t session) const noexcept;
    bool LiveLocked(const std::shared_ptr<const Record>& record) const noexcept;
    // Exact range lookup shared by the C observation API and proof leases.
    // A partial overlap is a conflict, even if the supplied bytes are unchanged.
    std::shared_ptr<const Record> FindRangeLocked(std::uint64_t session,
        std::uint32_t rva, std::size_t count, bool& conflict) const noexcept;
    std::shared_ptr<const Record> FindLocked(std::uint64_t session,
        std::uint32_t rva, std::span<const std::uint8_t> original) const noexcept;

    bool Publish(std::uint32_t rva, std::span<const std::uint8_t> original,
        std::span<const std::uint8_t> replacement,
        std::shared_ptr<const void> witness, Verifier verify) noexcept;
    void Close() noexcept;
    void Retire(std::uint64_t serial) noexcept;
private:
    std::mutex gate_;
    Limits limits_;
    std::uint64_t session_{1}, next_session_{1}, next_serial_{};
    std::size_t ever_sessions_{1}, ever_records_{};
    bool closed_{};
    std::vector<std::shared_ptr<Record>> records_;
};
OwnedSpanProofs& ExecutableProofs() noexcept;
}
