#include "player-input/world_cursor_state.h"
#include "ecs/ecs_session_state.h"

#include <algorithm>
#include <cstring>
#include <string>
#include <utility>
#include <vector>

// The local input belongs to the original main-client ECS owner. The default
// ECS session can separately belong to a hosted authority world, so its type
// indices and entity handles must never be reused for the input owner.
namespace WorldRuntime::Implementation {
namespace {
using namespace EcsImplementation;
constexpr const char* InputType = "keen::ecs::ClientPlayerInput";
constexpr const char* LocalMarkerType = "keen::ecs::ClientCycleInventoryStock";
constexpr std::size_t MaximumInputBytes = 65536;
constexpr std::size_t MaximumCandidates = 32;
// In multiplayer the main-client ECS holds a ClientPlayerInput for every
// player, the other players' copies being replicated: see choose_local_input.

struct LocalInputState {
    EngineContext::EcsView owner;
    ComponentType component{}, local_marker_component{};
    std::uintptr_t records{};
    std::size_t registrations{}, entity_count{}, cursor{}, candidate_slot{}, published_slot{};
    EntityView candidate, published;
    std::vector<std::pair<EntityView, std::size_t>> candidates;
    std::uint64_t session{}, registry_ms{}, completed_ms{};
    bool local_marker_available{}, configured{}, complete{}, ambiguous{}, candidate_overflow{};
};
std::mutex local_input_mutex;
LocalInputState local_input;
std::uint64_t local_input_sequence{};
// How the last complete scan chose, or why it could not: for error messages.
std::mutex local_input_detail_mutex;
std::string local_input_detail;

ResolvedLayout client_layout(const EngineContext::EcsView& owner){
    const auto& source = owner.layout;
    ResolvedLayout layout;
    layout.count_address = owner.manager + source.manager_count;
    layout.table_address = owner.manager + source.manager_table;
    layout.occupancy_address = source.manager_occupancy ? owner.manager + source.manager_occupancy : 0;
    layout.keys_address = source.manager_keys ? owner.manager + source.manager_keys : 0;
    layout.entity_id = source.entity_id;
    layout.entity_generation = source.entity_generation;
    layout.entity_layout = source.entity_layout;
    layout.entity_storage = source.entity_storage;
    layout.entity_definition = source.entity_definition;
    layout.entity_definition_uuid = source.entity_definition_uuid;
    layout.entity_definition_uuid_validated = source.entity_definition_uuid_validated;
    layout.entity_row = source.entity_row;
    layout.component_bits = source.component_bits;
    layout.component_offsets = source.component_offsets;
    layout.component_strides = source.component_strides;
    return layout;
}

bool resolve_input_registration(const EngineContext::EcsView& owner, std::size_t size,
                                ComponentType& component, std::uintptr_t& records, std::size_t& count,
                                ComponentType& local_marker, bool& local_marker_available){
    std::unordered_map<std::string, std::uint32_t> contract;
    { std::scoped_lock lock(state_mutex); contract = configured_types; }
    const auto expected = contract.find(InputType);
    if (expected == contract.end() || expected->second != size) return false;
    ComponentRegistry::Snapshot registry;
    std::string error;
    if (!ComponentRegistry::Read(owner.manager, owner.owners.image, image_size, read_bytes, registry, error) ||
        registry.owner != owner.registry_owner) return false;
    bool found{};
    local_marker_available = false;
    for (const auto& entry : registry.entries) {
        for (const auto* type : {&entry.storage, &entry.configuration}) {
            if (type->name.empty()) continue;
            const auto type_contract = contract.find(type->name);
            if (type_contract == contract.end() || type_contract->second != type->size) return false;
        }
        const auto& name = entry.storage.name.empty() ? entry.name : entry.storage.name;
        if (name == LocalMarkerType) {
            if (local_marker_available) return false;
            local_marker = {entry.index, entry.size};
            local_marker_available = true;
        }
        if (entry.storage.name != InputType) continue;
        if (found || entry.storage.size != size || entry.size != size) return false;
        component = {entry.index, entry.size};
        found = true;
    }
    if (!found) return false;
    records = registry.records;
    count = registry.entries.size();
    return true;
}

bool current_registry_header(const LocalInputState& state){
    std::uintptr_t records{}, owner{};
    std::uint64_t count{};
    return read(state.owner.manager, owner) && owner == state.owner.registry_owner &&
        read(owner + 8, records) && records == state.records &&
        read(owner + 16, count) && count == state.registrations;
}

bool current_entity_header(const LocalInputState& state, const ResolvedLayout& layout){
    std::uintptr_t table{};
    std::uint64_t count{};
    return read(layout.count_address, count) && count == state.entity_count &&
        read(layout.table_address, table) && table == state.owner.entity_table;
}

void restart_scan(LocalInputState& state, bool retain_published = false){
    state.cursor = 0;
    state.candidate = {};
    state.candidate_slot = 0;
    state.candidates.clear();
    state.candidate_overflow = false;
    state.complete = state.ambiguous = false;
    state.completed_ms = 0;
    if (!retain_published) { state.published = {}; state.published_slot = 0; }
}

void publish_detail(std::string detail){
    std::scoped_lock lock(local_input_detail_mutex);
    local_input_detail = std::move(detail);
}

// Several candidates: use only the client-only hammer-state component that
// was observed to differ between local and replicated inputs. A unique marker
// is required. Unknown, unreadable, oversized, or changing data stays ambiguous.
bool choose_local_input(LocalInputState& state, const ResolvedLayout& layout){
    const auto count = std::to_string(state.candidates.size()) + " player inputs";
    const auto decide = [&](std::size_t index, const std::string& reason){
        state.candidate = state.candidates[index].first;
        state.candidate_slot = state.candidates[index].second;
        publish_detail(count + ", the local one is entity " + std::to_string(state.candidates[index].first.id) +
            ": " + reason);
        return true;
    };
    if (!state.local_marker_available) {
        publish_detail(count + "; local hammer-state component is not registered");
        return false;
    }
    const auto type = state.local_marker_component;
    if (!type.size || type.size > 4096) {
        publish_detail(count + "; local hammer-state component size is outside the 1-4096 byte limit");
        return false;
    }
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(1);
    std::array<std::uint8_t, 4096> bytes{};
    std::size_t marked{};
    std::size_t marked_index{};
    bool selection_complete = true;
    for (std::size_t index = 0; index < state.candidates.size(); ++index) {
        if (std::chrono::steady_clock::now() >= deadline) {
            selection_complete = false;
            break;
        }
        bool present{};
        if (!component_presence(state.candidates[index].first, layout, type, present)) {
            selection_complete = false;
            break;
        }
        bool is_marked{};
        if (present) {
            std::uintptr_t address{};
            if (!component_address(state.candidates[index].first, layout, type, address) ||
                !read_bytes(address, bytes.data(), type.size)) {
                selection_complete = false;
                break;
            }
            is_marked = std::any_of(bytes.begin(), bytes.begin() + type.size,
                [](auto byte){ return byte != 0; });
        }
        if (is_marked) {
            ++marked;
            marked_index = index;
        }
        if (std::chrono::steady_clock::now() >= deadline) {
            selection_complete = false;
            break;
        }
    }
    if (selection_complete && marked == 1)
        return decide(marked_index, "only its ClientCycleInventoryStock payload is nonzero");
    publish_detail(count + (selection_complete
        ? "; ClientCycleInventoryStock has " + std::to_string(marked) + " nonzero payloads"
        : "; ClientCycleInventoryStock selection was incomplete or exceeded 1 ms"));
    return false;
}

std::uint32_t scan_local_input(LocalInputState& state, const ResolvedLayout& layout){
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(1);
    std::size_t visited{};
    while (state.cursor < state.entity_count) {
        if (visited >= 8192 || (visited && std::chrono::steady_clock::now() >= deadline)) return 5;
        const auto slot = state.cursor++;
        ++visited;
        if (layout.occupancy_address) {
            std::uintptr_t occupancy{};
            std::uint64_t bits{};
            if (!read(layout.occupancy_address, occupancy) || !occupancy ||
                !read(occupancy + (slot / 64) * sizeof(bits), bits)) { restart_scan(state); return 5; }
            if (!(bits & (std::uint64_t{1} << (slot % 64)))) continue;
        }
        std::uintptr_t pointer{};
        if (!read(state.owner.entity_table + slot * sizeof(pointer), pointer)) { restart_scan(state); return 5; }
        if (!pointer) {
            if (layout.occupancy_address) { restart_scan(state); return 5; }
            continue;
        }
        EntityView entity{};
        bool present{};
        if (!entity_view(pointer, layout, entity) || !component_presence(entity, layout, state.component, present)) {
            restart_scan(state);
            return 5;
        }
        if (!present) continue;
        if (state.candidates.size() < MaximumCandidates) state.candidates.emplace_back(entity, slot);
        else state.candidate_overflow = true;
    }
    if (state.candidate_overflow) {
        state.ambiguous = true;
        publish_detail("more than " + std::to_string(MaximumCandidates) + " player inputs; selection refused");
    } else if (state.candidates.size() == 1) {
        state.candidate = state.candidates.front().first;
        state.candidate_slot = state.candidates.front().second;
    } else if (state.candidates.size() > 1) {
        state.ambiguous = !choose_local_input(state, layout);
    }
    state.complete = true;
    state.completed_ms = GetTickCount64();
    state.published = state.ambiguous ? EntityView{} : state.candidate;
    state.published_slot = state.candidate_slot;
    return state.ambiguous ? 6u : state.candidate.pointer ? 1u : 4u;
}

// The sample is copied under SEH (read_bytes): the component's page is not
// queried first, each VirtualQuery taking about 30 ms in game.
bool sample_local_input(const LocalInputState& state, const ResolvedLayout& layout,
                        std::vector<std::uint8_t>& sample){
    std::uintptr_t pointer{}, address{}, again_address{};
    EntityView entity{}, again{};
    if (state.published_slot >= state.entity_count ||
        !read(state.owner.entity_table + state.published_slot * sizeof(pointer), pointer) || pointer != state.published.pointer ||
        !entity_view(pointer, layout, entity) || entity.id != state.published.id || entity.generation != state.published.generation ||
        !component_address(entity, layout, state.component, address, false)) return false;
    std::vector<std::uint8_t> verification(sample.size());
    return read_bytes(address, sample.data(), sample.size()) &&
        read_bytes(address, verification.data(), verification.size()) && verification == sample &&
        entity_view(pointer, layout, again) && again == entity &&
        component_address(again, layout, state.component, again_address, false) && again_address == address &&
        read(state.owner.entity_table + state.published_slot * sizeof(pointer), pointer) && pointer == entity.pointer;
}
}

bool LocalCursorInputAvailable(){
    EngineContext::EcsView owner;
    if (EcsImplementation::stop_requested.load(std::memory_order_acquire) ||
        !KfcRuntimeWorldSessionId() || !EngineContext::ResolveClientEcs(owner)) return false;
    std::scoped_lock lock(EcsImplementation::state_mutex);
    const auto contract = EcsImplementation::configured_types.find(InputType);
    return contract != EcsImplementation::configured_types.end() && contract->second && contract->second <= MaximumInputBytes;
}

std::uint32_t ReadLocalCursorInput(std::size_t component_size, std::uint64_t expected_session,
                                  std::uint8_t* buffer, std::size_t capacity, std::uint64_t* sequence){
    if (!component_size || component_size > MaximumInputBytes || !expected_session || !buffer ||
        capacity != component_size || !sequence) return 2;
    std::scoped_lock operation_lock(EcsImplementation::operation_mutex);
    std::unique_lock input_lock(local_input_mutex, std::try_to_lock);
    if (!input_lock.owns_lock()) return 5;
    EngineContext::EcsView owner;
    if (EcsImplementation::stop_requested.load(std::memory_order_acquire) ||
        KfcRuntimeWorldSessionId() != expected_session || !EngineContext::ResolveClientEcs(owner)) {
        local_input = {};
        return 4;
    }
    const auto now = GetTickCount64();
    const auto layout = client_layout(owner);
    std::uint64_t count{};
    std::uintptr_t table{};
    if (!read(layout.count_address, count) || !count || count > owner.layout.maximum_entities ||
        !read(layout.table_address, table) || table != owner.entity_table) return 4;
    const bool changed = !local_input.configured || local_input.owner != owner || local_input.session != expected_session ||
        local_input.component.size != component_size || local_input.entity_count != count;
    if (changed) {
        local_input = {};
        local_input.owner = owner;
        local_input.session = expected_session;
        local_input.entity_count = static_cast<std::size_t>(count);
    }
    if (changed || now - local_input.registry_ms >= 1000) {
        ComponentType component{};
        std::uintptr_t records{};
        std::size_t registrations{};
        ComponentType local_marker{};
        bool local_marker_available{};
        if (!resolve_input_registration(owner, component_size, component, records, registrations,
                                        local_marker, local_marker_available)) {
            local_input = {};
            return 3;
        }
        if (!local_input.configured || local_input.component != component || local_input.records != records ||
            local_input.registrations != registrations) restart_scan(local_input);
        local_input.local_marker_component = local_marker;
        local_input.local_marker_available = local_marker_available;
        local_input.component = component;
        local_input.records = records;
        local_input.registrations = registrations;
        local_input.registry_ms = now;
        local_input.configured = true;
    }
    if (!current_registry_header(local_input)) { local_input = {}; return 5; }
    // A periodic discovery refresh may span several short slices. Keep the
    // last completely validated unique input while refreshing, but revalidate
    // its original membership and payload for every published sample. Owner,
    // registry, entity and unreadable-slot failures discard it immediately.
    if (local_input.complete && now - local_input.completed_ms >= 1000) restart_scan(local_input, true);
    if (!local_input.complete) {
        const auto result = scan_local_input(local_input, layout);
        if (result != 1 && !(result == 5 && local_input.published.pointer)) return result;
    }
    if (local_input.ambiguous) return 6;
    if (!local_input.published.pointer) return 4;
    std::vector<std::uint8_t> sample(component_size);
    EngineContext::EcsView after;
    if (!sample_local_input(local_input, layout, sample) || !current_registry_header(local_input) ||
        !current_entity_header(local_input, layout) ||
        EcsImplementation::stop_requested.load(std::memory_order_acquire) ||
        KfcRuntimeWorldSessionId() != expected_session || !EngineContext::ResolveClientEcs(after) || after != owner) {
        restart_scan(local_input);
        return 5;
    }
    std::memcpy(buffer, sample.data(), sample.size());
    *sequence = ++local_input_sequence;
    return 1;
}
}

extern "C" std::uint32_t __cdecl KfcRuntimeWorldCursorReadInput(std::size_t component_size,
    std::uint64_t expected_session, std::uint8_t* buffer, std::size_t capacity, std::uint64_t* sequence){
    try { return WorldRuntime::Implementation::ReadLocalCursorInput(component_size, expected_session, buffer, capacity, sequence); }
    catch (...) { return 4; }
}

// How the last scan chose among several player inputs, or why it could not.
// Returns the text length (truncated to capacity - 1).
extern "C" std::size_t __cdecl KfcRuntimeWorldCursorInputDetail(char* buffer, std::size_t capacity){
    if (!buffer || !capacity) return 0;
    try {
        std::scoped_lock lock(WorldRuntime::Implementation::local_input_detail_mutex);
        const auto& detail = WorldRuntime::Implementation::local_input_detail;
        const auto length = std::min(detail.size(), capacity - 1);
        std::memcpy(buffer, detail.data(), length);
        buffer[length] = '\0';
        return length;
    } catch (...) { buffer[0] = '\0'; return 0; }
}
