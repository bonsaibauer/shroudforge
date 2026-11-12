#include "../startup_gate.h"
#include "../greeneye_entry.h"

namespace StartupGate { void ReportFailure(const char* detail,DWORD) noexcept {OutputDebugStringA(detail);} }
namespace {
int continuation_context;
std::atomic<bool> continued{};
int original_stage{};
HANDLE callback_entered{}, callback_release{};
void RequireStage(int expected) { if(original_stage++ != expected) ExitProcess(42); }
bool RejectWitness() {
    wchar_t value[2]{};
    return GetEnvironmentVariableW(L"SF_GATE_TEST_WITNESS_FAIL", value, 2) != 0;
}
std::uintptr_t __cdecl LoadOriginal(const wchar_t*,const GreeneyeEntry::TicketV1*) {
    RequireStage(1);
    return 123;
}
bool __cdecl ConfirmOriginal(const wchar_t*,const GreeneyeEntry::TicketV1*,std::uintptr_t module,bool witness) {
    RequireStage(3);
    if(module != 123 || witness == RejectWitness()) ExitProcess(42);
    return witness;
}
}
// Exercise the production continuation with inert owner and loader seams.
// No original DLL is loaded and no private scan is called by this fixture.
namespace GreeneyeDynamic {
bool Begin(unsigned char* entry,const unsigned char* original) noexcept {
    RequireStage(0);
    return !std::memcmp(entry,original,14) && !StartupGate::IsHeld(GetCurrentThreadId());
}
bool Witness(std::uintptr_t module) noexcept {
    RequireStage(2);
    return module == 123 && !RejectWitness();
}
bool Finalize() noexcept { RequireStage(4); return true; }
const char* Failure() noexcept {return "fixture-original-failure";}
}
bool __cdecl Continue(void* value,unsigned char* entry,const unsigned char* original) noexcept {
    if(callback_entered) {
        SetEvent(callback_entered);
        if(WaitForSingleObject(callback_release,5000)!=WAIT_OBJECT_0) ExitProcess(42);
    }
    const bool valid=value==&continuation_context && GetCurrentThreadId()==StartupGate::parked_thread_id.load() &&
        !StartupGate::IsHeld(GetCurrentThreadId()) && !std::memcmp(entry,original,14);
    GreeneyeEntry::Context context{L"inert-root",{48,1,2,0,{}},LoadOriginal,ConfirmOriginal};
    const bool ready=valid && GreeneyeEntry::Continue(context,entry,original);
    continued=ready && original_stage==5;
    return continued;
}
DWORD WINAPI Prepare(void*) {
    if (WaitForSingleObject(StartupGate::entered, 5000) != WAIT_OBJECT_0) return 1;
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
