// Read-only register sampler used to discover the live Keen entity manager.
// It does not inject code, patch the game, or retain transient engine pointers.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <tlhelp32.h>

#include <charconv>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <unordered_set>
#include <vector>

namespace {
struct Thread { HANDLE handle; DWORD id; };

template<class T>
bool read(HANDLE process, std::uintptr_t address, T& value) {
    SIZE_T received{};
    return address && ReadProcessMemory(process, reinterpret_cast<const void*>(address),
        &value, sizeof(value), &received) && received == sizeof(value);
}

std::uintptr_t image_base(DWORD pid) {
    const auto snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid);
    if (snapshot == INVALID_HANDLE_VALUE) return 0;
    MODULEENTRY32W module{sizeof(module)};
    const auto found = Module32FirstW(snapshot, &module);
    CloseHandle(snapshot);
    return found ? reinterpret_cast<std::uintptr_t>(module.modBaseAddr) : 0;
}
}

int main(int argc, char** argv) {
    if (argc != 2) return 2;
    DWORD pid{};
    const auto end = argv[1] + std::strlen(argv[1]);
    if (std::from_chars(argv[1], end, pid).ec != std::errc{}) return 2;
    const auto process = OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, FALSE, pid);
    const auto base = image_base(pid);
    if (!process || !base) return 3;
    const std::uint8_t expected[]{0x48,0x81,0xc3,0x88,0x00,0x00,0x00};
    std::uint8_t actual[sizeof(expected)]{};
    SIZE_T received{};
    if (!ReadProcessMemory(process, reinterpret_cast<const void*>(base + 0x23a159),
                           actual, sizeof(actual), &received) || received != sizeof(actual) ||
        std::memcmp(actual, expected, sizeof(actual))) return 4;

    const auto snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
    THREADENTRY32 entry{sizeof(entry)};
    std::vector<Thread> threads;
    if (Thread32First(snapshot, &entry)) do {
        if (entry.th32OwnerProcessID != pid) continue;
        const auto handle = OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT,
                                       FALSE, entry.th32ThreadID);
        if (handle) threads.push_back({handle, entry.th32ThreadID});
    } while (Thread32Next(snapshot, &entry));
    CloseHandle(snapshot);

    std::unordered_set<std::uintptr_t> found;
    const auto until = GetTickCount64() + 60'000;
    std::uint64_t samples{}, matches{};
    while (GetTickCount64() < until) {
        for (const auto& thread : threads) {
            CONTEXT context{};
            context.ContextFlags = CONTEXT_CONTROL | CONTEXT_INTEGER;
            if (SuspendThread(thread.handle) == DWORD(-1)) continue;
            const auto captured = GetThreadContext(thread.handle, &context) != FALSE;
            ResumeThread(thread.handle);
            ++samples;
            if (!captured || context.Rip < base + 0x239f50 || context.Rip >= base + 0x23a1c2) continue;
            ++matches;
            std::uintptr_t root{}, manager{}, table{};
            std::uint64_t slots{};
            if (!read(process, context.R15, root) || !root ||
                !read(process, root + 0x30, manager) || !manager ||
                !read(process, manager + 0x158, slots) || !slots || slots > (1u << 20) ||
                !read(process, manager + 0x188, table) || !table) continue;
            if (found.insert(manager).second) {
                std::cout << "manager=0x" << std::hex << manager << " table=0x" << table
                          << " r15=0x" << context.R15 << " root=0x" << root
                          << " rip_rva=0x" << context.Rip - base << std::dec
                          << " slots=" << slots << " thread=" << thread.id << '\n' << std::flush;
            }
        }
        Sleep(1);
    }
    for (const auto& thread : threads) CloseHandle(thread.handle);
    CloseHandle(process);
    std::cerr << "samples=" << samples << " code_matches=" << matches
              << " managers=" << found.size() << '\n';
    return found.empty() ? 5 : 0;
}
