#pragma once
#include "native_iterator_extent.h"
#include <array>
#include <cstring>

namespace NativeIteratorSnapshot {
using Reader=bool(*)(std::uintptr_t,void*,std::size_t);
inline constexpr std::size_t QualifiedMaximumCapacity=152;
struct Layout {
    std::uint32_t query_bytes{}, indirect{}, source_counts{}, list_counts{}, n5{}, n4{}, rows{};
    std::uint32_t p3_count{}, p8_count{}, query_copy_a{}, query_copy_b{};
};

// One callback-local snapshot. Native metadata must remain stable/live through
// the ensuing private Init/Next sequence; this does not provide TOCTOU immunity.
inline NativeIteratorExtent::IteratorExtentPlan ReadPlan(
    const std::uint64_t (&context)[2],void* destination,std::size_t capacity,Reader read,const Layout& layout) noexcept {
    using namespace NativeIteratorExtent;
    if(!read || capacity<8 || capacity>QualifiedMaximumCapacity || layout.query_bytes<32 || layout.query_bytes>65536 ||
       layout.indirect>layout.query_bytes-16 || layout.source_counts>layout.query_bytes-24 ||
       layout.list_counts>layout.query_bytes-12 || layout.n5>=layout.query_bytes || layout.n4>=layout.query_bytes ||
       layout.rows>layout.query_bytes-8 || layout.query_copy_a>=layout.query_bytes || layout.query_copy_b>=layout.query_bytes ||
       layout.p3_count>65535 || layout.p8_count>65535)return {};
    MetadataSnapshot snapshot{};
    snapshot.private_context_storage=reinterpret_cast<std::uintptr_t>(context);
    snapshot.query=static_cast<std::uintptr_t>(context[0]);
    snapshot.cursor=static_cast<std::uint32_t>(context[1]); // native DWORD only
    snapshot.query_control_bytes=layout.query_bytes;
    snapshot.p3_control_bytes=std::uint64_t(layout.p3_count)+1;
    snapshot.p8_control_bytes=std::uint64_t(layout.p8_count)+1;
    snapshot.query_copy_a=layout.query_copy_a;snapshot.query_copy_b=layout.query_copy_b;
    Interval queryRange{},destinationRange{},contextRange{};
    if(!MakeInterval(snapshot.query,layout.query_bytes,queryRange) ||
       !MakeInterval(reinterpret_cast<std::uintptr_t>(destination),capacity,destinationRange) ||
       !MakeInterval(snapshot.private_context_storage,sizeof(context),contextRange))return {};
    std::array<std::uint64_t,2> indirect{};
    std::array<std::uint8_t,24> sourceCounts{};
    std::array<std::uint8_t,12> listCounts{};
    const auto q=snapshot.query;
    if(!read(q+layout.indirect,indirect.data(),sizeof(indirect)) ||
       !read(q+layout.source_counts,sourceCounts.data(),sourceCounts.size()) ||
       !read(q+layout.list_counts,listCounts.data(),listCounts.size()) ||
       !read(q+layout.n5,&snapshot.counts.n5,1) ||
       !read(q+layout.n4,&snapshot.counts.n4,1) ||
       !read(q+layout.rows,&snapshot.row_count,8))return {};
    snapshot.p3=indirect[0];snapshot.p8=indirect[1];
    Interval p3Count{},p8Count{};
    if(!MakeOffsetInterval(snapshot.p3,layout.p3_count,1,p3Count) ||
       !MakeOffsetInterval(snapshot.p8,layout.p8_count,1,p8Count) ||
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
