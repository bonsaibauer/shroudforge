#include "dispatcher.h"

#include "profile.h"
#include "world_runtime.h"
#include "patch_runtime.h"
#include <nlohmann/json.hpp>

#include <windows.h>
#include <tlhelp32.h>
#include <algorithm>
#include <array>
#include <atomic>
#include <chrono>
#include <charconv>
#include <cstring>
#include <limits>
#include <memory>
#include <mutex>
#include <sstream>
#include <string_view>
#include <vector>

namespace {
struct Job {
    GameThreadDispatcher::Operation operation{};
    std::shared_ptr<void> context;
    HANDLE complete{CreateEventW(nullptr, TRUE, FALSE, nullptr)};
    std::atomic<bool> executed{};
    std::atomic<unsigned> state{}; // 0 queued, 1 executing, 2 complete, 3 cancelled
    ~Job() { if (complete) CloseHandle(complete); }
};

std::mutex queue_mutex;
std::vector<std::shared_ptr<Job>> queue;
std::atomic<bool> accepting{};
std::atomic<bool> entity_context_hooks_ready{};
std::atomic<std::uint32_t> engine_thread{};
std::atomic<std::uintptr_t> entity_manager{};
std::atomic<bool> command_observed{};
std::atomic<std::uint64_t> last_drain_ms{}, drain_count{}, completed_count{}, timeout_count{}, rejected_count{};
std::atomic<std::uint64_t> manager_changes{}, last_manager_observation_ms{}, last_operation_us{}, maximum_operation_us{};
struct InstalledHook {
    std::uintptr_t target{};
    void* payload{};
    std::vector<std::uint8_t> original;
};
std::vector<InstalledHook> hooks;

bool supported_image(std::uint8_t*& base, std::size_t& size) {
    base = reinterpret_cast<std::uint8_t*>(GetModuleHandleW(nullptr));
    if (!base) return false;
    const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base);
    if (dos->e_magic != IMAGE_DOS_SIGNATURE) return false;
    const auto nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(base + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE ||
        nt->FileHeader.TimeDateStamp != KfcRuntimeCompatibility::EnshroudedClient::image_timestamp ||
        nt->OptionalHeader.SizeOfImage != KfcRuntimeCompatibility::EnshroudedClient::image_size) return false;
    size = nt->OptionalHeader.SizeOfImage;
    return true;
}

std::vector<int> parse_signature(std::string_view text) {
    std::vector<int> result;
    std::istringstream input{std::string(text)};
    std::string token;
    while (input >> token) {
        if (token == "?" || token == "??") { result.push_back(-1); continue; }
        unsigned value{};
        const auto parsed = std::from_chars(token.data(), token.data() + token.size(), value, 16);
        if (parsed.ec != std::errc{} || parsed.ptr != token.data() + token.size() || value > 0xff) return {};
        result.push_back(static_cast<int>(value));
    }
    return result;
}

std::uintptr_t find_unique_executable_signature(std::uint8_t* base, std::string_view text) {
    const auto signature = parse_signature(text);
    if (signature.empty()) return 0;
    const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base);
    const auto nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(base + dos->e_lfanew);
    const auto sections = IMAGE_FIRST_SECTION(nt);
    std::uintptr_t found{};
    for (WORD section_index = 0; section_index < nt->FileHeader.NumberOfSections; ++section_index) {
        const auto& section = sections[section_index];
        if (!(section.Characteristics & IMAGE_SCN_MEM_EXECUTE)) continue;
        const auto section_size = (std::min)(static_cast<std::size_t>(section.Misc.VirtualSize),
            static_cast<std::size_t>(nt->OptionalHeader.SizeOfImage - section.VirtualAddress));
        if (section_size < signature.size()) continue;
        const auto start = base + section.VirtualAddress;
        for (std::size_t offset = 0; offset <= section_size - signature.size(); ++offset) {
            bool equal = true;
            for (std::size_t byte = 0; byte < signature.size(); ++byte) {
                if (signature[byte] >= 0 && start[offset + byte] != signature[byte]) { equal = false; break; }
            }
            if (!equal) continue;
            if (found) return 0;
            found = reinterpret_cast<std::uintptr_t>(start + offset);
        }
    }
    return found;
}

bool relative(std::uintptr_t instruction, std::uintptr_t destination, std::int32_t& value) {
    const auto delta = static_cast<std::int64_t>(destination) - static_cast<std::int64_t>(instruction + 5);
    if (delta < (std::numeric_limits<std::int32_t>::min)() ||
        delta > (std::numeric_limits<std::int32_t>::max)()) return false;
    value = static_cast<std::int32_t>(delta);
    return true;
}

void* allocate_near(std::uintptr_t target, std::size_t size) {
    SYSTEM_INFO system{};
    GetSystemInfo(&system);
    const auto granularity = static_cast<std::uintptr_t>(system.dwAllocationGranularity);
    constexpr std::uintptr_t range = 0x7fff0000;
    const auto minimum = (std::max)(reinterpret_cast<std::uintptr_t>(system.lpMinimumApplicationAddress),
        target > range ? target - range : 0);
    const auto maximum = (std::min)(reinterpret_cast<std::uintptr_t>(system.lpMaximumApplicationAddress),
        target <= UINTPTR_MAX - range ? target + range : target);
    for (auto cursor = minimum; cursor < maximum;) {
        MEMORY_BASIC_INFORMATION memory{};
        if (!VirtualQuery(reinterpret_cast<void*>(cursor), &memory, sizeof(memory))) break;
        const auto region = reinterpret_cast<std::uintptr_t>(memory.BaseAddress);
        const auto end = region + memory.RegionSize;
        if (memory.State == MEM_FREE) {
            const auto candidate = (region + granularity - 1) & ~(granularity - 1);
            if (candidate >= minimum && candidate + size <= end && candidate + size <= maximum) {
                if (auto allocation = VirtualAlloc(reinterpret_cast<void*>(candidate), size,
                    MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE)) return allocation;
            }
        }
        if (end <= cursor) break;
        cursor = end;
    }
    return nullptr;
}

bool write_code(std::uintptr_t address, const void* bytes, std::size_t size, const void* expected = nullptr) {
    if (size > 32) return false;
    // Prepare handles before suspending: never allocate while an engine thread
    // might hold the process heap lock. Refuse to patch an occupied instruction.
    std::vector<HANDLE> threads;
    HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
    if (snapshot == INVALID_HANDLE_VALUE) return false;
    THREADENTRY32 entry{sizeof(entry)};
    if (Thread32First(snapshot, &entry)) do {
        if (entry.th32OwnerProcessID != GetCurrentProcessId() || entry.th32ThreadID == GetCurrentThreadId()) continue;
        HANDLE thread = OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION, FALSE, entry.th32ThreadID);
        if (!thread) {
            CloseHandle(snapshot);
            for (auto opened : threads) CloseHandle(opened);
            return false;
        }
        threads.push_back(thread);
    } while (Thread32Next(snapshot, &entry));
    CloseHandle(snapshot);
    std::size_t suspended{};
    bool safe = true;
    for (auto thread : threads) {
        if (SuspendThread(thread) == DWORD(-1)) { safe = false; break; }
        ++suspended;
        CONTEXT context{};
        context.ContextFlags = CONTEXT_CONTROL;
        if (!GetThreadContext(thread, &context) || (context.Rip >= address && context.Rip < address + size)) {
            safe = false; break;
        }
    }
    DWORD previous{};
    bool result{};
    if (safe && (!expected || std::memcmp(reinterpret_cast<void*>(address), expected, size) == 0) &&
        VirtualProtect(reinterpret_cast<void*>(address), size, PAGE_EXECUTE_READWRITE, &previous)) {
        std::uint8_t original[32]{};
        std::memcpy(original, reinterpret_cast<void*>(address), size);
        std::memcpy(reinterpret_cast<void*>(address), bytes, size);
        FlushInstructionCache(GetCurrentProcess(), reinterpret_cast<void*>(address), size);
        DWORD ignored{};
        result = VirtualProtect(reinterpret_cast<void*>(address), size, previous, &ignored) != FALSE;
        if (!result) {
            std::memcpy(reinterpret_cast<void*>(address), original, size);
            FlushInstructionCache(GetCurrentProcess(), reinterpret_cast<void*>(address), size);
            VirtualProtect(reinterpret_cast<void*>(address), size, previous, &ignored);
        }
    }
    for (std::size_t index = 0; index < suspended; ++index) ResumeThread(threads[index]);
    for (auto thread : threads) CloseHandle(thread);
    return result;
}

void __cdecl drain(void*, void*) {
    if (!accepting.load(std::memory_order_acquire)) return;
    engine_thread.store(GetCurrentThreadId(), std::memory_order_release);
    last_drain_ms.store(GetTickCount64(), std::memory_order_relaxed);
    drain_count.fetch_add(1, std::memory_order_relaxed);
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(2);
    for (unsigned index = 0; index < 8; ++index) {
        std::shared_ptr<Job> job;
        {
            std::unique_lock lock(queue_mutex, std::try_to_lock);
            if (!lock || queue.empty() || !accepting.load(std::memory_order_acquire)) return;
            job = queue.front();
            queue.erase(queue.begin());
        }
        unsigned queued = 0;
        if (job->state.compare_exchange_strong(queued, 1, std::memory_order_acq_rel) && job->operation) {
            const auto operation_started = std::chrono::steady_clock::now();
            try {
                job->operation(job->context.get());
                job->executed.store(true, std::memory_order_release);
                command_observed.store(true, std::memory_order_release);
                completed_count.fetch_add(1, std::memory_order_relaxed);
            } catch (...) {
                // Never unwind a C++ exception through the engine trampoline.
            }
            const auto duration = static_cast<std::uint64_t>(std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::steady_clock::now() - operation_started).count());
            last_operation_us.store(duration, std::memory_order_relaxed);
            auto maximum = maximum_operation_us.load(std::memory_order_relaxed);
            while (maximum < duration && !maximum_operation_us.compare_exchange_weak(maximum, duration, std::memory_order_relaxed)) {}
            job->state.store(2, std::memory_order_release);
        }
        SetEvent(job->complete);
        if (std::chrono::steady_clock::now() >= deadline) return;
    }
}

void __cdecl capture_entity_manager(void* lookup_context, void*) {
    if (!lookup_context) return;
    __try {
        const auto root = *static_cast<std::uintptr_t*>(lookup_context);
        if (!root) return;
        const auto manager = *reinterpret_cast<std::uintptr_t*>(root + KfcRuntimeCompatibility::EnshroudedClient::lookup_manager);
        if (manager) {
            if (entity_manager.exchange(manager, std::memory_order_acq_rel) != manager)
                manager_changes.fetch_add(1, std::memory_order_relaxed);
            last_manager_observation_ms.store(GetTickCount64(), std::memory_order_relaxed);
        }
    } __except (EXCEPTION_EXECUTE_HANDLER) {}
}

std::vector<std::uint8_t> callback_code(void* callback) {
    // Exact register/XMM preservation used by the validated ShroudForge actor
    // callback. RCX/RDX are forwarded but the generic dispatcher ignores them.
    std::vector<std::uint8_t> code{0x9c,0x50,0x51,0x52,0x41,0x50,0x41,0x51,0x41,0x52,0x41,0x53,
        0x48,0x81,0xec,0x80,0,0,0};
    for (unsigned index = 0; index < 6; ++index) {
        const std::uint8_t store[]{0xf3,0x0f,0x7f,static_cast<std::uint8_t>(0x44+index*8),0x24,
            static_cast<std::uint8_t>(0x20+index*16)};
        code.insert(code.end(), std::begin(store), std::end(store));
    }
    const std::uint8_t args[]{0xfc,0x49,0x8b,0xcf,0x48,0x8b,0xd5,0x48,0xb8};
    code.insert(code.end(), std::begin(args), std::end(args));
    const auto address = reinterpret_cast<std::uintptr_t>(callback);
    for (unsigned index = 0; index < 8; ++index) code.push_back(static_cast<std::uint8_t>(address >> (index*8)));
    code.insert(code.end(), {0xff,0xd0});
    for (unsigned index = 0; index < 6; ++index) {
        const std::uint8_t restore[]{0xf3,0x0f,0x6f,static_cast<std::uint8_t>(0x44+index*8),0x24,
            static_cast<std::uint8_t>(0x20+index*16)};
        code.insert(code.end(), std::begin(restore), std::end(restore));
    }
    code.insert(code.end(), {0x48,0x81,0xc4,0x80,0,0,0,0x41,0x5b,0x41,0x5a,0x41,0x59,0x41,0x58,
        0x5a,0x59,0x58,0x9d});
    return code;
}

std::vector<std::uint8_t> cursor_callback_code(void* callback, std::size_t capture_offset) {
    // Cursor call site is profile-verified at an instruction that reads [R14+2F8].
    // The reflected cursor begins at [R14+270]; pass that address while the
    // owning object is live, then restore every volatile register before replay.
    std::vector<std::uint8_t> code{0x9c,0x50,0x51,0x52,0x41,0x50,0x41,0x51,0x41,0x52,0x41,0x53,
        0x48,0x81,0xec,0x80,0,0,0};
    for (unsigned index = 0; index < 6; ++index) {
        const std::uint8_t store[]{0xf3,0x0f,0x7f,static_cast<std::uint8_t>(0x44+index*8),0x24,
            static_cast<std::uint8_t>(0x20+index*16)};
        code.insert(code.end(), std::begin(store), std::end(store));
    }
    code.insert(code.end(), {0xfc,0x49,0x8d,0x8e}); // lea rcx,[r14+disp32]
    const auto offset = static_cast<std::uint32_t>(capture_offset);
    for (unsigned index = 0; index < sizeof(offset); ++index)
        code.push_back(static_cast<std::uint8_t>(offset >> (index * 8)));
    code.insert(code.end(), {0x31,0xd2,0x48,0xb8});
    const auto address = reinterpret_cast<std::uintptr_t>(callback);
    for (unsigned index = 0; index < 8; ++index) code.push_back(static_cast<std::uint8_t>(address >> (index*8)));
    code.insert(code.end(), {0xff,0xd0});
    for (unsigned index = 0; index < 6; ++index) {
        const std::uint8_t restore[]{0xf3,0x0f,0x6f,static_cast<std::uint8_t>(0x44+index*8),0x24,
            static_cast<std::uint8_t>(0x20+index*16)};
        code.insert(code.end(), std::begin(restore), std::end(restore));
    }
    code.insert(code.end(), {0x48,0x81,0xc4,0x80,0,0,0,0x41,0x5b,0x41,0x5a,0x41,0x59,0x41,0x58,
        0x5a,0x59,0x58,0x9d});
    return code;
}

std::vector<std::uint8_t> context_callback_code(void* callback) {
    // This profile hook is at the native building-dispatch entry. Preserve the
    // caller registers, then forward its original RCX context to the callback.
    std::vector<std::uint8_t> code{0x9c,0x50,0x51,0x52,0x41,0x50,0x41,0x51,0x41,0x52,0x41,0x53,
        0x48,0x81,0xec,0x88,0,0,0};
    for (unsigned index = 0; index < 6; ++index) {
        const std::uint8_t store[]{0xf3,0x0f,0x7f,static_cast<std::uint8_t>(0x44+index*8),0x24,
            static_cast<std::uint8_t>(0x20+index*16)};
        code.insert(code.end(), std::begin(store), std::end(store));
    }
    code.insert(code.end(), {0x48,0x8b,0x8c,0x24,0xb0,0,0,0,0x31,0xd2,0x48,0xb8});
    const auto address = reinterpret_cast<std::uintptr_t>(callback);
    for (unsigned index = 0; index < 8; ++index) code.push_back(static_cast<std::uint8_t>(address >> (index*8)));
    code.insert(code.end(), {0xff,0xd0});
    for (unsigned index = 0; index < 6; ++index) {
        const std::uint8_t restore[]{0xf3,0x0f,0x6f,static_cast<std::uint8_t>(0x44+index*8),0x24,
            static_cast<std::uint8_t>(0x20+index*16)};
        code.insert(code.end(), std::begin(restore), std::end(restore));
    }
    code.insert(code.end(), {0x48,0x81,0xc4,0x88,0,0,0,0x41,0x5b,0x41,0x5a,0x41,0x59,0x41,0x58,
        0x5a,0x59,0x58,0x9d});
    return code;
}


bool install_hook(std::uint8_t* base, std::string_view signature,
                  const std::uint8_t* original, std::size_t original_size, void* callback,
                  bool cursor_callback = false, bool context_callback = false) {
    if (!original || original_size < 5) return false;
    const auto target = find_unique_executable_signature(base, signature);
    if (!target || std::memcmp(reinterpret_cast<void*>(target), original, original_size)) return false;
    auto payload = cursor_callback
        ? cursor_callback_code(callback, KfcRuntimeCompatibility::EnshroudedClient::world_cursor_capture_offset)
        : context_callback ? context_callback_code(callback) : callback_code(callback);
    payload.insert(payload.end(), original, original + original_size);
    payload.push_back(0xe9);
    const auto return_offset = payload.size();
    payload.insert(payload.end(), 4, 0);
    auto allocation = allocate_near(target, payload.size());
    if (!allocation) return false;
    std::int32_t return_jump{};
    if (!relative(reinterpret_cast<std::uintptr_t>(allocation) + return_offset - 1,
        target + original_size, return_jump)) { VirtualFree(allocation, 0, MEM_RELEASE); return false; }
    std::memcpy(payload.data() + return_offset, &return_jump, sizeof(return_jump));
    std::memcpy(allocation, payload.data(), payload.size());
    FlushInstructionCache(GetCurrentProcess(), allocation, payload.size());
    std::vector<std::uint8_t> detour(original_size, 0x90);
    detour[0] = 0xe9;
    std::int32_t branch{};
    if (!relative(target, reinterpret_cast<std::uintptr_t>(allocation), branch)) {
        VirtualFree(allocation, 0, MEM_RELEASE);
        return false;
    }
    std::memcpy(detour.data() + 1, &branch, sizeof(branch));
    if (!write_code(target, detour.data(), detour.size())) {
        VirtualFree(allocation, 0, MEM_RELEASE);
        return false;
    }
    hooks.push_back(InstalledHook{target, allocation,
        std::vector<std::uint8_t>(original, original + original_size)});
    return true;
}
}

namespace GameThreadDispatcher {
bool Initialize() {
    if (!hooks.empty()) return accepting.load(std::memory_order_acquire);
    // Published trampolines and their callbacks remain mapped until process exit.
    HMODULE pinned{};
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            reinterpret_cast<LPCWSTR>(&Initialize), &pinned)) return false;
    std::uint8_t* base{};
    std::size_t image_size{};
    if (!supported_image(base, image_size)) return false;
    hooks.clear();
    // Drain on the actor-world update entry, which continues while the local
    // actor is active. The prop-system callback is event-driven and can stop
    // being called for long stretches, leaving queued ECS work permanently stale.
    if (!install_hook(base, KfcRuntimeCompatibility::EnshroudedClient::game_thread_signature,
            KfcRuntimeCompatibility::EnshroudedClient::game_thread_original.data(),
            KfcRuntimeCompatibility::EnshroudedClient::game_thread_original.size(),
            reinterpret_cast<void*>(&drain)) ||
        !install_hook(base, KfcRuntimeCompatibility::EnshroudedClient::entity_manager_signature,
            KfcRuntimeCompatibility::EnshroudedClient::entity_manager_original.data(),
            KfcRuntimeCompatibility::EnshroudedClient::entity_manager_original.size(),
            reinterpret_cast<void*>(&capture_entity_manager))) {
        Shutdown();
        return false;
    }
    const bool prop_hook = install_hook(base, KfcRuntimeCompatibility::EnshroudedClient::world_prop_update_signature,
        KfcRuntimeCompatibility::EnshroudedClient::world_prop_update_original.data(),
        KfcRuntimeCompatibility::EnshroudedClient::world_prop_update_original.size(),
        reinterpret_cast<void*>(&WorldRuntime::OnPropUpdate));
    const bool placement_hook = install_hook(base, KfcRuntimeCompatibility::EnshroudedClient::world_actor_placement_signature,
        KfcRuntimeCompatibility::EnshroudedClient::world_actor_placement_original.data(),
        KfcRuntimeCompatibility::EnshroudedClient::world_actor_placement_original.size(),
        reinterpret_cast<void*>(&WorldRuntime::OnActorPlacement));
    const bool building_dispatch_hook = !KfcRuntimeCompatibility::EnshroudedClient::world_building_dispatch_signature.empty() &&
        install_hook(base, KfcRuntimeCompatibility::EnshroudedClient::world_building_dispatch_signature,
            KfcRuntimeCompatibility::EnshroudedClient::world_building_dispatch_original.data(),
            KfcRuntimeCompatibility::EnshroudedClient::world_building_dispatch_original.size(),
            reinterpret_cast<void*>(&WorldRuntime::OnBuildingDispatch), false, true);
    entity_context_hooks_ready.store(prop_hook && placement_hook && building_dispatch_hook, std::memory_order_release);
    const bool cursor_hook = install_hook(base, KfcRuntimeCompatibility::EnshroudedClient::world_cursor_signature,
        KfcRuntimeCompatibility::EnshroudedClient::world_cursor_original.data(),
        KfcRuntimeCompatibility::EnshroudedClient::world_cursor_original.size(),
        reinterpret_cast<void*>(&WorldRuntime::OnCursorUpdate), true);
    WorldRuntime::SetCursorHookReady(cursor_hook);
    accepting.store(true, std::memory_order_release);
    PatchRuntime::Initialize();
    return true;
}

void Shutdown() {
    PatchRuntime::Shutdown();
    accepting.store(false, std::memory_order_release);
    entity_context_hooks_ready.store(false, std::memory_order_release);
    WorldRuntime::SetCursorHookReady(false);
    for (auto iterator = hooks.rbegin(); iterator != hooks.rend(); ++iterator) {
        if (iterator->target && iterator->original.size() &&
            write_code(iterator->target, iterator->original.data(), iterator->original.size()))
            iterator->target = 0;
        // A thread may still have a trampoline return address on its stack.
        // Retain the small published allocation until process exit.
    }
    std::erase_if(hooks, [](const InstalledHook& hook) { return !hook.target; });
    engine_thread.store(0, std::memory_order_release);
    entity_manager.store(0, std::memory_order_release);
    command_observed.store(false, std::memory_order_release);
    std::vector<std::shared_ptr<Job>> cancelled;
    {
        std::scoped_lock lock(queue_mutex);
        cancelled.swap(queue);
    }
    for (const auto& job : cancelled) SetEvent(job->complete);
}

bool Ready() {
    const auto drain = last_drain_ms.load(std::memory_order_acquire);
    return accepting.load(std::memory_order_acquire) &&
        engine_thread.load(std::memory_order_acquire) && drain &&
        GetTickCount64() - drain <= 500;
}
bool EntityContextReady() {
    return entity_context_hooks_ready.load(std::memory_order_acquire) && Ready();
}
std::uint32_t ThreadId() { return engine_thread.load(std::memory_order_acquire); }
std::uintptr_t EntityManager() { return entity_manager.load(std::memory_order_acquire); }
std::string Status() {
    if (!accepting.load(std::memory_order_acquire)) return "unavailable";
    const auto thread = engine_thread.load(std::memory_order_acquire);
    if (!thread || !Ready()) {
        const auto drain = last_drain_ms.load(std::memory_order_acquire);
        if (drain && GetTickCount64() - drain > 500) {
            std::size_t queued{};
            { std::scoped_lock lock(queue_mutex); queued = queue.size(); }
            return "stale-game-thread(thread=" + std::to_string(thread) +
                ",drain=over-500ms,queued=" + std::to_string(queued) +
                ",completed=" + std::to_string(completed_count.load(std::memory_order_acquire)) +
                ",timeouts=" + std::to_string(timeout_count.load(std::memory_order_acquire)) +
                ",rejected=" + std::to_string(rejected_count.load(std::memory_order_acquire)) + ')';
        }
        return "installed-awaiting-world";
    }
    const auto manager = entity_manager.load(std::memory_order_acquire);
    return "ready(thread=" + std::to_string(thread) + ",manager=" +
        (manager ? "ready" : "awaiting-lookup") + ",commands=" +
        (command_observed.load(std::memory_order_acquire) ? "executed" : "waiting") + ')';
}

bool Invoke(Operation operation, std::shared_ptr<void> context, std::uint32_t timeout_ms) {
    if (!operation || !accepting.load(std::memory_order_acquire)) return false;
    if (engine_thread.load(std::memory_order_acquire) == GetCurrentThreadId()) {
        operation(context.get());
        return true;
    }
    const auto job = std::make_shared<Job>();
    if (!job->complete) return false;
    job->operation = operation;
    job->context = std::move(context);
    {
        std::scoped_lock lock(queue_mutex);
        if (!accepting.load(std::memory_order_relaxed) || queue.size() >= 128) {
            rejected_count.fetch_add(1, std::memory_order_relaxed);
            return false;
        }
        queue.push_back(job);
    }
    if (WaitForSingleObject(job->complete, timeout_ms) == WAIT_OBJECT_0)
        return job->executed.load(std::memory_order_acquire);
    timeout_count.fetch_add(1, std::memory_order_relaxed);
    unsigned queued = 0;
    if (job->state.compare_exchange_strong(queued, 3, std::memory_order_acq_rel)) {
        std::scoped_lock lock(queue_mutex);
        std::erase(queue, job);
        return false;
    }
    // An executing operation retains its own buffers. The caller must not
    // inspect those buffers after a timeout or assume a write was cancelled.
    return false;
}

std::string Diagnostics() {
    const auto last = last_drain_ms.load(std::memory_order_relaxed);
    const auto manager_last = last_manager_observation_ms.load(std::memory_order_relaxed);
    std::unique_lock lock(queue_mutex, std::try_to_lock);
    return nlohmann::json({
        {"accepting", accepting.load()}, {"threadId", engine_thread.load()},
        {"ready", Ready()},
        {"entityContextHooksReady", EntityContextReady()},
        {"entityManagerObserved", entity_manager.load() != 0},
        {"entityManagerChanges", manager_changes.load()},
        {"lastManagerObservationAgeMs", manager_last ? nlohmann::json(GetTickCount64() - manager_last) : nlohmann::json(nullptr)},
        {"lastOperationUs", last_operation_us.load()}, {"maximumOperationUs", maximum_operation_us.load()},
        {"lastDrainAgeMs", last ? nlohmann::json(GetTickCount64() - last) : nlohmann::json(nullptr)},
        {"drainCount", drain_count.load()}, {"completed", completed_count.load()},
        {"timeouts", timeout_count.load()}, {"rejected", rejected_count.load()},
        {"queueDepth", lock ? nlohmann::json(queue.size()) : nlohmann::json(nullptr)},
        {"queueLimit",128}, {"batchLimit",8}, {"batchBudgetMs",2}
    }).dump();
}
bool WriteCode(std::uintptr_t address, const void* expected, const void* replacement, std::size_t size) {
    return address && expected && replacement && size && write_code(address, replacement, size, expected);
}
}
