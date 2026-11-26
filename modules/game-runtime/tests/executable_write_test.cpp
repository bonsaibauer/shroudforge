#include "engine-hooks/executable_write_transaction.h"
#include "engine-hooks/publish_engine_hook.h"
#include "engine-hooks/encode_relative_jump.h"
#include <windows.h>
#include <cstdio>
#include <cstdlib>
#include <cstring>
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
unsigned checks{},entry_calls{},call_calls{};
std::array<std::uintptr_t,4> entry_arguments{};
void check_at(int line,bool value) {
    ++checks;
    if (!value) { std::fprintf(stderr,"Executable publication assertion failed at line %d\n",line); std::exit(1); }
}
#define check(value) check_at(__LINE__,(value))
void __fastcall ObserveEntry(std::uintptr_t a,std::uintptr_t b,std::uintptr_t c,std::uintptr_t d) {
    ++entry_calls;entry_arguments={a,b,c,d};
}
int __fastcall ReplaceCall() {++call_calls;return 22;}
void CopyCode(std::uint8_t* target,std::span<const std::uint8_t> bytes) {
    DWORD previous{},ignored{};
    check(VirtualProtect(target,bytes.size(),PAGE_EXECUTE_READWRITE,&previous)!=FALSE);
    std::memcpy(target,bytes.data(),bytes.size());
    check(FlushInstructionCache(GetCurrentProcess(),target,bytes.size())!=FALSE);
    check(VirtualProtect(target,bytes.size(),previous,&ignored)!=FALSE);
}
void publications_and_restoration() {
    SYSTEM_INFO info{};GetSystemInfo(&info);
    auto* code=static_cast<std::uint8_t*>(VirtualAlloc(nullptr,info.dwPageSize*2,MEM_COMMIT|MEM_RESERVE,PAGE_EXECUTE_READWRITE));
    check(code!=nullptr);
    const std::array<std::uint8_t,6> function{0xb8,7,0,0,0,0xc3};
    const std::array<std::uint8_t,6> callee{0xb8,11,0,0,0,0xc3};
    std::memcpy(code,function.data(),function.size());
    std::memcpy(code+64,callee.data(),callee.size());
    // An ordinary aligned caller of the five-byte CALL at code+132.
    std::array<std::uint8_t,14> caller{0x48,0x83,0xec,0x28,0xe8,0,0,0,0,0x48,0x83,0xc4,0x28,0xc3};
    std::int32_t relative{};
    check(EngineHooks::EncodeRelativeJump(reinterpret_cast<std::uintptr_t>(code+132),reinterpret_cast<std::uintptr_t>(code+64),relative));
    std::memcpy(caller.data()+5,&relative,4);std::memcpy(code+128,caller.data(),caller.size());
    const auto crossing=code+info.dwPageSize-2;
    std::memcpy(crossing,function.data(),function.size());
    DWORD previous{};
    check(FlushInstructionCache(GetCurrentProcess(),code,info.dwPageSize*2)!=FALSE);
    check(VirtualProtect(code,info.dwPageSize*2,PAGE_EXECUTE_READ,&previous)!=FALSE);
    using Entry=int(__fastcall*)(std::uintptr_t,std::uintptr_t,std::uintptr_t,std::uintptr_t);
    using Call=int(__fastcall*)();
    const auto entry=reinterpret_cast<Entry>(code);
    const auto call=reinterpret_cast<Call>(code+128);
    check(entry(1,2,3,4)==7 && call()==11);
    std::vector<EngineHooks::PublishedEngineHook> world,capture;
    EngineHooks::HookPublicationResult result;
    const auto original=std::span(function).first<5>();
    auto bad_original=function;bad_original[0]^=1;
    check(!EngineHooks::PublishEntryHook(world,reinterpret_cast<std::uintptr_t>(code),std::span(bad_original).first<5>(),
        reinterpret_cast<void*>(&ObserveEntry),result) && world.empty());
    check(!EngineHooks::PublishEntryHook(world,reinterpret_cast<std::uintptr_t>(code),original,nullptr,result) && world.empty());
    check(EngineHooks::PublishEntryHook(capture,reinterpret_cast<std::uintptr_t>(code),original,reinterpret_cast<void*>(&ObserveEntry),result));
    check(capture.size()==1 && capture[0].installed && !capture[0].publicationQualified && !capture[0].residentOwner);
    check(entry(1,2,3,4)==7 && entry_calls==1);
    const std::array<std::uintptr_t,4> expected_arguments{1,2,3,4};
    check(entry_arguments==expected_arguments);
    MEMORY_BASIC_INFORMATION relay_region{};
    check(VirtualQuery(reinterpret_cast<void*>(capture[0].allocation),&relay_region,sizeof(relay_region))==sizeof(relay_region) &&
        relay_region.State==MEM_COMMIT && relay_region.Protect==PAGE_EXECUTE_READ);
    check(!EngineHooks::PublishEntryHook(world,reinterpret_cast<std::uintptr_t>(code),original,reinterpret_cast<void*>(&ObserveEntry),result) && world.empty());
    check(!EngineHooks::PublishCallHook(world,reinterpret_cast<std::uintptr_t>(code+132),reinterpret_cast<std::uintptr_t>(code+65),
        reinterpret_cast<void*>(&ReplaceCall),result) && world.empty());
    check(EngineHooks::PublishCallHook(world,reinterpret_cast<std::uintptr_t>(code+132),reinterpret_cast<std::uintptr_t>(code+64),
        reinterpret_cast<void*>(&ReplaceCall),result));
    check(call()==22 && call_calls==1 && world.size()==1 && world[0].relay.size()==14);
    check(EngineHooks::RestoreEngineHooks(world) && !world[0].installed && capture[0].installed);
    check(call()==11 && call_calls==1 && entry(5,6,7,8)==7 && entry_calls==2);
    // A foreign change must remain untouched and keep the receipt retryable.
    auto foreign=capture[0].replacement;foreign[0]=0xcc;CopyCode(code,foreign);
    check(!EngineHooks::RestoreEngineHooks(capture) && capture[0].installed && code[0]==0xcc);
    CopyCode(code,capture[0].replacement);
    check(EngineHooks::RestoreEngineHooks(capture) && !capture[0].installed);
    check(entry(9,10,11,12)==7 && entry_calls==2);
    check(VirtualQuery(reinterpret_cast<void*>(capture[0].allocation),&relay_region,sizeof(relay_region))==sizeof(relay_region) &&
        relay_region.State==MEM_COMMIT && relay_region.Protect==PAGE_EXECUTE_READ);
    // The generic world hook supplies its own register bridge and replay.
    auto replay=std::vector<std::uint8_t>(original.begin(),original.end());replay.insert(replay.end(),{0xe9,0,0,0,0});
    check(EngineHooks::PublishEngineHook(world,reinterpret_cast<std::uintptr_t>(code),original,std::move(replay),false,result));
    check(entry(0,0,0,0)==7 && entry_calls==2);
    check(EngineHooks::RestoreEngineHooks(world) && entry(0,0,0,0)==7);
    const auto receipts=capture.size();
    // Both pages are readable, but the existing writer refuses a crossing span.
    check(!EngineHooks::PublishEntryHook(capture,reinterpret_cast<std::uintptr_t>(crossing),original,
        reinterpret_cast<void*>(&ObserveEntry),result) && result.writer_failure && capture.size()==receipts);
    check(!std::memcmp(crossing,original.data(),original.size()));
    check(VirtualFree(code,0,MEM_RELEASE)!=FALSE);
}
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
    publications_and_restoration();
    std::printf("PASS: %u executable writer/publication checks, entry/CALL execution and guarded restoration\n",checks);
}
