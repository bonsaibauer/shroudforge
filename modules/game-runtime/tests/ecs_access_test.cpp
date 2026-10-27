// Exercise the public ECS read API against a private original-owner fixture.
// All objects belong to this test process. No game, patch or engine call is used.
#include <windows.h>
#include <cstdlib>
#include "voxels/voxel_request.h"
namespace {
unsigned page_query_count{};
unsigned mutate_on_page_query{};
void* page_query_mutation{};
DWORD page_query_protection{};
bool mutate_before_page_query{};
SIZE_T WINAPI counted_ecs_virtual_query(LPCVOID address, PMEMORY_BASIC_INFORMATION memory, SIZE_T length) {
    ++page_query_count;
    const auto mutate = [&] {
        DWORD previous{};
        if (!::VirtualProtect(page_query_mutation, 4096, page_query_protection, &previous)) std::abort();
        page_query_mutation = nullptr;
    };
    const bool inject = page_query_mutation && page_query_count == mutate_on_page_query;
    if (inject && mutate_before_page_query) mutate();
    const auto result = ::VirtualQuery(address, memory, length);
    if (inject && !mutate_before_page_query) mutate();
    return result;
}
}
#define VirtualQuery counted_ecs_virtual_query
#include "ecs/bind_world_entities.cpp"
#include "ecs/resolve_entity_handle.cpp"
#include "ecs/read_entity_membership.cpp"
#include "ecs/read_entity_components.cpp"
#include "ecs/write_entity_components.cpp"
#include "ecs/resolve_component_storage.cpp"
#include "ecs/query_building_props.cpp"
#include "ecs/read_building_prop.cpp"
#include "building/register_prop_recipes.cpp"
#include "ecs/set_building_prop_scale.cpp"
#include "ecs/query_entities.cpp"
#include "ecs/query_entities_in_bounds.cpp"
#include "ecs/registry/refresh_component_registry.cpp"
#include "ecs/registry/sample_component_layouts.cpp"
#include "ecs/describe_ecs_runtime.cpp"
#undef VirtualQuery
#include <cstdio>
#include <cstdlib>
#include <source_location>

namespace {
unsigned checks{};
void check(bool value, const std::source_location location = std::source_location::current()) {
    ++checks;
    if (!value) { std::fprintf(stderr, "ECS read check %u failed at line %u\n", checks, location.line()); std::exit(1); }
}
template<class T> void put(void* bytes, std::size_t offset, const T& value) {
    std::memcpy(static_cast<unsigned char*>(bytes) + offset, &value, sizeof(value));
}
struct OwnerObject { std::uintptr_t vtable{}, pointer{}, scene{}; };
struct ControllerObject { std::uintptr_t vtable{}, client{}, main{}; };
struct MainObject { std::uintptr_t execution{}; };
struct NativeThread {
    std::uintptr_t name{};
    std::uint64_t length{10}, capacity{64};
    char text[64]{"GameServer"};
    std::uintptr_t handle{123}, context{};
    std::uint32_t stop{};
    std::uintptr_t callback{};
};
struct SceneOwner {
    NativeThread thread;
    std::uintptr_t entries{};
    std::uint64_t count{}, capacity{};
    alignas(4) LONG gate{};
    std::uint8_t state{1};
    std::uint64_t loop{};
};
struct ThreadRow { std::uintptr_t handle{}, name{}; std::uint64_t length{}; };
alignas(4) LONG thread_registry_gate{};
ThreadRow thread_rows[2];
std::uint16_t thread_count{1};
void fixture_server_callback() {}
struct SceneEntry {
    std::uintptr_t context{};
    std::uint64_t uuid[2]{11,22};
    std::uint8_t state{5};
    unsigned char padding[119]{};
};
static_assert(sizeof(SceneEntry) == 144);
struct alignas(8) Execution {
    unsigned char registry[640]{};
    unsigned char manager[400]{};
    struct Queue { std::uintptr_t data{}; std::uint64_t count{}, capacity{4}; std::uintptr_t grow{}, allocator{}; };
    struct Tracker { Queue queues[16]; std::uint32_t count_a{}, count_b{}; std::uintptr_t reserved{}, manager{}; } tracker;
};
struct FixtureLayout { std::uint64_t bits{1}; std::uint16_t offset{}, stride{8}, next_stride{}; };
struct Component { std::uint32_t first{42}, second{84}; };
struct Entity {
    unsigned char reserved[16]{};
    std::uint32_t id{17}, generation{3};
    std::uintptr_t layout{}, storage{}, definition{};
    std::uint32_t row{};
};
unsigned tracker_calls{};
unsigned expected_tracker_index{};
bool tracker_contract{};
LONG* tracker_scene_gate{};
std::uint32_t tracker_before{};
void* tracker_fault_page{};
bool tracker_fault_applied{};
void __fastcall fixture_track_change(std::uintptr_t manager, std::uintptr_t context,
                                    std::uintptr_t entry, std::uint16_t index, std::uint8_t queue) {
    ++tracker_calls;
    tracker_contract = context == manager + sizeof(Execution::manager) && index == expected_tracker_index && queue == 0 &&
        thread_registry_gate > 0 && tracker_scene_gate && *tracker_scene_gate == static_cast<LONG>(0x80000000u);
    const auto* entity = reinterpret_cast<const Entity*>(entry);
    tracker_before = reinterpret_cast<const Component*>(entity->storage)->first;
    if (tracker_fault_page) {
        DWORD old_protection{};
        tracker_fault_applied = VirtualProtect(tracker_fault_page, 4096, PAGE_NOACCESS, &old_protection) != FALSE;
    }
}
// Reflection descriptors and their names are genuine image-backed test data,
// which lets the production registration validator enforce its actual bounds.
OwnerObject root_object;
std::uintptr_t root_virtuals[1]{}, client_virtuals[1]{}, controller_virtuals[1]{};
alignas(8) unsigned char descriptor[0x90]{};
const char component_name[] = "keen::ecs::FixtureComponent";
alignas(8) unsigned char editor_descriptors[4][0x90]{};
const char input_component_name[] = "keen::ecs::ClientPlayerInput";
const char used_item_component_name[] = "keen::ecs::UsedItem";
const char transform_component_name[] = "keen::ecs::CurrentTransform";
const char local_marker_component_name[] = "keen::ecs::ClientCycleInventoryStock";
}

int main() {
    const std::uintptr_t pointer_slots[]{0x1000, 0x2000, 0, 0x4000, 0x5000};
    const std::uint64_t occupied_bits[]{(std::uint64_t{1} << 1) | (std::uint64_t{1} << 3) | (std::uint64_t{1} << 4)};
    std::vector<EntityPointer> compacted;
    check(compact_entity_pointers(pointer_slots, std::size(pointer_slots), occupied_bits,
                                  std::size(occupied_bits), compacted));
    check(compacted.size() == 3);
    check(compacted[0].pointer == 0x2000 && compacted[0].table_index == 1);
    check(compacted[1].pointer == 0x4000 && compacted[1].table_index == 3);
    check(compacted[2].pointer == 0x5000 && compacted[2].table_index == 4);
    check(compact_entity_pointers(pointer_slots, std::size(pointer_slots), nullptr, 0, compacted));
    check(compacted.size() == 4 && compacted[0].table_index == 0 && compacted[3].table_index == 4);
    check(!compact_entity_pointers(pointer_slots, std::size(pointer_slots), occupied_bits, 0, compacted));
    const auto base = reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base);
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(base + dos->e_lfanew);
    image_base = base;
    image_size = nt->OptionalHeader.SizeOfImage;
    KfcRuntimeCompatibility::EnshroudedClient::image_size = image_size;
    KfcRuntimeCompatibility::EnshroudedClient::image_timestamp = nt->FileHeader.TimeDateStamp;
    KfcRuntimeCompatibility::EnshroudedClient::image_sha256 = "private-fixture-identity";
    KfcRuntimeCompatibility::EnshroudedClient::image_target = "private-ecs-test.exe";
    Execution execution;
    Execution authority_execution;
    MainObject authority_context{reinterpret_cast<std::uintptr_t>(&authority_execution)};
    SceneEntry scenes[2];
    scenes[0].context = reinterpret_cast<std::uintptr_t>(&authority_context);
    scenes[1].context = reinterpret_cast<std::uintptr_t>(&authority_context);
    SceneOwner scene_owner;
    scene_owner.entries = reinterpret_cast<std::uintptr_t>(scenes);
    scene_owner.capacity = 2;
    scene_owner.thread.name = reinterpret_cast<std::uintptr_t>(scene_owner.thread.text);
    scene_owner.thread.context = reinterpret_cast<std::uintptr_t>(&scene_owner.loop);
    scene_owner.thread.callback = reinterpret_cast<std::uintptr_t>(fixture_server_callback);
    thread_rows[0] = {scene_owner.thread.handle, scene_owner.thread.name, scene_owner.thread.length};
    MainObject main{reinterpret_cast<std::uintptr_t>(&execution)};
    ControllerObject controller{reinterpret_cast<std::uintptr_t>(controller_virtuals), 0, reinterpret_cast<std::uintptr_t>(&main)};
    OwnerObject client{reinterpret_cast<std::uintptr_t>(client_virtuals), reinterpret_cast<std::uintptr_t>(&controller)};
    client.scene = reinterpret_cast<std::uintptr_t>(&scene_owner);
    controller.client = reinterpret_cast<std::uintptr_t>(&client);
    root_object = {reinterpret_cast<std::uintptr_t>(root_virtuals), reinterpret_cast<std::uintptr_t>(&client)};
    FixtureLayout layout;
    Component component;
    Entity entity;
    std::array<std::uint8_t, 16> template_uuid{1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16};
    entity.layout = reinterpret_cast<std::uintptr_t>(&layout);
    entity.storage = reinterpret_cast<std::uintptr_t>(&component);
    entity.definition = reinterpret_cast<std::uintptr_t>(template_uuid.data());
    std::uintptr_t table[4]{reinterpret_cast<std::uintptr_t>(&entity), 0, 0, 0};
    auto table_pointer = reinterpret_cast<std::uintptr_t>(table);
    auto registry_pointer = reinterpret_cast<std::uintptr_t>(execution.registry);
    put(execution.manager, 0, registry_pointer);
    put(execution.manager, 344, std::uint64_t{4});
    put(execution.manager, 392, table_pointer);

    const auto name = reinterpret_cast<std::uintptr_t>(component_name);
    const auto hash = ComponentRegistry::hash_name(component_name);
    put(descriptor, 0x20, name);
    put(descriptor, 0x28, std::uint64_t{sizeof(component_name) - 1});
    put(descriptor, 0x40, std::uint32_t{sizeof(Component)});
    put(descriptor, 0x50, hash);
    unsigned char registration[256]{};
    put(registration, 16, name);
    put(registration, 24, std::uint64_t{sizeof(component_name)});
    put(registration, 32, hash);
    const auto descriptor_pointer = reinterpret_cast<std::uintptr_t>(descriptor);
    put(registration, 40, descriptor_pointer);
    put(registration, 56, std::uint32_t{sizeof(Component)});
    std::uint16_t sizes[]{sizeof(Component)};
    std::uintptr_t storage_types[]{descriptor_pointer};
    std::uint64_t callbacks[5]{};
    put(execution.registry, 8, reinterpret_cast<std::uintptr_t>(registration));
    put(execution.registry, 16, std::uint64_t{1});
    put(execution.registry, 24, std::uint64_t{1});
    put(execution.registry, 232, reinterpret_cast<std::uintptr_t>(sizes));
    put(execution.registry, 240, std::uint64_t{1});
    put(execution.registry, 256, reinterpret_cast<std::uintptr_t>(storage_types));
    put(execution.registry, 264, std::uint64_t{1});
    put(execution.registry, 280, reinterpret_cast<std::uintptr_t>(callbacks));
    put(execution.registry, 288, std::uint64_t{1});
    authority_execution = execution;
    put(authority_execution.manager, 0, reinterpret_cast<std::uintptr_t>(authority_execution.registry));
    std::uint32_t tracking_ids[16][4]{};
    authority_execution.tracker.manager = reinterpret_cast<std::uintptr_t>(authority_execution.manager);
    for (std::size_t index = 0; index < 16; ++index)
        authority_execution.tracker.queues[index].data = reinterpret_cast<std::uintptr_t>(tracking_ids[index]);
    nlohmann::json profile{
        {"target", "private-ecs-test.exe"},
        {"image", {{"sha256", "private-fixture-identity"}, {"timestamp", nt->FileHeader.TimeDateStamp}, {"size", image_size}}},
        {"provenance", {{"ecsReadValidated", true}, {"ecsAuthorityReadValidated", true}}},
        {"ownerLayout", {
            {"rootObjectRva", reinterpret_cast<std::uintptr_t>(&root_object) - base},
            {"rootVtableRva", reinterpret_cast<std::uintptr_t>(root_virtuals) - base},
            {"clientPointerOffset", offsetof(OwnerObject, pointer)},
            {"clientVtableRva", reinterpret_cast<std::uintptr_t>(client_virtuals) - base},
            {"controllerPointerOffset", offsetof(OwnerObject, pointer)},
            {"controllerVtableRva", reinterpret_cast<std::uintptr_t>(controller_virtuals) - base},
            {"controllerClientBackPointerOffset", offsetof(ControllerObject, client)},
            {"controllerMainOwnerOffset", offsetof(ControllerObject, main)}}},
        {"ecsReadLayout", {
            {"mainExecutionOwnerOffset", offsetof(MainObject, execution)},
            {"executionRegistryOwnerOffset", offsetof(Execution, registry)},
            {"executionEntityManagerOffset", offsetof(Execution, manager)}, {"entityManagerOwnerOffset", 0},
            {"entityManagerCount", 344}, {"entityManagerTable", 392},
            {"entityId", offsetof(Entity, id)}, {"entityGeneration", offsetof(Entity, generation)},
            {"entityLayout", offsetof(Entity, layout)}, {"entityStorage", offsetof(Entity, storage)},
            {"entityDefinitionPointer", offsetof(Entity, definition)}, {"entityRow", offsetof(Entity, row)},
            {"entityDefinitionUuidOffset", 0},
            {"componentBits", offsetof(FixtureLayout, bits)}, {"componentOffsets", offsetof(FixtureLayout, offset)},
            {"componentStrides", offsetof(FixtureLayout, stride)}, {"maximumEntities", 8},
            {"clientSceneOwnerOffset", offsetof(OwnerObject, scene)},
            {"sceneEntriesPointerOffset", offsetof(SceneOwner, entries)},
            {"sceneEntriesCountOffset", offsetof(SceneOwner, count)},
            {"sceneEntriesCapacityOffset", offsetof(SceneOwner, capacity)},
            {"sceneEntryStride", sizeof(SceneEntry)}, {"sceneEntryContextOffset", offsetof(SceneEntry, context)},
            {"sceneEntryUuidOffset", offsetof(SceneEntry, uuid)}, {"sceneEntryStateOffset", offsetof(SceneEntry, state)},
            {"sceneEntryReadyState", 5}, {"maximumSceneEntries", 8}}}
    };
    std::string error;
    auto unvalidated = profile;
    unvalidated["provenance"]["ecsReadValidated"] = false;
    check(!EngineContext::Configure(unvalidated, error));
    check(EngineContext::Configure(profile, error));
    const char* names[]{component_name};
    std::uint32_t component_size[]{sizeof(Component)};
    check(KfcRuntimeEcsConfigure(names, component_size, 1));
    EcsRuntime::Tick();
    check(component_registry_ready && component_registry_world_bound && read_only_binding);
    check(read_only_view.role == EngineContext::EcsRole::ClientReplica);
    check(KfcRuntimeEcsReady() && !KfcRuntimeEcsCanWrite());
    // The compact report uses the same validated catalog and retains every
    // matching role instead of choosing one alias and hiding ambiguity.
    const auto registration_json = [&](const char* selector) {
        const auto required = KfcRuntimeEcsRegistration(selector, nullptr, 0);
        check(required > 1 && required < 64 * 1024);
        std::vector<char> bytes(required);
        check(KfcRuntimeEcsRegistration(selector, bytes.data(), bytes.size()) == required);
        return nlohmann::json::parse(bytes.data());
    };
    auto compact_registration = registration_json(component_name);
    check(compact_registration["available"] == true && compact_registration["entries"].size() == 1);
    check(compact_registration["entries"][0]["runtime_type"]["size"] == sizeof(Component));
    check(!compact_registration["entries"][0].contains("callbacks"));
    check(registration_json("keen::ecs::Unregistered")["entries"].empty());
    check(KfcRuntimeEcsRegistration(nullptr, nullptr, 0) == 0);
    check(KfcRuntimeEcsRegistration("", nullptr, 0) == 0);
    auto alias_entry = component_registry.entries.front();
    alias_entry.index = 1;
    alias_entry.name = "keen::ecs::FixtureOther";
    alias_entry.configuration = alias_entry.storage;
    component_registry.entries.push_back(alias_entry);
    check(registration_json(component_name)["entries"].size() == 2);
    component_registry.entries.pop_back();
    check(!GameThreadDispatcher::Ready());
    check(!KfcRuntimeEcsPropQueryReady());
    const auto session = KfcRuntimeWorldSessionId();
    check(session != 0);
    std::uint32_t handle{};
    std::size_t result = SIZE_MAX - 1;
    for (int attempt = 0; attempt < 10 && result == SIZE_MAX - 1; ++attempt)
        result = KfcRuntimeEcsQuery(names, 1, &handle, 1);
    check(result == 1 && handle != 0);
    check(handles.at(handle).table_slot == table_pointer);
    // Frequent callers must not lose a valid player during each incremental
    // refresh. The exact same provider is shared by unrelated Lua mods.
    QueryOperation query_identity;
    query_identity.owned_names = {component_name};
    const auto cached_query_key = query_key(query_identity);
    const auto seed_refresh = [&]() {
        QueryScanState scan;
        scan.epoch = current_epoch();
        scan.touched_ms = GetTickCount64();
        // More than one scan slice, without dereferencing any foreign memory.
        scan.pointers.resize(20000);
        query_scans[cached_query_key] = std::move(scan);
    };
    const auto seed_cached_query = [&](std::uint64_t epoch) {
        const auto now = GetTickCount64();
        query_cache[cached_query_key] = QueryCacheEntry{epoch, now - 100, now, {handle}};
    };
    seed_cached_query(current_epoch());
    seed_refresh();
    std::uint32_t refreshed_handle{};
    check(KfcRuntimeEcsQuery(names, 1, &refreshed_handle, 1) == 1 && refreshed_handle == handle);
    check(query_scans.contains(cached_query_key));
    // Even a completed snapshot is never a license to return removed members.
    seed_refresh();
    table[0] = 0;
    check(KfcRuntimeEcsQuery(names, 1, &refreshed_handle, 1) == 0);
    table[0] = reinterpret_cast<std::uintptr_t>(&entity);
    seed_refresh();
    ++entity.generation;
    check(KfcRuntimeEcsQuery(names, 1, &refreshed_handle, 1) == 0);
    --entity.generation;
    seed_refresh();
    layout.bits = 0;
    check(KfcRuntimeEcsQuery(names, 1, &refreshed_handle, 1) == 0);
    layout.bits = 1;
    seed_refresh();
    check(KfcRuntimeEcsQuery(names, 1, &refreshed_handle, 1) == 1 && refreshed_handle == handle);
    // An epoch mismatch or missing first snapshot remains pending, never empty.
    seed_cached_query(current_epoch() - 1);
    seed_refresh();
    check(KfcRuntimeEcsQuery(names, 1, &refreshed_handle, 1) == SIZE_MAX - 1);
    check(!query_cache.contains(cached_query_key));
    seed_refresh();
    check(KfcRuntimeEcsQuery(names, 1, &refreshed_handle, 1) == SIZE_MAX - 1);
    // Finishing the refresh publishes new matches as well as the known member.
    Entity second_entity = entity;
    second_entity.id = entity.id + 1;
    table[1] = reinterpret_cast<std::uintptr_t>(&second_entity);
    seed_cached_query(current_epoch());
    QueryScanState finishing;
    finishing.epoch = current_epoch();
    finishing.touched_ms = GetTickCount64();
    finishing.pointers = {{table[0], 0}, {table[1], 1}};
    query_scans[cached_query_key] = std::move(finishing);
    std::uint32_t refreshed_handles[2]{};
    check(KfcRuntimeEcsQuery(names, 1, refreshed_handles, 2) == 2);
    check(refreshed_handles[0] == handle && refreshed_handles[1] != 0 && refreshed_handles[1] != handle);
    check(!query_scans.contains(cached_query_key));
    table[1] = 0;
    // The short completed-cache path applies the same membership validation.
    check(KfcRuntimeEcsQuery(names, 1, &refreshed_handle, 1) == 1 && refreshed_handle == handle);
    query_cache.clear();
    query_scans.clear();
    // An erased bucket can retain a plausible Entity and component payload.
    // Only the original occupied ID lookup makes it an active table member.
    std::uint64_t occupied{8};
    std::uint32_t keys[4]{0,0,0,entity.id};
    std::uintptr_t membership_table[4]{0,0,0,reinterpret_cast<std::uintptr_t>(&entity)};
    auto occupied_pointer = reinterpret_cast<std::uintptr_t>(&occupied);
    auto keys_pointer = reinterpret_cast<std::uintptr_t>(keys);
    auto membership_pointer = reinterpret_cast<std::uintptr_t>(membership_table);
    std::uint64_t membership_count{4};
    auto membership_layout = live_layout;
    membership_layout.count_address = reinterpret_cast<std::uintptr_t>(&membership_count);
    membership_layout.table_address = reinterpret_cast<std::uintptr_t>(&membership_pointer);
    membership_layout.occupancy_address = reinterpret_cast<std::uintptr_t>(&occupied_pointer);
    membership_layout.keys_address = reinterpret_cast<std::uintptr_t>(&keys_pointer);
    EntityView member{};
    check(entity_view(reinterpret_cast<std::uintptr_t>(&entity), membership_layout, member));
    check(entity_membership(member,membership_layout)==EntityMembership::Present);
    EntityView by_id{};
    std::size_t by_id_slot{SIZE_MAX};
    check(entity_for_id(entity.id,membership_layout,by_id,by_id_slot)==EntityMembership::Present &&
        by_id==member && by_id_slot==3);
    const auto unchanged_id=by_id;
    occupied = 0;
    check(entity_for_id(entity.id,membership_layout,by_id,by_id_slot)==EntityMembership::Absent &&
        by_id==unchanged_id && by_id_slot==3);
    check(entity_membership(member,membership_layout)==EntityMembership::Absent);
    check(!entity_view(reinterpret_cast<std::uintptr_t>(&entity), membership_layout, member));
    occupied = 8;
    keys[3] = entity.id + 1;
    check(!entity_view(reinterpret_cast<std::uintptr_t>(&entity), membership_layout, member));
    keys[3] = entity.id;
    membership_table[3] = 0;
    check(entity_for_id(entity.id,membership_layout,by_id,by_id_slot)==EntityMembership::Unresolved &&
        by_id==unchanged_id && by_id_slot==3);
    check(entity_membership(member,membership_layout)==EntityMembership::Unresolved);
    check(!entity_view(reinterpret_cast<std::uintptr_t>(&entity), membership_layout, member));
    // Linear collision handling follows the original open-addressing lookup.
    keys[3] = 99;
    keys[0] = entity.id;
    occupied = 9;
    membership_table[0] = reinterpret_cast<std::uintptr_t>(&entity);
    check(entity_view(reinterpret_cast<std::uintptr_t>(&entity), membership_layout, member));
    check(entity_for_id(entity.id,membership_layout,by_id,by_id_slot)==EntityMembership::Present && by_id_slot==0);
    ++entity.generation;
    check(entity_membership(by_id,membership_layout)==EntityMembership::Absent);
    check(entity_for_id(entity.id,membership_layout,member,by_id_slot)==EntityMembership::Present &&
        member.generation==entity.generation);
    --entity.generation;
    membership_count = 3;
    check(!entity_view(reinterpret_cast<std::uintptr_t>(&entity), membership_layout, member));
    check(entity_for_id(entity.id,membership_layout,by_id,by_id_slot)==EntityMembership::Unresolved);
    membership_count = 4;
    {
        // An occupied chain exceeding the bounded probe budget is unresolved,
        // even though no matching ID was found. It must never imply deletion.
        std::vector<std::uint64_t> long_bits(256,UINT64_MAX);
        std::vector<std::uint32_t> long_keys(16384,99);
        std::vector<std::uintptr_t> long_table(16384);
        const auto saved_occupancy=occupied_pointer, saved_keys=keys_pointer, saved_table=membership_pointer;
        occupied_pointer=reinterpret_cast<std::uintptr_t>(long_bits.data());
        keys_pointer=reinterpret_cast<std::uintptr_t>(long_keys.data());
        membership_pointer=reinterpret_cast<std::uintptr_t>(long_table.data());
        membership_count=long_table.size();
        check(entity_for_id(entity.id,membership_layout,by_id,by_id_slot)==EntityMembership::Unresolved);
        occupied_pointer=saved_occupancy; keys_pointer=saved_keys; membership_pointer=saved_table;
        membership_count=4;
    }
    occupied = 1;
    // A stale source bucket can alias the pointer of a different active bucket.
    // Filter source occupancy before resolving pointers by their current ID.
    membership_table[3] = membership_table[0];
    std::vector<EntityPointer> membership_snapshot;
    check(entity_pointers(membership_layout, membership_snapshot) && membership_snapshot.size() == 1 &&
        membership_snapshot[0].pointer == reinterpret_cast<std::uintptr_t>(&entity) &&
        membership_snapshot[0].table_index == 0);
    occupied = 0;
    check(entity_pointers(membership_layout, membership_snapshot) &&
        membership_snapshot.empty());
    Component value{};
    std::array<std::uint8_t, 16> observed_uuid{};
    check(KfcRuntimeEcsTemplateUuid(handle, observed_uuid.data()) && observed_uuid == template_uuid);
    check(!KfcRuntimeEcsTemplateUuid(0, observed_uuid.data()));
    check(!KfcRuntimeEcsTemplateUuid(handle, nullptr));
    live_layout.entity_definition_uuid_validated = false;
    check(!KfcRuntimeEcsTemplateUuid(handle, observed_uuid.data()));
    live_layout.entity_definition_uuid_validated = true;
    const auto definition_before = entity.definition;
    entity.definition = 0;
    check(!KfcRuntimeEcsTemplateUuid(handle, observed_uuid.data()));
    entity.definition = 1;
    check(!KfcRuntimeEcsTemplateUuid(handle, observed_uuid.data()));
    entity.definition = definition_before;
    const auto uuid_before = template_uuid;
    template_uuid.fill(0);
    check(!KfcRuntimeEcsTemplateUuid(handle, observed_uuid.data()));
    template_uuid = uuid_before;
    check(KfcRuntimeEcsTemplateUuid(handle, observed_uuid.data()) && observed_uuid == template_uuid);
    check(KfcRuntimeEcsRead(handle, component_name, &value, sizeof(value)) && value.first == 42 && value.second == 84);
    std::array<std::uint8_t, sizeof(Component)> batch_output{};
    std::array<std::uint8_t, 1> batch_present{};
    const auto read_batch = [&] {
        return KfcRuntimeEcsReadMany(handle, names, component_size, 1,
            batch_output.data(), batch_output.size(), batch_present.data());
    };
    check(read_batch() && batch_present[0] == 1);
    check(std::memcmp(batch_output.data(), &component, sizeof(component)) == 0);
    // A registered candidate absent from the current archetype is different
    // from an unreadable or invalid present candidate.
    types.emplace("keen::ecs::FixtureAbsent", ComponentType{1, sizeof(Component)});
    const char* batch_names[]{component_name, "keen::ecs::FixtureAbsent"};
    const std::uint32_t batch_sizes[]{sizeof(Component), sizeof(Component)};
    std::array<std::uint8_t, 2 * sizeof(Component)> mixed_output;
    std::array<std::uint8_t, 2> mixed_present{9,9};
    mixed_output.fill(0x5a);
    check(KfcRuntimeEcsReadMany(handle, batch_names, batch_sizes, 2,
        mixed_output.data(), mixed_output.size(), mixed_present.data()));
    check(mixed_present[0] == 1 && mixed_present[1] == 0);
    check(std::memcmp(mixed_output.data(), &component, sizeof(component)) == 0);
    check(std::all_of(mixed_output.begin() + sizeof(Component), mixed_output.end(), [](auto byte) { return byte == 0; }));
    // Two present candidates share one private memory region. The page check
    // happens once before the copies and once at the end, rather than 3*N.
    auto* shared_components = static_cast<Component*>(VirtualAlloc(nullptr, 4096,
        MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE));
    check(shared_components != nullptr);
    shared_components[0] = {101, 202};
    shared_components[1] = {303, 404};
    const auto previous_storage = entity.storage;
    entity.storage = reinterpret_cast<std::uintptr_t>(shared_components);
    layout.bits = 3;
    layout.next_stride = sizeof(Component);
    page_query_count = 0;
    check(KfcRuntimeEcsReadMany(handle, batch_names, batch_sizes, 2,
        mixed_output.data(), mixed_output.size(), mixed_present.data()));
    check(page_query_count == 2 && mixed_present[0] == 1 && mixed_present[1] == 1);
    check(std::memcmp(mixed_output.data(), shared_components, mixed_output.size()) == 0);
    const auto rejected_region_batch = [&] {
        mixed_output.fill(0x5a);
        mixed_present.fill(9);
        const auto result = KfcRuntimeEcsReadMany(handle, batch_names, batch_sizes, 2,
            mixed_output.data(), mixed_output.size(), mixed_present.data());
        return !result && mixed_present[0] == 9 && mixed_present[1] == 9 &&
            std::all_of(mixed_output.begin(), mixed_output.end(), [](auto byte) { return byte == 0x5a; });
    };
    DWORD prior_region_protection{};
    check(VirtualProtect(shared_components, 4096, PAGE_NOACCESS, &prior_region_protection));
    page_query_count = 0;
    check(rejected_region_batch() && page_query_count == 1);
    check(VirtualProtect(shared_components, 4096, PAGE_READWRITE, &prior_region_protection));
    check(VirtualProtect(shared_components, 4096, PAGE_READWRITE | PAGE_GUARD, &prior_region_protection));
    page_query_count = 0;
    check(rejected_region_batch() && page_query_count == 1);
    check(VirtualProtect(shared_components, 4096, PAGE_READWRITE, &prior_region_protection));
    // A changed protection at final validation rejects even a still-readable
    // region. The cache is not a permission retained across calls.
    page_query_count = 0;
    mutate_on_page_query = 2;
    page_query_mutation = shared_components;
    page_query_protection = PAGE_READONLY;
    mutate_before_page_query = true;
    check(rejected_region_batch() && page_query_count == 2 && !page_query_mutation);
    check(VirtualProtect(shared_components, 4096, PAGE_READWRITE, &prior_region_protection));
    page_query_count = 0;
    page_query_mutation = shared_components;
    page_query_protection = PAGE_NOACCESS;
    check(rejected_region_batch() && page_query_count == 2 && !page_query_mutation);
    check(VirtualProtect(shared_components, 4096, PAGE_READWRITE, &prior_region_protection));
    // Losing access after the initial observation fails in the actual SEH
    // payload copy, before any output or final page validation can escape.
    page_query_count = 0;
    mutate_on_page_query = 1;
    page_query_mutation = shared_components;
    mutate_before_page_query = false;
    check(rejected_region_batch() && page_query_count == 1 && !page_query_mutation);
    check(VirtualProtect(shared_components, 4096, PAGE_READWRITE, &prior_region_protection));
    page_query_count = 0;
    check(KfcRuntimeEcsReadMany(handle, batch_names, batch_sizes, 2,
        mixed_output.data(), mixed_output.size(), mixed_present.data()) && page_query_count == 2);
    entity.storage = previous_storage;
    layout.next_stride = 0;
    check(VirtualFree(shared_components, 0, MEM_RELEASE));
    // A present second candidate with an invalid stride rejects the whole
    // batch, including the first candidate already copied internally.
    layout.bits = 3;
    mixed_output.fill(0x5a);
    mixed_present.fill(9);
    check(!KfcRuntimeEcsReadMany(handle, batch_names, batch_sizes, 2,
        mixed_output.data(), mixed_output.size(), mixed_present.data()));
    check(std::all_of(mixed_output.begin(), mixed_output.end(), [](auto byte) { return byte == 0x5a; }) &&
        mixed_present[0] == 9 && mixed_present[1] == 9);
    layout.bits = 1;
    types.erase("keen::ecs::FixtureAbsent");
    layout.bits = 0;
    check(read_batch() && batch_present[0] == 0);
    check(std::all_of(batch_output.begin(), batch_output.end(), [](auto byte) { return byte == 0; }));
    layout.bits = 1;
    batch_output.fill(0x5a);
    batch_present[0] = 9;
    const auto untouched_output = batch_output;
    auto wrong_size = std::uint32_t{sizeof(Component)-1};
    check(!KfcRuntimeEcsReadMany(handle, names, &wrong_size, 1,
        batch_output.data(), sizeof(Component)-1, batch_present.data()));
    check(batch_output == untouched_output && batch_present[0] == 9);
    check(!KfcRuntimeEcsReadMany(handle, names, component_size, 0,
        batch_output.data(), batch_output.size(), batch_present.data()));
    check(!KfcRuntimeEcsReadMany(handle, names, component_size, 65,
        batch_output.data(), batch_output.size(), batch_present.data()));
    check(!KfcRuntimeEcsReadMany(handle, names, component_size, 1,
        batch_output.data(), batch_output.size()+1, batch_present.data()));
    check(!KfcRuntimeEcsReadMany(handle, names, component_size, 1,
        batch_output.data(), EcsRuntime::MaximumReadManyBytes+1, batch_present.data()));
    check(!KfcRuntimeEcsReadMany(handle, nullptr, component_size, 1,
        batch_output.data(), batch_output.size(), batch_present.data()));
    check(!KfcRuntimeEcsReadMany(handle, names, nullptr, 1,
        batch_output.data(), batch_output.size(), batch_present.data()));
    check(!KfcRuntimeEcsReadMany(handle, names, component_size, 1,
        nullptr, batch_output.size(), batch_present.data()));
    check(!KfcRuntimeEcsReadMany(handle, names, component_size, 1,
        batch_output.data(), batch_output.size(), nullptr));
    component.first = 123;
    check(KfcRuntimeEcsRead(handle, component_name, &value, sizeof(value)) && value.first == 123);
    check(!KfcRuntimeEcsRead(handle, component_name, &value, sizeof(value) - 1));
    layout.stride = 0;
    check(!read_batch() && batch_output == untouched_output && batch_present[0] == 9);
    query_cache.clear();
    check(KfcRuntimeEcsQuery(names, 1, &handle, 1) == 1);
    check(!KfcRuntimeEcsRead(handle, component_name, &value, sizeof(value)));
    layout.stride = sizeof(Component);
    std::uint32_t identity{};
    check(KfcRuntimeEcsEntityIdentity(handle, &identity) && identity == entity.id);
    std::uint8_t mask[sizeof(Component)]{};
    check(!KfcRuntimeEcsWrite(handle, component_name, mask, &value, sizeof(value)));
    table[0] = 0;
    check(!read_batch());
    check(!KfcRuntimeEcsTemplateUuid(handle, observed_uuid.data()));
    check(!KfcRuntimeEcsRead(handle, component_name, &value, sizeof(value)));
    check(!KfcRuntimeEcsEntityIdentity(handle, &identity));
    table[0] = reinterpret_cast<std::uintptr_t>(&entity);
    ++entity.generation;
    check(!read_batch());
    check(!KfcRuntimeEcsTemplateUuid(handle, observed_uuid.data()));
    check(!KfcRuntimeEcsRead(handle, component_name, &value, sizeof(value)));
    --entity.generation;
    std::uintptr_t replacement[4]{table[0], 0, 0, 0};
    put(execution.manager, 392, reinterpret_cast<std::uintptr_t>(replacement));
    check(!KfcRuntimeEcsReady());
    check(KfcRuntimeWorldSessionId() == 0);
    check(!KfcRuntimeEcsEntityIdentity(handle, &identity));
    check(!KfcRuntimeEcsRead(handle, component_name, &value, sizeof(value)));
    EcsRuntime::Tick();
    check(!read_batch());
    check(KfcRuntimeEcsReady() && KfcRuntimeWorldSessionId() != session);
    check(!KfcRuntimeEcsRead(handle, component_name, &value, sizeof(value)));
    result = KfcRuntimeEcsQuery(names, 1, &handle, 1);
    check(result == 1 && KfcRuntimeEcsRead(handle, component_name, &value, sizeof(value)));
    scene_owner.count = 1;
    check(!KfcRuntimeEcsReady() && KfcRuntimeWorldSessionId() == 0);
    check(!KfcRuntimeEcsEntityIdentity(handle, &identity));
    EcsRuntime::Tick();
    check(KfcRuntimeEcsReady() && !KfcRuntimeEcsCanWrite());
    check(read_only_view.role == EngineContext::EcsRole::LocalAuthority);
    check(KfcRuntimeEcsQuery(names, 1, &handle, 1) == 1 &&
        KfcRuntimeEcsRead(handle, component_name, &value, sizeof(value)));
    scene_owner.count = 2;
    check(!KfcRuntimeEcsReady() && KfcRuntimeWorldSessionId() == 0);
    scene_owner.count = 1;
    scenes[0].state = 4;
    check(!KfcRuntimeEcsReady());
    scenes[0].state = 5;
    ++scenes[0].uuid[0];
    check(!KfcRuntimeEcsReady());
    --scenes[0].uuid[0];
    auto context = std::make_shared<int>(0);
    check(!invoke_read_operation([](void*) { root_object.pointer = 0; }, context));
    check(!KfcRuntimeEcsReady() && !KfcRuntimeEcsCanWrite() && KfcRuntimeWorldSessionId() == 0);
    check(!KfcRuntimeEcsEntityIdentity(handle, &identity));
    check(!KfcRuntimeEcsRead(handle, component_name, &value, sizeof(value)));
    // The generic write bridge is enabled only for these private objects. Its
    // outer row registration, Scene gate and entity CAS are production code.
    root_object.pointer = reinterpret_cast<std::uintptr_t>(&client);
    profile["provenance"]["ecsWriteGateValidated"] = true;
    profile["provenance"]["ecsChangeTrackingValidated"] = true;
    profile["ecsWriteLayout"] = {
        {"id","ecs-native-thread-scene-gate-v1"}, {"enabled",true},
        {"phasePolicy","untracked-components"},
        {"threadRegistryLockRva",reinterpret_cast<std::uintptr_t>(&thread_registry_gate)-base},
        {"threadRegistryEntriesRva",reinterpret_cast<std::uintptr_t>(thread_rows)-base},
        {"threadRegistryCountRva",reinterpret_cast<std::uintptr_t>(&thread_count)-base},
        {"threadRegistryCapacity",2}, {"threadRegistryEntryStride",sizeof(ThreadRow)},
        {"registryEntryHandleOffset",offsetof(ThreadRow,handle)},
        {"registryEntryNamePointerOffset",offsetof(ThreadRow,name)},
        {"registryEntryNameLengthOffset",offsetof(ThreadRow,length)},
        {"threadInlineNameOffset",offsetof(NativeThread,text)}, {"sceneThreadOffset",offsetof(SceneOwner,thread)},
        {"threadNamePointerOffset",offsetof(NativeThread,name)}, {"threadNameLengthOffset",offsetof(NativeThread,length)},
        {"threadNameCapacityOffset",offsetof(NativeThread,capacity)}, {"threadNameCapacity",64},
        {"threadHandleOffset",offsetof(NativeThread,handle)}, {"threadContextOffset",offsetof(NativeThread,context)},
        {"threadStopOffset",offsetof(NativeThread,stop)}, {"threadCallbackOffset",offsetof(NativeThread,callback)},
        {"threadCallbackRva",reinterpret_cast<std::uintptr_t>(fixture_server_callback)-base}, {"threadName","GameServer"},
        {"sceneContextOffset",offsetof(SceneOwner,loop)}, {"sceneLockOffset",offsetof(SceneOwner,gate)},
        {"sceneStateOffset",offsetof(SceneOwner,state)}, {"sceneRunningState",1},
        {"executionModeOffset",offsetof(Execution,manager)+8}, {"executionAuthorityMode",0}, {"maximumWriteBytes",64},
        {"changeTrackerRva",reinterpret_cast<std::uintptr_t>(fixture_track_change)-base},
        {"changeTrackerContextOffset",offsetof(Execution,tracker)-offsetof(Execution,manager)},
        {"changeTrackerManagerOffset",offsetof(Execution::Tracker,manager)},
        {"changeTrackerQueueBOffset",sizeof(Execution::Queue)*8}, {"maximumTrackingQueueCapacity",64},
        {"changeTrackerRegistryAOffset",400}, {"changeTrackerRegistryBOffset",560}
    };
    std::string tracker_bytes;
    constexpr char hex[] = "0123456789abcdef";
    const auto* tracker_code = reinterpret_cast<const unsigned char*>(fixture_track_change);
    for (std::size_t index = 0; index < 33; ++index) {
        tracker_bytes += hex[tracker_code[index] >> 4];
        tracker_bytes += hex[tracker_code[index] & 15];
    }
    profile["ecsWriteLayout"]["changeTrackerBytes"] = tracker_bytes;
    // A valid recorder ABI and memory gate do not establish when the original
    // engine consumes its current-epoch queues. The private fixture owns that
    // phase explicitly. The production image must supply separate evidence.
    check(!EngineContext::Configure(profile, error) &&
        error == "ECS writes lack original command/epoch phase validation");
    profile["ecsWriteLayout"]["phaseValidated"] = true;
    auto invalid_policy = profile;
    invalid_policy["ecsWriteLayout"]["phasePolicy"] = "private-controlled";
    check(!EngineContext::Configure(invalid_policy, error));
    tracker_scene_gate = &scene_owner.gate;
    check(EngineContext::Configure(profile,error));
    EcsRuntime::Tick();
    check(KfcRuntimeEcsReady() && KfcRuntimeEcsCanWrite() && !GameThreadDispatcher::Ready());
    check(KfcRuntimeEcsQuery(names,1,&handle,1)==1);
    check(EngineContext::EcsComponentWriteSupported(read_only_view,0));
    check(!EngineContext::EcsComponentWriteSupported(read_only_view,1));
    const Component expected = component;
    Component changed{777,888};
    std::fill(std::begin(mask),std::end(mask),std::uint8_t{1});
    // Both registry tracking categories are checked dynamically. A type can be
    // writable while untracked, then become blocked without changing its name.
    put(authority_execution.registry,400,std::uint64_t{1});
    check(!EngineContext::EcsComponentWriteSupported(read_only_view,0));
    check(KfcRuntimeEcsCompareExchange(handle,component_name,&expected,mask,&changed,sizeof(changed))==0 &&
        component.first==expected.first && tracker_calls==0);
    check(KfcRuntimeEcsCompareExchange(handle,component_name,&expected,mask,&expected,sizeof(expected))==0 && tracker_calls==0);
    put(authority_execution.registry,400,std::uint64_t{0});
    put(authority_execution.registry,560,std::uint64_t{1});
    check(!EngineContext::EcsComponentWriteSupported(read_only_view,0));
    check(KfcRuntimeEcsCompareExchange(handle,component_name,&expected,mask,&changed,sizeof(changed))==0 &&
        component.first==expected.first && tracker_calls==0);
    put(authority_execution.registry,560,std::uint64_t{0});
    check(EngineContext::EcsComponentWriteSupported(read_only_view,0));
    check(KfcRuntimeEcsCompareExchange(handle,component_name,&expected,mask,&changed,sizeof(changed))==1);
    check(component.first==777 && component.second==888 && thread_registry_gate==0 && scene_owner.gate==0);
    check(tracker_calls==1 && tracker_contract && tracker_before==expected.first);
    check(KfcRuntimeEcsCompareExchange(handle,component_name,&expected,mask,&changed,sizeof(changed))==2);
    check(tracker_calls==1 && thread_registry_gate==0 && scene_owner.gate==0);
    check(KfcRuntimeEcsCompareExchange(handle,component_name,&changed,mask,&changed,sizeof(changed))==1 && tracker_calls==1);
    scene_owner.gate = 1;
    check(!KfcRuntimeEcsWrite(handle,component_name,mask,&expected,sizeof(expected)) && thread_registry_gate==0 && scene_owner.gate==1);
    scene_owner.gate = 0;
    thread_registry_gate = static_cast<LONG>(0x80000000u);
    check(!KfcRuntimeEcsWrite(handle,component_name,mask,&expected,sizeof(expected)) && scene_owner.gate==0);
    thread_registry_gate = 0;
    thread_count = 0;
    check(!KfcRuntimeEcsWrite(handle,component_name,mask,&expected,sizeof(expected)) && thread_registry_gate==0);
    thread_count = 1;
    scene_owner.thread.stop = 1;
    check(!KfcRuntimeEcsWrite(handle,component_name,mask,&expected,sizeof(expected)) && thread_registry_gate==0 && scene_owner.gate==0);
    scene_owner.thread.stop = 0;
    const auto original_scene = client.scene;
    client.scene = 0;
    check(!KfcRuntimeEcsCanWrite());
    check(!KfcRuntimeEcsWrite(handle,component_name,mask,&expected,sizeof(expected)) && thread_registry_gate==0 && scene_owner.gate==0);
    client.scene = original_scene;
    ++entity.generation;
    check(!KfcRuntimeEcsWrite(handle,component_name,mask,&expected,sizeof(expected)) && thread_registry_gate==0 && scene_owner.gate==0);
    --entity.generation;
    thread_rows[1] = thread_rows[0]; thread_count = 2;
    check(!KfcRuntimeEcsWrite(handle,component_name,mask,&expected,sizeof(expected)) && thread_registry_gate==0 && scene_owner.gate==0);
    thread_count = 1;
    authority_execution.tracker.queues[8].count = 4;
    check(KfcRuntimeEcsWrite(handle,component_name,mask,&expected,sizeof(expected)) && tracker_calls==2);
    check(thread_registry_gate==0 && scene_owner.gate==0 && component.first==expected.first);
    authority_execution.tracker.queues[8].count = 0;
    check(KfcRuntimeEcsWrite(handle,component_name,mask,&expected,sizeof(expected)) && component.first==expected.first);
    check(tracker_calls==2 && tracker_contract && tracker_before==777);
    check(thread_registry_gate==0 && scene_owner.gate==0);
    // A private page supplies deterministic fault paths, without product hooks.
    auto* protected_component = static_cast<Component*>(VirtualAlloc(nullptr,4096,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE));
    check(protected_component != nullptr);
    *protected_component = expected;
    const auto original_storage = entity.storage;
    entity.storage = reinterpret_cast<std::uintptr_t>(protected_component);
    DWORD old_protection{};
    batch_output.fill(0x5a);
    batch_present[0] = 9;
    check(VirtualProtect(protected_component,4096,PAGE_NOACCESS,&old_protection) != FALSE);
    check(!read_batch() && batch_output == untouched_output && batch_present[0] == 9);
    check(VirtualProtect(protected_component,4096,PAGE_READWRITE,&old_protection) != FALSE);
    check(VirtualProtect(protected_component,4096,PAGE_READONLY,&old_protection) != FALSE);
    check(KfcRuntimeEcsCompareExchange(handle,component_name,&expected,mask,&changed,sizeof(changed))==0);
    check(protected_component->first==expected.first && thread_registry_gate==0 && scene_owner.gate==0);
    check(VirtualProtect(protected_component,4096,PAGE_READWRITE,&old_protection) != FALSE);
    tracker_fault_page = protected_component;
    check(KfcRuntimeEcsCompareExchange(handle,component_name,&expected,mask,&changed,sizeof(changed))==3);
    check(tracker_fault_applied && thread_registry_gate==0 && scene_owner.gate==0 &&
        std::strcmp(last_write_reason.load(),"ecs-write-outcome-uncertain-rollback-unverified")==0);
    tracker_fault_page = nullptr;
    check(VirtualProtect(protected_component,4096,PAGE_READWRITE,&old_protection) != FALSE);
    // An unconfirmed rollback disables every generic ECS writer in this real
    // binding epoch, including callers outside the mod that caused the fault.
    check(!KfcRuntimeEcsCanWrite() && !KfcRuntimeEcsCanWriteComponent(component_name));
    check(KfcRuntimeEcsCompareExchange(handle,component_name,&expected,mask,&changed,sizeof(changed))==0);
    check(!KfcRuntimeEcsWrite(handle,component_name,mask,&changed,sizeof(changed)));
    check(nlohmann::json::parse(EcsRuntime::Diagnostics())["writeFault"]==true);
    entity.storage = original_storage;
    check(VirtualFree(protected_component,0,MEM_RELEASE) != FALSE);
    // A real owner/table transition advances the existing epoch and releases
    // the fault. Merely checking readiness or retrying never advances it.
    const auto fault_session = KfcRuntimeWorldSessionId();
    client.scene = 0;
    EcsRuntime::Tick();
    client.scene = original_scene;
    EcsRuntime::Tick();
    check(KfcRuntimeWorldSessionId()!=fault_session && KfcRuntimeEcsCanWrite());
    check(nlohmann::json::parse(EcsRuntime::Diagnostics())["writeFault"]==false);

    // Editor reads use the original owner binding even without a dispatcher.
    // Client input deliberately belongs to a different manager and entity.
    struct EditorLayout {
        std::uint64_t bits{15};
        std::uint16_t offsets[4]{}, strides[4]{4,64,56,8};
    } editor_layout;
    struct EditorTransform {
        std::int64_t position[3]{1ll<<32,2ll<<32,3ll<<32};
        float rotation[4]{0,0,0,1}, scale[3]{1,1,1};
        std::uint32_t padding{};
    };
    struct EditorStorage { std::uint32_t item{42}; std::uint8_t input[64]{}; EditorTransform transform; std::uint8_t stock[8]{}; };
    EditorStorage editor_storage, input_storage, second_input_storage;
    std::fill(std::begin(editor_storage.input), std::end(editor_storage.input), std::uint8_t{0x6b});
    std::fill(std::begin(input_storage.input), std::end(input_storage.input), std::uint8_t{0x2a});
    input_storage.stock[0] = 1;
    editor_layout.offsets[0] = offsetof(EditorStorage,item);
    editor_layout.offsets[1] = offsetof(EditorStorage,input);
    editor_layout.offsets[2] = offsetof(EditorStorage,transform);
    editor_layout.offsets[3] = offsetof(EditorStorage,stock);
    entity.layout = reinterpret_cast<std::uintptr_t>(&editor_layout);
    entity.storage = reinterpret_cast<std::uintptr_t>(&editor_storage);
    Entity input_entity = entity;
    input_entity.id = 23;
    input_entity.storage = reinterpret_cast<std::uintptr_t>(&input_storage);
    std::uintptr_t client_table[40]{reinterpret_cast<std::uintptr_t>(&input_entity)};
    put(execution.manager,392,reinterpret_cast<std::uintptr_t>(client_table));
    const char* editor_names[]{used_item_component_name,input_component_name,transform_component_name,local_marker_component_name};
    std::uint32_t editor_sizes[]{4,64,56,8};
    unsigned char editor_registration[4 * 256]{};
    std::uint16_t editor_storage_sizes[]{4,64,56,8};
    std::uintptr_t editor_types[4]{};
    std::uint64_t editor_callbacks[20]{};
    for (std::size_t index = 0; index < 4; ++index) {
        const auto descriptor_address = reinterpret_cast<std::uintptr_t>(editor_descriptors[index]);
        const auto name_address = reinterpret_cast<std::uintptr_t>(editor_names[index]);
        const auto name_size = std::strlen(editor_names[index]);
        const auto name_hash = ComponentRegistry::hash_name(editor_names[index]);
        put(editor_descriptors[index],0x20,name_address);
        put(editor_descriptors[index],0x28,static_cast<std::uint64_t>(name_size));
        put(editor_descriptors[index],0x40,editor_sizes[index]);
        put(editor_descriptors[index],0x50,name_hash);
        auto* record = editor_registration + index * 256;
        put(record,16,name_address);
        put(record,24,static_cast<std::uint64_t>(name_size + 1));
        put(record,32,name_hash);
        put(record,40,descriptor_address);
        put(record,56,editor_sizes[index]);
        editor_types[index] = descriptor_address;
    }
    for (auto* owner_registry : {execution.registry,authority_execution.registry}) {
        put(owner_registry,8,reinterpret_cast<std::uintptr_t>(editor_registration));
        put(owner_registry,16,std::uint64_t{4}); put(owner_registry,24,std::uint64_t{4});
        put(owner_registry,232,reinterpret_cast<std::uintptr_t>(editor_storage_sizes)); put(owner_registry,240,std::uint64_t{4});
        put(owner_registry,256,reinterpret_cast<std::uintptr_t>(editor_types)); put(owner_registry,264,std::uint64_t{4});
        put(owner_registry,280,reinterpret_cast<std::uintptr_t>(editor_callbacks)); put(owner_registry,288,std::uint64_t{4});
        put(owner_registry,400,std::uint64_t{0}); put(owner_registry,560,std::uint64_t{0});
    }
    profile["ecsReadLayout"]["componentBits"] = offsetof(EditorLayout,bits);
    profile["ecsReadLayout"]["componentOffsets"] = offsetof(EditorLayout,offsets);
    profile["ecsReadLayout"]["componentStrides"] = offsetof(EditorLayout,strides);
    profile["ecsReadLayout"]["maximumEntities"] = 40;
    check(EngineContext::Configure(profile,error));
    check(KfcRuntimeEcsConfigure(editor_names,editor_sizes,4));
    EcsRuntime::Tick();
    check(read_only_binding && !GameThreadDispatcher::Ready() && KfcRuntimeEcsPropQueryReady());
    KfcRuntimePropRecipe recipe{42,{-.5f,-.5f,-.5f,.5f,.5f,.5f},7};
    check(KfcRuntimeWorldEntityRegisterPropRecipes(&recipe,1));
    double editor_bounds[]{0,0,0,10,10,10};
    KfcRuntimePropRecord editor_prop{};
    check(KfcRuntimeWorldEntityQueryPropsInBounds(editor_bounds,nullptr,0)==1);
    check(KfcRuntimeWorldEntityQueryPropsInBounds(editor_bounds,&editor_prop,1)==1 && editor_prop.item_id==42);
    KfcRuntimeCompatibility::EnshroudedClient::definition_uuid = 64; // Must use owner-profile UUID offset instead.
    KfcRuntimePropRecord editor_observed{};
    check(KfcRuntimeWorldEntityGetTransform(editor_prop.entity_handle,&editor_observed));
    check(std::memcmp(editor_observed.template_uuid,template_uuid.data(),template_uuid.size())==0);
    const auto status_session=KfcRuntimeWorldSessionId();
    // Exercise the optional session status with the fixture's original ID
    // hash chain. Keep unreadable data separate from a proven empty bucket.
    occupied=9; keys[3]=99; keys[0]=entity.id;
    live_layout.occupancy_address=reinterpret_cast<std::uintptr_t>(&occupied_pointer);
    live_layout.keys_address=reinterpret_cast<std::uintptr_t>(&keys_pointer);
    KfcRuntimePropRecord status_prop{};
    check(KfcRuntimeWorldEntityReadTransformForSession(editor_prop.entity_handle,status_session,&status_prop)==1);
    const auto status_before=status_prop;
    check(KfcRuntimeWorldEntityReadTransformForSession(editor_prop.entity_handle,status_session+1,&status_prop)==4);
    occupied=8;
    check(KfcRuntimeWorldEntityReadTransformForSession(editor_prop.entity_handle,status_session,&status_prop)==6);
    check(std::memcmp(&status_prop,&status_before,sizeof(status_prop))==0);
    occupied=9;
    const auto status_entity=table[0]; table[0]=1;
    check(KfcRuntimeWorldEntityReadTransformForSession(editor_prop.entity_handle,status_session,&status_prop)==5);
    check(std::memcmp(&status_prop,&status_before,sizeof(status_prop))==0);
    table[0]=status_entity;
    ++entity.generation;
    check(KfcRuntimeWorldEntityReadTransformForSession(editor_prop.entity_handle,status_session,&status_prop)==6);
    --entity.generation;
    live_layout.occupancy_address=live_layout.keys_address=0;
    const auto editor_before = editor_storage;
    const double requested_scale[]{2,3,4};
    const auto previous_tracker_calls = tracker_calls;
    expected_tracker_index = 2;
    check(KfcRuntimeWorldEntitySetScale(editor_prop.entity_handle,requested_scale));
    check(tracker_calls==previous_tracker_calls+1 && tracker_contract && thread_registry_gate==0 && scene_owner.gate==0);
    check(editor_storage.transform.scale[0]==2 && editor_storage.transform.scale[1]==3 && editor_storage.transform.scale[2]==4);
    check(std::memcmp(editor_storage.transform.position,editor_before.transform.position,sizeof(editor_before.transform.position))==0 &&
        std::memcmp(editor_storage.transform.rotation,editor_before.transform.rotation,sizeof(editor_before.transform.rotation))==0);
    put(authority_execution.registry,400,std::uint64_t{1}<<2);
    check(!KfcRuntimeWorldEntitySetScale(editor_prop.entity_handle,requested_scale) && tracker_calls==previous_tracker_calls+1);
    put(authority_execution.registry,400,std::uint64_t{0});
    const auto editor_session = KfcRuntimeWorldSessionId();
    std::array<std::uint8_t,64> input_bytes{};
    std::uint64_t input_sequence{};
    std::uint32_t input_status{};
    for (unsigned attempt=0; attempt<20; ++attempt) {
        input_status=KfcRuntimeWorldCursorReadInput(input_bytes.size(),editor_session,input_bytes.data(),input_bytes.size(),&input_sequence);
        if (input_status!=5) break;
    }
    check(input_status==1 && input_sequence>0 && std::all_of(input_bytes.begin(),input_bytes.end(),[](auto byte) { return byte==0x2a; }));
    check(WorldRuntime::OperationAvailable("runtime.world.cursor.get"));
    std::array<std::uint8_t,0xa0> legacy_cursor{};
    check(!KfcRuntimeWorldCursorRead(legacy_cursor.data(),legacy_cursor.size(),&input_sequence));
    const auto untouched_input=input_bytes;
    const auto untouched_sequence=input_sequence;
    const auto read_input_until_done=[&](std::uint64_t session){
        std::uint32_t status=5;
        for (unsigned attempt=0; attempt<20 && status==5; ++attempt)
            status=KfcRuntimeWorldCursorReadInput(input_bytes.size(),session,input_bytes.data(),input_bytes.size(),&input_sequence);
        return status;
    };
    check(KfcRuntimeWorldCursorReadInput(input_bytes.size(),editor_session+1,input_bytes.data(),input_bytes.size(),&input_sequence)==4);
    check(input_bytes==untouched_input && input_sequence==untouched_sequence);
    check(KfcRuntimeWorldCursorReadInput(input_bytes.size(),editor_session,input_bytes.data(),0,&input_sequence)==2);
    check(KfcRuntimeWorldCursorReadInput(input_bytes.size()-1,editor_session,input_bytes.data(),input_bytes.size()-1,&input_sequence)==3);
    table[0]=0;
    check(!KfcRuntimeWorldEntityGetTransform(editor_prop.entity_handle,&editor_observed));
    table[0]=reinterpret_cast<std::uintptr_t>(&entity);
    ++entity.generation;
    check(!KfcRuntimeWorldEntityGetTransform(editor_prop.entity_handle,&editor_observed));
    Entity second_input = input_entity;
    second_input.id = 24;
    second_input.storage = reinterpret_cast<std::uintptr_t>(&second_input_storage);
    client_table[1]=reinterpret_cast<std::uintptr_t>(&second_input);
    std::uintptr_t next_authority_table[4]{reinterpret_cast<std::uintptr_t>(&entity)};
    put(authority_execution.manager,392,reinterpret_cast<std::uintptr_t>(next_authority_table));
    EcsRuntime::Tick();
    const auto selected_session=KfcRuntimeWorldSessionId();
    check(selected_session && selected_session!=editor_session);
    const auto selected_status=read_input_until_done(selected_session);
    if (selected_status!=1) {
        char detail[512]{};
        KfcRuntimeWorldCursorInputDetail(detail,sizeof(detail));
        std::fprintf(stderr,"Local cursor selection status %u: %s\n",selected_status,detail);
    }
    check(selected_status==1);
    check(input_bytes==untouched_input && input_sequence>untouched_sequence);
    const auto selected_sequence=input_sequence;
    second_input_storage.stock[0] = 1;
    put(authority_execution.manager,392,table_pointer);
    EcsRuntime::Tick();
    const auto ambiguous_session=KfcRuntimeWorldSessionId();
    check(ambiguous_session && ambiguous_session!=selected_session);
    check(read_input_until_done(ambiguous_session)==6);
    check(input_bytes==untouched_input && input_sequence==selected_sequence);
    // An unreadable occupied candidate cannot be treated as an absent type.
    client_table[1]=1;
    put(authority_execution.manager,392,reinterpret_cast<std::uintptr_t>(next_authority_table));
    EcsRuntime::Tick();
    const auto unreadable_session=KfcRuntimeWorldSessionId();
    check(KfcRuntimeWorldCursorReadInput(input_bytes.size(),unreadable_session,input_bytes.data(),input_bytes.size(),&input_sequence)==5);
    check(input_bytes==untouched_input && input_sequence==selected_sequence);
    client_table[1]=0;
    check(KfcRuntimeWorldCursorReadInput(input_bytes.size(),unreadable_session,input_bytes.data(),input_bytes.size(),&input_sequence)==1);
    client_table[0]=0;
    const auto stale_input_sequence=input_sequence;
    check(KfcRuntimeWorldCursorReadInput(input_bytes.size(),unreadable_session,input_bytes.data(),input_bytes.size(),&input_sequence)==5);
    check(input_bytes==untouched_input && input_sequence==stale_input_sequence);
    std::array<Entity,33> overflow_entities{};
    std::array<EditorStorage,33> overflow_storage{};
    for (std::size_t index = 0; index < overflow_entities.size(); ++index) {
        overflow_entities[index] = input_entity;
        overflow_entities[index].id = static_cast<std::uint32_t>(100 + index);
        overflow_entities[index].storage = reinterpret_cast<std::uintptr_t>(&overflow_storage[index]);
        client_table[index] = reinterpret_cast<std::uintptr_t>(&overflow_entities[index]);
    }
    put(execution.manager,344,std::uint64_t{34});
    check(read_input_until_done(unreadable_session)==6);
    check(input_bytes==untouched_input && input_sequence==stale_input_sequence);
    using WorldRuntime::Implementation::WriteRegion;
    using WorldRuntime::Implementation::plan_write_region;
    WriteRegion region;
    std::int32_t partial_origin[]{7,-1,8}; std::uint32_t partial_size[]{2,1,1};
    check(plan_write_region(partial_origin,partial_size,region));
    check((region.origin==std::array<std::int32_t,3>{0,-8,8}));
    check((region.dimensions==std::array<std::uint32_t,3>{16,8,8}));
    check((region.offset==std::array<std::size_t,3>{7,7,0}) && region.count==1024);
    std::int32_t full_origin[]{0,0,0}; std::uint32_t full_size[]{32,32,64};
    check(plan_write_region(full_origin,full_size,region) && region.count==65536);
    const auto unchanged_region=region;
    full_origin[0]=1;
    check(!plan_write_region(full_origin,full_size,region));
    check(region.origin==unchanged_region.origin && region.count==unchanged_region.count);
    full_origin[0]=INT32_MAX;
    check(!plan_write_region(full_origin,full_size,region));
    EngineContext::Shutdown();
    std::printf("%u ECS checks passed, including private original-gate writes and CAS\n", checks);
}
