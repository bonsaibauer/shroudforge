#pragma once
#include <array>
#include <cstddef>
#include <cstdint>
#include <limits>

namespace EngineHooks::ExecutableWrite {
constexpr std::size_t MaxPeers = 1024;
constexpr unsigned MaxInventories = 8;
enum class Failure : unsigned {
    None, InvalidRange, Enumeration, Capacity, OpenThread, SuspendThread,
    ThreadContext, OccupiedPc, UnstableInventory, ExpectedBytes, Protect,
    Flush, RestoreProtection, ResumeThread,
};
enum class Outcome { Rejected, Published, Indeterminate };
struct Result { Outcome outcome{Outcome::Rejected}; Failure failure{Failure::None}; };

// Backend methods in the frozen interval must not allocate, log or acquire
// application/loader locks. All handles/metadata use bounded storage.
template<class Os>
Result Run(Os& os, std::uintptr_t address, const void* bytes, std::size_t size, const void* expected) {
    if (!address || !bytes || !size || size > 32 ||
        address > (std::numeric_limits<std::uintptr_t>::max)() - size || !os.ValidRange(address, size))
        return {Outcome::Rejected, Failure::InvalidRange};
    struct Peer { std::uint32_t id{}; std::uintptr_t handle{}; std::uint32_t previous{}; bool suspended{}; };
    std::array<Peer, MaxPeers> peers{};
    std::array<std::uint32_t, MaxPeers> ids{};
    std::size_t count{}, found{};
    auto finish = [&](Result result) {
        bool resumed = true;
        for (std::size_t i = count; i; --i) {
            if (peers[i-1].suspended && !os.Resume(peers[i-1].handle, peers[i-1].previous)) resumed = false;
        }
        for (std::size_t i = 0; i < count; ++i) os.Close(peers[i].handle);
        return resumed ? result : Result{Outcome::Indeterminate, Failure::ResumeThread};
    };
    if (!os.Inventory(false, ids, found)) return finish({Outcome::Rejected, Failure::Enumeration});
    for (unsigned pass = 0; pass < MaxInventories; ++pass) {
        for (std::size_t i = 0; i < found; ++i) {
            if (ids[i] == os.CurrentThread()) continue;
            std::size_t index{};
            for (; index < count && peers[index].id != ids[i]; ++index) {}
            if (index != count) continue;
            if (count == peers.size()) return finish({Outcome::Rejected, Failure::Capacity});
            const auto handle = os.Open(ids[i]);
            if (!handle) return finish({Outcome::Rejected, Failure::OpenThread});
            peers[count++] = Peer{ids[i], handle};
        }
        for (std::size_t i = 0; i < count; ++i) {
            auto& peer = peers[i];
            if (peer.suspended) continue;
            if (!os.Suspend(peer.handle, peer.previous)) return finish({Outcome::Rejected, Failure::SuspendThread});
            peer.suspended = true;
            std::uintptr_t pc{};
            if (!os.Pc(peer.handle, pc)) return finish({Outcome::Rejected, Failure::ThreadContext});
            if (pc >= address && pc < address + size) return finish({Outcome::Rejected, Failure::OccupiedPc});
        }
        if (!os.Inventory(true, ids, found)) return finish({Outcome::Rejected, Failure::Enumeration});
        bool added{};
        for (std::size_t i = 0; i < found; ++i) {
            if (ids[i] == os.CurrentThread()) continue;
            std::size_t index{};
            for (; index < count && peers[index].id != ids[i]; ++index) {}
            added |= index == count;
        }
        // A suspended peer cannot disappear in the supported in-process model.
        for (std::size_t i = 0; i < count; ++i) {
            bool present{};
            for (std::size_t j = 0; j < found; ++j) present |= peers[i].id == ids[j];
            if (!present) return finish({Outcome::Rejected, Failure::UnstableInventory});
        }
        if (added) continue;

        std::array<std::uint8_t, 32> original{};
        auto* target = reinterpret_cast<volatile std::uint8_t*>(address);
        const auto* replacement = static_cast<const std::uint8_t*>(bytes);
        const auto* comparison = static_cast<const std::uint8_t*>(expected);
        for (std::size_t i = 0; i < size; ++i) {
            original[i] = target[i];
            if (comparison && original[i] != comparison[i]) return finish({Outcome::Rejected, Failure::ExpectedBytes});
        }
        std::uint32_t protection{};
        if (!os.Writable(address, size, protection)) return finish({Outcome::Rejected, Failure::Protect});
        for (std::size_t i = 0; i < size; ++i) target[i] = replacement[i];
        const bool flushed = os.Flush(address, size);
        if (flushed && os.Restore(address, size, protection))
            return finish({Outcome::Published, Failure::None});

        // Re-establish writable protection before attempting rollback. A failed
        // restore must not be interpreted as proof that the page stayed writable.
        std::uint32_t ignored{};
        if (!os.Writable(address, size, ignored)) return {Outcome::Indeterminate, Failure::RestoreProtection};
        for (std::size_t i = 0; i < size; ++i) target[i] = original[i];
        const bool rollback_flushed = os.Flush(address, size);
        const bool restored = os.Restore(address, size, protection);
        if (!rollback_flushed || !restored) return {Outcome::Indeterminate,
            rollback_flushed ? Failure::RestoreProtection : Failure::Flush};
        return finish({Outcome::Rejected, flushed ? Failure::RestoreProtection : Failure::Flush});
    }
    return finish({Outcome::Rejected, Failure::UnstableInventory});
}
}
