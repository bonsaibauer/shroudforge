// Real service exports, SessionGuard and SceneGuard against private objects.
// This executable never opens a game process or calls a gameplay function.
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
#include "named-services/named_service_contract.h"
#include "voxels/read_registered_voxel_grid.h"
#include "engine-context/engine_context_state.h"
#include <cstdio>
#include <cstdlib>
#include <source_location>

namespace {
unsigned checks{};
void check(bool ok, std::source_location location = std::source_location::current()) {
    ++checks;
    if (std::getenv("SHROUDFORGE_NATIVE_TEST_TRACE"))
        std::fprintf(stderr,"service check %u at line %u: %s\n",checks,location.line(),ok ? "passed" : "failed");
    if (!ok) { std::fprintf(stderr, "service check %u failed at line %u\n", checks, location.line()); std::exit(1); }
}
template<class T> void put(void* target, std::size_t offset, const T& value) {
    std::memcpy(static_cast<unsigned char*>(target) + offset, &value, sizeof(value));
}
struct Owner { std::uintptr_t vtable{}, pointer{}, scene{}; } root;
struct Controller { std::uintptr_t vtable{}, client{}, main{}; };
struct Context { std::uintptr_t execution{}; };
struct Thread {
    std::uintptr_t name{};
    std::uint64_t length{10}, capacity{64};
    char text[64]{"GameServer"};
    std::uintptr_t handle{123}, context{};
    std::uint32_t stop{};
    std::uintptr_t callback{};
};
struct Scene {
    Thread thread;
    std::uintptr_t entries{};
    std::uint64_t count{1}, capacity{1};
    alignas(4) LONG gate{};
    std::uint8_t state{1};
    std::uint64_t loop{};
};
struct Entry {
    std::uintptr_t context{};
    std::uint64_t uuid[2]{11,22};
    std::uint8_t state{5};
    unsigned char padding[119]{};
};
struct ThreadRow { std::uintptr_t handle{}, name{}; std::uint64_t length{}; } thread_rows[1];
alignas(4) LONG thread_gate{};
std::uint16_t thread_count{1};
struct Execution { unsigned char registry[4096]{}, manager[400]{}; };
struct ServicePayload {
    std::uint8_t enabled{1}, sibling{7}, padding[2]{0x44,0x55};
    std::uint32_t count{84}, other{123}, tail{0xabcdef01};
    bool operator==(const ServicePayload&) const = default;
};
struct StringView { std::uintptr_t text{}; std::uint64_t length{}; };
std::uintptr_t root_vtable[1]{}, client_vtable[1]{}, controller_vtable[1]{};
unsigned char metadata[0x90]{}, descriptor[0xd0]{};
std::uintptr_t metadata_slot{};
const char service_name[]{"ecs::FixtureSettings"};
const char collision_name[]{"ecs::FixtureSettinGs"};
StringView mutable_names[1]{{reinterpret_cast<std::uintptr_t>(service_name), sizeof(service_name)-1}};
void fixture_callback() {}
void fixture_mutable_system() {}
void fixture_unused_tracker() {}
constexpr std::uint32_t type_hash = 0x11223344;
constexpr std::size_t record_stride = 0xe8, system_stride = 0x450, systems_offset = 0x300;
unsigned comparison_calls{}, comparison_mode{};
bool comparison_gates_free{};
Scene* comparison_scene{};
unsigned voxel_origin_calls{}, voxel_ready_calls{}, voxel_cell_calls{}, voxel_mode{};
unsigned char* voxel_fixture_world{};
unsigned char* voxel_fixture_grid{};
std::uint32_t* voxel_fixture_handles{};

void check_voxel_gates() {
    LONG world_lock{};
    std::memcpy(&world_lock,voxel_fixture_world+0x174,sizeof(world_lock));
    check(comparison_scene && comparison_scene->gate==static_cast<LONG>(0x80000000u) &&
        thread_gate==1 && world_lock==0);
}
void* __fastcall fixture_voxel_origin(void* result, const void* wrapper) {
    ++voxel_origin_calls;
    check_voxel_gates();
    std::uintptr_t world{};
    std::memcpy(&world,static_cast<const unsigned char*>(wrapper)+8,sizeof(world));
    check(world==reinterpret_cast<std::uintptr_t>(voxel_fixture_world));
    std::memcpy(result,voxel_fixture_world+0x30,16);
    return voxel_mode==6 ? nullptr : result;
}
bool __fastcall fixture_voxel_ready(const void*, const void* position) {
    ++voxel_ready_calls;
    check_voxel_gates();
    std::array<float,4> point{};
    std::memcpy(point.data(),position,sizeof(point));
    check((point[0]==10.25f || point[0]==14.25f) && point[1]==20 &&
        (point[2]==30.25f || point[2]==34.25f));
    if (voxel_mode==4) voxel_fixture_handles[0]=0;
    return voxel_mode!=1;
}
bool __fastcall fixture_voxel_cells(void* request, const void*) {
    ++voxel_cell_calls;
    check_voxel_gates();
    std::uint16_t* cells{};
    std::uint64_t count{};
    std::uint32_t mode{};
    std::memcpy(&cells,request,sizeof(cells));
    std::memcpy(&count,static_cast<unsigned char*>(request)+8,sizeof(count));
    std::memcpy(&mode,static_cast<unsigned char*>(request)+28,sizeof(mode));
    check(mode==6 && count==8);
    for (std::size_t i=0;i<count;++i) cells[i]=static_cast<std::uint16_t>(0x3412+i);
    if (voxel_mode==5) put(voxel_fixture_grid,48,std::uintptr_t{});
    return voxel_mode!=2;
}

void* __fastcall fixture_is_same_entity(void* result, const void* argument_view, const void* vm_context) {
    ++comparison_calls;
    {
        std::unique_lock state(state_mutex,std::try_to_lock);
        std::unique_lock services(NamedServiceImplementation::operation_mutex,std::try_to_lock);
        std::unique_lock owners(EngineContextImplementation::context_mutex,std::try_to_lock);
        comparison_gates_free = state.owns_lock() && services.owns_lock() && owners.owns_lock() &&
            thread_gate==0 && comparison_scene && comparison_scene->gate==0;
    }
    check(comparison_gates_free);
    if (comparison_mode==1) RaiseException(0xe0000042,0,0,nullptr);
    std::uintptr_t indices{}, variables{}, data{}, execution_context{};
    std::uint64_t argument_count{}, variable_count{}, data_extent{};
    std::array<std::uintptr_t,2> reserved{};
    std::memcpy(&indices,argument_view,sizeof(indices));
    std::memcpy(&argument_count,static_cast<const unsigned char*>(argument_view)+8,sizeof(argument_count));
    std::memcpy(&variables,vm_context,sizeof(variables));
    std::memcpy(&variable_count,static_cast<const unsigned char*>(vm_context)+8,sizeof(variable_count));
    std::memcpy(&data,static_cast<const unsigned char*>(vm_context)+16,sizeof(data));
    std::memcpy(&data_extent,static_cast<const unsigned char*>(vm_context)+24,sizeof(data_extent));
    std::memcpy(reserved.data(),static_cast<const unsigned char*>(vm_context)+32,sizeof(reserved));
    std::memcpy(&execution_context,static_cast<const unsigned char*>(vm_context)+48,sizeof(execution_context));
    check(variable_count==3 && data_extent==8 && reserved==std::array<std::uintptr_t,2>{} && !execution_context);
    std::uint32_t index_a{}, index_b{}, left{}, right{};
    std::memcpy(&index_a,reinterpret_cast<const void*>(indices),4);
    std::memcpy(&index_b,reinterpret_cast<const void*>(indices+4),4);
    check(argument_count==2 && index_a==1 && index_b==2);
    std::uint16_t offset_a{}, offset_b{}, size_a{}, size_b{};
    std::memcpy(&size_a,reinterpret_cast<const void*>(variables+24+12),2);
    std::memcpy(&size_b,reinterpret_cast<const void*>(variables+48+12),2);
    std::memcpy(&offset_a,reinterpret_cast<const void*>(variables+24+14),2);
    std::memcpy(&offset_b,reinterpret_cast<const void*>(variables+48+14),2);
    check(size_a==4 && size_b==4 && offset_a==0 && offset_b==4);
    std::memcpy(&left,reinterpret_cast<const void*>(data+offset_a),4);
    std::memcpy(&right,reinterpret_cast<const void*>(data+offset_b),4);
    put(result,0,std::uint8_t{0}); put(result,4,std::uint32_t{0});
    put(result,8,static_cast<std::uint8_t>(left==right));
    if (comparison_mode==2) return nullptr;
    if (comparison_mode==3) put(result,0,std::uint8_t{1});
    if (comparison_mode==4) put(result,4,std::uint32_t{1});
    if (comparison_mode==5) put(result,8,std::uint8_t{2});
    if (comparison_mode==6) put(result,12,std::uint8_t{0});
    if (comparison_mode==7) put(reinterpret_cast<void*>(data),0,left^1);
    if (comparison_mode==8) ++layout_epoch;
    if (comparison_mode==9) put(const_cast<void*>(vm_context),8,std::uint64_t{4});
    if (comparison_mode==10) put(const_cast<void*>(vm_context),32,std::uintptr_t{1});
    if (comparison_mode==11) put(const_cast<void*>(vm_context),48,std::uintptr_t{1});
    return result;
}
}

int main(int argc, char** argv) {
    const auto base = reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base);
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(base + dos->e_lfanew);
    image_base = base; image_size = nt->OptionalHeader.SizeOfImage;
    KfcRuntimeCompatibility::EnshroudedClient::image_size = image_size;
    KfcRuntimeCompatibility::EnshroudedClient::image_timestamp = nt->FileHeader.TimeDateStamp;
    KfcRuntimeCompatibility::EnshroudedClient::image_sha256 = "private-service-fixture";
    KfcRuntimeCompatibility::EnshroudedClient::image_target = "private-service-test.exe";
    auto rva = [base](auto pointer) { return reinterpret_cast<std::uintptr_t>(pointer) - base; };
    Execution execution;
    Context context{reinterpret_cast<std::uintptr_t>(&execution)};
    Entry entry; entry.context = reinterpret_cast<std::uintptr_t>(&context);
    Scene scene;
    scene.entries = reinterpret_cast<std::uintptr_t>(&entry);
    scene.thread.name = reinterpret_cast<std::uintptr_t>(scene.thread.text);
    scene.thread.context = reinterpret_cast<std::uintptr_t>(&scene.loop);
    scene.thread.callback = reinterpret_cast<std::uintptr_t>(fixture_callback);
    thread_rows[0] = {scene.thread.handle,scene.thread.name,scene.thread.length};
    Controller controller{reinterpret_cast<std::uintptr_t>(controller_vtable),0,reinterpret_cast<std::uintptr_t>(&context)};
    Owner client{reinterpret_cast<std::uintptr_t>(client_vtable),reinterpret_cast<std::uintptr_t>(&controller),reinterpret_cast<std::uintptr_t>(&scene)};
    controller.client = reinterpret_cast<std::uintptr_t>(&client);
    root = {reinterpret_cast<std::uintptr_t>(root_vtable),reinterpret_cast<std::uintptr_t>(&client)};
    std::uintptr_t entity_table[1]{1};
    put(execution.manager,0,reinterpret_cast<std::uintptr_t>(execution.registry));
    put(execution.manager,344,std::uint64_t{1});
    put(execution.manager,392,reinterpret_cast<std::uintptr_t>(entity_table));
    put(execution.registry,16,std::uint64_t{1});
    metadata_slot = reinterpret_cast<std::uintptr_t>(metadata);
    put(metadata,0x40,std::uint32_t{sizeof(ServicePayload)}); put(metadata,0x50,type_hash);
    put(descriptor,0xb0,reinterpret_cast<std::uintptr_t>(mutable_names));
    put(descriptor,0xb8,std::uint64_t{1});
    auto* payload = static_cast<ServicePayload*>(VirtualAlloc(nullptr,4096,MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE));
    check(payload != nullptr);
    *payload = ServicePayload{};
    unsigned char records[record_stride*2]{};
    const auto name_hash = ComponentRegistry::hash_name(service_name);
    put(records,0,reinterpret_cast<std::uintptr_t>(service_name));
    put(records,8,std::uint64_t{sizeof(service_name)-1}); put(records,0xa0,name_hash);
    put(records,0xa8,reinterpret_cast<std::uintptr_t>(payload)); put(records,0xb0,reinterpret_cast<std::uintptr_t>(payload));
    std::uint32_t keys[4]{}; std::uint16_t values[4]{};
    const auto slot = name_hash & 3;
    std::uint64_t occupancy = std::uint64_t{1} << slot;
    keys[slot] = name_hash;
    put(execution.registry,640,reinterpret_cast<std::uintptr_t>(records)); put(execution.registry,648,std::uint64_t{1});
    put(execution.registry,656,reinterpret_cast<std::uintptr_t>(&occupancy)); put(execution.registry,664,std::uint64_t{4});
    put(execution.registry,672,reinterpret_cast<std::uintptr_t>(keys)); put(execution.registry,680,reinterpret_cast<std::uintptr_t>(values));
    put(execution.registry,688,std::uint32_t{1}); put(execution.registry,692,std::uint16_t{1});
    auto* system = execution.registry + systems_offset;
    put(system,0,reinterpret_cast<std::uintptr_t>(descriptor)); put(system,0x28,reinterpret_cast<std::uintptr_t>(fixture_mutable_system));
    put(system,0x40,reinterpret_cast<std::uintptr_t>(payload)); put(system,0x240,reinterpret_cast<std::uintptr_t>(payload));
    put(system,0x440,std::uint8_t{1}); put(system,0x441,std::uint8_t{1});
    nlohmann::json profile{
        {"schemaVersion",1},
        {"target","private-service-test.exe"},
        {"image",{{"sha256","private-service-fixture"},{"timestamp",nt->FileHeader.TimeDateStamp},{"size",image_size}}},
        {"provenance",{{"ecsReadValidated",true},{"ecsAuthorityReadValidated",true},{"ecsWriteGateValidated",true},
            {"ecsChangeTrackingValidated",true},{"servicePlainDataValidated",true}}},
        {"ownerLayout",{{"rootObjectRva",rva(&root)},{"rootVtableRva",rva(root_vtable)},
            {"clientPointerOffset",offsetof(Owner,pointer)},{"clientVtableRva",rva(client_vtable)},
            {"controllerPointerOffset",offsetof(Owner,pointer)},{"controllerVtableRva",rva(controller_vtable)},
            {"controllerClientBackPointerOffset",offsetof(Controller,client)},{"controllerMainOwnerOffset",offsetof(Controller,main)}}},
        {"ecsReadLayout",{{"mainExecutionOwnerOffset",offsetof(Context,execution)},
            {"executionRegistryOwnerOffset",offsetof(Execution,registry)},{"executionEntityManagerOffset",offsetof(Execution,manager)},
            {"entityManagerOwnerOffset",0},{"entityManagerCount",344},{"entityManagerTable",392},
            {"entityId",16},{"entityGeneration",20},{"entityLayout",24},{"entityStorage",32},{"entityDefinitionPointer",40},{"entityRow",48},
            {"componentBits",0},{"componentOffsets",132},{"componentStrides",2692},{"maximumEntities",8},
            {"clientSceneOwnerOffset",offsetof(Owner,scene)},{"sceneEntriesPointerOffset",offsetof(Scene,entries)},
            {"sceneEntriesCountOffset",offsetof(Scene,count)},{"sceneEntriesCapacityOffset",offsetof(Scene,capacity)},
            {"sceneEntryStride",sizeof(Entry)},{"sceneEntryContextOffset",offsetof(Entry,context)},{"sceneEntryUuidOffset",offsetof(Entry,uuid)},
            {"sceneEntryStateOffset",offsetof(Entry,state)},{"sceneEntryReadyState",5},{"maximumSceneEntries",8}}},
        {"ecsWriteLayout",{{"id","ecs-native-thread-scene-gate-v1"},{"enabled",true},{"phaseValidated",true},{"phasePolicy","untracked-components"},
            {"threadRegistryLockRva",rva(&thread_gate)},{"threadRegistryEntriesRva",rva(thread_rows)},{"threadRegistryCountRva",rva(&thread_count)},
            {"threadRegistryCapacity",1},{"threadRegistryEntryStride",sizeof(ThreadRow)},{"registryEntryHandleOffset",offsetof(ThreadRow,handle)},
            {"registryEntryNamePointerOffset",offsetof(ThreadRow,name)},{"registryEntryNameLengthOffset",offsetof(ThreadRow,length)},
            {"threadInlineNameOffset",offsetof(Thread,text)},{"sceneThreadOffset",offsetof(Scene,thread)},
            {"threadNamePointerOffset",offsetof(Thread,name)},{"threadNameLengthOffset",offsetof(Thread,length)},
            {"threadNameCapacityOffset",offsetof(Thread,capacity)},{"threadNameCapacity",64},{"threadHandleOffset",offsetof(Thread,handle)},
            {"threadContextOffset",offsetof(Thread,context)},{"threadStopOffset",offsetof(Thread,stop)},
            {"threadCallbackOffset",offsetof(Thread,callback)},{"threadCallbackRva",rva(fixture_callback)},{"threadName","GameServer"},
            {"sceneContextOffset",offsetof(Scene,loop)},{"sceneLockOffset",offsetof(Scene,gate)},{"sceneStateOffset",offsetof(Scene,state)},
            {"sceneRunningState",1},{"executionModeOffset",offsetof(Execution,manager)+8},{"executionAuthorityMode",0},{"maximumWriteBytes",64},
            {"changeTrackerRva",rva(fixture_unused_tracker)},{"changeTrackerContextOffset",400},{"changeTrackerManagerOffset",8},
            {"changeTrackerQueueBOffset",320},{"changeTrackerRegistryAOffset",400},{"changeTrackerRegistryBOffset",560},
            {"maximumTrackingQueueCapacity",8},{"changeTrackerBytes",std::string(66,'0')}}},
        {"serviceLayout",{{"id","engine-named-plain-services-v1"},{"source","private-service-fixture"},
            {"recordsPointerOffset",640},{"recordCountOffset",648},{"recordStride",record_stride},{"recordHashOffset",0xa0},
            {"recordNormalPointerOffset",0xa8},{"recordAlternatePointerOffset",0xb0},{"mapBitsOffset",656},{"mapCapacityOffset",664},
            {"mapKeysOffset",672},{"mapValuesOffset",680},{"mapCountOffset",688},{"systemsOffset",systems_offset},
            {"systemCountOffset",692},{"systemStride",system_stride},{"systemNormalBindingsOffset",0x40},
            {"systemAlternateBindingsOffset",0x240},{"systemNormalCountOffset",0x440},{"systemAlternateCountOffset",0x441},
            {"maximumRecords",2},{"maximumMapCapacity",4},{"maximumSystems",2},
            {"contracts",nlohmann::json::array({{{"key",service_name},{"qualifiedType",std::string("keen::")+service_name},
                {"qualifiedHash",type_hash},{"size",sizeof(ServicePayload)},{"metadataRva",rva(metadata)},
                {"registrySlotRva",rva(&metadata_slot)},{"mutableSystemDescriptorRva",rva(descriptor)},
                {"mutableSystemFunctionRva",rva(fixture_mutable_system)},{"plainData",true},{"scope","host-world-shared"}}})}}}
    };
    std::string error;
    check(EngineContext::Configure(profile,error));
    check(EngineContext::ResolveEcs(read_only_view));
    read_only_binding = layout_ready = component_registry_ready = component_registry_world_bound = true;
    component_registry.owner = read_only_view.registry_owner;
    component_registry_manager = read_only_view.manager;
    live_layout.count_address = read_only_view.manager+344;
    live_layout.table_address = read_only_view.manager+392;
    live_table = reinterpret_cast<std::uintptr_t>(entity_table);
    layout_epoch = 19;
    const std::uint64_t session = layout_epoch+1;

    // Completion must fence the published epoch even when the live owners or
    // normal admission flags are temporarily unavailable. No original gate is
    // needed, and a genuinely newer epoch must retain its own fault state.
    {
        const auto saved_fault_epoch = write_fault_epoch;
        const auto* saved_fault_reason = write_fault_reason;
        const auto* saved_last_reason = last_write_reason.load(std::memory_order_relaxed);
        const auto saved_root_pointer = root.pointer;
        const auto saved_stop = stop_requested.load(std::memory_order_acquire);
        const auto saved_ready = layout_ready;
        const auto saved_registry_ready = component_registry_ready;
        const auto saved_registry_bound = component_registry_world_bound;
        const auto saved_read_only = read_only_binding;
        constexpr const char* reason = "private-completion-outcome-uncertain";
        const auto reset_admission = [&] {
            root.pointer = saved_root_pointer;
            stop_requested.store(saved_stop,std::memory_order_release);
            layout_ready = saved_ready;
            component_registry_ready = saved_registry_ready;
            component_registry_world_bound = saved_registry_bound;
            read_only_binding = saved_read_only;
        };
        for (unsigned blocked = 0; blocked < 6; ++blocked) {
            reset_admission();
            write_fault_epoch = 0; write_fault_reason = nullptr;
            if (blocked == 0) root.pointer = 0;
            if (blocked == 1) component_registry_ready = false;
            if (blocked == 2) component_registry_world_bound = false;
            if (blocked == 3) layout_ready = false;
            if (blocked == 4) stop_requested.store(true,std::memory_order_release);
            if (blocked == 5) read_only_binding = false;
            {
                EcsRuntime::SessionGuard unavailable;
                check(!unavailable.TryAcquire(session));
            }
            thread_gate = static_cast<LONG>(0x80000000u); scene.gate = 1;
            check(EcsRuntime::MarkSessionWriteUncertain(session,reason));
            check(write_fault_active_locked() && write_fault_epoch == layout_epoch && write_fault_reason == reason);
            check(last_write_reason.load(std::memory_order_relaxed) == reason);
            check(thread_gate == static_cast<LONG>(0x80000000u) && scene.gate == 1);
            thread_gate = 0; scene.gate = 0;
            reset_admission();
            EcsRuntime::SessionGuard restored;
            check(restored.TryAcquire(session) && !restored.WritesAllowed());
        }
        check(!EcsRuntime::MarkSessionWriteUncertain(0,reason));
        check(!EcsRuntime::MarkSessionWriteUncertain(session,nullptr));
        check(write_fault_epoch == layout_epoch && write_fault_reason == reason);
        ++layout_epoch;
        write_fault_epoch = 0; write_fault_reason = nullptr;
        check(!EcsRuntime::MarkSessionWriteUncertain(session,reason) && !write_fault_active_locked());
        constexpr const char* newer_reason = "private-newer-epoch-fault";
        write_fault_epoch = layout_epoch; write_fault_reason = newer_reason;
        check(!EcsRuntime::MarkSessionWriteUncertain(session,reason));
        check(write_fault_epoch == layout_epoch && write_fault_reason == newer_reason);
        --layout_epoch;
        reset_admission();
        write_fault_epoch = saved_fault_epoch; write_fault_reason = saved_fault_reason;
        last_write_reason.store(saved_last_reason,std::memory_order_relaxed);
        check(scene.gate == 0 && thread_gate == 0);
        EcsRuntime::SessionGuard restored;
        check(restored.TryAcquire(session) && restored.WritesAllowed());
    }

    // Resolve the actual opaque-handle implementation with the existing
    // SessionGuard and original Scene gates. The occupied ID index remains
    // authoritative even when a removed object's memory is still readable.
    {
        const auto saved_layout = live_layout;
        const auto saved_handles = handles;
        const auto saved_reverse_handles = reverse_handles;
        const auto saved_next_handle = next_handle;
        std::array<unsigned char,64> identity_entity{}, replacement_entity{};
        std::array<unsigned char,16> identity_layout{}, identity_storage{};
        constexpr std::uint32_t identity_id = 0x12345678, identity_generation = 7;
        std::uint64_t identity_occupancy = 1;
        std::uint32_t identity_keys[1]{identity_id};
        live_layout.entity_id = 16; live_layout.entity_generation = 20;
        live_layout.entity_layout = 24; live_layout.entity_storage = 32;
        live_layout.entity_definition = 40; live_layout.entity_row = 48;
        live_layout.occupancy_address = read_only_view.manager + 336;
        live_layout.keys_address = read_only_view.manager + 368;
        put(execution.manager,336,reinterpret_cast<std::uintptr_t>(&identity_occupancy));
        put(execution.manager,368,reinterpret_cast<std::uintptr_t>(identity_keys));
        put(identity_entity.data(),16,identity_id);
        put(identity_entity.data(),20,identity_generation);
        put(identity_entity.data(),24,reinterpret_cast<std::uintptr_t>(identity_layout.data()));
        put(identity_entity.data(),32,reinterpret_cast<std::uintptr_t>(identity_storage.data()));
        entity_table[0] = reinterpret_cast<std::uintptr_t>(identity_entity.data());
        EcsImplementation::EntityView identity_view;
        check(entity_view(entity_table[0],live_layout,identity_view));
        const auto identity_handle = handle_for(identity_view,0);
        check(identity_handle && identity_handle != identity_id);
        auto reject_identity = [&](const EcsRuntime::SessionGuard& held, std::uint32_t handle) {
            std::uint32_t id = 0xaaaaaaaa, generation = 0xbbbbbbbb;
            check(!held.ResolveEntityIdentity(handle,id,generation));
            check(id == 0xaaaaaaaa && generation == 0xbbbbbbbb);
        };
        {
            EcsRuntime::SessionGuard unpinned;
            reject_identity(unpinned,identity_handle);
            check(!unpinned.TryAcquire(session-1));
            reject_identity(unpinned,identity_handle);
        }
        {
            EcsRuntime::SessionGuard held;
            check(held.TryAcquire(session));
            EngineContext::EcsWriteGuard held_scene;
            check(held_scene.TryAcquire(held.View()) && held_scene.Validate());
            check(scene.gate == static_cast<LONG>(0x80000000u) && thread_gate == 1);
            std::uint32_t id{}, generation{};
            check(held.ResolveEntityIdentity(identity_handle,id,generation) &&
                id == identity_id && generation == identity_generation);
            reject_identity(held,0);
            reject_identity(held,identity_handle+1);
            put(identity_entity.data(),20,identity_generation+1);
            reject_identity(held,identity_handle);
            put(identity_entity.data(),20,identity_generation);
            identity_occupancy = 0;
            reject_identity(held,identity_handle);
            identity_occupancy = 1;
            identity_keys[0] ^= 1;
            reject_identity(held,identity_handle);
            identity_keys[0] = identity_id;
            entity_table[0] = 0;
            reject_identity(held,identity_handle);
            // A different object with the same numeric identity must not
            // make the old borrowed pointer into the registered object.
            replacement_entity = identity_entity;
            entity_table[0] = reinterpret_cast<std::uintptr_t>(replacement_entity.data());
            reject_identity(held,identity_handle);
            entity_table[0] = reinterpret_cast<std::uintptr_t>(identity_entity.data());
            put(identity_entity.data(),16,std::uint32_t{});
            reject_identity(held,identity_handle);
            put(identity_entity.data(),16,identity_id);
            put(identity_entity.data(),32,std::uintptr_t{});
            reject_identity(held,identity_handle);
            put(identity_entity.data(),32,reinterpret_cast<std::uintptr_t>(identity_storage.data()));
            ++handles.at(identity_handle).epoch;
            reject_identity(held,identity_handle);
            --handles.at(identity_handle).epoch;
            // Simulate binding invalidation while retaining the old guard.
            // No lifecycle API is reentered while its state mutex is pinned.
            ++layout_epoch;
            reject_identity(held,identity_handle);
            --layout_epoch;
            check(held.ResolveEntityIdentity(identity_handle,id,generation) &&
                id == identity_id && generation == identity_generation && held_scene.Validate());
        }
        check(scene.gate == 0 && thread_gate == 0);
        ++layout_epoch;
        {
            EcsRuntime::SessionGuard next_binding;
            check(next_binding.TryAcquire(session+1));
            EngineContext::EcsWriteGuard held_scene;
            check(held_scene.TryAcquire(next_binding.View()) && held_scene.Validate());
            reject_identity(next_binding,identity_handle);
        }
        --layout_epoch;
        check(scene.gate == 0 && thread_gate == 0);
        live_layout = saved_layout;
        handles = saved_handles;
        reverse_handles = saved_reverse_handles;
        next_handle = saved_next_handle;
        entity_table[0] = 1;
        put(execution.manager,336,std::uintptr_t{});
        put(execution.manager,368,std::uintptr_t{});
    }
    ServicePayload snapshot{};
    auto load = [&](std::uint64_t token = UINT64_MAX) {
        if (token == UINT64_MAX) token = session;
        return KfcRuntimeServiceRead(service_name,type_hash,sizeof(snapshot),token,
            reinterpret_cast<unsigned char*>(&snapshot),sizeof(snapshot));
    };
    auto unchanged = [&] { check(scene.gate==0 && thread_gate==0); };
    check(load() && snapshot==*payload); unchanged();
    const auto baseline = *payload;
    auto desired = baseline;
    desired.enabled = 0; desired.count = 999;
    std::array<std::uint8_t,sizeof(ServicePayload)> mask{};
    mask[0] = 1;
    auto apply = [&](const ServicePayload& expected) {
        return KfcRuntimeServiceCompareExchange(service_name,type_hash,sizeof(snapshot),session,&expected,mask.data(),&desired);
    };
    check(apply(baseline)==1 && payload->enabled==0 && payload->count==baseline.count);
    check(payload->sibling==baseline.sibling && std::memcmp(payload->padding,baseline.padding,2)==0);
    check(load() && snapshot==*payload); unchanged();
    check(apply(baseline)==2 && payload->enabled==0); unchanged();
    check(apply(*payload)==1); // no-op still validates the full binding
    *payload = baseline;
    auto stale = baseline; ++stale.tail;
    check(apply(stale)==2 && *payload==baseline); unchanged();
    mask[0] = 2; check(apply(baseline)==0 && *payload==baseline); mask[0] = 1;
    check(!load(session-1) && !load(0)); unchanged();
    write_fault_epoch = layout_epoch; write_fault_reason = "private-service-fixture-fault";
    check(load() && apply(baseline)==0 && *payload==baseline);
    write_fault_epoch = 0; write_fault_reason = nullptr; unchanged();
    component_registry_ready = false; check(!load()); component_registry_ready = true;
    const auto role = read_only_view.role;
    read_only_view.role = EngineContext::EcsRole::ClientReplica; check(!load()); read_only_view.role = role;
    scene.gate = 1; check(!load()); scene.gate = 0; unchanged();
    scene.thread.stop = 1; check(!load()); scene.thread.stop = 0;
    ++entry.uuid[0]; check(!load()); --entry.uuid[0];
    put(records,0,reinterpret_cast<std::uintptr_t>(collision_name)); check(!load());
    put(records,0,reinterpret_cast<std::uintptr_t>(service_name));
    keys[slot]^=1; check(!load()); keys[slot]^=1;
    std::memcpy(records+record_stride,records,record_stride);
    put(execution.registry,648,std::uint64_t{2}); check(!load()); put(execution.registry,648,std::uint64_t{1});
    put(system,0x240,std::uintptr_t{1}); check(!load()); put(system,0x240,reinterpret_cast<std::uintptr_t>(payload));
    put(descriptor,0xb8,std::uint64_t{0}); check(!load()); put(descriptor,0xb8,std::uint64_t{1});
    put(descriptor,0x98,reinterpret_cast<std::uintptr_t>(mutable_names));
    put(descriptor,0xa0,std::uint64_t{1}); check(!load());
    put(descriptor,0x98,std::uintptr_t{}); put(descriptor,0xa0,std::uint64_t{});
    put(system,0x441,std::uint8_t{0}); check(!load()); put(system,0x441,std::uint8_t{1});
    std::memcpy(system+system_stride,system,system_stride);
    put(execution.registry,692,std::uint16_t{2}); check(!load()); put(execution.registry,692,std::uint16_t{1});
    metadata_slot = 1; check(!load()); metadata_slot = reinterpret_cast<std::uintptr_t>(metadata);
    put(metadata,0x50,type_hash+1); check(!load()); put(metadata,0x50,type_hash);
    put(records,0xa8,std::uintptr_t{1}); put(records,0xb0,std::uintptr_t{1});
    put(system,0x40,std::uintptr_t{1}); put(system,0x240,std::uintptr_t{1});
    check(!load() && apply(baseline)==0);
    put(records,0xa8,reinterpret_cast<std::uintptr_t>(payload)); put(records,0xb0,reinterpret_cast<std::uintptr_t>(payload));
    put(system,0x40,reinterpret_cast<std::uintptr_t>(payload)); put(system,0x240,reinterpret_cast<std::uintptr_t>(payload));
    DWORD old{};
    check(VirtualProtect(payload,4096,PAGE_READONLY,&old)!=FALSE);
    check(!load() && apply(baseline)==0);
    check(VirtualProtect(payload,4096,old,&old)!=FALSE && *payload==baseline);
    auto malformed = profile;
    malformed["serviceLayout"]["systemStride"] = 1;
    check(EngineContext::Configure(malformed,error) && !load());
    malformed = profile; malformed["serviceLayout"]["systemNormalBindingsOffset"] = system_stride-8;
    check(EngineContext::Configure(malformed,error) && !load());
    check(EngineContext::Configure(profile,error) && load());
    put(execution.registry,640,UINTPTR_MAX-16); check(!load());
    put(execution.registry,640,reinterpret_cast<std::uintptr_t>(records));
    check(KfcRuntimeServiceCompareExchange(service_name,type_hash,sizeof(snapshot),session,
        reinterpret_cast<void*>(1),mask.data(),&desired)==0);
    check(load() && *payload==baseline); unchanged();

    // Internal DI lookup admits an original opaque service name without a
    // reflected plain-data contract. The public snapshot/CAS admission remains
    // separate and must not expose or mutate this borrowed object.
    const char opaque_name[]{"impact::ImpactSystem"};
    const char opaque_collision_name[]{"impact::ImpactSysteM"};
    const auto saved_records = std::to_array(records);
    std::array<unsigned char,80> opaque_a{}, opaque_b{};
    const auto opaque_hash = NamedServiceImplementation::hash_name(opaque_name);
    const auto opaque_slot = opaque_hash & 3;
    put(records,0,reinterpret_cast<std::uintptr_t>(opaque_name));
    put(records,8,std::uint64_t{sizeof(opaque_name)-1}); put(records,0xa0,opaque_hash);
    put(records,0xa8,reinterpret_cast<std::uintptr_t>(opaque_a.data()));
    put(records,0xb0,reinterpret_cast<std::uintptr_t>(opaque_a.data()));
    std::fill(std::begin(keys),std::end(keys),0);
    keys[opaque_slot] = opaque_hash;
    occupancy = std::uint64_t{1} << opaque_slot;
    auto internal_profile = profile;
    internal_profile["serviceLayout"].erase("contracts");
    check(EngineContext::Configure(internal_profile,error));
    const auto opaque = NamedServiceImplementation::binding_contract(opaque_name);
    auto resolve_opaque = [&](std::uintptr_t& output) {
        std::scoped_lock operation(NamedServiceImplementation::operation_mutex);
        EcsRuntime::SessionGuard held_session;
        if (!held_session.TryAcquire(session)) return false;
        EngineContext::EcsWriteGuard held_scene;
        return held_scene.TryAcquire(held_session.View()) && held_scene.Validate() &&
            NamedServiceImplementation::resolve_binding(opaque,held_session.View(),output) && held_scene.Validate();
    };
    std::uintptr_t borrowed{};
    check(resolve_opaque(borrowed) && borrowed==reinterpret_cast<std::uintptr_t>(opaque_a.data())); unchanged();
    const auto public_snapshot = snapshot;
    check(!KfcRuntimeServiceRead(opaque_name,0,sizeof(snapshot),session,
        reinterpret_cast<unsigned char*>(&snapshot),sizeof(snapshot)) && snapshot==public_snapshot);
    check(KfcRuntimeServiceCompareExchange(opaque_name,0,sizeof(snapshot),session,
        &baseline,mask.data(),&desired)==0 && opaque_a==std::array<unsigned char,80>{}); unchanged();
    auto rejected_opaque = [&] {
        borrowed = UINTPTR_MAX;
        const auto rejected = !resolve_opaque(borrowed);
        check(rejected && borrowed==UINTPTR_MAX);
        unchanged();
    };
    // A matching map hash never substitutes for the complete original name.
    put(records,0,reinterpret_cast<std::uintptr_t>(opaque_collision_name)); rejected_opaque();
    put(records,0,reinterpret_cast<std::uintptr_t>(opaque_name));
    keys[opaque_slot]^=1; rejected_opaque(); keys[opaque_slot]^=1;
    occupancy = 0; rejected_opaque(); occupancy = std::uint64_t{1} << opaque_slot;
    values[opaque_slot] = 1; rejected_opaque(); values[opaque_slot] = 0;
    std::memcpy(records+record_stride,records,record_stride);
    put(execution.registry,648,std::uint64_t{2}); rejected_opaque(); put(execution.registry,648,std::uint64_t{1});
    put(records,0xb0,reinterpret_cast<std::uintptr_t>(opaque_b.data())); rejected_opaque();
    put(records,0xb0,reinterpret_cast<std::uintptr_t>(opaque_a.data()));

    // Resolve records and payloads afresh after relocation. A prepared contract
    // retains layout/name identity only, never a cached game pointer.
    auto relocated_records = std::to_array(records);
    put(execution.registry,640,reinterpret_cast<std::uintptr_t>(relocated_records.data()));
    put(records,0xa8,std::uintptr_t{1}); put(records,0xb0,std::uintptr_t{1});
    check(resolve_opaque(borrowed) && borrowed==reinterpret_cast<std::uintptr_t>(opaque_a.data())); unchanged();
    put(relocated_records.data(),0xa8,reinterpret_cast<std::uintptr_t>(opaque_b.data()));
    put(relocated_records.data(),0xb0,reinterpret_cast<std::uintptr_t>(opaque_b.data()));
    check(resolve_opaque(borrowed) && borrowed==reinterpret_cast<std::uintptr_t>(opaque_b.data())); unchanged();
    put(execution.registry,640,UINTPTR_MAX-16); rejected_opaque();
    put(execution.registry,640,reinterpret_cast<std::uintptr_t>(relocated_records.data()));
    put(execution.registry,672,UINTPTR_MAX-2); rejected_opaque();
    put(execution.registry,672,reinterpret_cast<std::uintptr_t>(keys));
    std::memcpy(records,saved_records.data(),sizeof(records));
    put(execution.registry,640,reinterpret_cast<std::uintptr_t>(records));
    std::fill(std::begin(keys),std::end(keys),0);
    keys[slot] = name_hash; occupancy = std::uint64_t{1} << slot;
    check(EngineContext::Configure(profile,error) && load() && snapshot==baseline); unchanged();

    // A real registered callback pointer is selected by its complete operation
    // name. Its fixture verifies the native-owned ABI and every gate released.
    std::array<unsigned char,0x78> impact_system{};
    std::uint64_t operation_bits{1};
    std::uint32_t operation_keys[4]{};
    std::array<unsigned char,4*80> operation_records{};
    auto* operation_record = operation_records.data();
    const char operation_name[]{"isSameEntity"};
    auto set_operation = [&](unsigned char* record) {
        std::fill(record,record+80,0);
        put(record,0,reinterpret_cast<std::uintptr_t>(fixture_is_same_entity));
        put(record,8,reinterpret_cast<std::uintptr_t>(record+40));
        put(record,16,std::uint64_t{sizeof(operation_name)-1}); put(record,24,std::uint64_t{32});
        std::memcpy(record+40,operation_name,sizeof(operation_name)); put(record,72,std::uint8_t{2});
        std::fill(record+73,record+80,0x6a); // only the original byte at72 is arity
    };
    set_operation(operation_record);
    put(impact_system.data(),0x28,reinterpret_cast<std::uintptr_t>(&operation_bits));
    put(impact_system.data(),0x30,std::uint64_t{4});
    put(impact_system.data(),0x48,reinterpret_cast<std::uintptr_t>(operation_keys));
    put(impact_system.data(),0x60,reinterpret_cast<std::uintptr_t>(operation_records.data()));
    put(impact_system.data(),0x74,std::uint32_t{1});
    put(records,0,reinterpret_cast<std::uintptr_t>(opaque_name));
    put(records,8,std::uint64_t{sizeof(opaque_name)-1}); put(records,0xa0,opaque_hash);
    put(records,0xa8,reinterpret_cast<std::uintptr_t>(impact_system.data()));
    put(records,0xb0,reinterpret_cast<std::uintptr_t>(impact_system.data()));
    std::fill(std::begin(keys),std::end(keys),0);
    keys[opaque_slot] = opaque_hash; occupancy = std::uint64_t{1} << opaque_slot;
    comparison_scene = &scene;
    std::uint8_t equal = 0x77;
    auto compare = [&](std::uint32_t left, std::uint32_t right) {
        return KfcRuntimeImpactCompareEntityIds(session,left,right,&equal);
    };
    check(compare(0x12345678,0x12345678)==1 && equal==1 && comparison_calls==1); unchanged();
    check(compare(0x12345678,0x87654321)==1 && equal==0 && comparison_calls==2); unchanged();
    check(compare(0,0)==1 && equal==1); check(compare(0,UINT32_MAX)==1 && equal==0);
    check(compare(UINT32_MAX,UINT32_MAX)==1 && equal==1);
    const auto calls_before_rejection = comparison_calls;
    check(KfcRuntimeImpactCompareEntityIds(0,1,1,&equal)==2);
    check(KfcRuntimeImpactCompareEntityIds(session,1,1,nullptr)==2);
    check(KfcRuntimeImpactCompareEntityIds(session,1,1,reinterpret_cast<std::uint8_t*>(1))==2);
    check(KfcRuntimeImpactCompareEntityIds(session-1,1,1,&equal)==4 && comparison_calls==calls_before_rejection);
    auto reject_comparison = [&](std::uint32_t expected_status) {
        equal = 0x77;
        const auto calls = comparison_calls;
        check(compare(1,1)==expected_status && equal==0x77 && comparison_calls==calls);
        unchanged();
    };
    put(records,0,reinterpret_cast<std::uintptr_t>(opaque_collision_name)); reject_comparison(3);
    put(records,0,reinterpret_cast<std::uintptr_t>(opaque_name));
    operation_record[40]='I'; reject_comparison(3); operation_record[40]='i';
    put(operation_record,72,std::uint8_t{3}); reject_comparison(5); put(operation_record,72,std::uint8_t{2});
    put(operation_record,0,std::uintptr_t{1}); reject_comparison(5);
    put(operation_record,0,reinterpret_cast<std::uintptr_t>(fixture_is_same_entity));
    put(operation_record,8,std::uintptr_t{1}); reject_comparison(5);
    put(operation_record,8,reinterpret_cast<std::uintptr_t>(operation_record+40));
    operation_keys[0]=1; reject_comparison(5); operation_keys[0]=0;
    set_operation(operation_records.data()+80); operation_bits=3;
    operation_keys[1]=1; put(impact_system.data(),0x74,std::uint32_t{2}); reject_comparison(5);
    operation_bits=1; put(impact_system.data(),0x74,std::uint32_t{1});
    put(impact_system.data(),0x30,std::uint64_t{3}); reject_comparison(5);
    put(impact_system.data(),0x30,std::uint64_t{4});
    put(impact_system.data(),0x74,std::uint32_t{2}); reject_comparison(5);
    put(impact_system.data(),0x74,std::uint32_t{1});
    put(impact_system.data(),0x60,UINTPTR_MAX-16); reject_comparison(5);
    put(impact_system.data(),0x60,reinterpret_cast<std::uintptr_t>(operation_records.data()));
    for (comparison_mode=1;comparison_mode<=7;++comparison_mode) {
        equal = 0x77;
        check(compare(7,7)==(comparison_mode==1 ? 6u : 7u) && equal==0x77); unchanged();
    }
    comparison_mode=8; equal=0x77;
    check(compare(7,7)==4 && equal==0x77); --layout_epoch; unchanged();
    for (comparison_mode=9;comparison_mode<=11;++comparison_mode) {
        equal = 0x77;
        check(compare(7,7)==7 && equal==0x77); unchanged();
    }
    comparison_mode=0;
    check(compare(9,9)==1 && equal==1 && comparison_gates_free); unchanged();
    // Only the host output is read-only here. The owned callback can complete,
    // but a failed final publication cannot claim a confirmed exported result.
    check(VirtualProtect(payload,4096,PAGE_READONLY,&old)!=FALSE);
    check(KfcRuntimeImpactCompareEntityIds(session,3,3,reinterpret_cast<std::uint8_t*>(payload))==2);
    check(VirtualProtect(payload,4096,old,&old)!=FALSE && *payload==baseline); unchanged();
    std::memcpy(records,saved_records.data(),sizeof(records));
    std::fill(std::begin(keys),std::end(keys),0);
    keys[slot]=name_hash; occupancy=std::uint64_t{1}<<slot;
    check(load() && snapshot==baseline); unchanged();

    // The registered voxel reader is exercised only against private DI and
    // task-pool objects. Every original role is a fixture callback in this EXE.
    // Public exports use this same named-service contract without requiring a
    // legacy execution profile or the hook-driven GameThreadDispatcher.
    namespace Voxel=WorldRuntime::RegisteredVoxelGrid;
    const char voxel_grid_name[]{"ecs::VoxelGrid"}, voxel_world_name[]{"VoxelWorld"};
    std::array<unsigned char,96> voxel_grid{};
    std::array<std::uintptr_t,2> voxel_wrapper{};
    auto* voxel_world=static_cast<unsigned char*>(VirtualAlloc(nullptr,0x2000,
        MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE));
    check(voxel_world!=nullptr);
    voxel_fixture_world=voxel_world; voxel_fixture_grid=voxel_grid.data();
    std::array<unsigned char,0x300> voxel_resources{}, voxel_storage{};
    std::array<unsigned char,64> voxel_groups{};
    std::array<unsigned char,80*5> voxel_tasks{};
    std::array<std::uint32_t,4> voxel_handles{0x01000001,0x01000002,0x01000003,0x01000004};
    voxel_fixture_handles=voxel_handles.data();
    voxel_wrapper[1]=reinterpret_cast<std::uintptr_t>(voxel_world);
    put(voxel_grid.data(),16,std::uint32_t{3});
    put(voxel_grid.data(),20,std::uint32_t{16}); put(voxel_grid.data(),24,std::uint32_t{16});
    put(voxel_grid.data(),28,std::uint32_t{4}); put(voxel_grid.data(),32,std::uint32_t{4});
    put(voxel_grid.data(),40,reinterpret_cast<std::uintptr_t>(voxel_wrapper.data()));
    for (std::size_t i=0;i<6;++i)
        put(voxel_grid.data(),48+i*8,reinterpret_cast<std::uintptr_t>(fixture_voxel_ready));
    put(voxel_grid.data(),48,reinterpret_cast<std::uintptr_t>(fixture_voxel_cells));
    put(voxel_grid.data(),56,reinterpret_cast<std::uintptr_t>(fixture_voxel_origin));
    put(voxel_world,0x14,std::uint32_t{1});
    const std::array<float,3> voxel_origin{10,20,30};
    std::memcpy(voxel_world+0x30,voxel_origin.data(),sizeof(voxel_origin));
    put(voxel_world,0x3c,std::uint32_t{512});
    constexpr std::size_t mode6=0x40+6*40;
    put(voxel_world,mode6,std::uint32_t{8}); put(voxel_world,mode6+4,std::uint32_t{8});
    put(voxel_world,mode6+8,std::uint32_t{2}); put(voxel_world,mode6+12,std::uint32_t{2});
    put(voxel_world,mode6+24,reinterpret_cast<std::uintptr_t>(voxel_handles.data()));
    put(voxel_world,mode6+32,std::uint64_t{4});
    put(voxel_world,0x160,reinterpret_cast<std::uintptr_t>(voxel_resources.data()));
    put(voxel_world,0x1220,reinterpret_cast<std::uintptr_t>(voxel_storage.data()));
    put(voxel_resources.data(),0x190,reinterpret_cast<std::uintptr_t>(voxel_groups.data()));
    put(voxel_resources.data(),0x198,std::uint64_t{1});
    put(voxel_resources.data(),0x1b8,std::uint64_t{3}); put(voxel_resources.data(),0x1c8,std::uint64_t{7});
    put(voxel_groups.data(),8,reinterpret_cast<std::uintptr_t>(voxel_tasks.data()));
    put(voxel_groups.data(),16,std::uint64_t{5});
    for (std::size_t i=1;i<5;++i) {
        put(voxel_tasks.data(),i*80,std::uint32_t{1});
        put(voxel_tasks.data(),i*80+0x48,std::uint32_t{2});
    }
    std::fill(std::begin(records),std::end(records),0);
    std::fill(std::begin(keys),std::end(keys),0); occupancy=0;
    const std::array<const char*,2> voxel_names{voxel_grid_name,voxel_world_name};
    const std::array<std::uintptr_t,2> voxel_payloads{
        reinterpret_cast<std::uintptr_t>(voxel_grid.data()),reinterpret_cast<std::uintptr_t>(voxel_wrapper.data())};
    for (std::size_t i=0;i<2;++i) {
        const auto hash=NamedServiceImplementation::hash_name(voxel_names[i]);
        auto bucket=static_cast<std::size_t>(hash&3);
        while (occupancy & (std::uint64_t{1}<<bucket)) bucket=(bucket+1)&3;
        occupancy|=std::uint64_t{1}<<bucket; keys[bucket]=hash; values[bucket]=static_cast<std::uint16_t>(i);
        auto* record=records+i*record_stride;
        put(record,0,reinterpret_cast<std::uintptr_t>(voxel_names[i]));
        put(record,8,static_cast<std::uint64_t>(std::strlen(voxel_names[i]))); put(record,0xa0,hash);
        put(record,0xa8,voxel_payloads[i]); put(record,0xb0,voxel_payloads[i]);
    }
    put(execution.registry,648,std::uint64_t{2}); put(execution.registry,688,std::uint32_t{2});
    check(EngineContext::Configure(internal_profile,error));
    KfcRuntimeGridSpec voxel_spec{};
    check(Voxel::Available() && Voxel::GetSpec("voxel",&voxel_spec)==Voxel::Result::Success);
    check(std::strcmp(voxel_spec.id,"voxel")==0 && voxel_spec.origin[0]==10 && voxel_spec.origin[1]==20 &&
        voxel_spec.origin[2]==30 && voxel_spec.cell_size[0]==0.5 && voxel_spec.cell_size[1]==0.5 &&
        voxel_spec.maximum[0]==16 && voxel_spec.maximum[2]==16 && voxel_spec.maximum[1]==INT32_MAX-7);
    check(KfcRuntimeWorldOperationAvailable("runtime.world.voxel.read") &&
        KfcRuntimeWorldOperationAvailable("runtime.world.voxel.grid_spec"));
    check(KfcRuntimeWorldContextActive() && KfcRuntimeWorldContextKind()==1 &&
        !KfcRuntimeWorldOperationAvailable("runtime.world.voxel.write"));
    try {
        const auto status=EcsRuntime::Status();
        check(status.find("voxel_context=ready")!=std::string::npos &&
            status.find("source=ecs-local-authority")!=std::string::npos);
    } catch (const std::exception& exception) {
        std::fprintf(stderr,"native status exception: %s\n",exception.what());
        return 1;
    }
    check(nlohmann::json::parse(EcsRuntime::Diagnostics())["worldContext"]["source"]=="ecs-local-authority");
    KfcRuntimeGridSpec exported_spec{};
    check(KfcRuntimeWorldGridGetSpecForSession(session,"voxel",&exported_spec)==1 &&
        std::memcmp(&exported_spec,&voxel_spec,sizeof(voxel_spec))==0);
    auto reject_grid=[&](std::uint64_t expected_session,const char* id,std::uint32_t expected_status) {
        std::memset(&exported_spec,0x77,sizeof(exported_spec));
        const auto before=exported_spec;
        check(KfcRuntimeWorldGridGetSpecForSession(expected_session,id,&exported_spec)==expected_status &&
            std::memcmp(&exported_spec,&before,sizeof(before))==0);
    };
    reject_grid(session-1,"voxel",4);
    reject_grid(session,"unknown-grid",3);
    scene.gate=1;
    reject_grid(session,"voxel",2);
    check(scene.gate==1 && thread_gate==0);
    scene.gate=0;
    check(KfcRuntimeWorldGridGetSpecForSession(session,"voxel",&exported_spec)==1);
    unchanged();
    std::array<std::int32_t,3> voxel_from{7,0,7};
    std::array<std::uint32_t,3> voxel_size{2,2,2};
    std::array<std::uint16_t,8> voxel_cells{};
    std::size_t voxel_actual{99};
    auto voxel_read=[&] { return Voxel::Read(voxel_from.data(),voxel_size.data(),
        voxel_cells.data(),voxel_cells.size(),&voxel_actual); };
    check(voxel_read()==Voxel::Result::Success && voxel_actual==8 && voxel_ready_calls==4 && voxel_cell_calls==1);
    for (std::size_t i=0;i<voxel_cells.size();++i) check(voxel_cells[i]==0x3412+i);
    std::array<std::uint16_t,8> session_cells{};
    std::size_t session_actual{};
    check(Voxel::ReadForSession(session,voxel_from.data(),voxel_size.data(),session_cells.data(),
        session_cells.size(),&session_actual)==Voxel::Result::Success && session_actual==session_cells.size());
    for (std::size_t i=0;i<session_cells.size();++i) check(session_cells[i]==0x3412+i);
    session_cells.fill(0x77); session_actual=99;
    check(Voxel::ReadForSession(session-1,voxel_from.data(),voxel_size.data(),session_cells.data(),
        session_cells.size(),&session_actual)==Voxel::Result::StaleSession && session_actual==99 &&
        std::all_of(session_cells.begin(),session_cells.end(),[](auto value){return value==0x77;}));
    voxel_mode=2; session_cells.fill(0x77); session_actual=99;
    check(Voxel::ReadForSession(session,voxel_from.data(),voxel_size.data(),session_cells.data(),
        session_cells.size(),&session_actual)==Voxel::Result::Pending && session_actual==99 &&
        std::all_of(session_cells.begin(),session_cells.end(),[](auto value){return value==0x77;}));
    voxel_mode=0;
    session_cells.fill(0); session_actual=99;
    check(KfcRuntimeWorldVoxelReadForSession(session,voxel_from.data(),voxel_size.data(),session_cells.data(),
        session_cells.size(),&session_actual)==1 && session_actual==session_cells.size());
    for (std::size_t i=0;i<session_cells.size();++i) check(session_cells[i]==0x3412+i);
    session_cells.fill(0x77); session_actual=99;
    check(KfcRuntimeWorldVoxelReadForSession(session-1,voxel_from.data(),voxel_size.data(),session_cells.data(),
        session_cells.size(),&session_actual)==4 && session_actual==99 &&
        std::all_of(session_cells.begin(),session_cells.end(),[](auto value){return value==0x77;}));
    voxel_mode=2;
    session_cells.fill(0x77); session_actual=99;
    check(KfcRuntimeWorldVoxelReadForSession(session,voxel_from.data(),voxel_size.data(),session_cells.data(),
        session_cells.size(),&session_actual)==2 && session_actual==99 &&
        std::all_of(session_cells.begin(),session_cells.end(),[](auto value){return value==0x77;}));
    voxel_mode=0;
    voxel_from[0]=-1;
    session_cells.fill(0x77); session_actual=99;
    check(KfcRuntimeWorldVoxelReadForSession(session,voxel_from.data(),voxel_size.data(),session_cells.data(),
        session_cells.size(),&session_actual)==3 && session_actual==99 &&
        std::all_of(session_cells.begin(),session_cells.end(),[](auto value){return value==0x77;}));
    voxel_from[0]=7;
    auto reject_voxel=[&](bool native_read=false) {
        const auto reads=voxel_cell_calls;
        voxel_cells.fill(0x77); voxel_actual=99;
        const auto result=voxel_read();
        const bool untouched=voxel_actual==99 &&
            voxel_cells==std::array<std::uint16_t,8>{0x77,0x77,0x77,0x77,0x77,0x77,0x77,0x77};
        if (result==Voxel::Result::Success || !untouched)
            std::fprintf(stderr,"voxel reject mode=%u result=%u actual=%zu cell0=%u\n",voxel_mode,
                static_cast<unsigned>(result),voxel_actual,voxel_cells[0]);
        check(result!=Voxel::Result::Success && untouched);
        check(voxel_cell_calls==reads+(native_read?1u:0u)); unchanged();
    };
    voxel_mode=1; reject_voxel(); voxel_mode=2; reject_voxel(true); voxel_mode=0;
    put(voxel_tasks.data(),80+0x48,std::uint32_t{1}); reject_voxel();
    put(voxel_tasks.data(),80+0x48,std::uint32_t{2});
    voxel_handles[0]=0; reject_voxel(); voxel_handles[0]=0x01000001;
    voxel_handles[0]=0x02000001; reject_voxel(); voxel_handles[0]=0x01000001;
    put(voxel_resources.data(),0x1c8,std::uint64_t{6}); reject_voxel();
    put(voxel_resources.data(),0x1c8,std::uint64_t{7});
    put(voxel_world,0x1a34,std::uint32_t{0x01000009}); reject_voxel();
    put(voxel_world,0x1a34,std::uint32_t{0x01000004}); check(voxel_read()==Voxel::Result::Success);
    put(voxel_world,0x1a34,std::uint32_t{});
    put(voxel_world,mode6+32,std::uint64_t{3}); reject_voxel();
    put(voxel_world,mode6+32,std::uint64_t{4});
    put(voxel_world,mode6,std::uint32_t{0}); reject_voxel();
    put(voxel_world,mode6,std::uint32_t{8});
    put(voxel_world,0x174,std::uint32_t{0x10000}); reject_voxel();
    put(voxel_world,0x174,std::uint32_t{});
    voxel_from[0]=-1; reject_voxel(); voxel_from[0]=15; reject_voxel(); voxel_from[0]=7;
    voxel_from[1]=-1; reject_voxel(); voxel_from[1]=INT32_MAX-7; reject_voxel(); voxel_from[1]=0;
    voxel_size[1]=65536; reject_voxel(); voxel_size[1]=2;
    voxel_mode=4; reject_voxel(); voxel_handles[0]=0x01000001; voxel_mode=0;
    voxel_mode=5; reject_voxel(true);
    put(voxel_grid.data(),48,reinterpret_cast<std::uintptr_t>(fixture_voxel_cells)); voxel_mode=0;
    check(voxel_read()==Voxel::Result::Success && Voxel::Healthy());
    if (argc>1 && std::strcmp(argv[1],"--voxel-session-read")==0) {
        check(VirtualFree(voxel_world,0,MEM_RELEASE)!=FALSE);
        check(VirtualFree(payload,0,MEM_RELEASE)!=FALSE);
        std::printf("service runtime: %u checks passed\n",checks);
        return 0;
    }
    // Fault cases run in separate invocations because the process latch has no
    // reset. No new callback or metadata gate is attempted after its first fault.
    if (argc>1 && std::strcmp(argv[1],"--voxel-origin-pointer")==0) {
        voxel_mode=6;
        check(Voxel::GetSpec("voxel",&voxel_spec)==Voxel::Result::Fault && !Voxel::Healthy());
    } else if (argc>1 && std::strcmp(argv[1],"--voxel-metadata-seh")==0) {
        check(VirtualProtect(voxel_world,4096,PAGE_READONLY,&old)!=FALSE);
        voxel_cells.fill(0x77); voxel_actual=99;
        check(voxel_read()==Voxel::Result::Success && voxel_actual==8 && voxel_cells[0]==0x3412);
        check(VirtualProtect(voxel_world,4096,old,&old)!=FALSE);
    } else {
        voxel_mode=2; reject_voxel(true); check(Voxel::Healthy());
    }
    const auto calls_after_fault=voxel_origin_calls+voxel_ready_calls+voxel_cell_calls;
    if (!Voxel::Healthy()) {
        check(!Voxel::Available() && Voxel::GetSpec("voxel",&voxel_spec)==Voxel::Result::Fault);
        reject_voxel();
        check(voxel_origin_calls+voxel_ready_calls+voxel_cell_calls==calls_after_fault);
    } else {
        check(Voxel::Available() && Voxel::GetSpec("voxel",&voxel_spec)==Voxel::Result::Success);
        voxel_mode=2; reject_voxel(true); voxel_mode=0;
        check(voxel_origin_calls+voxel_ready_calls+voxel_cell_calls>calls_after_fault);
    }
    check(VirtualFree(voxel_world,0,MEM_RELEASE)!=FALSE);
    check(VirtualFree(payload,0,MEM_RELEASE)!=FALSE);
    std::printf("service runtime: %u checks passed\n",checks);
}
