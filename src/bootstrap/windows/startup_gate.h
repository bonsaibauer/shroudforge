#pragma once
#include <windows.h>
#include <cstring>

// Installed by the startup proxy before the executable entrypoint runs. The
// wait happens at that entrypoint, outside DllMain and the Windows loader lock,
// so the preparation worker can load DLLs and use parser worker threads.
namespace StartupGate {
inline HANDLE entered{};
inline HANDLE completed{};
inline unsigned char* entrypoint{};
inline unsigned char original[14]{};
inline volatile LONG succeeded{};

inline void Complete(bool success) {
    InterlockedExchange(&succeeded, success ? 1 : 0);
    SetEvent(completed);
}

inline int __cdecl Run() {
    SetEvent(entered);
    if (WaitForSingleObject(completed, INFINITE) != WAIT_OBJECT_0 ||
        InterlockedCompareExchange(&succeeded, 0, 0) != 1) {
        ExitProcess(ERROR_DLL_INIT_FAILED);
    }
    DWORD protection{};
    if (!VirtualProtect(entrypoint, sizeof(original), PAGE_EXECUTE_READWRITE, &protection))
        ExitProcess(ERROR_DLL_INIT_FAILED);
    std::memcpy(entrypoint, original, sizeof(original));
    DWORD ignored{};
    if (!VirtualProtect(entrypoint, sizeof(original), protection, &ignored) ||
        !FlushInstructionCache(GetCurrentProcess(), entrypoint, sizeof(original)))
        ExitProcess(ERROR_DLL_INIT_FAILED);
    return reinterpret_cast<int (__cdecl*)()>(entrypoint)();
}

inline bool Install() {
    auto* image = reinterpret_cast<unsigned char*>(GetModuleHandleW(nullptr));
    if (!image) return false;
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(image);
    if (dos->e_magic != IMAGE_DOS_SIGNATURE || dos->e_lfanew <= 0) return false;
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(image + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE ||
        nt->FileHeader.Machine != IMAGE_FILE_MACHINE_AMD64 ||
        nt->OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR64_MAGIC ||
        nt->OptionalHeader.SizeOfImage < sizeof(original) ||
        !nt->OptionalHeader.AddressOfEntryPoint ||
        nt->OptionalHeader.AddressOfEntryPoint > nt->OptionalHeader.SizeOfImage - sizeof(original))
        return false;
    entered = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    completed = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    if (!entered || !completed) return false;
    entrypoint = image + nt->OptionalHeader.AddressOfEntryPoint;
    // RIP-relative indirect jump preserves registers. No instruction relocation
    // is needed: restore the entire entrypoint before executing any of its code.
    unsigned char jump[14]{0xff, 0x25, 0, 0, 0, 0};
    auto destination = &Run;
    static_assert(sizeof(destination) == 8);
    std::memcpy(jump + 6, &destination, sizeof(destination));
    DWORD protection{};
    if (!VirtualProtect(entrypoint, sizeof(jump), PAGE_EXECUTE_READWRITE, &protection)) return false;
    std::memcpy(original, entrypoint, sizeof(original));
    std::memcpy(entrypoint, jump, sizeof(jump));
    DWORD ignored{};
    return VirtualProtect(entrypoint, sizeof(jump), protection, &ignored) &&
        FlushInstructionCache(GetCurrentProcess(), entrypoint, sizeof(jump));
}
}
