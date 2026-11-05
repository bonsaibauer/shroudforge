#include "live_profile_layout.h"
#include <windows.h>
#include <algorithm>
#include <array>
#include <cstring>
#include <cwctype>
#include <limits>
#include <map>
#include <set>
#include <stdexcept>
#include <tuple>
#include <vector>

namespace GameProfileDiscovery::Detail {
namespace {
using json = nlohmann::json;
using Address = std::uint64_t;

[[noreturn]] void fail(const char* reason) { throw std::runtime_error(reason); }
void require(bool condition, const char* reason) { if (!condition) fail(reason); }

template<class T> T number(const json& object, const char* key) {
    const auto& value = object.at(key);
    if (!value.is_number_integer()) fail("live-profile-layout-number-invalid");
    const auto raw = value.get<std::int64_t>();
    if (raw < 0 || static_cast<std::uint64_t>(raw) > std::numeric_limits<T>::max())
        fail("live-profile-layout-number-out-of-range");
    return static_cast<T>(raw);
}

class Memory {
    Address base_{};
public:
    explicit Memory(Address base) : base_(base) {}
    Address base() const { return base_; }
    template<class T> T read(Address address) const {
        T value{};
        SIZE_T received{};
        if (!ReadProcessMemory(GetCurrentProcess(), reinterpret_cast<const void*>(address),
                &value, sizeof(value), &received) || received != sizeof(value))
            fail("live-profile-memory-read-failed");
        return value;
    }
    std::vector<std::uint8_t> bytes(Address address, std::size_t size) const {
        require(address && size && size <= (1ull << 30), "live-profile-memory-span-invalid");
        std::vector<std::uint8_t> result(size);
        SIZE_T received{};
        if (!ReadProcessMemory(GetCurrentProcess(), reinterpret_cast<const void*>(address),
                result.data(), result.size(), &received) || received != result.size())
            fail("live-profile-memory-span-unreadable");
        return result;
    }
    Address pointer(Address address) const {
        const auto value = read<Address>(address);
        require(value >= 0x10000000000ull && value < 0x800000000000ull,
            "live-profile-pointer-out-of-user-range");
        return value;
    }
    Address rva(std::uint32_t value) const { return base_ + value; }
};

std::wstring lower_path(std::wstring value) {
    std::replace(value.begin(), value.end(), L'/', L'\\');
    if (value.starts_with(L"\\\\?\\")) value.erase(0, 4);
    std::transform(value.begin(), value.end(), value.begin(), [](wchar_t c) { return towlower(c); });
    while (value.size() > 3 && value.back() == L'\\') value.pop_back();
    return value;
}

void verify_process_image(const std::filesystem::path& expected, const ImageIdentity& identity,
                          Address& module_base) {
    std::array<wchar_t, 32768> path{};
    const auto length = GetModuleFileNameW(nullptr, path.data(), static_cast<DWORD>(path.size()));
    require(length && length < path.size(), "live-profile-process-path-unavailable");
    auto actual = lower_path(std::wstring(path.data(), length));
    auto wanted = lower_path(std::filesystem::absolute(expected).lexically_normal().wstring());
    require(actual == wanted, "live-profile-process-image-path-mismatch");
    const auto module = reinterpret_cast<Address>(GetModuleHandleW(nullptr));
    require(module, "live-profile-main-module-unavailable");
    IMAGE_DOS_HEADER dos{};
    SIZE_T received{};
    require(ReadProcessMemory(GetCurrentProcess(), reinterpret_cast<const void*>(module),
        &dos, sizeof(dos), &received) && received == sizeof(dos) && dos.e_magic == IMAGE_DOS_SIGNATURE,
        "live-profile-loaded-dos-header-invalid");
    IMAGE_NT_HEADERS64 nt{};
    const auto nt_address = module + static_cast<std::uint32_t>(dos.e_lfanew);
    require(ReadProcessMemory(GetCurrentProcess(), reinterpret_cast<const void*>(nt_address),
        &nt, sizeof(nt), &received) && received == sizeof(nt) && nt.Signature == IMAGE_NT_SIGNATURE &&
        nt.FileHeader.Machine == IMAGE_FILE_MACHINE_AMD64 && nt.FileHeader.TimeDateStamp == identity.timestamp &&
        nt.OptionalHeader.SizeOfImage == identity.size,
        "live-profile-loaded-image-identity-mismatch");
    module_base = module;
}

void set_pointer(json& root, const std::string& path, const json& value) {
    try { root[json::json_pointer(path)] = value; }
    catch (const json::exception&) { fail("live-profile-field-path-invalid"); }
}

bool alphanumeric_name(const Memory& memory, Address pointer, std::uint64_t length) {
    if (pointer < 0x10000000000ull || pointer >= 0x800000000000ull || !length || length > 48) return false;
    try {
        const auto name = memory.bytes(pointer, static_cast<std::size_t>(length));
        bool letter{};
        for (const auto byte : name) {
            const bool alpha = (byte >= 'A' && byte <= 'Z') || (byte >= 'a' && byte <= 'z');
            const bool digit = byte >= '0' && byte <= '9';
            if (!alpha && !digit && byte != '_' && byte != '-' && byte != '.') return false;
            letter |= alpha;
        }
        return letter;
    } catch (...) { return false; }
}

struct ThreadRow { std::uint64_t handle{}, name{}, length{}; };
struct ThreadLayout { std::uint32_t stride{}, handle{}, name{}, length{}; std::vector<ThreadRow> rows; };

ThreadLayout discover_thread_layout(const Memory& memory, const json& layout) {
    const auto entries = memory.rva(number<std::uint32_t>(layout, "threadRegistryEntriesRva"));
    const auto count = memory.read<std::uint16_t>(memory.rva(number<std::uint32_t>(layout, "threadRegistryCountRva")));
    require(count && count <= number<std::uint32_t>(layout, "threadRegistryCapacity"),
        "live-profile-thread-count-invalid");
    std::vector<ThreadLayout> matches;
    std::map<std::pair<Address, std::uint64_t>, bool> names;
    for (std::uint32_t stride = 16; stride <= 64; stride += 8) {
        for (std::uint32_t handle = 0; handle + 8 <= stride; handle += 8)
            for (std::uint32_t name = 0; name + 8 <= stride; name += 8) {
                if (name == handle) continue;
                for (std::uint32_t length = 0; length + 8 <= stride; length += 8) {
                    if (length == handle || length == name) continue;
                    ThreadLayout candidate{stride, handle, name, length, {}};
                    bool valid = true;
                    for (std::uint32_t index = 0; index < count; ++index) {
                        const auto row = entries + static_cast<Address>(index) * stride;
                        const auto h = memory.read<std::uint64_t>(row + handle);
                        const auto n = memory.read<Address>(row + name);
                        const auto l = memory.read<std::uint64_t>(row + length);
                        const auto key = std::pair{n, l};
                        const auto found = names.find(key);
                        const auto good_name = found == names.end() ? alphanumeric_name(memory, n, l) : found->second;
                        if (found == names.end()) names.emplace(key, good_name);
                        if (!h || !good_name) { valid = false; break; }
                        candidate.rows.push_back({h, n, l});
                    }
                    if (valid) matches.push_back(std::move(candidate));
                }
            }
    }
    require(matches.size() == 1, "live-profile-thread-layout-not-unique");
    return std::move(matches.front());
}

struct Scene { Address context{}; json observation; };
Scene ready_scene(const Memory& memory, Address owner, const json& ecs) {
    const auto entries = memory.pointer(owner + number<std::uint32_t>(ecs, "sceneEntriesPointerOffset"));
    const auto count = memory.read<std::uint64_t>(owner + number<std::uint32_t>(ecs, "sceneEntriesCountOffset"));
    const auto capacity = memory.read<std::uint64_t>(owner + number<std::uint32_t>(ecs, "sceneEntriesCapacityOffset"));
    require(count <= capacity && capacity <= number<std::uint64_t>(ecs, "maximumSceneEntries"),
        "live-profile-scene-list-bounds-invalid");
    require(!count || entries, "live-profile-scene-array-missing");
    std::vector<std::pair<Address, Address>> ready;
    for (std::uint64_t index = 0; index < count; ++index) {
        const auto entry = entries + index * number<std::uint32_t>(ecs, "sceneEntryStride");
        if (memory.read<std::uint8_t>(entry + number<std::uint32_t>(ecs, "sceneEntryStateOffset")) !=
            number<std::uint8_t>(ecs, "sceneEntryReadyState")) continue;
        const auto context = memory.pointer(entry + number<std::uint32_t>(ecs, "sceneEntryContextOffset"));
        const auto uuid = memory.bytes(entry + number<std::uint32_t>(ecs, "sceneEntryUuidOffset"), 16);
        require(std::any_of(uuid.begin(), uuid.end(), [](std::uint8_t byte) { return byte != 0; }),
            "live-profile-ready-scene-uuid-invalid");
        ready.emplace_back(entry, context);
    }
    require(ready.size() == 1, "live-profile-ready-scene-not-unique");
    require(memory.pointer(owner + number<std::uint32_t>(ecs, "sceneEntriesPointerOffset")) == entries &&
        memory.read<std::uint64_t>(owner + number<std::uint32_t>(ecs, "sceneEntriesCountOffset")) == count &&
        memory.read<std::uint64_t>(owner + number<std::uint32_t>(ecs, "sceneEntriesCapacityOffset")) == capacity,
        "live-profile-scene-list-changed-during-read");
    return {ready.front().second, {{"entry", ready.front().first}, {"count", count}, {"capacity", capacity}}};
}

json validate_entities(const Memory& memory, Address manager, Address registry_owner, const json& ecs) {
    require(memory.pointer(manager + number<std::uint32_t>(ecs, "entityManagerOwnerOffset")) == registry_owner,
        "live-profile-entity-manager-owner-mismatch");
    const auto capacity = memory.read<std::uint64_t>(manager + number<std::uint32_t>(ecs, "entityManagerCount"));
    const auto table = memory.pointer(manager + number<std::uint32_t>(ecs, "entityManagerTable"));
    const auto occupancy = memory.pointer(manager + number<std::uint32_t>(ecs, "entityManagerOccupancy"));
    const auto keys = memory.pointer(manager + number<std::uint32_t>(ecs, "entityManagerKeys"));
    require(capacity && capacity <= number<std::uint64_t>(ecs, "maximumEntities") &&
        !(capacity & (capacity - 1)), "live-profile-entity-manager-capacity-invalid");
    const auto table_bytes = memory.bytes(table, static_cast<std::size_t>(capacity * 8));
    const auto occupancy_bytes = memory.bytes(occupancy, static_cast<std::size_t>(((capacity + 63) / 64) * 8));
    const auto key_bytes = memory.bytes(keys, static_cast<std::size_t>(capacity * 4));
    const auto id_offset = number<std::uint32_t>(ecs, "entityId");
    const auto generation_offset = number<std::uint32_t>(ecs, "entityGeneration");
    const auto entity_layout = number<std::uint32_t>(ecs, "entityLayout");
    const auto storage = number<std::uint32_t>(ecs, "entityStorage");
    const auto definition = number<std::uint32_t>(ecs, "entityDefinitionPointer");
    const auto row_offset = number<std::uint32_t>(ecs, "entityRow");
    require(id_offset + 4 <= 64 && generation_offset + 4 <= 64 && entity_layout + 8 <= 64 &&
        storage + 8 <= 64 && definition + 8 <= 64 && row_offset + 4 <= 64,
        "live-profile-entity-record-field-out-of-bounds");
    std::uint64_t occupied{}, matches{}, stable{};
    std::set<Address> pointers;
    std::vector<std::tuple<Address, std::uint32_t, std::uint32_t>> samples;
    for (std::uint64_t slot = 0; slot < capacity; ++slot) {
        std::uint64_t bits{};
        std::memcpy(&bits, occupancy_bytes.data() + (slot / 64) * 8, 8);
        if (!((bits >> (slot % 64)) & 1)) continue;
        ++occupied;
        std::uint32_t key{};
        Address pointer{};
        std::memcpy(&key, key_bytes.data() + slot * 4, 4);
        std::memcpy(&pointer, table_bytes.data() + slot * 8, 8);
        require(key && pointer, "live-profile-occupied-entity-entry-empty");
        const auto header = memory.bytes(pointer, 64);
        std::uint32_t entity_id{}, generation{}, row{};
        Address layout{}, entity_storage{}, entity_definition{};
        std::memcpy(&entity_id, header.data() + id_offset, 4);
        std::memcpy(&generation, header.data() + generation_offset, 4);
        std::memcpy(&layout, header.data() + entity_layout, 8);
        std::memcpy(&entity_storage, header.data() + storage, 8);
        std::memcpy(&entity_definition, header.data() + definition, 8);
        std::memcpy(&row, header.data() + row_offset, 4);
        require(entity_id == key && layout && entity_storage && entity_definition && row <= (1u << 24) &&
            pointers.insert(pointer).second, "live-profile-entity-record-join-invalid");
        ++matches;
        if (samples.size() < 512) samples.emplace_back(pointer, key, generation);
    }
    require(occupied && occupied == matches, "live-profile-entity-table-empty");
    for (const auto& [pointer, key, generation] : samples) {
        const auto first = memory.bytes(pointer + id_offset, 8);
        const auto second = memory.bytes(pointer + id_offset, 8);
        std::uint32_t first_id{}, first_generation{};
        std::memcpy(&first_id, first.data(), 4);
        std::memcpy(&first_generation, first.data() + 4, 4);
        if (first == second && first_id == key && first_generation == generation) ++stable;
    }
    require(stable == samples.size(), "live-profile-entity-generation-readback-unstable");
    require(memory.read<std::uint64_t>(manager + number<std::uint32_t>(ecs, "entityManagerCount")) == capacity &&
        memory.pointer(manager + number<std::uint32_t>(ecs, "entityManagerTable")) == table &&
        memory.pointer(manager + number<std::uint32_t>(ecs, "entityManagerOccupancy")) == occupancy &&
        memory.pointer(manager + number<std::uint32_t>(ecs, "entityManagerKeys")) == keys,
        "live-profile-entity-manager-changed-during-read");
    return {{"capacity", capacity}, {"occupied", occupied}, {"idKeyMatches", matches},
        {"stableGenerationSamples", stable}, {"generationOffset", generation_offset}};
}

json validate_queues(const Memory& memory, Address manager, const json& layout) {
    const auto context = manager + number<std::uint32_t>(layout, "changeTrackerContextOffset");
    require(memory.pointer(context + number<std::uint32_t>(layout, "changeTrackerManagerOffset")) == manager,
        "live-profile-change-tracker-owner-mismatch");
    struct Queue { std::uint32_t offset{}; std::uint64_t count{}, capacity{}, callback{}; };
    std::vector<Queue> queues;
    for (std::uint32_t offset = 0; offset < 640; offset += 40) {
        const auto data = memory.read<Address>(context + offset);
        const auto count = memory.read<std::uint64_t>(context + offset + 8);
        const auto capacity = memory.read<std::uint64_t>(context + offset + 16);
        const auto callback = memory.read<Address>(context + offset + 24);
        if (data && capacity && !(capacity & (capacity - 1)) && count <= capacity)
            queues.push_back({offset, count, capacity, callback});
    }
    require(queues.size() == 16, "live-profile-change-tracker-queue-count-invalid");
    for (std::size_t index = 0; index < queues.size(); ++index) {
        const auto expected = index < 8 ? 16384ull : 4096ull;
        require(queues[index].capacity == expected, "live-profile-change-tracker-queue-groups-invalid");
        require(queues[index].callback >= memory.base() && queues[index].callback < memory.base() + 0x10000000,
            "live-profile-change-tracker-callback-invalid");
        if (index) require(queues[index].callback == queues[0].callback,
            "live-profile-change-tracker-callback-disagrees");
    }
    require(number<std::uint32_t>(layout, "changeTrackerQueueBOffset") == queues[8].offset,
        "live-profile-change-tracker-queue-b-offset-mismatch");
    require(memory.pointer(context + number<std::uint32_t>(layout, "changeTrackerManagerOffset")) == manager,
        "live-profile-change-tracker-owner-changed");
    return {{"queueCount", queues.size()}, {"categoryAOffset", queues.front().offset},
        {"categoryACapacity", queues.front().capacity}, {"categoryBOffset", queues[8].offset},
        {"categoryBCapacity", queues[8].capacity}};
}

json validate_systems(const Memory& memory, Address owner, const json& layout) {
    const auto count = memory.read<std::uint64_t>(owner + number<std::uint32_t>(layout, "systemCountOffset"));
    require(count && count <= number<std::uint64_t>(layout, "maximumSystems"),
        "live-profile-system-count-invalid");
    const auto normal_offset = number<std::uint32_t>(layout, "systemNormalCountOffset");
    std::uint64_t matches{};
    json contracts = json::array();
    for (std::uint64_t index = 0; index < count; ++index) {
        const auto record = owner + number<std::uint32_t>(layout, "systemsOffset") +
            index * number<std::uint32_t>(layout, "systemStride");
        const auto descriptor = memory.read<Address>(record);
        if (!descriptor) continue;
        const auto reads = memory.read<std::uint64_t>(descriptor + 0xa0);
        const auto writes = memory.read<std::uint64_t>(descriptor + 0xb8);
        const auto normal = memory.read<std::uint8_t>(record + normal_offset);
        if (reads + writes == normal) ++matches;
        for (const auto& contract : layout.at("contracts")) {
            const auto descriptor_rva = number<std::uint32_t>(contract, "mutableSystemDescriptorRva");
            if (descriptor != memory.rva(descriptor_rva)) continue;
            const auto function = memory.read<Address>(record + 0x28);
            const auto expected_function = memory.rva(number<std::uint32_t>(contract, "mutableSystemFunctionRva"));
            contracts.push_back({{"key", contract.at("key")}, {"functionMatches", function == expected_function},
                {"readCount", reads}, {"writeCount", writes}, {"normalCount", normal}});
        }
    }
    require(matches == count && contracts.size() == 1 && contracts[0].at("functionMatches") == true,
        "live-profile-system-binding-counts-invalid");
    require(number<std::uint32_t>(layout, "systemNormalBindingsOffset") + 1024 == normal_offset &&
        normal_offset + 8 == number<std::uint32_t>(layout, "systemAlternateBindingsOffset") &&
        number<std::uint32_t>(layout, "systemAlternateBindingsOffset") + 1024 ==
            number<std::uint32_t>(layout, "systemAlternateCountOffset") &&
        number<std::uint32_t>(layout, "systemAlternateCountOffset") + 8 ==
            number<std::uint32_t>(layout, "systemStride"), "live-profile-system-binding-layout-invalid");
    return {{"systemCount", count}, {"normalCountsMatched", matches}, {"contract", contracts[0]},
        {"normalCountOffset", normal_offset}, {"alternateCountOffset", layout.at("systemAlternateCountOffset")}};
}

struct Chunk { Address base{}; std::uint64_t size{}; };

std::pair<json, std::vector<Chunk>> validate_resource_map(const Memory& memory, Address service,
    Address registry, const json& layout) {
    const auto reader = memory.pointer(service + number<std::uint32_t>(layout, "resourceServiceReaderOffset"));
    const auto metadata = memory.pointer(reader + number<std::uint32_t>(layout, "readerMetadataOffset"));
    const auto header = metadata + number<std::uint32_t>(layout, "metadataMapOffset");
    const auto& map = layout.at("map");
    const auto rel32 = [&](std::uint32_t offset) {
        const auto location = header + offset;
        const auto delta = memory.read<std::int32_t>(location);
        return static_cast<Address>(static_cast<std::int64_t>(location) + delta);
    };
    const auto buckets = rel32(number<std::uint32_t>(map, "bucketRel32Offset"));
    const auto keys = rel32(number<std::uint32_t>(map, "keysRel32Offset"));
    const auto values = rel32(number<std::uint32_t>(map, "valuesRel32Offset"));
    const auto bucket_count = memory.read<std::uint32_t>(header + number<std::uint32_t>(map, "bucketCountOffset"));
    const auto key_count = memory.read<std::uint32_t>(header + number<std::uint32_t>(map, "keyCountOffset"));
    const auto value_count = memory.read<std::uint32_t>(header + number<std::uint32_t>(map, "valueCountOffset"));
    require(bucket_count && !(bucket_count & (bucket_count - 1)) && key_count && key_count == value_count &&
        key_count <= number<std::uint32_t>(layout, "maximumEntries"), "live-profile-resource-map-counts-invalid");
    const auto chunk_table = memory.pointer(service + number<std::uint32_t>(layout, "resourceServiceChunkTableOffset"));
    const auto chunk_count = memory.read<std::uint32_t>(service + number<std::uint32_t>(layout, "resourceServiceChunkCountOffset"));
    require(chunk_count && chunk_count <= number<std::uint32_t>(layout, "maximumChunks"),
        "live-profile-resource-chunk-count-invalid");
    std::vector<Chunk> chunks;
    const auto& chunk_entry = layout.at("chunkEntry");
    for (std::uint32_t index = 0; index < chunk_count; ++index) {
        const auto entry = chunk_table + static_cast<Address>(index) * number<std::uint32_t>(chunk_entry, "stride");
        const auto base = memory.pointer(entry + number<std::uint32_t>(chunk_entry, "basePointerOffset"));
        const auto size = memory.read<std::uint64_t>(entry + number<std::uint32_t>(chunk_entry, "sizeOffset"));
        require(size && base <= UINT64_MAX - size, "live-profile-resource-chunk-range-invalid");
        chunks.push_back({base, size});
    }

    struct Interval { std::uint32_t first{}, end{}; };
    const auto bucket_stride = number<std::uint32_t>(map, "bucketStride");
    const auto bucket_data = memory.bytes(buckets, static_cast<std::size_t>(bucket_count) * bucket_stride);
    std::vector<Interval> intervals;
    std::uint64_t bucket_sum{};
    for (std::uint32_t index = 0; index < bucket_count; ++index) {
        std::uint32_t first{}, length{};
        std::memcpy(&first, bucket_data.data() + index * bucket_stride, 4);
        std::memcpy(&length, bucket_data.data() + index * bucket_stride + 4, 4);
        require(first <= key_count && length <= key_count - first, "live-profile-resource-bucket-invalid");
        bucket_sum += length;
        if (length) intervals.push_back({first, first + length});
    }
    std::sort(intervals.begin(), intervals.end(), [](const auto& a, const auto& b) { return a.first < b.first; });
    std::uint64_t cursor{};
    for (const auto& interval : intervals) {
        require(interval.first == cursor, "live-profile-resource-bucket-partition-invalid");
        cursor = interval.end;
    }
    require(cursor == key_count && bucket_sum == key_count, "live-profile-resource-bucket-partition-incomplete");

    const auto value_stride = number<std::uint32_t>(map, "valueStride");
    const auto value_data = memory.bytes(values, static_cast<std::size_t>(value_count) * value_stride);
    const auto& value_entry = layout.at("valueEntry");
    const auto resource_offset_offset = number<std::uint32_t>(value_entry, "resourceOffsetOffset");
    const auto packed_offset = number<std::uint32_t>(value_entry, "packedSizeOffset");
    const auto chunk_offset = number<std::uint32_t>(value_entry, "chunkIndexOffset");
    const auto size_mask = number<std::uint32_t>(value_entry, "sizeMask");
    const auto handle = memory.read<std::uint32_t>(registry + number<std::uint32_t>(layout, "itemRegistryHandleOffset"));
    require(handle && handle <= value_count, "live-profile-item-registry-handle-invalid");
    std::uint64_t valid_values{};
    Address mapped_payload{};
    for (std::uint32_t index = 0; index < value_count; ++index) {
        const auto offset = static_cast<std::size_t>(index) * value_stride;
        std::uint32_t resource_offset{};
        std::memcpy(&resource_offset, value_data.data() + offset + resource_offset_offset, 4);
        const auto packed = static_cast<std::uint32_t>(value_data[offset + packed_offset]) |
            (static_cast<std::uint32_t>(value_data[offset + packed_offset + 1]) << 8) |
            (static_cast<std::uint32_t>(value_data[offset + packed_offset + 2]) << 16);
        const auto chunk_index = value_data[offset + chunk_offset];
        const auto size = packed & size_mask;
        require(chunk_index < chunks.size() && resource_offset <= chunks[chunk_index].size &&
            size <= chunks[chunk_index].size - resource_offset, "live-profile-resource-value-out-of-chunk");
        ++valid_values;
        if (index == handle - 1) mapped_payload = chunks[chunk_index].base + resource_offset;
    }
    const auto payload = memory.pointer(registry + number<std::uint32_t>(layout, "itemRegistryResourcePointerOffset"));
    require(mapped_payload == payload, "live-profile-item-registry-resource-handle-mismatch");

    const auto key_stride = number<std::uint32_t>(map, "keyStride");
    const auto key_size = number<std::uint32_t>(map, "resourceKeySize");
    require(key_stride == 32 && key_size == 24, "live-profile-resource-key-contract-unsupported");
    const auto key_data = memory.bytes(keys, static_cast<std::size_t>(key_count) * key_stride);
    for (std::uint32_t index = 0; index < key_count; ++index) {
        const auto offset = static_cast<std::size_t>(index) * key_stride;
        std::uint32_t id{};
        std::memcpy(&id, key_data.data() + offset, 4);
        require(id && std::all_of(key_data.begin() + offset + key_size,
            key_data.begin() + offset + key_stride, [](std::uint8_t byte) { return byte == 0; }),
            "live-profile-resource-key-stride-invalid");
    }
    json observed{{"bucketCount", bucket_count}, {"keyCount", key_count}, {"valueCount", value_count},
        {"bucketRanges", intervals.size()}, {"bucketSum", bucket_sum}, {"validValues", valid_values},
        {"chunkCount", chunk_count}, {"mappedPayloadMatches", true}, {"keyStride", key_stride},
        {"keyPayloadBytes", key_size}, {"zeroPaddingRecords", key_count},
        {"chunkTableOffset", layout.at("resourceServiceChunkTableOffset")}};
    std::vector<Chunk> result_chunks = std::move(chunks);
    return {std::move(observed), std::move(result_chunks)};
}

json validate_item_map(const Memory& memory, Address registry, const json& layout,
                       const std::vector<Chunk>& chunks) {
    const auto& item = layout.at("itemIdMap");
    const auto address = registry + number<std::uint32_t>(item, "mapOffset");
    const auto bits = memory.pointer(address + number<std::uint32_t>(item, "bitsPointerOffset"));
    const auto capacity = memory.read<std::uint64_t>(address + number<std::uint32_t>(item, "capacityOffset"));
    const auto keys = memory.pointer(address + number<std::uint32_t>(item, "keysPointerOffset"));
    const auto values = memory.pointer(address + number<std::uint32_t>(item, "valuesPointerOffset"));
    const auto count = memory.read<std::uint32_t>(address + number<std::uint32_t>(item, "countOffset"));
    require(capacity && capacity <= number<std::uint64_t>(item, "maximumCapacity") &&
        !(capacity & (capacity - 1)) && count <= capacity, "live-profile-item-map-header-invalid");
    const auto bit_data = memory.bytes(bits, static_cast<std::size_t>(((capacity + 63) / 64) * 8));
    const auto key_data = memory.bytes(keys, static_cast<std::size_t>(capacity * 4));
    const auto value_data = memory.bytes(values, static_cast<std::size_t>(capacity * 8));
    std::set<std::uint32_t> ids;
    std::uint64_t occupied{}, key_matches{}, in_chunks{};
    const auto id_offset = number<std::uint32_t>(item, "itemIdOffset");
    const auto target_size = number<std::uint32_t>(item, "targetSize");
    for (std::uint64_t slot = 0; slot < capacity; ++slot) {
        std::uint64_t word{};
        std::memcpy(&word, bit_data.data() + (slot / 64) * 8, 8);
        if (!((word >> (slot % 64)) & 1)) continue;
        ++occupied;
        std::uint32_t key{};
        Address pointer{};
        std::memcpy(&key, key_data.data() + slot * 4, 4);
        std::memcpy(&pointer, value_data.data() + slot * 8, 8);
        require(key && pointer && ids.insert(key).second, "live-profile-item-map-duplicate-or-empty-entry");
        if (memory.read<std::uint32_t>(pointer + id_offset) == key) ++key_matches;
        const auto within_chunk = std::any_of(chunks.begin(), chunks.end(), [&](const auto& chunk) {
            return pointer >= chunk.base && pointer <= chunk.base + chunk.size &&
                target_size <= chunk.base + chunk.size - pointer;
        });
        require(within_chunk, "live-profile-item-map-value-outside-resource-chunks");
        ++in_chunks;
    }
    require(occupied == count && ids.size() == count && key_matches == count && in_chunks == count,
        "live-profile-item-map-entries-do-not-match-resource-items");
    return {{"mapOffset", item.at("mapOffset")}, {"capacity", capacity}, {"count", count},
        {"occupied", occupied}, {"uniqueIds", ids.size()}, {"keyMatchesPayloadId", key_matches},
        {"payloadsInResourceChunks", in_chunks}};
}

const std::set<std::string>& supported_fields() {
    static const std::set<std::string> paths = {
        "/ecsReadLayout/entityGeneration", "/ecsWriteLayout/changeTrackerQueueBOffset",
        "/ecsWriteLayout/registryEntryHandleOffset", "/ecsWriteLayout/registryEntryNameLengthOffset",
        "/ecsWriteLayout/registryEntryNamePointerOffset", "/ecsWriteLayout/threadRegistryEntryStride",
        "/resourceLayout/itemIdMap/bitsPointerOffset", "/resourceLayout/itemIdMap/capacityOffset",
        "/resourceLayout/itemIdMap/countOffset", "/resourceLayout/itemIdMap/keysPointerOffset",
        "/resourceLayout/itemIdMap/mapOffset", "/resourceLayout/itemIdMap/valuesPointerOffset",
        "/resourceLayout/itemRegistryHandleOffset", "/resourceLayout/itemRegistryResourcePointerOffset",
        "/resourceLayout/itemRegistryServiceOffset", "/resourceLayout/map/bucketCountOffset",
        "/resourceLayout/map/bucketRel32Offset", "/resourceLayout/map/bucketStride",
        "/resourceLayout/map/keyCountOffset", "/resourceLayout/map/keyStride",
        "/resourceLayout/map/keysRel32Offset", "/resourceLayout/map/resourceKeySize",
        "/resourceLayout/map/valueCountOffset", "/resourceLayout/map/valueStride",
        "/resourceLayout/map/valuesRel32Offset", "/resourceLayout/resourceServiceChunkTableOffset",
        "/serviceLayout/systemNormalCountOffset"};
    return paths;
}

void add_fields(const json& source, std::map<std::string, json>& values) {
    if (!source.is_array()) return;
    for (const auto& field : source) {
        if (!field.is_object() || !field.contains("path") || !field.contains("after")) continue;
        if (!field.at("path").is_string()) continue;
        values[field.at("path").get<std::string>()] = field.at("after");
    }
}
}

bool ResolveLiveProfileLayouts(const std::filesystem::path& executable_path, const ImageIdentity& identity,
    json& profile, const json& rebase_evidence, json& evidence) {
    evidence = {{"method", "exact-process-live-profile-layout-validation"}, {"complete", false},
        {"fields", json::array()}, {"limitations", {"live layouts do not approve native call ABIs or gameplay effects"}}};
    try {
        std::map<std::string, json> values;
        if (rebase_evidence.contains("layoutProof")) {
            add_fields(rebase_evidence.at("layoutProof").value("fields", json::array()), values);
            add_fields(rebase_evidence.at("layoutProof").value("unresolved", json::array()), values);
        }
        if (rebase_evidence.contains("fields")) add_fields(rebase_evidence.at("fields"), values);
        std::set<std::string> unresolved;
        for (const auto& row : rebase_evidence.at("unresolved")) {
            if (!row.is_object() || !row.contains("path") || !row.at("path").is_string())
                fail("live-profile-unresolved-non-layout-contract");
            unresolved.insert(row.at("path").get<std::string>());
        }
        require(!unresolved.empty(), "live-profile-no-unresolved-layout-fields");
        for (const auto& path : unresolved) {
            require(supported_fields().contains(path), "live-profile-unresolved-layout-not-dynamically-supported");
            const auto found = values.find(path);
            require(found != values.end() && found->second.is_number_integer(),
                "live-profile-layout-candidate-value-missing");
            set_pointer(profile, path, found->second);
        }
        Address module_base{};
        verify_process_image(executable_path, identity, module_base);
        Memory memory(module_base);
        evidence["pid"] = GetCurrentProcessId();
        evidence["moduleBase"] = module_base;
        evidence["image"] = {{"target", identity.target}, {"sha256", identity.sha256},
            {"timestamp", identity.timestamp}, {"size", identity.size}};
        evidence["candidateFieldCount"] = unresolved.size();
        std::map<std::string, json> observations;
        const auto add_observation = [&](const std::string& path, const std::string& method, const json& details) {
            if (!unresolved.contains(path)) return;
            observations[path] = {{"method", method}, {"observations", details}};
        };

        const auto& write = profile.at("ecsWriteLayout");
        const auto thread = discover_thread_layout(memory, write);
        require(thread.stride == number<std::uint32_t>(write, "threadRegistryEntryStride") &&
            thread.handle == number<std::uint32_t>(write, "registryEntryHandleOffset") &&
            thread.name == number<std::uint32_t>(write, "registryEntryNamePointerOffset") &&
            thread.length == number<std::uint32_t>(write, "registryEntryNameLengthOffset"),
            "live-profile-thread-candidate-does-not-match-unique-layout");
        const json thread_observation{{"rows", thread.rows.size()}, {"validTupleCount", 1},
            {"stride", thread.stride}, {"handleOffset", thread.handle},
            {"namePointerOffset", thread.name}, {"nameLengthOffset", thread.length}};
        for (const auto* path : {"/ecsWriteLayout/threadRegistryEntryStride", "/ecsWriteLayout/registryEntryHandleOffset",
            "/ecsWriteLayout/registryEntryNamePointerOffset", "/ecsWriteLayout/registryEntryNameLengthOffset"})
            add_observation(path, "unique-live-thread-registry-row-shape", thread_observation);

        const auto& ecs = profile.at("ecsReadLayout");
        Address scene_owner{}, resource_owner{};
        if (identity.target == "enshrouded.exe") {
            const auto& owner = profile.at("ownerLayout");
            const auto root = memory.rva(number<std::uint32_t>(owner, "rootObjectRva"));
            const auto client = memory.pointer(root + number<std::uint32_t>(owner, "clientPointerOffset"));
            const auto controller = memory.pointer(client + number<std::uint32_t>(owner, "controllerPointerOffset"));
            const auto main_owner = memory.pointer(controller + number<std::uint32_t>(owner, "controllerMainOwnerOffset"));
            require(memory.read<Address>(root) == memory.rva(number<std::uint32_t>(owner, "rootVtableRva")) &&
                memory.read<Address>(client) == memory.rva(number<std::uint32_t>(owner, "clientVtableRva")) &&
                memory.read<Address>(controller) == memory.rva(number<std::uint32_t>(owner, "controllerVtableRva")) &&
                memory.pointer(controller + number<std::uint32_t>(owner, "controllerClientBackPointerOffset")) == client,
                "live-profile-client-owner-chain-invalid");
            scene_owner = memory.pointer(client + number<std::uint32_t>(ecs, "clientSceneOwnerOffset"));
            resource_owner = main_owner;
        } else if (identity.target == "enshrouded_server.exe") {
            const auto name = write.at("threadName").get<std::string>();
            const auto thread_name_length = write.at("threadName").get<std::string>().size();
            std::vector<ThreadRow> named;
            for (const auto& row : thread.rows) {
                const auto bytes = memory.bytes(row.name, static_cast<std::size_t>(row.length));
                if (std::string(bytes.begin(), bytes.end()) == name) named.push_back(row);
            }
            require(named.size() == 1 && named.front().length == thread_name_length,
                "live-profile-server-thread-not-unique");
            const auto thread_object = named.front().name - number<std::uint32_t>(write, "threadInlineNameOffset");
            scene_owner = thread_object - number<std::uint32_t>(write, "sceneThreadOffset");
            require(memory.pointer(thread_object + number<std::uint32_t>(write, "threadNamePointerOffset")) == named.front().name &&
                memory.read<std::uint64_t>(thread_object + number<std::uint32_t>(write, "threadNameLengthOffset")) == thread_name_length &&
                memory.read<std::uint64_t>(thread_object + number<std::uint32_t>(write, "threadNameCapacityOffset")) ==
                    number<std::uint64_t>(write, "threadNameCapacity") &&
                memory.read<Address>(thread_object + number<std::uint32_t>(write, "threadHandleOffset")) == named.front().handle &&
                memory.pointer(thread_object + number<std::uint32_t>(write, "threadContextOffset")) ==
                    scene_owner + number<std::uint32_t>(write, "sceneContextOffset") &&
                memory.read<std::uint32_t>(thread_object + number<std::uint32_t>(write, "threadStopOffset")) == 0 &&
                memory.read<Address>(thread_object + number<std::uint32_t>(write, "threadCallbackOffset")) ==
                    memory.rva(number<std::uint32_t>(write, "threadCallbackRva")) &&
                memory.read<std::uint8_t>(scene_owner + number<std::uint32_t>(write, "sceneStateOffset")) ==
                    number<std::uint8_t>(write, "sceneRunningState"), "live-profile-server-thread-owner-chain-invalid");
            resource_owner = 0;
        } else fail("live-profile-target-image-kind-invalid");

        const auto scene = ready_scene(memory, scene_owner, ecs);
        if (identity.target == "enshrouded_server.exe") resource_owner = scene.context;
        const auto execution = memory.pointer(scene.context + number<std::uint32_t>(ecs, "mainExecutionOwnerOffset"));
        const auto registry_owner = execution + number<std::uint32_t>(ecs, "executionRegistryOwnerOffset");
        const auto manager = execution + number<std::uint32_t>(ecs, "executionEntityManagerOffset");
        auto entity_observation = validate_entities(memory, manager, registry_owner, ecs);
        entity_observation["readyScene"] = scene.observation;
        add_observation("/ecsReadLayout/entityGeneration", "live-entity-table-key-generation-join", entity_observation);

        const auto queue_observation = validate_queues(memory, manager, write);
        add_observation("/ecsWriteLayout/changeTrackerQueueBOffset", "live-change-tracker-queue-groups", queue_observation);
        const auto system_observation = validate_systems(memory, registry_owner, profile.at("serviceLayout"));
        add_observation("/serviceLayout/systemNormalCountOffset", "live-named-system-binding-counts", system_observation);

        const auto& resource = profile.at("resourceLayout");
        const auto active = memory.pointer(resource_owner + number<std::uint32_t>(resource, "mainActiveContextOffset"));
        const auto registry = memory.pointer(active + number<std::uint32_t>(resource, "contextItemRegistryOffset"));
        const auto service = memory.pointer(registry + number<std::uint32_t>(resource, "itemRegistryServiceOffset"));
        require(memory.read<Address>(service + number<std::uint32_t>(resource, "resourceServiceIdCallbackOffset")) ==
                memory.rva(number<std::uint32_t>(resource, "resourceServiceIdCallbackRva")) &&
            memory.read<Address>(service + number<std::uint32_t>(resource, "resourceServiceHandleCallbackOffset")) ==
                memory.rva(number<std::uint32_t>(resource, "resourceServiceHandleCallbackRva")),
            "live-profile-resource-service-callback-mismatch");
        auto [resource_observation, chunks] = validate_resource_map(memory, service, registry, resource);
        const auto item_observation = validate_item_map(memory, registry, resource, chunks);
        json item_join = item_observation;
        item_join["callbacksMatch"] = true;
        item_join["registryHandle"] = memory.read<std::uint32_t>(registry + number<std::uint32_t>(resource, "itemRegistryHandleOffset"));
        item_join["registryPayloadMatchesResourceIndex"] = resource_observation.at("mappedPayloadMatches");
        for (const auto& [path, value] : values) {
            if (path.starts_with("/resourceLayout/itemRegistry") || path.starts_with("/resourceLayout/itemIdMap/"))
                add_observation(path, "live-item-registry-resource-join", item_join);
            else if (path.starts_with("/resourceLayout/map/") || path == "/resourceLayout/resourceServiceChunkTableOffset")
                add_observation(path, "live-resource-map-chunk-and-key-validation", resource_observation);
        }

        require(observations.size() == unresolved.size(), "live-profile-layout-evidence-coverage-incomplete");
        for (const auto& path : unresolved) {
            const auto& observed = observations.at(path);
            evidence["fields"].push_back({{"path", path}, {"value", values.at(path)}, {"verified", true},
                {"method", observed.at("method")}, {"observations", observed.at("observations")}});
        }
        evidence["complete"] = true;
        return true;
    } catch (const std::exception& error) {
        evidence["error"] = error.what();
        return false;
    }
}
}
