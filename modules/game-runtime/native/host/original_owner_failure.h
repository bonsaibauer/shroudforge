#pragma once
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <type_traits>

// Private fixed bootstrap/provider diagnostic, independent of stable ABI12.
// No pointers, paths, allocation, locks, probing, or callback/function table.
namespace OriginalOwnerFailure {
enum Operation : std::uint32_t { None=0,Require=1,Finalize=2 };
enum Status : std::uint32_t { Empty=0,Collecting=1,Succeeded=2,Failed=3 };
enum Layer : unsigned { Host=0,Map=1,Hud=2 };
struct SnapshotV1 {
    std::uint32_t bytes=sizeof(SnapshotV1),version=1,operation{},status{};
    std::uint32_t checked[3]{},passed[3]{};
    std::uint32_t family{},profile{},rva{},node{},readIndex{};
    std::uint32_t beforeModules{},afterModules{},retainedModules{},reads{};
    std::uint64_t beforeEpoch{},beforeInvalidation{},afterEpoch{},afterInvalidation{};
    char stage[32]{},reason[80]{};
};
static_assert(sizeof(SnapshotV1)==224&&std::is_trivially_copyable_v<SnapshotV1>);
struct Context {std::uint32_t family{},profile{},rva{},node{},readIndex{};};
inline thread_local SnapshotV1 nativeSnapshot{};
inline thread_local Context context{};
struct ContextScope {Context prior=context;~ContextScope(){context=prior;}};
inline bool Active() noexcept {return nativeSnapshot.status==Collecting;}
template<std::size_t N> inline void Text(char(&out)[N],const char* value) noexcept {
    std::size_t n=0;if(value)while(n+1<N&&value[n]){out[n]=value[n];++n;}out[n]=0;
}
inline void Begin(Operation operation) noexcept {
    nativeSnapshot={};nativeSnapshot.operation=operation;nativeSnapshot.status=Collecting;context={};
}
inline void Record(const char* stage,const char* reason) noexcept {
    if(!Active()||nativeSnapshot.reason[0])return;
    Text(nativeSnapshot.stage,stage);Text(nativeSnapshot.reason,reason);
    nativeSnapshot.family=context.family;nativeSnapshot.profile=context.profile;nativeSnapshot.rva=context.rva;
    nativeSnapshot.node=context.node;nativeSnapshot.readIndex=context.readIndex;
}
inline bool Refuse(const char* stage,const char* reason) noexcept {Record(stage,reason);return false;}
inline bool RejectIf(bool failed,const char* stage,const char* reason) noexcept {if(failed)Record(stage,reason);return failed;}
inline bool Result(bool result,const char* stage,const char* reason) noexcept {
    if(!result)Record(stage,reason);return result;
}
inline bool Check(Layer layer,unsigned bit,bool result,const char* reason) noexcept {
    if(Active()){nativeSnapshot.checked[layer]|=bit;if(result)nativeSnapshot.passed[layer]|=bit;}
    if(!result)Record(layer==Host?"host":layer==Map?"map":"hud",reason);return result;
}
inline bool End(bool result) noexcept {
    if(Active()){
        if(result){nativeSnapshot.reason[0]=0;nativeSnapshot.stage[0]=0;nativeSnapshot.status=Succeeded;}
        else {Record("finalization","unclassified-refusal");nativeSnapshot.status=Failed;}
    }return result;
}
inline bool Copy(SnapshotV1* out,std::uint32_t bytes) noexcept {
    if(!out||bytes!=sizeof(SnapshotV1)||out->bytes!=sizeof(SnapshotV1)||out->version!=1)return false;
    std::memcpy(out,&nativeSnapshot,sizeof(nativeSnapshot));return true;
}
// Only already observed scalars are copied. Never re-sample epoch/cache state.
inline void Graph(unsigned before,unsigned after,unsigned retained,unsigned reads,
    std::uint64_t be,std::uint64_t bi,std::uint64_t ae,std::uint64_t ai) noexcept {
    if(!Active())return;
    nativeSnapshot.beforeModules=before;nativeSnapshot.afterModules=after;nativeSnapshot.retainedModules=retained;nativeSnapshot.reads=reads;
    nativeSnapshot.beforeEpoch=be;nativeSnapshot.beforeInvalidation=bi;nativeSnapshot.afterEpoch=ae;nativeSnapshot.afterInvalidation=ai;
}
inline void Module(unsigned family,unsigned profile,std::uintptr_t base,std::uintptr_t callback,unsigned node) noexcept {
    if(!Active())return;
    context.family=family;context.profile=profile;context.node=node;
    context.rva=callback>=base&&callback-base<=UINT32_MAX?static_cast<std::uint32_t>(callback-base):UINT32_MAX;
}
inline void Read(unsigned node,unsigned index) noexcept {if(Active()){context.node=node;context.readIndex=index;}}
using Getter=bool (__cdecl*)(SnapshotV1*,std::uint32_t);
class Writer {
    char* out_;std::size_t capacity_,used_{};
    void terminate() noexcept {if(out_&&capacity_)out_[used_]=0;}
public:
    Writer(char* out,std::size_t capacity) noexcept:out_(out),capacity_(capacity){terminate();}
    void Text(const char* p,std::size_t bound) noexcept {
        if(!out_||!capacity_||!p)return;
        for(std::size_t i=0;i<bound&&p[i]&&used_+1<capacity_;++i)out_[used_++]=p[i];terminate();
    }
    template<std::size_t N> void Text(const char(&p)[N]) noexcept {Text(p,N);}
    void Number(std::uint64_t value) noexcept {
        char reverse[20];unsigned n=0;do{reverse[n++]=char('0'+value%10);value/=10;}while(value);
        if(out_&&capacity_)while(n&&used_+1<capacity_)out_[used_++]=reverse[--n];terminate();
    }
    std::size_t Size() const noexcept {return used_;}
};
// Allocation/locale-free bounded formatter for an OWN copied POD, never a getter.
inline std::size_t Format(const SnapshotV1& s,char* out,std::size_t capacity) noexcept {
    if(!out||!capacity)return 0;std::size_t used=0;
    const auto text=[&](const char* p,std::size_t bound){for(std::size_t i=0;i<bound&&p[i]&&used+1<capacity;++i)out[used++]=p[i];};
    const auto number=[&](std::uint64_t value){char reverse[20];unsigned n=0;do{reverse[n++]=char('0'+value%10);value/=10;}while(value);
        while(n&&used+1<capacity)out[used++]=reverse[--n];};
    text("native-owner-stage=",20);text(s.stage,sizeof(s.stage));text(",reason=",8);text(s.reason,sizeof(s.reason));
    text(",operation=",11);number(s.operation);text(",family=",8);number(s.family);text(",profile=",9);number(s.profile);
    text(",rva=",5);number(s.rva);text(",node=",6);number(s.node);text(",read=",6);number(s.readIndex);
    text(",modules=",9);number(s.beforeModules);text("/",1);number(s.afterModules);text(",leases=",8);number(s.retainedModules);
    text(",reads=",7);number(s.reads);text(",checked=",9);for(unsigned i=0;i<3;++i){if(i)text("/",1);number(s.checked[i]);}
    text(",passed=",8);for(unsigned i=0;i<3;++i){if(i)text("/",1);number(s.passed[i]);}
    text(",epoch=",7);number(s.beforeEpoch);text("/",1);number(s.afterEpoch);
    text(",invalidation=",14);number(s.beforeInvalidation);text("/",1);number(s.afterInvalidation);
    out[used]=0;return used;
}
}
