#include "world_runtime.h"
#include "building_input.h"
#include "profile.h"
#include "dispatcher.h"
#include <windows.h>
#include <algorithm>
#include <array>
#include <cstring>
#include <memory>
#include <mutex>
#include <atomic>
#include <cmath>
#include <condition_variable>
#include <cstdint>
#include <vector>
#include <unordered_set>
#include <nlohmann/json.hpp>

namespace WorldRuntime {
namespace {
constexpr std::size_t MaximumCells = 65'536;
constexpr std::size_t NativeCursorSize = 0xa0;
constexpr std::uint32_t OperationTimeoutMs = 3000;
struct Cell { std::uint8_t material{}, density{}; };
static_assert(sizeof(Cell) == 2);
struct CellSpan { Cell* data{}; std::uint64_t size{}; };
using NativeRead = bool (__fastcall*)(CellSpan*, const std::uint32_t*, const void*, std::uint32_t, const std::int32_t*);
using NativeWrite = void (__fastcall*)(void*, CellSpan*, const std::uint32_t*, const std::int32_t*);

struct Operation {
    enum class Kind { Read, Write } kind{};
    std::array<std::int32_t, 3> origin{};
    std::array<std::uint32_t, 3> dimensions{};
    std::vector<Cell> cells;
    bool result{};
    bool write_attempted{};
    bool rollback_verified{};
    std::uint64_t context_generation{};
};

struct EngineTransform {
    std::int64_t position[3]{};
    float rotation[4]{};
    float scale[3]{1,1,1};
    std::uint32_t padding{};
};
static_assert(sizeof(EngineTransform) == 0x38);
struct alignas(16) PlacementBounds { float minimum[4]{}; float maximum[4]{}; };
static_assert(sizeof(PlacementBounds) == 32 && offsetof(PlacementBounds, maximum) == 16);
struct EntityRequest {
    enum class Kind { Spawn, Place, Destroy, Finish } kind{};
    std::uint64_t template_uuid[2]{};
    double position[3]{}, rotation[4]{0,0,0,1};
    float scale[3]{1,1,1};
    float bounds[6]{};
    EngineTransform exact_transform{};
    std::uint32_t tracking{}, feedback{}, flags{}, event_id{};
    bool complete{};
    bool has_exact_transform{};
    HANDLE completed{CreateEventW(nullptr, TRUE, FALSE, nullptr)};
    std::atomic<bool> done{}, success{};
    std::atomic<bool> cancelled{};
    std::uint32_t queue_token{};
    ~EntityRequest() { if (completed) CloseHandle(completed); }
};
// Prop deletion follows Shroudtopia's synchronous WorldApi path. The entity
// placement callback owns this POD request until it publishes phase 3; unlike
// the generic request/event path it cannot be cancelled after the game hook
// has started consuming it.
struct RemovalRequest {
    EngineTransform transform{};
    PlacementBounds bounds{};
    std::uint32_t tracking{}, feedback{};
    volatile long phase{}; // 0 idle, 1 queued, 2 consumed, 3 complete
    bool success{};
    std::uint32_t thread_id{};
    std::uintptr_t actor_world{}, remove_queue{};
    std::uint32_t owner{};
};
std::atomic<std::shared_ptr<EntityRequest>> pending_entity_request;
RemovalRequest pending_removal;
std::mutex removal_mutex;
thread_local bool inside_building_dispatch{};
std::atomic<std::uintptr_t> observed_actor_world{};
std::atomic<std::uint64_t> observed_actor_world_ms{};
// Pin the actual editable grid world after a successful read/write, matching
// Shroudtopia. The actor-placement hook also sees transient preview worlds.
std::atomic<std::uintptr_t> preferred_voxel_world{};
std::atomic<std::uint64_t> actor_placement_hook_hits{}, actor_placement_request_hits{};
std::atomic<std::uint64_t> building_dispatch_hook_hits{}, building_dispatch_request_hits{};
std::atomic<std::uint64_t> entity_dispatch_completions{};
std::atomic<std::uint64_t> prop_update_request_hits{};
std::atomic<std::uintptr_t> observed_execution_view{};
std::atomic<std::uint64_t> prop_update_hook_hits{}, cursor_hook_hits{}, context_resets{};
std::atomic<std::uintptr_t> local_execution_root{};
std::atomic<std::uintptr_t> client_read_world{}, client_read_manager{};
std::atomic<std::uint64_t> client_read_ms{}, client_read_generation{};
enum class ActorProbe { Never, MissingFrame, MissingLayout, MissingService, InvalidWorld, Valid };
std::atomic<ActorProbe> actor_probe{ActorProbe::Never};
struct CursorMailbox {
    volatile long lock{};
    volatile long hook_ready{};
    std::uint64_t sequence{};
    std::uint64_t context_generation{};
    std::array<std::uint8_t, NativeCursorSize> bytes{};
};
CursorMailbox cursor_mailbox;

bool read_memory(std::uintptr_t address, void* output, std::size_t size) {
    SIZE_T read{};
    return address && output && size && ReadProcessMemory(GetCurrentProcess(),
        reinterpret_cast<const void*>(address), output, size, &read) && read == size;
}

bool copy_cursor_safely(const void* source, std::uint8_t* output) {
    if (!source || !output) return false;
    __try { std::memcpy(output, source, NativeCursorSize); return true; }
    __except(EXCEPTION_EXECUTE_HANDLER) { return false; }
}

const KfcRuntimeCompatibility::EnshroudedClient::RuntimeOperation* find_operation(const char* name) {
    if (!name) return nullptr;
    const auto& operations = KfcRuntimeCompatibility::EnshroudedClient::runtime_operations;
    const auto found = std::find_if(operations.begin(), operations.end(), [name](const auto& op) {
        return op.name == name;
    });
    return found == operations.end() ? nullptr : &*found;
}

bool valid_world_context(std::uintptr_t world) {
    const auto* profile = find_operation("runtime.world.context.active");
    std::uintptr_t store{};
    return world && profile && profile->available && profile->validation_offset &&
        read_memory(world + profile->validation_offset, &store, sizeof(store)) && store;
}

bool resolve_voxel_world(std::uintptr_t& world) {
    const auto* profile = find_operation("runtime.world.context.active");
    const auto base = reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
    std::uintptr_t singleton{}, context{};
    if (!profile || !profile->available) return false;
    // Dedicated servers have no client singleton. Their context must come
    // from a recently observed engine execution frame on this world.
    if (profile->actor_context) {
        if (GetTickCount64() - observed_actor_world_ms.load(std::memory_order_acquire) > 500) return false;
        const auto current = observed_actor_world.load(std::memory_order_acquire);
        if (!valid_world_context(current)) return false;
        world = current;
        return true;
    }

    const auto preferred = preferred_voxel_world.load(std::memory_order_acquire);
    if (valid_world_context(preferred)) { world = preferred; return true; }
    if (preferred) preferred_voxel_world.store(0, std::memory_order_release);

    // Prefer the validated actor-world context captured from ShroudForge's
    // build-verified actor hooks. The singleton is the fallback when the
    // current actor frame has not published a usable world yet.
    const auto actor_candidate = observed_actor_world.load(std::memory_order_acquire);
    if (valid_world_context(actor_candidate)) { world = actor_candidate; return true; }

    if (profile->global_rva && profile->context_pointer_offset && profile->world_offset &&
        read_memory(base + profile->global_rva, &singleton, sizeof(singleton)) && singleton) {
        const auto context_slot = static_cast<std::uintptr_t>(static_cast<std::intptr_t>(singleton) + profile->context_pointer_offset);
        if (read_memory(context_slot, &context, sizeof(context)) && context) {
            const auto signed_world = static_cast<std::intptr_t>(context) + profile->world_offset;
            if (signed_world > 0) {
                const auto candidate = static_cast<std::uintptr_t>(signed_world);
                if (valid_world_context(candidate)) { world = candidate; return true; }
            }
        }
    }
    return false;
}

bool resolve_client_read_world(std::uintptr_t& world) {
    const auto generation = context_resets.load(std::memory_order_acquire);
    const auto observed = client_read_ms.load(std::memory_order_acquire);
    const auto candidate = client_read_world.load(std::memory_order_acquire);
    if (!observed || GetTickCount64() - observed > 500 ||
        client_read_generation.load(std::memory_order_acquire) != generation ||
        client_read_manager.load(std::memory_order_acquire) != GameThreadDispatcher::EntityManager() ||
        !valid_world_context(candidate) || context_resets.load(std::memory_order_acquire) != generation) return false;
    world = candidate;
    return true;
}

void observe_client_read_world(void* cursor_frame, void* execution_view) {
    const auto& layout = KfcRuntimeCompatibility::EnshroudedClient::world_context_layout;
    if (!cursor_frame || !layout.client_cursor_service_view || !layout.client_cursor_service_world) return;
    std::uintptr_t service{}, world{}, root{}, frame_manager{};
    std::uint64_t count{};
    const auto manager = GameThreadDispatcher::EntityManager();
    if (!manager || !read_memory(reinterpret_cast<std::uintptr_t>(execution_view), &root, sizeof(root)) || !root ||
        !read_memory(root + KfcRuntimeCompatibility::EnshroudedClient::lookup_manager, &frame_manager, sizeof(frame_manager)) || frame_manager != manager ||
        !read_memory(manager + KfcRuntimeCompatibility::EnshroudedClient::entity_manager_count, &count, sizeof(count)) || !count || count > (1u<<20) ||
        !read_memory(reinterpret_cast<std::uintptr_t>(cursor_frame) + layout.client_cursor_service_view, &service, sizeof(service)) ||
        !read_memory(service + layout.client_cursor_service_world, &world, sizeof(world)) || !valid_world_context(world)) return;
    client_read_world.store(world, std::memory_order_release);
    client_read_manager.store(manager, std::memory_order_release);
    client_read_generation.store(context_resets.load(std::memory_order_acquire), std::memory_order_release);
    client_read_ms.store(GetTickCount64(), std::memory_order_release);
}

void observe_actor_world(void* actor_frame) {
    if (!actor_frame) { actor_probe.store(ActorProbe::MissingFrame); return; }
    const auto& layout = KfcRuntimeCompatibility::EnshroudedClient::world_context_layout;
    if (!layout.actor_frame_service_view || !layout.service_view_world) {
        actor_probe.store(ActorProbe::MissingLayout); return;
    }
    const auto frame = reinterpret_cast<std::uintptr_t>(actor_frame);
    std::uintptr_t service_view{}, candidate{};
    if (!read_memory(frame + layout.actor_frame_service_view, &service_view, sizeof(service_view)) || !service_view) {
        actor_probe.store(ActorProbe::MissingService); return;
    }
    if (!read_memory(service_view + layout.service_view_world, &candidate, sizeof(candidate)) ||
        !valid_world_context(candidate)) { actor_probe.store(ActorProbe::InvalidWorld); return; }
    actor_probe.store(ActorProbe::Valid);
    observed_actor_world.store(candidate, std::memory_order_release);
    observed_actor_world_ms.store(GetTickCount64(), std::memory_order_release);
}

bool actor_frame_matches_active_world(void* actor_frame) {
    if (!actor_frame) return false;
    const auto& layout = KfcRuntimeCompatibility::EnshroudedClient::world_context_layout;
    std::uintptr_t service_view{}, actor_world{}, active_world{};
    const auto frame = reinterpret_cast<std::uintptr_t>(actor_frame);
    if (!layout.actor_frame_service_view || !layout.service_view_world ||
        !read_memory(frame + layout.actor_frame_service_view, &service_view, sizeof(service_view)) || !service_view ||
        !read_memory(service_view + layout.service_view_world, &actor_world, sizeof(actor_world)) || !actor_world ||
        !resolve_voxel_world(active_world)) return false;
    return actor_world == active_world;
}

bool safe_read(NativeRead function, CellSpan* span, const std::uint32_t* dimensions,
               const void* world, std::uint32_t mode, const std::int32_t* origin) {
    __try { return function(span, dimensions, world, mode, origin); }
    __except(EXCEPTION_EXECUTE_HANDLER) { return false; }
}

bool safe_write(NativeWrite function, void* world, CellSpan* span,
                const std::uint32_t* dimensions, const std::int32_t* origin) {
    __try { function(world, span, dimensions, origin); return true; }
    __except(EXCEPTION_EXECUTE_HANDLER) { return false; }
}

using NativeCreate = std::uint32_t (__fastcall*)(void*, const std::uint64_t*, const float*, const float*,
                                                  std::uint32_t, std::uint32_t, const std::uint64_t*);
using NativePlace = void (__fastcall*)(void*, const EngineTransform*, const float*,
                                       std::uint32_t material_feedback_id, std::uint32_t tracking_item_id);
using NativeDestroy = void (__fastcall*)(void*, const EngineTransform*, const float*, std::uint32_t);
using NativeFinish = void (__fastcall*)(void*, void*, std::uint32_t, bool);

bool safe_destroy(NativeDestroy function, void* context, const EngineTransform* transform,
                  const float* bounds, std::uint32_t feedback) {
    __try { function(context, transform, bounds, feedback); return true; }
    __except(EXCEPTION_EXECUTE_HANDLER) { return false; }
}

bool valid_transform(const EntityRequest& request, EngineTransform& transform) {
    for (int axis = 0; axis < 3; ++axis) {
        if (!std::isfinite(request.position[axis])) return false;
        const auto fixed = std::ldexp(request.position[axis], 32);
        if (!std::isfinite(fixed) || fixed < static_cast<double>(INT64_MIN) || fixed > static_cast<double>(INT64_MAX)) return false;
        transform.position[axis] = static_cast<std::int64_t>(std::llround(fixed));
    }
    long double norm{};
    for (int index = 0; index < 4; ++index) {
        if (!std::isfinite(request.rotation[index])) return false;
        norm += static_cast<long double>(request.rotation[index]) * request.rotation[index];
    }
    if (!std::isfinite(norm) || norm < 1e-12L) return false;
    const auto inverse_norm = 1.0L / std::sqrt(norm);
    for (int index = 0; index < 4; ++index)
        transform.rotation[index] = static_cast<float>(request.rotation[index] * inverse_norm);
    for (int axis = 0; axis < 3; ++axis) {
        if (!std::isfinite(request.scale[axis])) return false;
        transform.scale[axis] = request.scale[axis];
    }
    return true;
}

std::uintptr_t image_base() { return reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr)); }

bool native_finish(void* context, bool complete) {
    const auto* profile = find_operation("runtime.world.entity.finish_building");
    const auto event_rva = KfcRuntimeCompatibility::EnshroudedClient::world_finish_event_id_rva;
    std::uint32_t event_id{};
    const auto base = image_base();
    if (!context || !profile || !profile->available || !event_rva ||
        !read_memory(base + event_rva, &event_id, sizeof(event_id))) return false;
    // The original client and server call sites pass this value even when it
    // is zero. Zero is not a missing event/address sentinel.
    auto function = reinterpret_cast<NativeFinish>(base + profile->function_rva);
    __try { function(context, nullptr, event_id, complete); return true; }
    __except(EXCEPTION_EXECUTE_HANDLER) { return false; }
}

void complete_entity_request(const std::shared_ptr<EntityRequest>& request, bool success, std::uint32_t token = 0) {
    request->queue_token = token;
    request->success.store(success, std::memory_order_release);
    request->done.store(true, std::memory_order_release);
    if (request->completed) SetEvent(request->completed);
    auto expected = request;
    pending_entity_request.compare_exchange_strong(expected, {}, std::memory_order_acq_rel);
}

bool perform_spawn(const std::shared_ptr<EntityRequest>& request, void* execution_view) {
    const auto* profile = find_operation("runtime.world.entity.spawn");
    const auto base = image_base();
    if (!execution_view || !profile || !profile->available ||
        !(request->template_uuid[0] || request->template_uuid[1])) return false;
    EngineTransform transform{};
    if (!valid_transform(*request, transform)) return false;
    float position[4]{};
    for (int axis = 0; axis < 3; ++axis) {
        position[axis] = static_cast<float>(request->position[axis]);
        if (!std::isfinite(position[axis])) return false;
    }
    std::uintptr_t context[1]{reinterpret_cast<std::uintptr_t>(execution_view)};
    const std::uint64_t auxiliary[2]{};
    std::uint32_t token{};
    auto function = reinterpret_cast<NativeCreate>(base + profile->function_rva);
    __try { token = function(context, request->template_uuid, position, transform.rotation,
                             request->tracking, request->flags, auxiliary); }
    __except(EXCEPTION_EXECUTE_HANDLER) { return false; }
    request->queue_token = token;
    return token != 0;
}

void execute_entity_request(const std::shared_ptr<EntityRequest>& request, void* execution_view, void* actor_frame) {
    if (!request || request->cancelled.load(std::memory_order_acquire) || request->done.load(std::memory_order_acquire)) return;
    if (request->kind == EntityRequest::Kind::Spawn) {
        const auto ok = actor_frame_matches_active_world(actor_frame) && perform_spawn(request, execution_view);
        complete_entity_request(request, ok, request->queue_token);
        return;
    }
    const auto* place_profile = find_operation(request->kind == EntityRequest::Kind::Destroy
        ? "runtime.world.entity.destroy" : "runtime.world.entity.place");
    const auto* finish_profile = find_operation("runtime.world.entity.finish_building");
    const auto base = image_base();
    if (request->kind != EntityRequest::Kind::Finish && (!place_profile || !place_profile->available)) {
        complete_entity_request(request, false); return;
    }
    if (!finish_profile || !finish_profile->available || !actor_frame) { complete_entity_request(request, false); return; }
    const auto frame = reinterpret_cast<std::uintptr_t>(actor_frame);
    if (!actor_frame_matches_active_world(actor_frame)) { complete_entity_request(request, false); return; }
    const auto& layout = KfcRuntimeCompatibility::EnshroudedClient::world_context_layout;
    const auto native_address = frame + layout.placement_context;
    std::uintptr_t root{}, place_queue{}, remove_queue{}, publish_state{}, publish_commands{};
    std::uint32_t owner{};
    if (!read_memory(native_address, &root, sizeof(root)) || root != reinterpret_cast<std::uintptr_t>(execution_view) ||
        !read_memory(native_address + layout.place_queue, &place_queue, sizeof(place_queue)) ||
        !read_memory(native_address + layout.remove_queue, &remove_queue, sizeof(remove_queue)) ||
        !read_memory(native_address + layout.publish_state, &publish_state, sizeof(publish_state)) ||
        !read_memory(native_address + layout.publish_commands, &publish_commands, sizeof(publish_commands)) ||
        !read_memory(native_address + layout.owner, &owner, sizeof(owner)) || !owner || !publish_state || !publish_commands ||
        (request->kind == EntityRequest::Kind::Destroy ? !remove_queue : request->kind == EntityRequest::Kind::Place && !place_queue)) {
        complete_entity_request(request, false); return;
    }
    bool called = true;
    if (request->kind == EntityRequest::Kind::Finish) {
        called = native_finish(reinterpret_cast<void*>(native_address), request->complete);
    } else {
        EngineTransform transform{};
        if (request->has_exact_transform) transform = request->exact_transform;
        else if (!valid_transform(*request, transform)) { complete_entity_request(request, false); return; }
        PlacementBounds bounds{};
        std::copy_n(request->bounds, 3, bounds.minimum);
        std::copy_n(request->bounds + 3, 3, bounds.maximum);
        if (request->kind == EntityRequest::Kind::Place) {
            auto function = reinterpret_cast<NativePlace>(base + place_profile->function_rva);
            // Original code writes argument 4 to BuildingPlaceEvent.material
            // (MaterialFeedbackId), argument 5 to trackingItemId (ItemId).
            // The public Lua/C ABI keeps its existing tracking, feedback order.
            __try { function(reinterpret_cast<void*>(native_address), &transform, bounds.minimum, request->feedback, request->tracking); }
            __except(EXCEPTION_EXECUTE_HANDLER) { called = false; }
        } else {
            auto function = reinterpret_cast<NativeDestroy>(base + place_profile->function_rva);
            // Shroudtopia passes the recipe's materialFeedbackId here; the
            // fourth native argument is not the prop item/tracking ID.
            const auto was_inside_dispatch = inside_building_dispatch;
            inside_building_dispatch = true;
            called = safe_destroy(function, reinterpret_cast<void*>(native_address), &transform,
                bounds.minimum, request->feedback);
            if (called) called = native_finish(reinterpret_cast<void*>(native_address), true);
            inside_building_dispatch = was_inside_dispatch;
            complete_entity_request(request, called);
            return;
        }
        if (called) {
            if (request->kind != EntityRequest::Kind::Finish) inside_building_dispatch = true;
            called = native_finish(reinterpret_cast<void*>(native_address), request->kind == EntityRequest::Kind::Finish
                ? request->complete : true);
            inside_building_dispatch = false;
        }
    }
    complete_entity_request(request, called);
}

void execute_pending_removal(void* execution_view, void* actor_frame) {
    if (!actor_frame) return;
    const auto frame = reinterpret_cast<std::uintptr_t>(actor_frame);
    const auto& layout = KfcRuntimeCompatibility::EnshroudedClient::world_context_layout;
    std::uintptr_t service_view{}, actor_world{};
    std::uintptr_t native_address{}, root{}, remove_queue{}, publish_state{}, publish_commands{};
    std::uint32_t owner{};
    bool success = false;
    if (!read_memory(frame + layout.actor_frame_service_view, &service_view, sizeof(service_view)) || !service_view ||
        !read_memory(service_view + layout.service_view_world, &actor_world, sizeof(actor_world)) || !actor_world)
        return;
    // Match Shroudtopia's ordering: discard preview-world ticks before claiming
    // the request, so a later tick can consume it against the pinned grid world.
    const auto preferred = preferred_voxel_world.load(std::memory_order_acquire);
    if (preferred && actor_world != preferred) return;
    if (InterlockedCompareExchange(&pending_removal.phase, 2, 1) != 1) return;
    if (execution_view &&
        (native_address = frame + layout.placement_context) != 0 &&
        read_memory(native_address, &root, sizeof(root)) && root == reinterpret_cast<std::uintptr_t>(execution_view) &&
        read_memory(native_address + layout.remove_queue, &remove_queue, sizeof(remove_queue)) && remove_queue &&
        read_memory(native_address + layout.publish_state, &publish_state, sizeof(publish_state)) && publish_state &&
        read_memory(native_address + layout.publish_commands, &publish_commands, sizeof(publish_commands)) && publish_commands &&
        read_memory(native_address + layout.owner, &owner, sizeof(owner)) && owner) {
        const auto* operation = find_operation("runtime.world.entity.destroy");
        const auto* finish = find_operation("runtime.world.entity.finish_building");
        if (operation && operation->available && finish && finish->available) {
            const auto base = image_base();
            auto destroy = reinterpret_cast<NativeDestroy>(base + operation->function_rva);
            success = safe_destroy(destroy, reinterpret_cast<void*>(native_address), &pending_removal.transform,
                pending_removal.bounds.minimum, pending_removal.feedback);
            if (success) success = native_finish(reinterpret_cast<void*>(native_address), true);
        }
    }
    pending_removal.success = success;
    pending_removal.thread_id = GetCurrentThreadId();
    pending_removal.actor_world = actor_world;
    pending_removal.remove_queue = remove_queue;
    pending_removal.owner = owner;
    InterlockedExchange(&pending_removal.phase, 3);
}

bool queue_removal(const EngineTransform& transform, const float bounds[6],
                   std::uint32_t tracking, std::uint32_t feedback) {
    pending_removal.transform = transform;
    std::copy_n(bounds, 3, pending_removal.bounds.minimum);
    std::copy_n(bounds + 3, 3, pending_removal.bounds.maximum);
    pending_removal.tracking = tracking;
    pending_removal.feedback = feedback;
    pending_removal.success = false;
    pending_removal.thread_id = 0;
    pending_removal.actor_world = 0;
    pending_removal.remove_queue = 0;
    pending_removal.owner = 0;
    InterlockedExchange(&pending_removal.phase, 1);
    const auto deadline = GetTickCount64() + OperationTimeoutMs;
    for (;;) {
        const auto phase = InterlockedCompareExchange(&pending_removal.phase, 0, 0);
        if (phase == 3) {
            const auto success = pending_removal.success;
            return success;
        }
        // Match Shroudtopia: only cancel a request that no native hook has
        // consumed. Once phase 2 is published, let that synchronous operation
        // finish instead of freeing/cancelling state underneath the game.
        if (GetTickCount64() >= deadline &&
            InterlockedCompareExchange(&pending_removal.phase, 0, 1) == 1) return false;
        Sleep(1);
    }
}

bool invoke_entity_request(const std::shared_ptr<EntityRequest>& request, std::uint32_t* outcome) {
    if (!outcome) return false;
    *outcome = 1;
    if (!request->completed) return false;
    std::shared_ptr<EntityRequest> empty;
    if (!pending_entity_request.compare_exchange_strong(empty, request, std::memory_order_acq_rel)) return false;
    if (WaitForSingleObject(request->completed, OperationTimeoutMs) != WAIT_OBJECT_0) {
        request->cancelled.store(true, std::memory_order_release);
        auto expected = request;
        pending_entity_request.compare_exchange_strong(expected, {}, std::memory_order_acq_rel);
        *outcome = 3;
        return false;
    }
    if (request->success.load(std::memory_order_acquire)) { *outcome = 0; return true; }
    *outcome = 2;
    return false;
}

bool matching_prop_handles(const EntityRequest& request, std::vector<std::uint32_t>& handles);
bool count_spawn_matches(const EntityRequest& request, std::size_t& matches) {
    std::vector<std::uint32_t> handles;
    if (!matching_prop_handles(request, handles)) return false;
    matches = handles.size();
    return true;
}

bool verify_spawn(const EntityRequest& request, const std::unordered_set<std::uint32_t>& before_ids,
                  std::uint32_t& created_handle) {
    const auto deadline = GetTickCount64() + 2000;
    auto next_scan = GetTickCount64();
    do {
        const auto now = GetTickCount64();
        if (now >= next_scan) {
            std::vector<std::uint32_t> after_ids;
            if (EcsRuntime::SnapshotEntityIds(after_ids)) {
                for (const auto id : after_ids) {
                    if (before_ids.contains(id)) continue;
                    KfcRuntimePropRecord prop{};
                    if (!EcsRuntime::ResolvePropEntityId(id, &prop) || prop.item_id != request.tracking ||
                        prop.template_uuid[0] != request.template_uuid[0] ||
                        prop.template_uuid[1] != request.template_uuid[1]) continue;
                    bool position_matches = true;
                    for (int axis = 0; axis < 3; ++axis) {
                        const auto expected = std::ldexp(request.position[axis], 32);
                        if (!std::isfinite(expected) || std::abs(static_cast<long double>(prop.position[axis]) - expected) > (1LL << 24)) {
                            position_matches = false;
                            break;
                        }
                    }
                    if (!position_matches) continue;
                    created_handle = prop.entity_handle;
                    return true;
                }
            }
            next_scan = now + 50;
        }
        if (GetTickCount64() < deadline) Sleep(5);
    } while (GetTickCount64() < deadline);
    return false;
}

bool verify_destroy(const EntityRequest& request, std::size_t previous_matches) {
    const auto deadline = GetTickCount64() + 2000;
    do {
        std::size_t current_matches{};
        if (count_spawn_matches(request, current_matches) && current_matches < previous_matches) return true;
        if (GetTickCount64() < deadline) Sleep(10);
    } while (GetTickCount64() < deadline);
    return false;
}

void execute(void* opaque) {
    auto& op = *static_cast<Operation*>(opaque);
    std::uintptr_t world{};
    const bool direct_context = resolve_voxel_world(world);
    if (!direct_context && (op.kind != Operation::Kind::Read || !resolve_client_read_world(world))) return;
    if (op.kind == Operation::Kind::Read && op.context_generation != context_resets.load(std::memory_order_acquire)) return;
    const auto* read_profile = find_operation("runtime.world.voxel.read");
    const auto* write_profile = find_operation("runtime.world.voxel.write");
    const auto base = reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
    if (!read_profile || !read_profile->available) return;
    auto read_fn = reinterpret_cast<NativeRead>(base + read_profile->function_rva);
    CellSpan span{op.cells.data(), op.cells.size()};
    if (op.kind == Operation::Kind::Read) {
        op.result = safe_read(read_fn, &span, op.dimensions.data(), reinterpret_cast<void*>(world), read_profile->mode, op.origin.data());
        if (op.result && direct_context) {
            std::uintptr_t unset{};
            preferred_voxel_world.compare_exchange_strong(unset, world, std::memory_order_acq_rel);
        }
        return;
    }
    if (!write_profile || !write_profile->available) return;
    auto write_fn = reinterpret_cast<NativeWrite>(base + write_profile->function_rva);
    std::array<std::int32_t, 3> expanded_origin{};
    std::array<std::uint32_t, 3> expanded_dimensions{};
    std::size_t expanded_count = 1;
    std::array<std::size_t, 3> offset{};
    for (int axis = 0; axis < 3; ++axis) {
        const auto low = static_cast<std::int64_t>(op.origin[axis]);
        const auto high = low + op.dimensions[axis];
        auto chunk_low = low / 8;
        if (low < 0 && low % 8) --chunk_low;
        auto chunk_high = high / 8;
        if (high > 0 && high % 8) ++chunk_high;
        const auto aligned_low = chunk_low * 8;
        const auto aligned_high = chunk_high * 8;
        if (aligned_low < INT32_MIN || aligned_low > INT32_MAX || aligned_high <= aligned_low ||
            static_cast<std::uint64_t>(aligned_high - aligned_low) > MaximumCells / expanded_count) return;
        expanded_origin[axis] = static_cast<std::int32_t>(aligned_low);
        expanded_dimensions[axis] = static_cast<std::uint32_t>(aligned_high - aligned_low);
        offset[axis] = static_cast<std::size_t>(low - aligned_low);
        expanded_count *= expanded_dimensions[axis];
    }
    std::vector<Cell> expanded(expanded_count);
    CellSpan expanded_span{expanded.data(), expanded.size()};
    if (!safe_read(read_fn, &expanded_span, expanded_dimensions.data(), reinterpret_cast<void*>(world), read_profile->mode, expanded_origin.data())) return;
    auto original = expanded;
    for (std::size_t z = 0; z < op.dimensions[2]; ++z)
        for (std::size_t y = 0; y < op.dimensions[1]; ++y)
            for (std::size_t x = 0; x < op.dimensions[0]; ++x) {
                const auto source = (z * op.dimensions[1] + y) * op.dimensions[0] + x;
                const auto target = ((z + offset[2]) * expanded_dimensions[1] + y + offset[1]) * expanded_dimensions[0] + x + offset[0];
                expanded[target] = op.cells[source];
            }
    const auto restore_original = [&]() {
        CellSpan restore_span{original.data(), original.size()};
        if (!safe_write(write_fn, reinterpret_cast<void*>(world), &restore_span, expanded_dimensions.data(), expanded_origin.data())) return;
        std::vector<Cell> restored(expanded_count);
        CellSpan restored_span{restored.data(), restored.size()};
        op.rollback_verified = safe_read(read_fn, &restored_span, expanded_dimensions.data(), reinterpret_cast<void*>(world), read_profile->mode, expanded_origin.data()) &&
            std::memcmp(restored.data(), original.data(), original.size() * sizeof(Cell)) == 0;
    };
    op.result = false;
    op.write_attempted = true;
    if (!safe_write(write_fn, reinterpret_cast<void*>(world), &expanded_span, expanded_dimensions.data(), expanded_origin.data())) {
        restore_original();
        op.result = false;
        return;
    }
    std::vector<Cell> verify(expanded_count);
    CellSpan verify_span{verify.data(), verify.size()};
    if (!safe_read(read_fn, &verify_span, expanded_dimensions.data(), reinterpret_cast<void*>(world), read_profile->mode, expanded_origin.data()) ||
        std::memcmp(verify.data(), expanded.data(), expanded.size() * sizeof(Cell)) != 0) {
        restore_original();
        op.result = false;
        return;
    }
    op.result = true;
    std::uintptr_t unset{};
    preferred_voxel_world.compare_exchange_strong(unset, world, std::memory_order_acq_rel);
}

bool cell_count(const std::uint32_t dimensions[3], std::size_t& count) {
    if (!dimensions) return false;
    count = 1;
    for (int axis = 0; axis < 3; ++axis) {
        if (!dimensions[axis] || dimensions[axis] > MaximumCells / count) return false;
        count *= dimensions[axis];
    }
    return count <= MaximumCells;
}

bool get_grid_spec_native(const char* grid_id, KfcRuntimeGridSpec* spec) {
    using namespace KfcRuntimeCompatibility::EnshroudedClient;
    if (!grid_id || !spec || !OperationAvailable("runtime.world.voxel.read")) return false;
    const auto found = std::find_if(runtime_world_grids.begin(), runtime_world_grids.end(),
        [grid_id](const auto& grid) { return grid.id == grid_id; });
    if (found == runtime_world_grids.end() || found->id.size() >= sizeof(spec->id)) return false;
    *spec = {};
    std::memcpy(spec->id, found->id.c_str(), found->id.size() + 1);
    for (int axis = 0; axis < 3; ++axis) {
        spec->origin[axis] = found->origin[axis];
        spec->cell_size[axis] = found->cell_size[axis];
        spec->maximum[axis] = found->maximum[axis];
    }
    return true;
}

bool matching_prop_handles(const EntityRequest& request, std::vector<std::uint32_t>& handles) {
    constexpr double tolerance = 0.01;
    double bounds[6]{};
    for (int axis = 0; axis < 3; ++axis) {
        bounds[axis] = request.position[axis] - tolerance;
        bounds[axis + 3] = request.position[axis] + tolerance;
    }
    const auto count = KfcRuntimeWorldEntityQueryProps(bounds, 0.0, nullptr, 0);
    if (count == SIZE_MAX || count > 1'000'000) return false;
    std::vector<KfcRuntimePropRecord> props(count);
    if (count) {
        const auto actual = KfcRuntimeWorldEntityQueryProps(bounds, 0.0, props.data(), props.size());
        if (actual == SIZE_MAX || actual > props.size()) return false;
        props.resize(actual);
    }
    handles.clear();
    for (const auto& prop : props) {
        if (prop.item_id != request.tracking) continue;
        bool matches = true;
        for (int axis = 0; axis < 3; ++axis) {
            const auto expected = std::ldexp(request.position[axis], 32);
            if (!std::isfinite(expected) || std::abs(static_cast<long double>(prop.position[axis]) - expected) > (1LL << 24)) {
                matches = false;
                break;
            }
        }
        if (matches && prop.entity_handle) handles.push_back(prop.entity_handle);
    }
    return true;
}
}

bool GetGridSpec(const char* grid_id, KfcRuntimeGridSpec* spec) {
    return get_grid_spec_native(grid_id, spec);
}

void ResetContext() {
    BuildingInput::Reset();
    observed_actor_world_ms.store(0, std::memory_order_release);
    observed_actor_world.store(0, std::memory_order_release);
    preferred_voxel_world.store(0, std::memory_order_release);
    observed_execution_view.store(0, std::memory_order_release);
    local_execution_root.store(0, std::memory_order_release);
    actor_probe.store(ActorProbe::Never);
    context_resets.fetch_add(1, std::memory_order_relaxed);
    client_read_ms.store(0, std::memory_order_release);
    client_read_world.store(0, std::memory_order_release);
    client_read_manager.store(0, std::memory_order_release);
}

bool OperationAvailable(const char* name) {
    if (name && std::strcmp(name, "runtime.world.building.input") == 0)
        return BuildingInput::Available();
    if (name && std::strcmp(name, "runtime.world.cursor.get") == 0)
        return InterlockedCompareExchange(&cursor_mailbox.hook_ready, 0, 0) != 0;
    if (name && std::strcmp(name, "runtime.world.voxel.write") == 0) {
        std::uintptr_t world{};
        const auto* operation = find_operation(name);
        return operation && operation->available && resolve_voxel_world(world);
    }
    const auto* operation = find_operation(name);
    return operation && operation->available;
}

bool EntityContextReady() {
    std::uintptr_t world{};
    return GameThreadDispatcher::EntityContextReady() && resolve_voxel_world(world) &&
        OperationAvailable("runtime.world.entity.spawn") &&
        OperationAvailable("runtime.world.entity.place") &&
        OperationAvailable("runtime.world.entity.destroy") &&
        OperationAvailable("runtime.world.entity.finish_building");
}

void OnPropUpdate(void* execution_view, void* actor_frame) {
    prop_update_hook_hits.fetch_add(1, std::memory_order_relaxed);
    if (execution_view) observed_execution_view.store(reinterpret_cast<std::uintptr_t>(execution_view), std::memory_order_release);
    observe_actor_world(actor_frame);
    const auto request = pending_entity_request.load(std::memory_order_acquire);
    if (!request) return;
    const auto* context = find_operation("runtime.world.context.active");
    const bool server_actor_context = context && context->actor_context;
    if (request->kind != EntityRequest::Kind::Spawn &&
        !(server_actor_context && request->kind == EntityRequest::Kind::Destroy)) return;
    prop_update_request_hits.fetch_add(1, std::memory_order_relaxed);
    execute_entity_request(request, execution_view, actor_frame);
}

void OnActorPlacement(void* execution_view, void* actor_frame) {
    actor_placement_hook_hits.fetch_add(1, std::memory_order_relaxed);
    if (execution_view) observed_execution_view.store(reinterpret_cast<std::uintptr_t>(execution_view), std::memory_order_release);
    observe_actor_world(actor_frame);
    if (InterlockedCompareExchange(&pending_removal.phase, 0, 0) == 1) {
        actor_placement_request_hits.fetch_add(1, std::memory_order_relaxed);
        execute_pending_removal(execution_view, actor_frame);
        if (InterlockedCompareExchange(&pending_removal.phase, 0, 0) == 3)
            entity_dispatch_completions.fetch_add(1, std::memory_order_relaxed);
        return;
    }
    const auto request = pending_entity_request.load(std::memory_order_acquire);
    if (!request || request->kind == EntityRequest::Kind::Spawn) return;
    actor_placement_request_hits.fetch_add(1, std::memory_order_relaxed);
    execute_entity_request(request, execution_view, actor_frame);
    if (request->done.load(std::memory_order_acquire))
        entity_dispatch_completions.fetch_add(1, std::memory_order_relaxed);
}

void OnBuildingDispatch(void* placement_context) {
    building_dispatch_hook_hits.fetch_add(1, std::memory_order_relaxed);
    if (inside_building_dispatch || !placement_context) return;
    const auto request = pending_entity_request.load(std::memory_order_acquire);
    if (!request || request->kind != EntityRequest::Kind::Destroy ||
        request->cancelled.load(std::memory_order_acquire) || request->done.load(std::memory_order_acquire)) return;
    building_dispatch_request_hits.fetch_add(1, std::memory_order_relaxed);
    const auto* operation = find_operation("runtime.world.entity.destroy");
    const auto* finish = find_operation("runtime.world.entity.finish_building");
    if (!operation || !operation->available || !finish || !finish->available) {
        complete_entity_request(request, false);
        return;
    }
    const auto context = reinterpret_cast<std::uintptr_t>(placement_context);
    const auto& layout = KfcRuntimeCompatibility::EnshroudedClient::world_context_layout;
    std::uintptr_t root{}, remove_queue{}, publish_state{}, publish_commands{};
    std::uint32_t owner{};
    const auto active_execution_view = observed_execution_view.load(std::memory_order_acquire);
    if (!active_execution_view || !read_memory(context, &root, sizeof(root)) || root != active_execution_view ||
        !read_memory(context + layout.remove_queue, &remove_queue, sizeof(remove_queue)) || !remove_queue ||
        !read_memory(context + layout.publish_state, &publish_state, sizeof(publish_state)) || !publish_state ||
        !read_memory(context + layout.publish_commands, &publish_commands, sizeof(publish_commands)) || !publish_commands ||
        !read_memory(context + layout.owner, &owner, sizeof(owner)) || !owner) {
        complete_entity_request(request, false);
        return;
    }
    EngineTransform transform{};
    if (!valid_transform(*request, transform)) {
        complete_entity_request(request, false);
        return;
    }
    PlacementBounds bounds{};
    std::copy_n(request->bounds, 3, bounds.minimum);
    std::copy_n(request->bounds + 3, 3, bounds.maximum);
    auto function = reinterpret_cast<NativeDestroy>(image_base() + operation->function_rva);
    inside_building_dispatch = true;
    bool called = safe_destroy(function, placement_context, &transform, bounds.minimum, request->feedback);
    if (called) called = native_finish(placement_context, true);
    inside_building_dispatch = false;
    complete_entity_request(request, called);
    entity_dispatch_completions.fetch_add(1, std::memory_order_relaxed);
}

std::string EntityHookStatus() {
    return "actor-placement=" + std::to_string(actor_placement_hook_hits.load(std::memory_order_relaxed)) +
        "/" + std::to_string(actor_placement_request_hits.load(std::memory_order_relaxed)) +
        ",building-dispatch=" + std::to_string(building_dispatch_hook_hits.load(std::memory_order_relaxed)) +
        "/" + std::to_string(building_dispatch_request_hits.load(std::memory_order_relaxed)) +
        ",prop-update-requests=" + std::to_string(prop_update_request_hits.load(std::memory_order_relaxed)) +
        ",completed=" + std::to_string(entity_dispatch_completions.load(std::memory_order_relaxed)) +
        ",removal{phase=" + std::to_string(InterlockedCompareExchange(&pending_removal.phase, 0, 0)) +
        ",success=" + std::to_string(pending_removal.success ? 1 : 0) +
        ",thread=" + std::to_string(pending_removal.thread_id) +
        ",owner=" + std::to_string(pending_removal.owner) + '}';
}

void SetCursorHookReady(bool ready) {
    InterlockedExchange(&cursor_mailbox.hook_ready, ready ? 1 : 0);
}

void OnCursorUpdate(void* cursor, void* execution_view, void* cursor_frame) {
    std::array<std::uint8_t, NativeCursorSize> sample{};
    if (!copy_cursor_safely(cursor, sample.data()) || sample[0x98] > 1) return;
    for (const auto base : {std::size_t{0}, std::size_t{0x38}}) {
        for (const auto offset : {std::size_t{0x18}, std::size_t{0x1c}, std::size_t{0x20}, std::size_t{0x24},
                                  std::size_t{0x28}, std::size_t{0x2c}, std::size_t{0x30}}) {
            float value{};
            std::memcpy(&value, sample.data() + base + offset, sizeof(value));
            if (!std::isfinite(value)) return;
        }
    }
    // R13 is the live execution view passed to this engine system (verified
    // at the profiled cursor call site). The entity lookup hook need not run
    // again when entering a remote world; refresh through this recurring hook.
    GameThreadDispatcher::ObserveExecutionView(execution_view, true);
    observe_client_read_world(cursor_frame, execution_view);
    // Evidence only: this root belongs to client_cursor, unlike the two
    // player_building_place_prop callbacks. No unverified world offset is used.
    std::uintptr_t root{};
    if (read_memory(reinterpret_cast<std::uintptr_t>(execution_view), &root, sizeof(root)))
        local_execution_root.store(root, std::memory_order_release);
    cursor_hook_hits.fetch_add(1, std::memory_order_relaxed);
    BuildingInput::Observe(cursor, execution_view);
    if (InterlockedCompareExchange(&cursor_mailbox.lock, 1, 0) != 0) return;
    cursor_mailbox.bytes = sample;
    cursor_mailbox.context_generation = context_resets.load(std::memory_order_acquire);
    ++cursor_mailbox.sequence;
    InterlockedExchange(&cursor_mailbox.lock, 0);
}

bool ReadCursorSnapshot(std::uint8_t* bytes, std::size_t capacity, std::uint64_t* sequence) {
    if (!bytes || capacity < NativeCursorSize || !sequence ||
        !OperationAvailable("runtime.world.cursor.get")) return false;
    if (InterlockedCompareExchange(&cursor_mailbox.lock, 1, 0) != 0) return false;
    const auto current_sequence = cursor_mailbox.sequence;
    const auto sample = cursor_mailbox.bytes;
    const auto sample_generation = cursor_mailbox.context_generation;
    InterlockedExchange(&cursor_mailbox.lock, 0);
    if (!current_sequence || sample_generation != context_resets.load(std::memory_order_acquire)) return false;
    std::memcpy(bytes, sample.data(), sample.size());
    *sequence = current_sequence;
    return true;
}

bool ActiveContextAvailable() {
    std::uintptr_t world{};
    return GameThreadDispatcher::Ready() && (resolve_voxel_world(world) || resolve_client_read_world(world));
}

std::uint32_t ContextKind() {
    // This is a classification query, not a write/readiness assertion. The
    // Lua caller also requires a live ECS session; keeping this independent of
    // the dispatcher's short readiness window lets the client distinguish a
    // validated remote cursor-world from an unknown context during a brief
    // game-thread stall. Actual world operations retain their own readiness
    // and permission checks.
    std::uintptr_t world{};
    if (resolve_voxel_world(world)) return 1;
    if (resolve_client_read_world(world)) return 2;
    return 0;
}

std::string ContextDiagnostics() {
    const auto* profile = find_operation("runtime.world.context.active");
    std::uintptr_t world{}, singleton{}, context{}, candidate{};
    const bool direct = resolve_voxel_world(world);
    const bool client_read = !direct && resolve_client_read_world(world);
    const bool resolved = direct || client_read;
    const char* global_reason = "profile-unavailable";
    if (profile && profile->available) {
        global_reason = "not-configured";
        if (profile->global_rva && profile->context_pointer_offset && profile->world_offset) {
            const auto base = reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
            global_reason = "singleton-unreadable";
            if (read_memory(base + profile->global_rva, &singleton, sizeof(singleton))) {
                global_reason = "singleton-null";
                if (singleton) {
                    global_reason = "context-unreadable";
                    if (read_memory(singleton + profile->context_pointer_offset, &context, sizeof(context))) {
                        global_reason = "context-null";
                        if (context) {
                            const auto address = static_cast<std::intptr_t>(context) + profile->world_offset;
                            candidate = address > 0 ? static_cast<std::uintptr_t>(address) : 0;
                            global_reason = valid_world_context(candidate) ? "valid" : "voxel-store-invalid";
                        }
                    }
                }
            }
        }
    }
    const auto actor = observed_actor_world.load(std::memory_order_acquire);
    const auto preferred = preferred_voxel_world.load(std::memory_order_acquire);
    const auto observed = observed_actor_world_ms.load(std::memory_order_acquire);
    const char* actor_reason = "not-observed";
    switch (actor_probe.load()) {
    case ActorProbe::MissingFrame: actor_reason = "frame-null"; break;
    case ActorProbe::MissingLayout: actor_reason = "layout-unavailable"; break;
    case ActorProbe::MissingService: actor_reason = "service-view-unavailable"; break;
    case ActorProbe::InvalidWorld: actor_reason = "voxel-store-invalid"; break;
    case ActorProbe::Valid: actor_reason = "validated-at-observation"; break;
    default: break;
    }
    const char* source = !resolved ? "none" : client_read ? "client-cursor-read" : world == preferred ? "pinned-world" :
        world == actor ? "actor-frame" : "client-singleton";
    return nlohmann::json({{"revision", "world-context-session-20261009-a2"}, {"resolved", resolved}, {"source", source},
        {"directWorldAvailable", direct}, {"clientReadWorld", client_read_world.load()},
        {"processRole", KfcRuntimeCompatibility::EnshroudedClient::image_target == "enshrouded_server.exe" ? "dedicated-server" : "client"},
        // Process role and context source cannot identify a client's network mode.
        {"clientSessionMode", "unknown"}, {"resets", context_resets.load()},
        {"global", {{"reason", global_reason}, {"singleton", singleton}, {"context", context}, {"candidate", candidate}}},
        {"actor", {{"reason", actor_reason}, {"candidate", actor},
            {"ageMs", observed ? nlohmann::json(GetTickCount64() - observed) : nlohmann::json(nullptr)}}},
        {"hooks", {{"propUpdate", prop_update_hook_hits.load()}, {"actorPlacement", actor_placement_hook_hits.load()},
            {"cursor", cursor_hook_hits.load()}, {"localExecutionRoot", local_execution_root.load()}}}
    }).dump();
}

std::string ContextStatus() {
    const auto evidence = nlohmann::json::parse(ContextDiagnostics());
    return "revision=world-context-session-20261009-a2,source=" + evidence["source"].get<std::string>() +
        ",global=" + evidence["global"]["reason"].get<std::string>() +
        ",actor=" + evidence["actor"]["reason"].get<std::string>() +
        ",prop-updates=" + std::to_string(prop_update_hook_hits.load()) +
        ",cursor=" + std::to_string(cursor_hook_hits.load());
}

bool ReadVoxels(const std::int32_t origin[3], const std::uint32_t dimensions[3],
                std::uint16_t* values, std::size_t capacity, std::size_t* actual) {
    std::size_t count{};
    if (!origin || !values || !actual || !cell_count(dimensions, count) || count > capacity ||
        !OperationAvailable("runtime.world.voxel.read") || !GameThreadDispatcher::Ready()) return false;
    auto op = std::make_shared<Operation>();
    op->kind = Operation::Kind::Read;
    op->context_generation = context_resets.load(std::memory_order_acquire);
    std::copy_n(origin, 3, op->origin.begin());
    std::copy_n(dimensions, 3, op->dimensions.begin());
    op->cells.resize(count);
    if (!GameThreadDispatcher::Invoke(execute, op, 500) || !op->result) return false;
    for (std::size_t i = 0; i < count; ++i)
        values[i] = static_cast<std::uint16_t>(op->cells[i].material | (static_cast<std::uint16_t>(op->cells[i].density) << 8));
    *actual = count;
    return true;
}

bool WriteVoxels(const std::int32_t origin[3], const std::uint32_t dimensions[3],
                 const std::uint16_t* values, std::size_t count, std::uint32_t* outcome) {
    if (!outcome) return false;
    *outcome = 1; // rejected before writing
    std::size_t expected{};
    if (!origin || !values || !cell_count(dimensions, expected) || count != expected ||
        !OperationAvailable("runtime.world.voxel.read") ||
        !OperationAvailable("runtime.world.voxel.write") || !GameThreadDispatcher::Ready()) return false;
    auto op = std::make_shared<Operation>();
    op->kind = Operation::Kind::Write;
    std::copy_n(origin, 3, op->origin.begin());
    std::copy_n(dimensions, 3, op->dimensions.begin());
    op->cells.resize(count);
    for (std::size_t i = 0; i < count; ++i) {
        op->cells[i].material = static_cast<std::uint8_t>(values[i]);
        op->cells[i].density = static_cast<std::uint8_t>(values[i] >> 8);
    }
    if (!GameThreadDispatcher::Invoke(execute, op, 500)) {
        *outcome = 3; // the game-thread request may have started before timeout
        return false;
    }
    if (op->result) { *outcome = 0; return true; }
    *outcome = op->write_attempted ? (op->rollback_verified ? 2u : 3u) : 1u;
    return false;
}

bool SpawnEntity(const std::uint64_t template_uuid[2], const double position[3],
                 const double rotation[4], std::uint32_t tracking, std::uint32_t flags,
                 std::uint32_t* entity_handle, std::uint32_t* outcome) {
    if (!entity_handle || !template_uuid || !position || !rotation || !outcome || !tracking ||
        !OperationAvailable("runtime.world.entity.spawn")) return false;
    *entity_handle = 0;
    auto request = std::make_shared<EntityRequest>();
    request->kind = EntityRequest::Kind::Spawn;
    std::copy_n(template_uuid, 2, request->template_uuid);
    std::copy_n(position, 3, request->position);
    std::copy_n(rotation, 4, request->rotation);
    request->tracking = tracking;
    request->flags = flags;
    // Match Shroudtopia: snapshot existing entity IDs, dispatch creation in the
    // prop-update hook, then resolve only newly appeared IDs to live props.
    std::vector<std::uint32_t> id_snapshot;
    if (!EcsRuntime::SnapshotEntityIds(id_snapshot)) { *outcome = 1; return false; }
    const std::unordered_set<std::uint32_t> before_ids(id_snapshot.begin(), id_snapshot.end());
    const auto ok = invoke_entity_request(request, outcome);
    if (!ok) return false;
    if (verify_spawn(*request, before_ids, *entity_handle)) return true;
    *outcome = 4; // command ran, but no matching live ECS entity was observed
    return false;
}

bool PlaceEntity(const double position[3], const double rotation[4], const float bounds[6],
                 std::uint32_t tracking, std::uint32_t feedback, std::uint32_t* outcome) {
    if (!position || !rotation || !bounds || !tracking || !feedback || !outcome ||
        !OperationAvailable("runtime.world.entity.place") ||
        !OperationAvailable("runtime.world.entity.finish_building")) return false;
    for (int axis = 0; axis < 3; ++axis)
        if (!std::isfinite(bounds[axis]) || !std::isfinite(bounds[axis + 3]) || bounds[axis] > bounds[axis + 3]) return false;
    auto request = std::make_shared<EntityRequest>();
    request->kind = EntityRequest::Kind::Place;
    std::copy_n(position, 3, request->position);
    std::copy_n(rotation, 4, request->rotation);
    std::copy_n(bounds, 6, request->bounds);
    request->tracking = tracking;
    request->feedback = feedback;
    return invoke_entity_request(request, outcome);
}

bool DestroyEntity(const double position[3], const double rotation[4], const float bounds[6],
                   std::uint32_t tracking, std::uint32_t feedback, std::uint32_t* outcome) {
    if (!position || !rotation || !bounds || !tracking || !outcome ||
        !OperationAvailable("runtime.world.entity.destroy") ||
        !OperationAvailable("runtime.world.entity.finish_building")) return false;
    for (int axis = 0; axis < 3; ++axis)
        if (!std::isfinite(bounds[axis]) || !std::isfinite(bounds[axis + 3]) || bounds[axis] > bounds[axis + 3]) return false;
    auto request = std::make_shared<EntityRequest>();
    request->kind = EntityRequest::Kind::Destroy;
    std::copy_n(position, 3, request->position);
    std::copy_n(rotation, 4, request->rotation);
    std::copy_n(bounds, 6, request->bounds);
    request->tracking = tracking;
    request->feedback = feedback;
    std::size_t previous_matches{};
    if (!count_spawn_matches(*request, previous_matches) || !previous_matches) { *outcome = 1; return false; }
    if (!invoke_entity_request(request, outcome)) return false;
    if (verify_destroy(*request, previous_matches)) return true;
    *outcome = 4;
    return false;
}

bool DestroyEntityHandle(std::uint32_t entity_handle, std::uint32_t* outcome) {
    if (!entity_handle || !outcome) return false;
    KfcRuntimePropRecord prop{};
    if (!KfcRuntimeWorldEntityGetTransform(entity_handle, &prop)) {
        *outcome = 1;
        return false;
    }
    KfcRuntimePropRecipe recipe{};
    if (!KfcRuntimeWorldEntityGetPropRecipe(prop.item_id, &recipe)) {
        *outcome = 1;
        return false;
    }
    if (!OperationAvailable("runtime.world.entity.destroy") ||
        !OperationAvailable("runtime.world.entity.finish_building")) { *outcome = 1; return false; }
    std::scoped_lock lock(removal_mutex);
    // Resolve the live handle again after acquiring the single-request lock,
    // then preserve its exact fixed-point transform and scale for native removal.
    if (!KfcRuntimeWorldEntityGetTransform(entity_handle, &prop) || prop.item_id != recipe.item_id) {
        *outcome = 1;
        return false;
    }
    const auto* context_profile = find_operation("runtime.world.context.active");
    if (context_profile && context_profile->actor_context) {
        // The headless server has no local player-input callback to consume
        // queue_removal(). Dispatch this exact handle through the server's
        // profiled prop-update frame, which carries the validated actor and
        // placement context used by its native destroy operation.
        auto request = std::make_shared<EntityRequest>();
        request->kind = EntityRequest::Kind::Destroy;
        request->has_exact_transform = true;
        std::copy_n(prop.position, 3, request->exact_transform.position);
        std::copy_n(prop.orientation, 4, request->exact_transform.rotation);
        std::copy_n(prop.scale, 3, request->exact_transform.scale);
        std::copy_n(recipe.bounds, 6, request->bounds);
        request->tracking = recipe.item_id;
        request->feedback = recipe.feedback;
        if (!invoke_entity_request(request, outcome)) return false;
        for (int attempt = 0; attempt < 200; ++attempt) {
            KfcRuntimePropRecord current{};
            if (!KfcRuntimeWorldEntityGetTransform(entity_handle, &current)) {
                *outcome = 0;
                return true;
            }
            Sleep(10);
        }
        *outcome = 4;
        return false;
    }
    EngineTransform transform{};
    std::copy_n(prop.position, 3, transform.position);
    std::copy_n(prop.orientation, 4, transform.rotation);
    std::copy_n(prop.scale, 3, transform.scale);
    const auto actor_hooks_before = actor_placement_hook_hits.load(std::memory_order_acquire);
    const auto actor_requests_before = actor_placement_request_hits.load(std::memory_order_acquire);
    const auto dispatch_hooks_before = building_dispatch_hook_hits.load(std::memory_order_acquire);
    const auto dispatch_requests_before = building_dispatch_request_hits.load(std::memory_order_acquire);
    if (!queue_removal(transform, recipe.bounds, recipe.item_id, recipe.feedback)) {
        const auto phase = InterlockedCompareExchange(&pending_removal.phase, 0, 0);
        if (phase == 0) {
            // Preserve hook evidence in the API result so the Lua error written
            // to shroudforge.log identifies which native callback actually ran.
            std::uint32_t flags{};
            if (actor_placement_hook_hits.load(std::memory_order_acquire) != actor_hooks_before) flags |= 1u;
            if (actor_placement_request_hits.load(std::memory_order_acquire) != actor_requests_before) flags |= 2u;
            if (building_dispatch_hook_hits.load(std::memory_order_acquire) != dispatch_hooks_before) flags |= 4u;
            if (building_dispatch_request_hits.load(std::memory_order_acquire) != dispatch_requests_before) flags |= 8u;
            if (GameThreadDispatcher::EntityContextReady()) flags |= 16u;
            *outcome = 30u + flags;
        } else {
            *outcome = 2;
        }
        return false;
    }
    for (int attempt = 0; attempt < 200; ++attempt) {
        KfcRuntimePropRecord current{};
        if (!KfcRuntimeWorldEntityGetTransform(entity_handle, &current)) { *outcome = 0; return true; }
        Sleep(10);
    }
    *outcome = 4;
    return false;
}

bool FinishBuilding(bool complete, std::uint32_t* outcome) {
    if (!outcome || !OperationAvailable("runtime.world.entity.finish_building")) return false;
    auto request = std::make_shared<EntityRequest>();
    request->kind = EntityRequest::Kind::Finish;
    request->complete = complete;
    return invoke_entity_request(request, outcome);
}
}

extern "C" {
bool __cdecl KfcRuntimeWorldOperationAvailable(const char* name) { return WorldRuntime::OperationAvailable(name); }
bool __cdecl KfcRuntimeWorldContextActive() { return WorldRuntime::ActiveContextAvailable(); }
std::uint32_t __cdecl KfcRuntimeWorldContextKind() { return WorldRuntime::ContextKind(); }
bool __cdecl KfcRuntimeWorldEntityContextReady() { return WorldRuntime::EntityContextReady(); }
bool __cdecl KfcRuntimeWorldCursorRead(std::uint8_t* cursor, std::size_t capacity, std::uint64_t* sequence) {
    return WorldRuntime::ReadCursorSnapshot(cursor, capacity, sequence);
}
bool __cdecl KfcRuntimeWorldVoxelRead(const std::int32_t* origin, const std::uint32_t* dimensions,
    std::uint16_t* values, std::size_t capacity, std::size_t* actual) {
    return WorldRuntime::ReadVoxels(origin, dimensions, values, capacity, actual);
}
bool __cdecl KfcRuntimeWorldGridGetSpec(const char* grid_id, KfcRuntimeGridSpec* spec) {
    return WorldRuntime::GetGridSpec(grid_id, spec);
}
bool __cdecl KfcRuntimeWorldVoxelWrite(const std::int32_t* origin, const std::uint32_t* dimensions,
    const std::uint16_t* values, std::size_t count, std::uint32_t* outcome) {
    return WorldRuntime::WriteVoxels(origin, dimensions, values, count, outcome);
}
bool __cdecl KfcRuntimeWorldEntitySpawn(const std::uint64_t* template_uuid, const double* position,
    const double* rotation, std::uint32_t tracking, std::uint32_t flags,
    std::uint32_t* entity_handle, std::uint32_t* outcome) {
    return WorldRuntime::SpawnEntity(template_uuid, position, rotation, tracking, flags, entity_handle, outcome);
}
bool __cdecl KfcRuntimeWorldEntityPlace(const double* position, const double* rotation,
    const float* bounds, std::uint32_t tracking, std::uint32_t feedback, std::uint32_t* outcome) {
    return WorldRuntime::PlaceEntity(position, rotation, bounds, tracking, feedback, outcome);
}
bool __cdecl KfcRuntimeWorldEntityDestroy(const double* position, const double* rotation,
    const float* bounds, std::uint32_t tracking, std::uint32_t feedback, std::uint32_t* outcome) {
    return WorldRuntime::DestroyEntity(position, rotation, bounds, tracking, feedback, outcome);
}
bool __cdecl KfcRuntimeWorldEntityDestroyHandle(std::uint32_t entity_handle, std::uint32_t* outcome) {
    return WorldRuntime::DestroyEntityHandle(entity_handle, outcome);
}
bool __cdecl KfcRuntimeWorldEntityFinishBuilding(bool complete, std::uint32_t* outcome) {
    return WorldRuntime::FinishBuilding(complete, outcome);
}
}
