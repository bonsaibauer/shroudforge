#include "../startup_gate.h"

DWORD WINAPI Prepare(void*) {
    if (WaitForSingleObject(StartupGate::entered, 5000) != WAIT_OBJECT_0) return 1;
    // Loading a DLL here would deadlock if the game waited inside DllMain.
    auto library = LoadLibraryW(L"version.dll");
    if (!library) { StartupGate::Complete(false); return 1; }
    FreeLibrary(library);
    Sleep(100);
    wchar_t fail[2]{};
    StartupGate::Complete(GetEnvironmentVariableW(L"SF_GATE_TEST_FAIL", fail, 2) == 0);
    return 0;
}

extern "C" __declspec(dllexport) int GateVerified() {
    return WaitForSingleObject(StartupGate::completed, 0) == WAIT_OBJECT_0 &&
        StartupGate::succeeded == 1 &&
        std::memcmp(StartupGate::entrypoint, StartupGate::original, sizeof(StartupGate::original)) == 0;
}

BOOL WINAPI DllMain(HMODULE, DWORD reason, LPVOID) {
    if (reason != DLL_PROCESS_ATTACH) return TRUE;
    if (!StartupGate::Install()) return FALSE;
    auto worker = CreateThread(nullptr, 0, Prepare, nullptr, 0, nullptr);
    if (!worker) return FALSE;
    CloseHandle(worker);
    return TRUE;
}
