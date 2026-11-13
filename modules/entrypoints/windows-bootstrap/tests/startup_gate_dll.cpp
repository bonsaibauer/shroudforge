#include "../startup_gate.h"

namespace StartupGate { void ReportFailure(const char* detail,DWORD) noexcept {OutputDebugStringA(detail);} }
namespace {
int continuation_context;
std::atomic<bool> continued{};
unsigned activation_attempts{}, commit_attempts{};
HANDLE callback_entered{}, callback_release{};
bool RejectCompletion() {
    wchar_t value[2]{};
    return GetEnvironmentVariableW(L"SF_GATE_TEST_COMPLETION_FAIL", value, 2) != 0;
}
bool DeadlineDuringActivationRefusesCommit() {
    const auto previous=StartupGate::post_restore_state.exchange(2,std::memory_order_acq_rel);
    unsigned activated{},committed{};
    const bool accepted=StartupGate::ActivateSelectedAndCommitIfPostRestoreActive(
        [&] { ++activated;StartupGate::post_restore_state.store(4,std::memory_order_release);return true; },
        [&] { ++committed;return true; });
    StartupGate::post_restore_state.store(previous,std::memory_order_release);
    return !accepted&&activated==1&&committed==0;
}
}
bool __cdecl Continue(void* value,unsigned char* entry,const unsigned char* original) noexcept {
    if(callback_entered) {
        SetEvent(callback_entered);
        if(WaitForSingleObject(callback_release,5000)!=WAIT_OBJECT_0) ExitProcess(42);
    }
    const bool valid=value==&continuation_context && GetCurrentThreadId()==StartupGate::parked_thread_id.load() &&
        !StartupGate::IsHeld(GetCurrentThreadId()) && !std::memcmp(entry,original,14);
    if(!valid || RejectCompletion()) {continued=false;return false;}
    const bool accepted=StartupGate::ActivateSelectedAndCommitIfPostRestoreActive(
        [] { ++activation_attempts; return true; },
        [] { ++commit_attempts; return true; });
    const bool late=callback_entered &&
        StartupGate::post_restore_state.load(std::memory_order_acquire)!=2;
    if(late && (activation_attempts || commit_attempts)) ExitProcess(42);
    continued=accepted;
    return continued;
}
DWORD WINAPI Prepare(void*) {
    if (WaitForSingleObject(StartupGate::entered, 5000) != WAIT_OBJECT_0) return 1;
    if(StartupGate::post_restore_state.load(std::memory_order_acquire)!=0 ||
        !DeadlineDuringActivationRefusesCommit()) {StartupGate::Complete(false);return 1;}
    // Loading a DLL here would deadlock if the game waited inside DllMain.
    auto library = LoadLibraryW(L"version.dll");
    if (!library) { StartupGate::Complete(false); return 1; }
    FreeLibrary(library);
    const auto proof=StartupGate::CaptureHeldEntryProof();
    if(!proof.is_held || !proof.is_held(proof.parked_thread_id) ||
        !StartupGate::ArmPostRestore(proof.parked_thread_id,Continue,&continuation_context) ||
        StartupGate::ArmPostRestore(proof.parked_thread_id,Continue,&continuation_context)) {
        StartupGate::Complete(false);return 1;
    }
    Sleep(100);
    wchar_t fail[2]{};
    StartupGate::Complete(GetEnvironmentVariableW(L"SF_GATE_TEST_FAIL", fail, 2) == 0);
    if(callback_entered) {
        if(WaitForSingleObject(callback_entered,5000)!=WAIT_OBJECT_0 || StartupGate::AwaitPostRestore(0)) ExitProcess(42);
        SetEvent(callback_release);
    }
    return 0;
}

extern "C" __declspec(dllexport) int GateVerified() {
    return WaitForSingleObject(StartupGate::completed, 0) == WAIT_OBJECT_0 &&
        StartupGate::succeeded == 1 && continued && StartupGate::post_restore_state.load()==3 &&
        activation_attempts==1 && commit_attempts==1 &&
        std::memcmp(StartupGate::entrypoint, StartupGate::original, sizeof(StartupGate::original)) == 0;
}

BOOL WINAPI DllMain(HMODULE, DWORD reason, LPVOID) {
    if (reason != DLL_PROCESS_ATTACH) return TRUE;
    wchar_t deadline[2]{};
    if(GetEnvironmentVariableW(L"SF_GATE_TEST_DEADLINE",deadline,2)) {
        callback_entered=CreateEventW(nullptr,TRUE,FALSE,nullptr);
        callback_release=CreateEventW(nullptr,TRUE,FALSE,nullptr);
        if(!callback_entered || !callback_release) return FALSE;
    }
    if (!StartupGate::Install()) return FALSE;
    auto worker = CreateThread(nullptr, 0, Prepare, nullptr, 0, nullptr);
    if (!worker) return FALSE;
    CloseHandle(worker);
    return TRUE;
}
