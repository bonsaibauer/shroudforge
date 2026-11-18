#include "engine-hooks/executable_write_transaction.h"
#include <cstdio>
#include <cstdlib>
#include <initializer_list>

namespace {
using namespace EngineHooks::ExecutableWrite;
struct Fixture {
    Failure fail{};
    unsigned inventories{}, opens{}, suspends{}, resumes{}, closes{}, flushes{};
    std::uint8_t bytes[2]{1, 2};
    std::uint32_t CurrentThread() const { return 1; }
    bool ValidRange(std::uintptr_t, std::size_t) const { return true; }
    bool Inventory(bool, std::array<std::uint32_t, MaxPeers>& ids, std::size_t& count) {
        if (fail == Failure::Enumeration) return false;
        ids[0] = 1; ids[1] = 2; count = 2;
        if (++inventories > 1 && fail == Failure::UnstableInventory) count = 1;
        return true;
    }
    std::uintptr_t Open(std::uint32_t id) { if (fail == Failure::OpenThread) return 0; ++opens; return id; }
    bool Suspend(std::uintptr_t, std::uint32_t& previous) {
        if (fail == Failure::SuspendThread) return false;
        ++suspends; previous = 3; return true;
    }
    bool Pc(std::uintptr_t, std::uintptr_t& pc) {
        pc = fail == Failure::OccupiedPc ? reinterpret_cast<std::uintptr_t>(bytes) : 0;
        return fail != Failure::ThreadContext;
    }
    bool Resume(std::uintptr_t, std::uint32_t previous) { ++resumes; return previous == 3 && fail != Failure::ResumeThread; }
    void Close(std::uintptr_t) { ++closes; }
    bool Writable(std::uintptr_t, std::size_t, std::uint32_t& previous) { previous = 0x20; return fail != Failure::Protect; }
    bool Restore(std::uintptr_t, std::size_t, std::uint32_t) { return true; }
    bool Flush(std::uintptr_t, std::size_t) { return ++flushes != 1 || fail != Failure::Flush; }
};
void check(bool value) { if (!value) { std::fputs("Executable write preservation failed\n", stderr); std::exit(1); } }
}

int main() {
    const std::uint8_t original[]{1, 2}, replacement[]{3, 4}, mismatch[]{0, 0};
    for (const auto failure : {Failure::None, Failure::Enumeration, Failure::OpenThread,
             Failure::SuspendThread, Failure::ThreadContext, Failure::OccupiedPc,
             Failure::UnstableInventory, Failure::ExpectedBytes, Failure::Protect,
             Failure::Flush, Failure::ResumeThread}) {
        Fixture os; os.fail = failure;
        const auto result = Run(os, reinterpret_cast<std::uintptr_t>(os.bytes), replacement,
            sizeof(replacement), failure == Failure::ExpectedBytes ? mismatch : original);
        check(result.failure == failure);
        check(os.opens == os.closes && os.suspends == os.resumes);
        if (failure == Failure::None || failure == Failure::ResumeThread) {
            check(os.bytes[0] == 3 && os.bytes[1] == 4);
            check(result.outcome == (failure == Failure::None ? Outcome::Published : Outcome::Indeterminate));
        } else {
            check(result.outcome == Outcome::Rejected);
            check(os.bytes[0] == 1 && os.bytes[1] == 2);
        }
    }
    std::puts("PASS: executable write success, rollback and peer cleanup");
}
