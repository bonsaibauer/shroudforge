#include "engine-hooks/write_executable_bytes.h"
#include "engine-hooks/executable_write_transaction.h"
#include "engine-hooks/held_entry_preparation.h"
#include <windows.h>
#include <tlhelp32.h>
#include <atomic>
#include <mutex>
#include <intrin.h>

namespace {
std::mutex executable_writer_mutex;
std::atomic<std::uint32_t> held_entry_worker{};
std::atomic<unsigned> last_failure{};
using NextThread = LONG (NTAPI*)(HANDLE,HANDLE,ACCESS_MASK,ULONG,ULONG,PHANDLE);
constexpr LONG NoMoreThreads = static_cast<LONG>(0x8000001aU);

struct WindowsThreads {
    NextThread next{};
    DWORD current{GetCurrentThreadId()};
    DWORD process{GetCurrentProcessId()};
    DWORD page_size{};
    WindowsThreads() {
        // Resolve before suspending peers. GetProcAddress/loader operations are
        // forbidden inside the frozen window.
        const auto module = GetModuleHandleW(L"ntdll.dll");
        next = module ? reinterpret_cast<NextThread>(GetProcAddress(module,"NtGetNextThread")) : nullptr;
        SYSTEM_INFO system{}; GetSystemInfo(&system); page_size = system.dwPageSize;
    }
    std::uint32_t CurrentThread() const { return current; }
    bool ValidRange(std::uintptr_t address, std::size_t size) const {
        if (!next || !page_size || address/page_size != (address+size-1)/page_size) return false;
        MEMORY_BASIC_INFORMATION info{};
        if (VirtualQuery(reinterpret_cast<void*>(address),&info,sizeof(info)) != sizeof(info) ||
            info.State != MEM_COMMIT || (info.Protect & (PAGE_GUARD|PAGE_NOACCESS))) return false;
        const auto access = info.Protect & 0xff;
        return access == PAGE_EXECUTE_READ || access == PAGE_EXECUTE_READWRITE || access == PAGE_EXECUTE_WRITECOPY;
    }
    bool Inventory(bool frozen, std::array<std::uint32_t,EngineHooks::ExecutableWrite::MaxPeers>& ids,
                   std::size_t& count) const {
        count = 0;
        bool saw_current{};
        if (!frozen) {
            const auto snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD,0);
            if (snapshot == INVALID_HANDLE_VALUE) return false;
            THREADENTRY32 entry{sizeof(entry)};
            SetLastError(ERROR_SUCCESS);
            // An initial failure is never a valid empty process: the calling
            // thread necessarily exists in a complete thread snapshot.
            if (!Thread32First(snapshot,&entry)) { CloseHandle(snapshot); return false; }
            for (unsigned inspected = 0; inspected < 65536; ++inspected) {
                if (entry.dwSize < offsetof(THREADENTRY32,th32OwnerProcessID)+sizeof(entry.th32OwnerProcessID)) {
                    CloseHandle(snapshot); return false;
                }
                if (entry.th32OwnerProcessID == process) {
                    if (!entry.th32ThreadID) { CloseHandle(snapshot); return false; }
                    if (entry.th32ThreadID == current) saw_current = true;
                    else {
                        if (count == ids.size()) { CloseHandle(snapshot); return false; }
                        ids[count++] = entry.th32ThreadID;
                    }
                }
                entry.dwSize = sizeof(entry); SetLastError(ERROR_SUCCESS);
                if (Thread32Next(snapshot,&entry)) continue;
                const auto error = GetLastError();
                const bool closed = CloseHandle(snapshot) != FALSE;
                return error == ERROR_NO_MORE_FILES && closed && saw_current;
            }
            CloseHandle(snapshot); return false;
        }
        // Toolhelp can allocate from the application heap. The fresh fixed-point
        // inventory instead walks kernel thread handles into preallocated storage.
        // Missing/failed native iteration is a refusal, never an empty inventory.
        if (!next) return false;
        HANDLE previous{};
        for (std::size_t inspected = 0; inspected < ids.size()+2; ++inspected) {
            HANDLE thread{};
            const auto status = next(GetCurrentProcess(),previous,
                THREAD_SUSPEND_RESUME|THREAD_GET_CONTEXT|THREAD_QUERY_INFORMATION,0,0,&thread);
            if (previous) CloseHandle(previous);
            previous = nullptr;
            if (status == NoMoreThreads) {
                if (thread) { CloseHandle(thread); return false; }
                return saw_current;
            }
            if (status < 0 || !thread) { if (thread) CloseHandle(thread); return false; }
            const auto id = GetThreadId(thread);
            if (!id) { CloseHandle(thread); return false; }
            if (id == current) saw_current = true;
            else {
                if (count == ids.size()) { CloseHandle(thread); return false; }
                ids[count++] = id;
            }
            previous = thread;
        }
        if (previous) CloseHandle(previous);
        return false;
    }
    std::uintptr_t Open(std::uint32_t id) const {
        const auto thread = OpenThread(
            THREAD_SUSPEND_RESUME|THREAD_GET_CONTEXT|THREAD_QUERY_INFORMATION,FALSE,id);
        if (!thread) return 0;
        // Snapshot IDs may become stale before opening. Never suspend a reused
        // identifier belonging to another process or a different thread object.
        if (GetProcessIdOfThread(thread) != process || GetThreadId(thread) != id) {
            CloseHandle(thread); return 0;
        }
        return reinterpret_cast<std::uintptr_t>(thread);
    }
    bool Suspend(std::uintptr_t handle, std::uint32_t& previous) const {
        previous = SuspendThread(reinterpret_cast<HANDLE>(handle));
        return previous != DWORD(-1);
    }
    bool Pc(std::uintptr_t handle, std::uintptr_t& pc) const {
        CONTEXT context{}; context.ContextFlags = CONTEXT_CONTROL;
        if (!GetThreadContext(reinterpret_cast<HANDLE>(handle),&context)) return false;
        pc = context.Rip; return true;
    }
    bool Resume(std::uintptr_t handle, std::uint32_t previous) const {
        const auto count = ResumeThread(reinterpret_cast<HANDLE>(handle));
        return count != DWORD(-1) && count == previous+1;
    }
    void Close(std::uintptr_t handle) const { CloseHandle(reinterpret_cast<HANDLE>(handle)); }
    bool Writable(std::uintptr_t address, std::size_t size, std::uint32_t& previous) const {
        DWORD protection{};
        const auto result = VirtualProtect(reinterpret_cast<void*>(address),size,PAGE_EXECUTE_READWRITE,&protection);
        previous = protection; return result != FALSE;
    }
    bool Restore(std::uintptr_t address, std::size_t size, std::uint32_t previous) const {
        DWORD ignored{};
        return VirtualProtect(reinterpret_cast<void*>(address),size,previous,&ignored) != FALSE;
    }
    bool Flush(std::uintptr_t address, std::size_t size) const {
        return FlushInstructionCache(GetCurrentProcess(),reinterpret_cast<void*>(address),size) != FALSE;
    }
};

[[noreturn]] void uncertain_write() {
    // Returning false could make an existing caller free a published relay.
    // Do not unwind, log, allocate, or resume uncertain execution. Process exit
    // retains all potentially reachable code until the address space is gone.
    TerminateProcess(GetCurrentProcess(),ERROR_WRITE_FAULT);
    __fastfail(FAST_FAIL_FATAL_APP_EXIT);
}
EngineHooks::ExecutableWrite::Result write_locked(std::uintptr_t address,const void* bytes,
        std::size_t size,const void* expected) {
    WindowsThreads os;
    const auto result = EngineHooks::ExecutableWrite::Run(os,address,bytes,size,expected);
    last_failure.store(static_cast<unsigned>(result.failure),std::memory_order_relaxed);
    if (result.outcome == EngineHooks::ExecutableWrite::Outcome::Indeterminate) uncertain_write();
    return result;
}
}

namespace EngineHooks {
bool WriteExecutableBytes(std::uintptr_t address,const void* bytes,std::size_t size,const void* expected) {
    std::lock_guard lock(executable_writer_mutex);
    return write_locked(address,bytes,size,expected).outcome == ExecutableWrite::Outcome::Published;
}
bool ReplaceExecutableBytes(std::uintptr_t address,const void* expected,const void* replacement,std::size_t size) {
    if (!expected) return false;
    std::lock_guard lock(executable_writer_mutex);
    for (unsigned attempt=0; attempt<8; ++attempt) {
        const auto result = write_locked(address,replacement,size,expected);
        if (result.outcome == ExecutableWrite::Outcome::Published) return true;
        if (result.failure != ExecutableWrite::Failure::OccupiedPc &&
            result.failure != ExecutableWrite::Failure::UnstableInventory &&
            result.failure != ExecutableWrite::Failure::OpenThread &&
            result.failure != ExecutableWrite::Failure::SuspendThread) return false;
        Sleep(2); // Peers have been resumed before this bounded retry.
    }
    return false;
}
unsigned LastExecutableWriteFailure() noexcept { return last_failure.load(std::memory_order_relaxed); }

bool HeldEntryToken::ValidForCurrentThread() const noexcept {
    return worker_ && worker_ == GetCurrentThreadId() &&
        held_entry_worker.load(std::memory_order_acquire) == worker_ &&
        proof_.parked_thread_id && proof_.parked_thread_id != worker_ &&
        proof_.is_held && proof_.is_held(proof_.parked_thread_id);
}
bool RunHeldEntryPreparation(const HeldEntryProof& proof,HeldEntryCallback prepare,void* context) noexcept {
    const auto worker = GetCurrentThreadId();
    if (proof.size != sizeof(proof) || proof.version != 1 || !prepare || !proof.is_held ||
        !proof.parked_thread_id || proof.parked_thread_id == worker || !proof.is_held(proof.parked_thread_id)) return false;
    std::uint32_t empty{};
    if (!held_entry_worker.compare_exchange_strong(empty,worker,std::memory_order_acq_rel)) return false;
    const HeldEntryToken token(proof,worker);
    const bool result = token.ValidForCurrentThread() && prepare(token,context) && token.ValidForCurrentThread();
    held_entry_worker.store(0,std::memory_order_release);
    return result;
}
}
