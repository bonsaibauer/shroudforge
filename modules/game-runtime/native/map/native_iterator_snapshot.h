#pragma once
#include "native_iterator_extent.h"
#include <array>
#include <cstring>

namespace NativeIteratorSnapshot {
using Reader=bool(*)(std::uintptr_t,void*,std::size_t);
inline constexpr std::size_t QualifiedMaximumCapacity=152;

// One callback-local snapshot. Native metadata must remain stable/live through
// the ensuing private Init/Next sequence; this does not provide TOCTOU immunity.
inline NativeIteratorExtent::IteratorExtentPlan ReadPlan(
    const std::uint64_t (&context)[2],void* destination,std::size_t capacity,Reader read) noexcept {
    using namespace NativeIteratorExtent;
    if(!read || capacity<8 || capacity>QualifiedMaximumCapacity)return {};
    MetadataSnapshot snapshot{};
    snapshot.private_context_storage=reinterpret_cast<std::uintptr_t>(context);
    snapshot.query=static_cast<std::uintptr_t>(context[0]);
    snapshot.cursor=static_cast<std::uint32_t>(context[1]); // native DWORD only
    Interval queryRange{},destinationRange{},contextRange{};
    if(!MakeInterval(snapshot.query,QueryControlBytes,queryRange) ||
       !MakeInterval(reinterpret_cast<std::uintptr_t>(destination),capacity,destinationRange) ||
       !MakeInterval(snapshot.private_context_storage,sizeof(context),contextRange))return {};
    std::array<std::uint64_t,2> indirect{};
    std::array<std::uint8_t,24> sourceCounts{};
    std::array<std::uint8_t,12> listCounts{};
    const auto q=snapshot.query;
    if(!read(q+0xc0,indirect.data(),sizeof(indirect)) ||
       !read(q+0x1a0,sourceCounts.data(),sourceCounts.size()) ||
       !read(q+0x4b0,listCounts.data(),listCounts.size()) ||
       !read(q+0x7a8,&snapshot.counts.n5,1) ||
       !read(q+0x7f8,&snapshot.counts.n4,1) ||
       !read(q+0x838,&snapshot.row_count,8))return {};
    snapshot.p3=indirect[0];snapshot.p8=indirect[1];
    Interval p3Count{},p8Count{};
    if(!MakeOffsetInterval(snapshot.p3,0x400,1,p3Count) ||
       !MakeOffsetInterval(snapshot.p8,0x80,1,p8Count) ||
       !read(p3Count.begin,&snapshot.counts.n3,1) ||
       !read(p8Count.begin,&snapshot.counts.n8,1))return {};
    snapshot.counts.n0=listCounts[0];
    std::memcpy(&snapshot.counts.n1,listCounts.data()+8,2);
    std::memcpy(&snapshot.counts.n2,listCounts.data()+10,2);
    std::memcpy(&snapshot.s6,sourceCounts.data(),8);
    std::memcpy(&snapshot.counts.n6,sourceCounts.data()+8,8);
    std::memcpy(&snapshot.counts.n7,sourceCounts.data()+16,4);
    return TryBuildPlan(snapshot,reinterpret_cast<std::uintptr_t>(destination),capacity);
}
}
