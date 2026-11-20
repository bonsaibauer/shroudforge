#pragma once
#include <array>
#include <cstddef>
#include <cstdint>
#include <cstring>

namespace EngineHooks {
// Deep-copied only after trusted publication qualification. This helper never
// accesses mutable hook containers or acquires a framework control lock.
struct SharedCallWitness {
    std::uintptr_t image{}, target{}, original_target{}, allocation{}, handler{}, resident{};
    std::array<std::uint8_t,5> original{},replacement{};
    std::array<std::uint8_t,14> relay{};
};
using ProofReader=bool (*)(std::uintptr_t,void*,std::size_t) noexcept;
using ProofRegion=bool (*)(std::uintptr_t,std::size_t,std::uintptr_t,bool) noexcept;
using ProofResident=bool (*)(std::uintptr_t,std::uintptr_t) noexcept;
inline bool VerifySharedCallWitness(const SharedCallWitness& value,ProofReader read,
        ProofRegion region,ProofResident resident) noexcept {
    if(!read || !region || !resident || !value.image || !value.target || !value.original_target ||
        !value.allocation || !value.handler || !value.resident || value.target>UINTPTR_MAX-5 ||
        value.original[0]!=0xe8 || value.replacement[0]!=0xe8)return false;
    constexpr std::uint8_t jump[]{0xff,0x25,0,0,0,0};
    if(std::memcmp(value.relay.data(),jump,sizeof(jump)))return false;
    std::int32_t original{},replacement{};std::uintptr_t handler{};
    std::memcpy(&original,value.original.data()+1,4);
    std::memcpy(&replacement,value.replacement.data()+1,4);
    std::memcpy(&handler,value.relay.data()+6,8);
    if(std::int64_t(value.target)+5+original!=std::int64_t(value.original_target) ||
        std::int64_t(value.target)+5+replacement!=std::int64_t(value.allocation) || handler!=value.handler ||
        !resident(value.handler,value.resident) ||
        !region(value.target,5,value.image,true) ||
        !region(value.allocation,14,value.allocation,false))return false;
    std::array<std::uint8_t,5> actual{};std::array<std::uint8_t,14> relay{};
    return read(value.target,actual.data(),actual.size()) && actual==value.replacement &&
        read(value.allocation,relay.data(),relay.size()) && relay==value.relay;
}
}
