#pragma once
#include <cstdint>

namespace EngineHooks {
// Private trusted-host assertion, not a sandbox against native code in this
// address space. The bootstrap verifier owns the real entrypoint gate.
struct HeldEntryProof {
    std::uint32_t size{sizeof(HeldEntryProof)};
    std::uint32_t version{1};
    std::uint32_t parked_thread_id{};
    bool (__cdecl* is_held)(std::uint32_t) noexcept{};
};
class HeldEntryToken;
using HeldEntryCallback = bool (*)(const HeldEntryToken&, void*) noexcept;
bool RunHeldEntryPreparation(const HeldEntryProof&, HeldEntryCallback, void*) noexcept;
bool HeldEntryPreparationActiveOnCurrentThread() noexcept;

class HeldEntryToken final {
public:
    HeldEntryToken(const HeldEntryToken&) = delete;
    HeldEntryToken& operator=(const HeldEntryToken&) = delete;
    bool ValidForCurrentThread() const noexcept;
private:
    friend bool RunHeldEntryPreparation(const HeldEntryProof&, HeldEntryCallback, void*) noexcept;
    HeldEntryToken(const HeldEntryProof& proof, std::uint32_t worker) noexcept
        : proof_(proof), worker_(worker) {}
    HeldEntryProof proof_;
    std::uint32_t worker_{};
};
}
