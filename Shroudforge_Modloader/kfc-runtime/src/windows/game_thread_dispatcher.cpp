#include "game_thread_dispatcher.h"

#include "../../../../Shroudforge_Compatibility/windows/enshrouded_client_1076226.h"

#include <windows.h>
#include <algorithm>
#include <array>
#include <atomic>
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
    void* context{};
    HANDLE complete{CreateEventW(nullptr, TRUE, FALSE, nullptr)};
    std::atomic<bool> executed{};
    std::atomic<unsigned> state{}; // 0 queued, 1 executing, 2 complete, 3 cancelled
    ~Job() { if (complete) CloseHandle(complete); }
};

std::mutex queue_mutex;
std::vector<std::shared_ptr<Job>> queue;
std::atomic<bool> accepting{};
std::atomic<std::uint32_t> engine_thread{};
std::atomic<std::uintptr_t> entity_manager{};
std::atomic<bool> command_observed{};
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
        nt->FileHeader.TimeDateStamp != ShroudforgeCompatibility::EnshroudedClient::image_timestamp ||
        nt->OptionalHeader.SizeOfImage != ShroudforgeCompatibility::EnshroudedClient::image_size) return false;
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

bool write_code(std::uintptr_t address, const void* bytes, std::size_t size) {
    DWORD previous{};
    if (!VirtualProtect(reinterpret_cast<void*>(address), size, PAGE_EXECUTE_READWRITE, &previous)) return false;
    std::memcpy(reinterpret_cast<void*>(address), bytes, size);
    FlushInstructionCache(GetCurrentProcess(), reinterpret_cast<void*>(address), size);
    DWORD ignored{};
    return VirtualProtect(reinterpret_cast<void*>(address), size, previous, &ignored) != FALSE;
}

void __cdecl drain(void*, void*) {
    engine_thread.store(GetCurrentThreadId(), std::memory_order_release);
    std::vector<std::shared_ptr<Job>> work;
    {
        std::scoped_lock lock(queue_mutex);
        work.swap(queue);
    }
    for (const auto& job : work) {
        unsigned queued = 0;
        if (job->state.compare_exchange_strong(queued, 1, std::memory_order_acq_rel) && job->operation) {
            job->operation(job->context);
            job->executed.store(true, std::memory_order_release);
            job->state.store(2, std::memory_order_release);
            command_observed.store(true, std::memory_order_release);
        }
        SetEvent(job->complete);
    }
}

void __cdecl capture_entity_manager(void* lookup_context, void*) {
    if (!lookup_context) return;
    __try {
        const auto root = *static_cast<std::uintptr_t*>(lookup_context);
        if (!root) return;
        const auto manager = *reinterpret_cast<std::uintptr_t*>(root + 0x30);
        if (manager) entity_manager.store(manager, std::memory_order_release);
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


bool install_hook(std::uint8_t* base, std::string_view signature,
                  const std::uint8_t* original, std::size_t original_size, void* callback) {
    if (!original || original_size < 5) return false;
    const auto target = find_unique_executable_signature(base, signature);
    if (!target || std::memcmp(reinterpret_cast<void*>(target), original, original_size)) return false;
    auto payload = callback_code(callback);
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
    std::uint8_t* base{};
    std::size_t image_size{};
    if (!supported_image(base, image_size)) return false;
    hooks.clear();
    if (!install_hook(base, ShroudforgeCompatibility::EnshroudedClient::game_thread_signature,
            ShroudforgeCompatibility::EnshroudedClient::game_thread_original.data(),
            ShroudforgeCompatibility::EnshroudedClient::game_thread_original.size(),
            reinterpret_cast<void*>(&drain)) ||
        !install_hook(base, ShroudforgeCompatibility::EnshroudedClient::entity_manager_signature,
            ShroudforgeCompatibility::EnshroudedClient::entity_manager_original.data(),
            ShroudforgeCompatibility::EnshroudedClient::entity_manager_original.size(),
            reinterpret_cast<void*>(&capture_entity_manager))) {
        Shutdown();
        return false;
    }
    accepting.store(true, std::memory_order_release);
    return true;
}

void Shutdown() {
    accepting.store(false, std::memory_order_release);
    for (auto iterator = hooks.rbegin(); iterator != hooks.rend(); ++iterator) {
        if (iterator->target && iterator->original.size())
            write_code(iterator->target, iterator->original.data(), iterator->original.size());
        if (iterator->payload) VirtualFree(iterator->payload, 0, MEM_RELEASE);
    }
    hooks.clear();
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

bool Ready() { return accepting.load(std::memory_order_acquire) && engine_thread.load(std::memory_order_acquire); }
std::uint32_t ThreadId() { return engine_thread.load(std::memory_order_acquire); }
std::uintptr_t EntityManager() { return entity_manager.load(std::memory_order_acquire); }
std::string Status() {
    if (!accepting.load(std::memory_order_acquire)) return "unavailable";
    const auto thread = engine_thread.load(std::memory_order_acquire);
    if (!thread) return "installed-awaiting-world";
    const auto manager = entity_manager.load(std::memory_order_acquire);
    return "ready(thread=" + std::to_string(thread) + ",manager=" +
        (manager ? "ready" : "awaiting-lookup") + ",commands=" +
        (command_observed.load(std::memory_order_acquire) ? "executed" : "waiting") + ')';
}

bool Invoke(Operation operation, void* context, std::uint32_t timeout_ms) {
    if (!operation || !accepting.load(std::memory_order_acquire)) return false;
    if (engine_thread.load(std::memory_order_acquire) == GetCurrentThreadId()) {
        operation(context);
        return true;
    }
    const auto job = std::make_shared<Job>();
    if (!job->complete) return false;
    job->operation = operation;
    job->context = context;
    {
        std::scoped_lock lock(queue_mutex);
        if (!accepting.load(std::memory_order_relaxed)) return false;
        queue.push_back(job);
    }
    if (WaitForSingleObject(job->complete, timeout_ms) == WAIT_OBJECT_0)
        return job->executed.load(std::memory_order_acquire);
    unsigned queued = 0;
    if (job->state.compare_exchange_strong(queued, 3, std::memory_order_acq_rel)) {
        std::scoped_lock lock(queue_mutex);
        std::erase(queue, job);
        return false;
    }
    // The engine thread already owns the command. Keep the caller's context
    // alive until completion; returning now would create a use-after-return.
    WaitForSingleObject(job->complete, INFINITE);
    return job->executed.load(std::memory_order_acquire);
}
}
