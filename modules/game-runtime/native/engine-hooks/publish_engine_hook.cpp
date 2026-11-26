// Capture bridges adapted from ESM Minimap, MIT, Copyright (c) 2024 Miguel
// Oppermann. The original notice and permission are in map/SOURCE-NOTICE.txt.
#include "engine-hooks/publish_engine_hook.h"
#include "engine-hooks/allocate_near_code.h"
#include "engine-hooks/encode_relative_jump.h"
#include "engine-hooks/write_executable_bytes.h"
#include <windows.h>
#include <array>
#include <cstring>
#include <memory>

namespace EngineHooks {
namespace {
bool Fail(HookPublicationResult& result,const char* reason,unsigned error=0) {
    result.reason=reason;result.windows_error=error;return false;
}
bool Read(std::uintptr_t address,void* output,std::size_t size) {
    SIZE_T copied{};
    return address && output && size &&
        ReadProcessMemory(GetCurrentProcess(),reinterpret_cast<const void*>(address),output,size,&copied) && copied==size;
}
struct FreeUnpublished {
    void operator()(void* allocation) const noexcept {if(allocation)VirtualFree(allocation,0,MEM_RELEASE);}
};
}
bool PublishEngineHook(std::vector<PublishedEngineHook>& owned,
    std::uintptr_t target,std::span<const std::uint8_t> original,
    std::vector<std::uint8_t> relay,bool call,HookPublicationResult& result) {
    result={};
    if(!target || !original.data() || original.size()<5 || original.size()>32 ||
       target>UINTPTR_MAX-original.size() || relay.empty() ||
       (!call && (relay.size()<5 || relay[relay.size()-5]!=0xe9)))return Fail(result,"hook-extent");
    std::array<std::uint8_t,32> observed{};
    if(!Read(target,observed.data(),original.size()) ||
       std::memcmp(observed.data(),original.data(),original.size()))return Fail(result,"original-bytes-mismatch");
    std::unique_ptr<void,FreeUnpublished> allocation(AllocateNearCode(target,relay.size()));
    if(!allocation)return Fail(result,"near-allocation",GetLastError());
    const auto relay_address=reinterpret_cast<std::uintptr_t>(allocation.get());
    if(!call) {
        std::int32_t back{};
        if(!EncodeRelativeJump(relay_address+relay.size()-5,target+original.size(),back))return Fail(result,"return-jump-range");
        std::memcpy(relay.data()+relay.size()-4,&back,4);
    }
    std::memcpy(allocation.get(),relay.data(),relay.size());
    DWORD previous{};
    if(!VirtualProtect(allocation.get(),relay.size(),PAGE_EXECUTE_READ,&previous))return Fail(result,"relay-protection",GetLastError());
    if(!FlushInstructionCache(GetCurrentProcess(),allocation.get(),relay.size()))return Fail(result,"relay-cache-flush",GetLastError());
    PublishedEngineHook hook;
    hook.target=target;hook.allocation=relay_address;hook.relay=std::move(relay);
    hook.original.assign(original.begin(),original.end());
    hook.replacement.assign(original.size(),0x90);hook.replacement[0]=call?0xe8:0xe9;
    std::int32_t displacement{};
    if(!EncodeRelativeJump(target,relay_address,displacement))return Fail(result,"entry-jump-range");
    std::memcpy(hook.replacement.data()+1,&displacement,4);
    owned.push_back(std::move(hook));auto& stored=owned.back();
    // The existing writer owns bounded retries, exact preimage checks and
    // fail-fast handling of an indeterminate executable-memory transaction.
    if(!ReplaceExecutableBytes(target,stored.original.data(),stored.replacement.data(),stored.original.size())) {
        result.writer_failure=LastExecutableWriteFailure();owned.pop_back();
        return Fail(result,"executable-write-rejected");
    }
    stored.installed=true;allocation.release();return true;
}
bool PublishEntryHook(std::vector<PublishedEngineHook>& owned,
    std::uintptr_t target,std::span<const std::uint8_t> original,
    void* callback,HookPublicationResult& result) {
    result={};if(!callback)return Fail(result,"callback-unavailable");
    if(!original.data() || original.size()<5 || original.size()>32)return Fail(result,"hook-extent");
    // Preserve the existing four-argument entry bridge byte-for-byte.
    std::vector<std::uint8_t> code{0x48,0x83,0xec,0x48,0x48,0x89,0x4c,0x24,0x20,
        0x48,0x89,0x54,0x24,0x28,0x4c,0x89,0x44,0x24,0x30,0x4c,0x89,0x4c,0x24,0x38,0x48,0xb8};
    const auto handler=reinterpret_cast<std::uintptr_t>(callback);
    for(unsigned i=0;i<8;++i)code.push_back(std::uint8_t(handler>>(i*8)));
    code.insert(code.end(),{0xff,0xd0,0x48,0x8b,0x4c,0x24,0x20,0x48,0x8b,0x54,0x24,0x28,
        0x4c,0x8b,0x44,0x24,0x30,0x4c,0x8b,0x4c,0x24,0x38,0x48,0x83,0xc4,0x48});
    code.insert(code.end(),original.begin(),original.end());code.insert(code.end(),{0xe9,0,0,0,0});
    return PublishEngineHook(owned,target,original,std::move(code),false,result);
}
bool PublishCallHook(std::vector<PublishedEngineHook>& owned,
    std::uintptr_t target,std::uintptr_t original_target,
    void* callback,HookPublicationResult& result) {
    result={};if(!callback)return Fail(result,"callback-unavailable");
    std::array<std::uint8_t,5> original{};std::int32_t offset{};
    if(!Read(target,original.data(),original.size()) || original[0]!=0xe8)return Fail(result,"call-preimage");
    std::memcpy(&offset,original.data()+1,4);
    if(std::int64_t(target)+5+offset!=std::int64_t(original_target))return Fail(result,"call-target");
    std::vector<std::uint8_t> relay{0xff,0x25,0,0,0,0};
    const auto handler=reinterpret_cast<std::uintptr_t>(callback);
    for(unsigned i=0;i<8;++i)relay.push_back(std::uint8_t(handler>>(i*8)));
    return PublishEngineHook(owned,target,original,std::move(relay),true,result);
}
bool RestoreEngineHooks(std::span<PublishedEngineHook> owned) {
    bool restored=true;
    for(auto it=owned.rbegin();it!=owned.rend();++it)if(it->installed) {
        if(ReplaceExecutableBytes(it->target,it->replacement.data(),it->original.data(),it->original.size()))it->installed=false;
        else restored=false;
    }
    return restored;
}
}
