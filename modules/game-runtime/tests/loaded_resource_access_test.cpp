// Private ResourceService fixture. No game process or engine calls.
#include <windows.h>
#include <chrono>
namespace {
unsigned virtual_query_calls{}, read_process_memory_calls{};
unsigned virtual_protect_calls{};
bool fail_next_restore{}, fail_next_prepare{};
const void* scalar_mutation_trigger{};
unsigned char* scalar_mutation_target{};
SIZE_T WINAPI counted_virtual_query(LPCVOID address, PMEMORY_BASIC_INFORMATION info, SIZE_T length) {
    ++virtual_query_calls;
    return ::VirtualQuery(address, info, length);
}
BOOL WINAPI counted_read_process_memory(HANDLE process, LPCVOID source, LPVOID target, SIZE_T size, SIZE_T* actual) {
    ++read_process_memory_calls;
    const auto result = ::ReadProcessMemory(process, source, target, size, actual);
    if (result && size == sizeof(DWORD) && source == scalar_mutation_trigger && scalar_mutation_target) {
        *scalar_mutation_target ^= 1;
        scalar_mutation_target = nullptr;
    }
    return result;
}
BOOL WINAPI counted_virtual_protect(LPVOID address, SIZE_T size, DWORD protection, PDWORD previous) {
    ++virtual_protect_calls;
    if (fail_next_prepare && protection == PAGE_READWRITE) {
        fail_next_prepare = false;
        SetLastError(ERROR_ACCESS_DENIED);
        return FALSE;
    }
    if (fail_next_restore && protection == PAGE_READONLY) {
        fail_next_restore = false;
        SetLastError(ERROR_ACCESS_DENIED);
        return FALSE;
    }
    return ::VirtualProtect(address, size, protection, previous);
}
}
#define VirtualQuery counted_virtual_query
#define ReadProcessMemory counted_read_process_memory
#define VirtualProtect counted_virtual_protect
#include "loaded-resources/initialize_resource_access.cpp"
#include "loaded-resources/parse_resource_layout.cpp"
#include "loaded-resources/resolve_loaded_resource.cpp"
#include "loaded-resources/find_loaded_resources_by_type.cpp"
#include "loaded-resources/read_loaded_resource.cpp"
#include "loaded-resources/read_resource_scalars.cpp"
#include "loaded-resources/resolve_item_resource.cpp"
#include "loaded-resources/compare_exchange_resource_scalar.cpp"
#undef VirtualQuery
#undef ReadProcessMemory
#undef VirtualProtect
#include <cstdio>
#include <cstdlib>

// This isolated fixture admits one explicitly configured read session. It does
// not admit writes. The ECS fixture separately links the real SessionGuard.
namespace EcsRuntime {
std::uint64_t fixture_read_session{};
bool fixture_client_replica{};
struct SessionGuard::State { EngineContext::EcsView view; };
SessionGuard::SessionGuard() = default;
SessionGuard::~SessionGuard() = default;
bool SessionGuard::TryAcquire(std::uint64_t expected) {
    if (!expected || expected != fixture_read_session || fixture_client_replica) return false;
    state_ = std::make_unique<State>();
    return EngineContext::ResolveOwners(state_->view.owners);
}
const EngineContext::EcsView& SessionGuard::View() const {
    static const EngineContext::EcsView unavailable;
    return state_ ? state_->view : unavailable;
}
bool SessionGuard::WritesAllowed() const { return false; }
void SessionGuard::MarkWriteUncertain(const char*) {}
}
uint64_t KFC_RUNTIME_CALL KfcRuntimeWorldSessionId(void) {
    return EcsRuntime::fixture_read_session;
}

namespace {
unsigned checks{};
void check(bool ok) {
    ++checks;
    if (!ok) {
        std::fprintf(stderr, "resource runtime check %u failed\n", checks);
        std::exit(1);
    }
}
std::uintptr_t root_type{}, client_type{}, controller_type{};
void id_callback() {}
void handle_callback() {}
void worker_callback() {}
struct NativeThread {
    std::uintptr_t name{};
    std::uint64_t length{}, capacity{}, reserved{};
    char text[64]{};
    std::uintptr_t handle{}, reserved1{}, reserved2{}, context{};
    std::uint32_t stop{}, reserved3{};
    std::uintptr_t callback{};
};
struct ThreadRow { std::uintptr_t handle{}, name{}; std::uint64_t length{}; } thread_rows[2];
LONG thread_registry_gate{};
std::uint16_t thread_count{};
struct Root { std::uintptr_t type{}, client{}; } root;
struct Client { std::uintptr_t type{}, controller{}, scene_owner{}; };
struct Controller { std::uintptr_t type{}, client{}, main{}; };
struct Main { std::uintptr_t allocator{}, context{}; };
struct Context { std::uintptr_t allocator{}, registry{}; };
struct Registry {
    std::uintptr_t allocator{}, service{};
    std::uint32_t handle{};
    std::uintptr_t resource{};
    std::array<unsigned char, 96> item_map{};
};
struct Reader { std::uintptr_t allocator{}, metadata{}; };
struct Chunk { std::uintptr_t data{}; std::uint64_t size{}; };
struct Service {
    std::uintptr_t allocator{}, reader{}, chunks{};
    std::uint32_t chunk_count{};
    std::uintptr_t id_callback{}, handle_callback{};
    std::uintptr_t payload{};
    std::uint64_t payload_size{};
    std::uint32_t initialized{};
    NativeThread worker;
};
struct Map {
    std::int32_t buckets{}; std::uint32_t bucket_count{};
    std::int32_t keys{}; std::uint32_t key_count{};
    std::int32_t values{}; std::uint32_t value_count{};
};
struct Bucket { std::uint32_t first{}, count{}; };
struct Key { KfcRuntimeResourceId id{}; std::uint64_t reserved{}; };
struct Value { std::uint32_t offset{}, packed{}; };
struct Metadata { std::uint64_t prefix{}; Map map; Bucket bucket; Key key; Value value; };
struct TwoResources { std::uint64_t prefix{}; Map map; Bucket bucket; Key keys[2]; Value values[2]; };
template<class T> std::uintptr_t address(T* value) { return reinterpret_cast<std::uintptr_t>(value); }
std::int32_t relative(const void* field, const void* target) {
    return static_cast<std::int32_t>(reinterpret_cast<std::intptr_t>(target) - reinterpret_cast<std::intptr_t>(field));
}
}

int main() {
    namespace Profile = KfcRuntimeCompatibility::EnshroudedClient;
    const auto image = address(GetModuleHandleW(nullptr));
    const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(image);
    const auto nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(image + dos->e_lfanew);
    Profile::image_size = nt->OptionalHeader.SizeOfImage;
    Profile::image_timestamp = nt->FileHeader.TimeDateStamp;
    Profile::image_sha256 = "private-fixture";
    Profile::image_target = "private-fixture.exe";

    std::uint64_t allocator{};
    std::array<unsigned char, 8> payload{1, 2, 3, 4, 5, 6, 7, 8};
    Chunk chunk{address(payload.data()), payload.size()};
    Metadata metadata;
    metadata.map = {relative(&metadata.map.buckets, &metadata.bucket), 1,
        relative(&metadata.map.keys, &metadata.key), 1,
        relative(&metadata.map.values, &metadata.value), 1};
    metadata.bucket.count = 1;
    metadata.key.id.guid[0] = 7;
    metadata.key.id.type_hash = 0x12345678;
    metadata.key.id.part_index = 3;
    metadata.value.packed = static_cast<std::uint32_t>(payload.size());
    Reader reader{address(&allocator), address(&metadata)};
    Service service{address(&allocator), address(&reader), address(&chunk), 1,
        reinterpret_cast<std::uintptr_t>(&id_callback), reinterpret_cast<std::uintptr_t>(&handle_callback)};
    Registry registry{address(&allocator), address(&service), 1, address(payload.data())};
    Context context{address(&allocator), address(&registry)};
    Main main{address(&allocator), address(&context)};
    Client client{address(&client_type), 0};
    Controller controller{address(&controller_type), address(&client), address(&main)};
    client.controller = address(&controller);
    root = {address(&root_type), address(&client)};
    check(main.allocator < image || main.allocator - image >= Profile::image_size);

    json owner_profile = {{"schemaVersion", 1}, {"target", Profile::image_target},
        {"image", {{"size", Profile::image_size}, {"timestamp", Profile::image_timestamp},
            {"sha256", Profile::image_sha256}}},
        {"ownerLayout", {{"rootObjectRva", address(&root) - image},
            {"rootVtableRva", address(&root_type) - image},
            {"clientPointerOffset", offsetof(Root, client)},
            {"clientVtableRva", address(&client_type) - image},
            {"controllerPointerOffset", offsetof(Client, controller)},
            {"controllerVtableRva", address(&controller_type) - image},
            {"controllerClientBackPointerOffset", offsetof(Controller, client)},
            {"controllerMainOwnerOffset", offsetof(Controller, main)}}}};
    std::string reason;
    check(EngineContext::Configure(owner_profile, reason));

    Layout layout;
    layout.profile_id = "private-fixture";
    layout.main_active_context_offset = offsetof(Main, context);
    layout.context_item_registry_offset = offsetof(Context, registry);
    layout.item_registry_service_offset = offsetof(Registry, service);
    layout.item_registry_handle_offset = offsetof(Registry, handle);
    layout.item_registry_resource_pointer_offset = offsetof(Registry, resource);
    layout.resource_service_reader_offset = offsetof(Service, reader);
    layout.resource_service_id_callback_offset = offsetof(Service, id_callback);
    layout.resource_service_id_callback_rva = service.id_callback - image;
    layout.resource_service_handle_callback_offset = offsetof(Service, handle_callback);
    layout.resource_service_handle_callback_rva = service.handle_callback - image;
    layout.resource_service_chunk_table_offset = offsetof(Service, chunks);
    layout.resource_service_chunk_count_offset = offsetof(Service, chunk_count);
    layout.reader_metadata_offset = offsetof(Reader, metadata);
    layout.metadata_map_offset = offsetof(Metadata, map);
    layout.bucket_rel32_offset = offsetof(Map, buckets);
    layout.bucket_count_offset = offsetof(Map, bucket_count);
    layout.keys_rel32_offset = offsetof(Map, keys);
    layout.key_count_offset = offsetof(Map, key_count);
    layout.values_rel32_offset = offsetof(Map, values);
    layout.value_count_offset = offsetof(Map, value_count);
    layout.map_header_size = sizeof(Map);
    layout.bucket_stride = sizeof(Bucket);
    layout.key_stride = sizeof(Key);
    layout.resource_key_size = sizeof(KfcRuntimeResourceId);
    layout.value_stride = sizeof(Value);
    layout.resource_offset_offset = offsetof(Value, offset);
    layout.packed_size_offset = offsetof(Value, packed);
    layout.chunk_index_offset = 7;
    layout.size_mask = 0xffffff;
    layout.chunk_stride = sizeof(Chunk);
    layout.chunk_base_pointer_offset = offsetof(Chunk, data);
    layout.chunk_size_offset = offsetof(Chunk, size);
    layout.maximum_entries = 8;
    layout.maximum_chunks = 4;
    layout.maximum_resource_bytes = 1024;
    active_layout = layout;
    profile_loaded = true;

    View view;
    check(resolve_view(layout, view, &reason));
    check(view.registry == address(&registry)); // context field stores a pointer
    check(registry_is_bound(layout, view));
    check(ResourceRuntime::Ready());
    check(json::parse(ResourceRuntime::Status())["available"] == true);
    std::size_t count{};
    check(ResourceRuntime::ResourcesByType(metadata.key.id.type_hash, nullptr, 0, &count) && count == 1);
    KfcRuntimeResourceId listed{};
    check(ResourceRuntime::ResourcesByType(metadata.key.id.type_hash, &listed, 1, &count));
    check(listed.part_index == 3 && listed.guid[0] == 7);
    std::array<unsigned char, 8> copied{};
    std::size_t actual{};
    check(ResourceRuntime::Read(listed, 0, nullptr, 0, &actual) && actual == payload.size());
    check(ResourceRuntime::Read(listed, 0, copied.data(), copied.size(), &actual) && copied == payload);
    listed.part_index = 0;
    check(!ResourceRuntime::Read(listed, 0, copied.data(), copied.size(), &actual));
    listed = metadata.key.id;
    service.id_callback = 0;
    check(!resolve_view(layout, view, &reason) && reason == "resource-service-callback-mismatch");
    service.id_callback = image + layout.resource_service_id_callback_rva;
    context.registry = 0;
    check(!resolve_view(layout, view, &reason) && reason == "item-registry-pointer-unavailable");
    context.registry = address(&registry);
    registry.resource += 1;
    check(!ResourceRuntime::Ready());
    registry.resource -= 1;
    metadata.bucket.count = 2;
    check(!ResourceRuntime::Read(listed, 0, copied.data(), copied.size(), &actual));
    metadata.bucket.count = 1;
    chunk.size = 4;
    check(!ResourceRuntime::Read(listed, 0, copied.data(), copied.size(), &actual));
    chunk.size = payload.size();
    check(ResourceRuntime::Ready());
    controller.client = 0;
    check(!ResourceRuntime::Ready());
    controller.client = address(&client);
    auto incorrect_profile = owner_profile;
    incorrect_profile["image"]["sha256"] = "different-image";
    check(!EngineContext::Configure(incorrect_profile, reason));
    check(!ResourceRuntime::Ready());
    check(EngineContext::Configure(owner_profile, reason));
    check(ResourceRuntime::Ready());

    // Inaccessible payloads fail at the actual copy without a speculative page
    // query or an exception escaping through the public API.
    auto* inaccessible = static_cast<unsigned char*>(VirtualAlloc(nullptr, 4096,
        MEM_RESERVE | MEM_COMMIT, PAGE_NOACCESS));
    check(inaccessible != nullptr);
    chunk.data = address(inaccessible);
    registry.resource = chunk.data;
    const auto queries_before_fault = virtual_query_calls;
    check(!ResourceRuntime::Read(metadata.key.id, 0, copied.data(), copied.size(), &actual));
    check(virtual_query_calls == queries_before_fault);
    check(VirtualFree(inaccessible, 0, MEM_RELEASE));
    chunk.data = address(payload.data());
    registry.resource = chunk.data;
    check(!copy_memory(UINTPTR_MAX - 3, copied.data(), 8));
    check(!copy_memory(0, copied.data(), 1));

    // A chunk can contain unreadable gaps. Only the selected resource spans
    // decide listing membership, not readability of unrelated chunk bytes.
    auto* mixed_chunk = static_cast<unsigned char*>(VirtualAlloc(nullptr, 3 * 4096,
        MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE));
    check(mixed_chunk != nullptr);
    std::memcpy(mixed_chunk, payload.data(), payload.size());
    std::memcpy(mixed_chunk + 2 * 4096, payload.data(), payload.size());
    DWORD original_protection{};
    check(VirtualProtect(mixed_chunk + 4096, 4096, PAGE_NOACCESS, &original_protection));
    TwoResources mixed{};
    mixed.map = {relative(&mixed.map.buckets, &mixed.bucket), 1,
        relative(&mixed.map.keys, mixed.keys), 2, relative(&mixed.map.values, mixed.values), 2};
    mixed.bucket = {0, 2};
    mixed.keys[0].id = mixed.keys[1].id = metadata.key.id;
    mixed.keys[1].id.part_index += 1;
    mixed.values[0] = {0, static_cast<std::uint32_t>(payload.size())};
    mixed.values[1] = {2 * 4096, static_cast<std::uint32_t>(payload.size())};
    reader.metadata = address(&mixed);
    chunk = {address(mixed_chunk), 3 * 4096};
    registry.resource = chunk.data;
    check(ResourceRuntime::ResourcesByType(metadata.key.id.type_hash, nullptr, 0, &count) && count == 2);
    check(ResourceRuntime::Read(mixed.keys[0].id, 0, copied.data(), copied.size(), &actual) && copied == payload);
    check(VirtualProtect(mixed_chunk + 2 * 4096, 4096, PAGE_NOACCESS, &original_protection));
    check(ResourceRuntime::ResourcesByType(metadata.key.id.type_hash, nullptr, 0, &count) && count == 1);
    KfcRuntimeResourceId readable_id{};
    check(ResourceRuntime::ResourcesByType(metadata.key.id.type_hash, &readable_id, 1, &count) &&
        readable_id.part_index == mixed.keys[0].id.part_index);
    check(!ResourceRuntime::Read(mixed.keys[1].id, 0, nullptr, 0, &actual));
    check(!ResourceRuntime::Read(mixed.keys[1].id, 0, copied.data(), copied.size(), &actual));
    check(ResourceRuntime::Read(mixed.keys[0].id, 0, nullptr, 0, &actual) && actual == payload.size());
    check(VirtualFree(mixed_chunk, 0, MEM_RELEASE));
    chunk = {address(payload.data()), payload.size()};
    registry.resource = chunk.data;
    reader.metadata = address(&metadata);

    // Match the observed 4,262 ItemInfo records in one stable chunk. The page
    // query cost must follow chunk count rather than resource count.
    constexpr std::size_t resource_count = 4262;
    constexpr auto map_offset = offsetof(Metadata, map);
    constexpr auto bucket_offset = map_offset + sizeof(Map);
    constexpr auto keys_offset = bucket_offset + sizeof(Bucket);
    constexpr auto values_offset = keys_offset + resource_count * sizeof(Key);
    std::vector<unsigned char> large_metadata(values_offset + resource_count * sizeof(Value));
    auto* large_map = reinterpret_cast<Map*>(large_metadata.data() + map_offset);
    auto* large_bucket = reinterpret_cast<Bucket*>(large_metadata.data() + bucket_offset);
    auto* large_keys = reinterpret_cast<Key*>(large_metadata.data() + keys_offset);
    auto* large_values = reinterpret_cast<Value*>(large_metadata.data() + values_offset);
    *large_map = {relative(&large_map->buckets, large_bucket), 1,
        relative(&large_map->keys, large_keys), resource_count,
        relative(&large_map->values, large_values), resource_count};
    *large_bucket = {0, resource_count};
    for (std::size_t index = 0; index < resource_count; ++index) {
        large_keys[index].id = metadata.key.id;
        large_keys[index].id.part_index = static_cast<std::uint32_t>(index);
        large_values[index] = {0, static_cast<std::uint32_t>(payload.size())};
    }
    reader.metadata = address(large_metadata.data());
    active_layout.maximum_entries = resource_count;
    virtual_query_calls = read_process_memory_calls = 0;
    const auto started = std::chrono::steady_clock::now();
    check(ResourceRuntime::ResourcesByType(metadata.key.id.type_hash, nullptr, 0, &count) && count == resource_count);
    std::vector<KfcRuntimeResourceId> all_resources(count);
    check(ResourceRuntime::ResourcesByType(metadata.key.id.type_hash, all_resources.data(), all_resources.size(), &count));
    const auto elapsed_ms = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - started).count();
    check(virtual_query_calls == 2); // One chunk for each count/fill operation.
    std::printf("resource listing benchmark: %zu records, count+fill %.3f ms, VirtualQuery=%u, ReadProcessMemory=%u\n",
        count, elapsed_ms, virtual_query_calls, read_process_memory_calls);
    // Direct GUID lookup and payload reads do not walk the complete type list.
    // The size query checks its selected payload once. Copy calls use RPM only.
    large_bucket->count = 1;
    virtual_query_calls = read_process_memory_calls = 0;
    const auto read_started = std::chrono::steady_clock::now();
    for (unsigned iteration = 0; iteration < 100; ++iteration) {
        check(ResourceRuntime::Read(all_resources[0], 0, nullptr, 0, &actual) && actual == payload.size());
        check(ResourceRuntime::Read(all_resources[0], 0, copied.data(), copied.size(), &actual) && copied == payload);
    }
    const auto read_ms = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - read_started).count();
    const auto benchmark_reads = read_process_memory_calls;
    check(virtual_query_calls == 100);
    const auto queries_before_copy = virtual_query_calls;
    check(ResourceRuntime::Read(all_resources[0], 0, copied.data(), copied.size(), &actual) && copied == payload);
    check(virtual_query_calls == queries_before_copy);
    std::printf("resource read benchmark: 100 size+copy operations %.3f ms, VirtualQuery=%u, ReadProcessMemory=%u\n",
        read_ms, virtual_query_calls, benchmark_reads);

    // The production writer and its original lifetime gate run only against
    // private fixture allocations. No process outside this executable is opened.
    reader.metadata = address(&metadata);
    active_layout = layout;
    auto* scalar_page = static_cast<unsigned char*>(VirtualAlloc(nullptr, 4096,
        MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE));
    check(scalar_page != nullptr);
    std::memcpy(scalar_page, payload.data(), payload.size());
    chunk = {address(scalar_page), 4096};
    registry.resource = address(scalar_page);
    service.payload = address(scalar_page);
    service.payload_size = 4096;
    service.initialized = 1;
    constexpr char worker_name[] = "KfcResourceContent";
    std::memcpy(service.worker.text, worker_name, sizeof(worker_name));
    service.worker.name = address(service.worker.text);
    service.worker.length = sizeof(worker_name) - 1;
    service.worker.capacity = sizeof(service.worker.text);
    service.worker.handle = 123;
    service.worker.context = address(&service.worker);
    service.worker.callback = address(&worker_callback);
    thread_rows[0] = {service.worker.handle, service.worker.name, service.worker.length};
    thread_count = 1;
    owner_profile["provenance"] = {{"ecsReadValidated",true}, {"ecsAuthorityReadValidated",true},
        {"ecsWriteGateValidated",true}, {"resourceAtomicScalarWriteValidated",true}};
    auto& er = owner_profile["ecsReadLayout"];
    for (const char* key : {"mainExecutionOwnerOffset", "executionRegistryOwnerOffset", "executionEntityManagerOffset",
        "entityManagerOwnerOffset", "entityManagerCount", "entityManagerTable", "entityId", "entityGeneration",
        "entityLayout", "entityStorage", "entityDefinitionPointer", "entityRow", "componentBits", "componentOffsets",
        "componentStrides", "clientSceneOwnerOffset", "sceneEntriesPointerOffset", "sceneEntriesCountOffset",
        "sceneEntriesCapacityOffset", "sceneEntryContextOffset"}) er[key] = 0;
    er["maximumEntities"] = 1;
    er["sceneEntryStride"] = 32;
    er["sceneEntryUuidOffset"] = 8;
    er["sceneEntryStateOffset"] = 24;
    er["sceneEntryReadyState"] = 1;
    er["maximumSceneEntries"] = 1;
    owner_profile["ecsWriteLayout"] = {
        {"id","ecs-native-thread-scene-gate-v1"}, {"enabled",false}, {"phasePolicy","untracked-components"},
        {"threadRegistryLockRva",address(&thread_registry_gate)-image},
        {"threadRegistryEntriesRva",address(thread_rows)-image}, {"threadRegistryCountRva",address(&thread_count)-image},
        {"threadRegistryCapacity",2}, {"threadRegistryEntryStride",sizeof(ThreadRow)},
        {"registryEntryHandleOffset",offsetof(ThreadRow,handle)}, {"registryEntryNamePointerOffset",offsetof(ThreadRow,name)},
        {"registryEntryNameLengthOffset",offsetof(ThreadRow,length)}, {"threadInlineNameOffset",offsetof(NativeThread,text)},
        {"sceneThreadOffset",8}, {"threadNamePointerOffset",offsetof(NativeThread,name)},
        {"threadNameLengthOffset",offsetof(NativeThread,length)}, {"threadNameCapacityOffset",offsetof(NativeThread,capacity)},
        {"threadNameCapacity",64}, {"threadHandleOffset",offsetof(NativeThread,handle)},
        {"threadContextOffset",offsetof(NativeThread,context)}, {"threadStopOffset",offsetof(NativeThread,stop)},
        {"threadCallbackOffset",offsetof(NativeThread,callback)}, {"threadCallbackRva",address(&worker_callback)-image},
        {"threadName","GameServer"}, {"sceneContextOffset",0}, {"sceneLockOffset",0}, {"sceneStateOffset",0},
        {"sceneRunningState",1}, {"executionModeOffset",0}, {"executionAuthorityMode",0}, {"maximumWriteBytes",4},
        {"changeTrackerRva",address(&worker_callback)-image}, {"changeTrackerContextOffset",0},
        {"changeTrackerManagerOffset",0}, {"changeTrackerQueueBOffset",0}, {"maximumTrackingQueueCapacity",1},
        {"changeTrackerRegistryAOffset",8}, {"changeTrackerRegistryBOffset",16}, {"changeTrackerBytes",std::string(66,'0')}};
    owner_profile["resourceLayout"] = {{"resourceServiceReaderOffset",offsetof(Service,reader)}};
    owner_profile["resourceWriteLayout"] = {{"id","resource-atomic-scalar-v1"}, {"enabled",true},
        {"serviceThreadOffset",offsetof(Service,worker)}, {"threadName",worker_name},
        {"threadCallbackRva",address(&worker_callback)-image}, {"serviceInitializedOffset",offsetof(Service,initialized)},
        {"serviceInitializedValue",1}, {"servicePayloadPointerOffset",offsetof(Service,payload)},
        {"servicePayloadSizeOffset",offsetof(Service,payload_size)}};
    check(EngineContext::Configure(owner_profile, reason));
    check(EngineContext::ResourceWriteConfigured() && ResourceRuntime::CanWrite());
    // Projection samples fixed scalar bits under the original content-worker
    // lifetime. A late failure publishes neither a prefix nor a partial count.
    EcsRuntime::fixture_read_session = 4242;
    std::array<KfcRuntimeResourceScalarRequest, 64> scalar_requests{};
    for (auto& request : scalar_requests) request = {metadata.key.id, 0};
    std::array<std::uint32_t, 64> scalar_keys{};
    const auto projection = [&](std::size_t requested = 1, std::size_t root_bytes = 8,
        std::size_t key_offset = 0, std::uint64_t session = 4242) {
        actual = 999;
        const auto result = KfcRuntimeResourceReadScalarBatch(scalar_requests.data(), requested,
            metadata.key.id.type_hash, root_bytes, key_offset, session,
            scalar_keys.data(), scalar_keys.size(), &actual);
        check(thread_registry_gate == 0);
        return result;
    };
    std::uint32_t expected_key{};
    std::memcpy(&expected_key, scalar_page, sizeof(expected_key));
    check(projection() == 1 && actual == 1 && scalar_keys[0] == expected_key);
    check(projection(1, 8, 4) == 1 && actual == 1);
    check(projection(0) == 2 && actual == 0);
    check(projection(65) == 2 && actual == 0);
    check(projection(1, 3) == 2 && actual == 0);
    check(projection(1, 8, 1) == 2 && actual == 0);
    check(projection(1, 8, 8) == 2 && actual == 0);
    check(projection(1, 2048) == 2 && actual == 0);
    check(projection(1, 8, 0, 0) == 2 && actual == 0);
    check(projection(1, 8, 0, 4243) == 4 && actual == 0);
    scalar_requests[0].resource.type_hash ^= 1;
    check(projection() == 2 && actual == 0);
    scalar_requests[0].resource = metadata.key.id;
    scalar_keys.fill(0xfeedface);
    scalar_requests[1].resource.part_index += 99;
    check(projection(2) == 5 && actual == 0 && scalar_keys[0] == 0xfeedface);
    scalar_requests[1].resource = metadata.key.id;
    // A changed key, selected entry or owner cannot become a valid projected
    // row merely because the content-worker lifetime remains protected.
    scalar_mutation_trigger = scalar_page;
    scalar_mutation_target = scalar_page;
    check(projection() == 5 && actual == 0 && scalar_keys[0] == 0xfeedface);
    scalar_page[0] ^= 1;
    scalar_mutation_target = reinterpret_cast<unsigned char*>(&metadata.value.offset);
    check(projection() == 5 && actual == 0 && scalar_keys[0] == 0xfeedface);
    metadata.value.offset ^= 1;
    scalar_mutation_target = reinterpret_cast<unsigned char*>(&service.worker.stop);
    check(projection() == 4 && actual == 0 && scalar_keys[0] == 0xfeedface);
    service.worker.stop = 0;
    scalar_mutation_trigger = scalar_mutation_target = nullptr;
    thread_registry_gate = static_cast<LONG>(0x80000000u);
    actual = 999;
    check(KfcRuntimeResourceReadScalarBatch(scalar_requests.data(), 1, metadata.key.id.type_hash,
        8, 0, 4242, scalar_keys.data(), scalar_keys.size(), &actual) == 5 && actual == 0 &&
        thread_registry_gate == static_cast<LONG>(0x80000000u));
    thread_registry_gate = 0;
    actual = 999;
    check(KfcRuntimeResourceReadScalarBatch(reinterpret_cast<const KfcRuntimeResourceScalarRequest*>(1),
        1, metadata.key.id.type_hash, 8, 0, 4242, scalar_keys.data(), scalar_keys.size(), &actual) == 2 && actual == 0);
    check(KfcRuntimeResourceReadScalarBatch(scalar_requests.data(), 1, metadata.key.id.type_hash,
        8, 0, 4242, reinterpret_cast<std::uint32_t*>(1), 1, &actual) == 2 && actual == 0 && thread_registry_gate == 0);
    DWORD projection_protection{};
    check(::VirtualProtect(scalar_page, 4096, PAGE_NOACCESS, &projection_protection));
    check(projection() == 5 && actual == 0);
    check(::VirtualProtect(scalar_page, 4096, projection_protection, &projection_protection));
    profile_loaded = false;
    check(projection() == 3 && actual == 0);
    profile_loaded = true;

    // A stable remote client can read resources but cannot acquire the local
    // authority SessionGuard. Decline only the projection optimization, keeping
    // a stale session or invalid original owner out of the fallback path.
    struct ReplicaManager { std::uintptr_t owner{}; std::uint64_t count{}; std::uintptr_t table{}; };
    struct ReplicaExecution { std::uintptr_t registry{}; ReplicaManager manager; } replica;
    struct EmptyScenes { std::uintptr_t entries{}; std::uint64_t count{}, capacity{}; } scenes;
    std::uintptr_t replica_table[]{1};
    replica.manager = {address(&replica.registry), 1, address(replica_table)};
    const auto previous_allocator = main.allocator;
    main.allocator = address(&replica);
    client.scene_owner = address(&scenes);
    auto replica_profile = owner_profile;
    auto& replica_ecs = replica_profile["ecsReadLayout"];
    replica_ecs["mainExecutionOwnerOffset"] = offsetof(Main,allocator);
    replica_ecs["executionRegistryOwnerOffset"] = offsetof(ReplicaExecution,registry);
    replica_ecs["executionEntityManagerOffset"] = offsetof(ReplicaExecution,manager);
    replica_ecs["entityManagerOwnerOffset"] = offsetof(ReplicaManager,owner);
    replica_ecs["entityManagerCount"] = offsetof(ReplicaManager,count);
    replica_ecs["entityManagerTable"] = offsetof(ReplicaManager,table);
    replica_ecs["clientSceneOwnerOffset"] = offsetof(Client,scene_owner);
    replica_ecs["sceneEntriesPointerOffset"] = offsetof(EmptyScenes,entries);
    replica_ecs["sceneEntriesCountOffset"] = offsetof(EmptyScenes,count);
    replica_ecs["sceneEntriesCapacityOffset"] = offsetof(EmptyScenes,capacity);
    check(EngineContext::Configure(replica_profile, reason));
    EngineContext::EcsView replica_view;
    check(EngineContext::ResolveEcs(replica_view) && replica_view.role == EngineContext::EcsRole::ClientReplica);
    EcsRuntime::fixture_client_replica = true;
    scalar_keys.fill(0xfeedface);
    check(ResourceRuntime::Read(metadata.key.id, 0, copied.data(), copied.size(), &actual));
    check(projection() == 3 && actual == 0 && scalar_keys[0] == 0xfeedface);
    check(projection(1, 8, 0, 4243) == 4 && actual == 0);
    ++replica.manager.owner;
    check(projection() == 4 && actual == 0 && scalar_keys[0] == 0xfeedface);
    --replica.manager.owner;
    EcsRuntime::fixture_client_replica = false;
    main.allocator = previous_allocator;
    client.scene_owner = 0;
    check(EngineContext::Configure(owner_profile, reason));

    read_process_memory_calls = 0;
    for (unsigned iteration = 0; iteration < 64; ++iteration) {
        check(ResourceRuntime::Read(metadata.key.id, 0, nullptr, 0, &actual));
        check(ResourceRuntime::Read(metadata.key.id, 0, copied.data(), copied.size(), &actual));
    }
    const auto individual_projection_reads = read_process_memory_calls;
    read_process_memory_calls = 0;
    const auto projection_started = std::chrono::steady_clock::now();
    check(projection(64) == 1 && actual == 64);
    const auto projection_ms = std::chrono::duration<double, std::milli>(
        std::chrono::steady_clock::now() - projection_started).count();
    check(std::all_of(scalar_keys.begin(), scalar_keys.end(), [&](auto value) { return value == expected_key; }));
    check(read_process_memory_calls < individual_projection_reads);
    std::printf("resource scalar projection: 64 rows %.3f ms, resource RPM=%u vs full size+copy=%u\n",
        projection_ms, read_process_memory_calls, individual_projection_reads);

    // Canonical resource hash vectors use the parser's little-endian GUID
    // seed, followed by the type and part bytes in their original order.
    KfcRuntimeResourceId hash_vector{};
    const unsigned char vector_guid[]{0x8b,0x7d,0x16,0x32,0x09,0x5f,0xd6,0x42,
        0x83,0x90,0x69,0x04,0x49,0x5d,0x6c,0x10};
    std::memcpy(hash_vector.guid, vector_guid, sizeof(vector_guid));
    hash_vector.type_hash = 0xb5ce8765;
    check(canonical_resource_hash(hash_vector) == 0xb761514a);
    hash_vector = {};
    const std::uint32_t seed = 0x12345678;
    std::memcpy(hash_vector.guid, &seed, sizeof(seed));
    hash_vector.type_hash = 0x89abcdef;
    hash_vector.part_index = 0x01020304;
    check(canonical_resource_hash(hash_vector) == 0x73849c34);
    check(canonical_resource_hash({}) == 0);

    // One original open-addressed ItemRegistry entry joins directly to the
    // identical generic ResourceService span. No complete resource walk.
    const auto saved_id = metadata.key.id;
    metadata.key.id.part_index = 0;
    metadata.value.packed = 32;
    std::uint64_t item_bits = 1u << 2;
    std::array<std::uint32_t, 8> item_keys{};
    std::array<std::uintptr_t, 8> item_values{};
    item_keys[2] = 42;
    item_values[2] = address(scalar_page);
    const auto set_map_field = [&](std::size_t offset, auto value) {
        std::memcpy(registry.item_map.data() + offset, &value, sizeof(value));
    };
    set_map_field(8, address(&item_bits));
    set_map_field(16, std::uint64_t{8});
    set_map_field(40, address(item_keys.data()));
    set_map_field(64, address(item_values.data()));
    set_map_field(84, std::uint32_t{1});
    auto& item_layout = active_layout.item_map;
    item_layout.enabled = true;
    item_layout.type_hash = metadata.key.id.type_hash;
    item_layout.root_size = 32;
    item_layout.guid_offset = 4;
    item_layout.map_offset = offsetof(Registry, item_map);
    item_layout.header_size = registry.item_map.size();
    item_layout.bits_offset = 8;
    item_layout.capacity_offset = 16;
    item_layout.keys_offset = 40;
    item_layout.values_offset = 64;
    item_layout.count_offset = 84;
    item_layout.maximum_capacity = 64;
    item_layout.maximum_probes = 8;
    const std::uint32_t item_id = 42;
    std::memcpy(scalar_page, &item_id, sizeof(item_id));
    std::memcpy(scalar_page + 4, metadata.key.id.guid, sizeof(metadata.key.id.guid));
    KfcRuntimeResourceId resolved_item{};
    const auto resolve_item = [&](std::uint32_t wanted = 42, std::uint64_t session = 4242,
        std::uint32_t target_hash = 0x12345678, std::size_t root_bytes = 32,
        std::size_t item_offset = 0, std::size_t guid_offset = 4) {
        const auto result = KfcRuntimeResourceResolveItem(wanted, target_hash, root_bytes,
            item_offset, guid_offset, session, &resolved_item);
        check(thread_registry_gate == 0);
        return result;
    };
    check(resolve_item() == 1 && std::memcmp(&resolved_item, &metadata.key.id, sizeof(resolved_item)) == 0);
    check(json::parse(ResourceRuntime::Status())["itemRegistryResolverConfigured"] == true);
    check(resolve_item(43) == 6 && resolved_item.guid[0] == 0);
    check(resolve_item(0) == 2 && resolve_item(42, 0) == 2);
    check(resolve_item(42, 4243) == 4);
    check(resolve_item(42, 4242, 1) == 2);
    check(resolve_item(42, 4242, 0x12345678, 31) == 2);
    check(resolve_item(42, 4242, 0x12345678, 32, 1) == 2);
    check(resolve_item(42, 4242, 0x12345678, 32, 0, 8) == 2);
    set_map_field(84, std::uint32_t{0});
    check(resolve_item() == 6);
    set_map_field(84, std::uint32_t{1});
    set_map_field(16, std::uint64_t{7});
    check(resolve_item() == 5);
    set_map_field(16, std::uint64_t{128});
    check(resolve_item() == 5);
    set_map_field(16, std::uint64_t{8});
    set_map_field(84, std::uint32_t{9});
    check(resolve_item() == 5);
    set_map_field(84, std::uint32_t{1});
    scalar_page[0] ^= 1;
    check(resolve_item() == 5 && resolved_item.guid[0] == 0);
    scalar_page[0] ^= 1;
    scalar_page[4] ^= 1;
    check(resolve_item() == 5);
    scalar_page[4] ^= 1;
    // Original probing wraps around to slot 0 after a collision at slot 7.
    item_bits = (1u << 7) | 1;
    item_keys[7] = 15;
    item_keys[0] = 7;
    item_values[0] = address(scalar_page);
    scalar_page[0] = 7;
    set_map_field(84, std::uint32_t{2});
    check(resolve_item(7) == 1);
    // A bounded occupied probe prefix is incomplete, not a negative lookup.
    item_bits = 255;
    item_keys.fill(9);
    item_layout.maximum_probes = 2;
    check(resolve_item() == 5);
    item_layout.maximum_probes = 8;
    item_bits = 1u << 2;
    item_keys[2] = 42;
    scalar_page[0] = 42;
    set_map_field(84, std::uint32_t{1});
    scalar_mutation_trigger = scalar_page;
    scalar_mutation_target = reinterpret_cast<unsigned char*>(&item_values[2]);
    check(resolve_item() == 5);
    item_values[2] ^= 1;
    scalar_mutation_target = registry.item_map.data() + 84;
    check(resolve_item() == 5);
    registry.item_map[84] ^= 1;
    scalar_mutation_target = reinterpret_cast<unsigned char*>(&metadata.value.offset);
    check(resolve_item() == 5);
    metadata.value.offset ^= 1;
    scalar_mutation_target = reinterpret_cast<unsigned char*>(&service.worker.stop);
    check(resolve_item() == 4);
    service.worker.stop = 0;
    scalar_mutation_trigger = scalar_mutation_target = nullptr;
    item_layout.enabled = false;
    check(resolve_item() == 3);
    item_layout.enabled = true;
    check(KfcRuntimeResourceResolveItem(42, metadata.key.id.type_hash, 32, 0, 4, 4242,
        reinterpret_cast<KfcRuntimeResourceId*>(1)) == 2 && thread_registry_gate == 0);
    const auto item_started = std::chrono::steady_clock::now();
    for (unsigned iteration = 0; iteration < 100; ++iteration) check(resolve_item() == 1);
    const auto item_ms = std::chrono::duration<double, std::milli>(
        std::chrono::steady_clock::now() - item_started).count();
    std::printf("original ItemRegistry: 100 checked direct lookups %.3f ms\n", item_ms);
    metadata.key.id = saved_id;
    metadata.value.packed = static_cast<std::uint32_t>(payload.size());
    std::memcpy(scalar_page, payload.data(), payload.size());
    item_layout.enabled = false;
    std::uint32_t session_expected{}, session_desired{42};
    std::memcpy(&session_expected, scalar_page, sizeof(session_expected));
    std::array<unsigned char, 8> session_snapshot{};
    check(copy_expected_snapshot(scalar_page, session_snapshot.data(), session_snapshot.size()));
    check(snapshot_matches(address(scalar_page), session_snapshot.data(), session_snapshot.size()));
    // An unchanged scalar cannot authorize a field selected through a changed
    // sibling/variant descriptor in the same resource-sized payload.
    scalar_page[7] ^= 1;
    check(!snapshot_matches(address(scalar_page), session_snapshot.data(), session_snapshot.size()) &&
        std::memcmp(scalar_page, &session_expected, sizeof(session_expected)) == 0);
    scalar_page[7] ^= 1;
    check(!copy_expected_snapshot(reinterpret_cast<const void*>(UINTPTR_MAX - 3), session_snapshot.data(), 8));
    check(!copy_expected_snapshot(reinterpret_cast<const void*>(1), session_snapshot.data(), session_snapshot.size()));
    check(KfcRuntimeResourceCompareExchangeScalarForSession(&metadata.key.id, 0, payload.size(), 0,
        session_snapshot.data(), &session_desired, sizeof(session_desired), 0) == 0);
    check(KfcRuntimeResourceCompareExchangeScalarForSession(&metadata.key.id, 0, payload.size(), 0,
        session_snapshot.data(), &session_desired, sizeof(session_desired), 123) == 0);
    check(std::memcmp(scalar_page, &session_expected, sizeof(session_expected)) == 0);
    check(thread_registry_gate == 0);
    {
        EngineContext::ResourceLifeGuard guard;
        EngineContext::Owners owners;
        check(guard.TryAcquire(address(&service),address(&reader)) && thread_registry_gate == 1);
        check(guard.Validate(owners));
        check(guard.Contains(address(scalar_page),8) && !guard.Contains(address(scalar_page)+4092,8));
        service.reader = 0;
        check(!guard.Validate(owners));
        service.reader = address(&reader);
        check(guard.Validate(owners));
    }
    check(thread_registry_gate == 0);
    {
        EngineContext::ResourceLifeGuard guard;
        check(!guard.TryAcquire(address(&service)+8,address(&reader)) && thread_registry_gate == 0);
        check(!guard.TryAcquire(address(&service),address(&reader)+8) && thread_registry_gate == 0);
    }
    auto unproven_writer = owner_profile;
    unproven_writer["provenance"]["resourceAtomicScalarWriteValidated"] = false;
    check(!EngineContext::Configure(unproven_writer,reason) && !EngineContext::ResourceWriteConfigured());
    check(EngineContext::Configure(owner_profile,reason));
    auto oversized_worker_name = owner_profile;
    oversized_worker_name["resourceWriteLayout"]["threadName"] = std::string(65,'X');
    check(!EngineContext::Configure(oversized_worker_name,reason));
    check(EngineContext::Configure(owner_profile,reason));
    DWORD saved_protection{};
    check(::VirtualProtect(scalar_page,4096,PAGE_READONLY,&saved_protection));
    // Reflected bools and Time values use their own atomic widths. Verify the
    // original data-page protection, conflicts and every unselected byte too.
    for (const std::size_t width : {1u, 2u, 8u}) {
        const std::size_t offset = width == 8 ? 0 : width;
        std::array<unsigned char, 8> original{};
        std::memcpy(original.data(), scalar_page, original.size());
        std::uint64_t before{}, after = width == 8 ? 10000000000ull : 0x5a;
        std::memcpy(&before, original.data() + offset, width);
        if (before == after) ++after;
        check(ResourceRuntime::CompareExchangeScalar(metadata.key.id,0,8,offset,&before,&after,width) == 1);
        std::uint64_t observed{};
        std::memcpy(&observed,scalar_page+offset,width);
        check(observed == after);
        for (std::size_t index = 0; index < original.size(); ++index)
            if (index < offset || index >= offset + width) check(scalar_page[index] == original[index]);
        check(ResourceRuntime::CompareExchangeScalar(metadata.key.id,0,8,offset,&before,&after,width) == 2);
        MEMORY_BASIC_INFORMATION protected_page{};
        check(::VirtualQuery(scalar_page,&protected_page,sizeof(protected_page)) == sizeof(protected_page) &&
            protected_page.Protect == PAGE_READONLY);
        check(ResourceRuntime::CompareExchangeScalar(metadata.key.id,0,8,offset,&after,&before,width) == 1);
        check(std::memcmp(scalar_page,original.data(),original.size()) == 0);
        if (width > 1)
            check(ResourceRuntime::CompareExchangeScalar(metadata.key.id,0,8,1,&before,&after,width) == 0);
    }
    std::uint32_t expected{}, desired = 0x44332211;
    std::memcpy(&expected,payload.data(),4);
    const auto cas = [&](std::size_t size = 8, std::size_t offset = 0, std::size_t width = 4) {
        const auto result = ResourceRuntime::CompareExchangeScalar(metadata.key.id,0,size,offset,&expected,&desired,width);
        check(thread_registry_gate == 0);
        return result;
    };
    const auto is_readonly = [&] {
        MEMORY_BASIC_INFORMATION page{};
        return ::VirtualQuery(scalar_page,&page,sizeof(page)) == sizeof(page) && page.Protect == PAGE_READONLY;
    };
    check(cas() == 1 && is_readonly());
    std::uint32_t actual_scalar{};
    std::memcpy(&actual_scalar,scalar_page,4);
    check(actual_scalar == desired);
    check(cas() == 2 && is_readonly());
    check(cas(7) == 0 && cas(8,5) == 0 && cas(8,1) == 0 && cas(8,0,3) == 0);
    expected = desired;
    desired += 1;
    service.worker.stop = 1;
    check(cas() == 0 && !ResourceRuntime::CanWrite());
    service.worker.stop = 0;
    service.initialized = 0;
    check(cas() == 0);
    service.initialized = 1;
    service.worker.context = 0;
    check(cas() == 0);
    service.worker.context = address(&service.worker);
    service.worker.callback += 1;
    check(cas() == 0);
    service.worker.callback -= 1;
    thread_count = 0;
    check(cas() == 0);
    thread_count = 2;
    thread_rows[1] = thread_rows[0];
    check(cas() == 0);
    thread_count = 1;
    thread_registry_gate = static_cast<LONG>(0x80000000u);
    check(ResourceRuntime::CompareExchangeScalar(metadata.key.id,0,8,0,&expected,&desired,4) == 0 &&
        thread_registry_gate == static_cast<LONG>(0x80000000u));
    thread_registry_gate = 0;
    service.payload_size = 4;
    check(cas() == 0);
    service.payload_size = 4096;
    auto* other_page = static_cast<unsigned char*>(VirtualAlloc(nullptr,4096,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE));
    check(other_page != nullptr);
    chunk.data = registry.resource = address(other_page);
    check(cas() == 0);
    check(VirtualFree(other_page,0,MEM_RELEASE));
    chunk.data = registry.resource = address(scalar_page);
    // Executable private memory and image memory are rejected before protection
    // changes or CAS. The image test uses existing data, never a code address.
    check(::VirtualProtect(scalar_page,4096,PAGE_EXECUTE_READ,&saved_protection));
    const auto protections_before_reject = virtual_protect_calls;
    check(cas() == 0 && virtual_protect_calls == protections_before_reject);
    check(::VirtualProtect(scalar_page,4096,PAGE_READONLY,&saved_protection));
    const auto previous_payload = service.payload;
    service.payload = chunk.data = registry.resource = address(&root);
    check(cas() == 0 && virtual_protect_calls == protections_before_reject);
    service.payload = chunk.data = registry.resource = previous_payload;
    fail_next_prepare = true;
    check(cas() == 0 && is_readonly() && !fail_next_prepare);
    std::memcpy(&actual_scalar,scalar_page,4);
    check(actual_scalar == expected);
    // A preexisting private RW page keeps its original protection and needs no
    // protection transition. It is still an aligned atomic scalar operation.
    check(::VirtualProtect(scalar_page,4096,PAGE_READWRITE,&saved_protection));
    const auto protects_before_rw = virtual_protect_calls;
    check(cas() == 1 && virtual_protect_calls == protects_before_rw);
    MEMORY_BASIC_INFORMATION rw_page{};
    check(::VirtualQuery(scalar_page,&rw_page,sizeof(rw_page)) == sizeof(rw_page) && rw_page.Protect == PAGE_READWRITE);
    expected = desired;
    desired += 1;
    check(::VirtualProtect(scalar_page,4096,PAGE_READONLY,&saved_protection));
    // Inject one failed restore. The result remains uncertain, while the RAII
    // destructor makes a final successful cleanup attempt on this private page.
    fail_next_restore = true;
    check(cas() == 3 && is_readonly() && !fail_next_restore);
    std::memcpy(&actual_scalar,scalar_page,4);
    check(actual_scalar == desired);
    check(!ResourceRuntime::CanWrite() && ResourceRuntime::Ready());
    const auto fault_status = json::parse(ResourceRuntime::Status());
    check(fault_status["resourceWriteWidths"] == json::array({1,2,4,8}));
    check(fault_status["resourceWriteFault"] == true && fault_status["resourceWrites"] == false &&
        fault_status["resourceWriteReason"] == "resource-page-protection-restore-uncertain");
    expected = desired;
    desired += 1;
    const auto protects_after_fault = virtual_protect_calls;
    check(cas() == 0 && virtual_protect_calls == protects_after_fault);
    std::memcpy(&actual_scalar,scalar_page,4);
    check(actual_scalar == expected && is_readonly());
    check(VirtualFree(scalar_page,0,MEM_RELEASE));
    check(EngineContext::Configure(owner_profile,reason));

    // Resource initialization owns only a model published from its exact read
    // profile. An independent legacy execution-profile model remains intact.
    Profile::attribute_calculation_model = "attribute-command-v1";
    clear_owned_attribute_model();
    check(Profile::attribute_calculation_model == "attribute-command-v1");
    json model_profile{{"attributeCalculationModel", "attribute-command-v1"}, {"provenance", json::object()}};
    publish_attribute_model(model_profile);
    check(!owns_attribute_model && Profile::attribute_calculation_model == "attribute-command-v1");
    model_profile["provenance"]["attributeCalculationEvidence"] = "private-original-vectors.json";
    model_profile["attributeCalculationModel"] = "unproven-model";
    publish_attribute_model(model_profile);
    check(!owns_attribute_model);
    model_profile["attributeCalculationModel"] = "attribute-command-v1";
    publish_attribute_model(model_profile);
    check(owns_attribute_model && Profile::attribute_calculation_model == "attribute-command-v1");
    ResourceRuntime::Shutdown();
    check(scalar_write_fault == nullptr);
    check(!owns_attribute_model && Profile::attribute_calculation_model.empty());
    Profile::attribute_calculation_model = "attribute-command-v1";
    ResourceRuntime::Shutdown();
    check(Profile::attribute_calculation_model == "attribute-command-v1");
    std::printf("resource runtime: %u checks passed\n", checks);
}
