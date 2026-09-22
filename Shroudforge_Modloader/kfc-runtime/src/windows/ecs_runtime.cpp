#include "ecs_runtime.h"
#include "game_thread_dispatcher.h"
#include "../../../../Shroudforge_Compatibility/windows/enshrouded_client_1076226.h"

#include <windows.h>
#include <algorithm>
#include <array>
#include <atomic>
#include <cmath>
#include <cstring>
#include <mutex>
#include <sstream>
#include <string>
#include <unordered_map>
#include <vector>

namespace {
constexpr std::size_t max_components = 1024;
constexpr std::size_t manager_neighbourhood = 0x80;
constexpr std::size_t entity_search_bytes = 0x60;
constexpr std::size_t bitset_search_bytes = 0x200;
constexpr std::size_t layout_search_bytes = 0x1800;

struct ComponentType { std::uint16_t index; std::uint32_t size; };
struct ResolvedLayout {
    std::uintptr_t count_address{};
    std::uintptr_t table_address{};
    std::size_t entity_layout{};
    std::size_t entity_storage{};
    std::size_t entity_row{};
    std::size_t entity_id{};
    std::size_t entity_generation{};
    std::size_t component_bits{};
    std::size_t component_offsets{};
    std::size_t component_strides{};
    std::size_t confidence{};
};
struct EntityView {
    std::uintptr_t pointer{};
    std::uintptr_t layout{};
    std::uintptr_t storage{};
    std::uint32_t row{};
    std::uint32_t id{};
    std::uint32_t generation{};
};
struct HandleRecord {
    std::uint32_t id{};
    std::uint32_t generation{};
    std::uint64_t epoch{};
};

std::mutex state_mutex;
std::unordered_map<std::string, ComponentType> types;
std::unordered_map<std::string, std::uint32_t> configured_types;
ResolvedLayout live_layout{};
bool layout_ready{};
std::unordered_map<std::uint32_t, HandleRecord> handles;
std::unordered_map<std::uint64_t, std::uint32_t> reverse_handles;
std::uint32_t next_handle{1};
std::uint64_t layout_epoch{1};
std::mutex write_mutex;
std::atomic<bool> stop_requested{};
std::atomic<std::uint64_t> type_resolution_attempts{};
std::atomic<std::uint64_t> layout_resolution_attempts{};
std::uintptr_t image_base{};

bool readable(std::uintptr_t address, std::size_t size) {
    if (!address || !size || address > UINTPTR_MAX - size) return false;
    MEMORY_BASIC_INFORMATION memory{};
    if (!VirtualQuery(reinterpret_cast<const void*>(address), &memory, sizeof(memory))) return false;
    const auto end = reinterpret_cast<std::uintptr_t>(memory.BaseAddress) + memory.RegionSize;
    return memory.State == MEM_COMMIT && !(memory.Protect & (PAGE_GUARD | PAGE_NOACCESS)) &&
        address + size <= end;
}

bool read_bytes(std::uintptr_t address, void* value, std::size_t size) {
    SIZE_T received{};
    return readable(address, size) && ReadProcessMemory(GetCurrentProcess(),
        reinterpret_cast<const void*>(address), value, size, &received) && received == size;
}
template<class T> bool read(std::uintptr_t address, T& value) {
    return read_bytes(address, &value, sizeof(value));
}

bool read_c_string(std::uintptr_t address, std::string& value) {
    value.clear();
    for (std::size_t index = 0; index < 200; ++index) {
        char character{};
        if (!read(address + index, character)) return false;
        if (!character) return !value.empty();
        if (static_cast<unsigned char>(character) < 0x20 || static_cast<unsigned char>(character) > 0x7e) return false;
        value.push_back(character);
    }
    return false;
}

std::vector<std::uintptr_t> find_string_references(const std::string& wanted) {
    std::vector<std::uintptr_t> strings;
    std::vector<std::uintptr_t> references;
    const auto base = reinterpret_cast<const std::uint8_t*>(image_base);
    const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base);
    const auto nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(base + dos->e_lfanew);
    const auto sections = IMAGE_FIRST_SECTION(nt);
    const auto length = wanted.size();
    for (WORD section_index = 0; section_index < nt->FileHeader.NumberOfSections; ++section_index) {
        const auto& section = sections[section_index];
        if (!(section.Characteristics & IMAGE_SCN_MEM_READ) || section.Characteristics & IMAGE_SCN_MEM_EXECUTE) continue;
        const auto start = base + section.VirtualAddress;
        const auto bytes = static_cast<std::size_t>(section.Misc.VirtualSize);
        for (std::size_t offset = 0; offset + length + 1 <= bytes; ++offset) {
            if (!std::memcmp(start + offset, wanted.data(), length) && !start[offset + length])
                strings.push_back(reinterpret_cast<std::uintptr_t>(start + offset));
        }
    }
    for (WORD section_index = 0; section_index < nt->FileHeader.NumberOfSections; ++section_index) {
        const auto& section = sections[section_index];
        if (!(section.Characteristics & IMAGE_SCN_MEM_READ) || section.Characteristics & IMAGE_SCN_MEM_EXECUTE) continue;
        const auto start = base + section.VirtualAddress;
        const auto bytes = static_cast<std::size_t>(section.Misc.VirtualSize);
        for (std::size_t offset = 0; offset + 8 <= bytes; offset += 8) {
            std::uintptr_t pointer{};
            std::memcpy(&pointer, start + offset, 8);
            if (std::find(strings.begin(), strings.end(), pointer) != strings.end())
                references.push_back(reinterpret_cast<std::uintptr_t>(start + offset));
        }
    }
    return references;
}

bool metadata_name(std::uintptr_t metadata, std::size_t name_field, std::string& name) {
    std::uintptr_t text{};
    return read(metadata + name_field, text) && text && read_c_string(text, name) && name.starts_with("keen::ecs::");
}

bool inspect_component_table(std::uintptr_t table, std::size_t name_field, std::size_t size_field,
                             const std::unordered_map<std::string, std::uint32_t>& expected,
                             std::unordered_map<std::string, ComponentType>& found) {
    found.clear();
    for (std::size_t index = 0; index < max_components; ++index) {
        std::uintptr_t metadata{};
        std::string name;
        std::uint32_t size{};
        if (!read(table + index * 8, metadata) || !metadata ||
            !metadata_name(metadata, name_field, name) || !read(metadata + size_field, size)) continue;
        const auto contract = expected.find(name);
        if (contract == expected.end() || contract->second != size) continue;
        found.emplace(std::move(name), ComponentType{static_cast<std::uint16_t>(index), size});
    }
    return found.size() > 400 && found.contains("keen::ecs::CurrentTransform");
}

bool plausible_transform(std::uintptr_t address, std::uint32_t size) {
    if (!size || size > 512 || !readable(address, size)) return false;
    const auto count = (std::min)(std::size_t{14}, static_cast<std::size_t>(size / sizeof(float)));
    if (!count) return false;
    std::array<float, 14> values{};
    if (!read_bytes(address, values.data(), count * sizeof(float))) return false;
    bool meaningful{};
    for (std::size_t index = 0; index < count; ++index) {
        if (!std::isfinite(values[index]) || std::abs(values[index]) > 100'000'000.0f) return false;
        meaningful = meaningful || std::abs(values[index]) > 0.00001f;
    }
    return meaningful;
}

bool plausible_entity_pointer(std::uintptr_t pointer) {
    if (!readable(pointer, entity_search_bytes)) return false;
    for (std::size_t layout_field = 8; layout_field + 16 <= entity_search_bytes;
         layout_field += 8) {
        std::uint32_t id{};
        std::uintptr_t layout{}, storage{};
        if (read(pointer + layout_field - 8, id) && id &&
            read(pointer + layout_field, layout) && readable(layout, bitset_search_bytes) &&
            read(pointer + layout_field + 8, storage) && readable(storage, 1)) return true;
    }
    return false;
}

std::vector<std::uintptr_t> sample_pointer_table(std::uintptr_t table, std::uint64_t count) {
    const auto probe_count = static_cast<std::size_t>((std::min)(count, std::uint64_t{4096}));
    std::vector<std::uintptr_t> probe(probe_count);
    if (!read_bytes(table, probe.data(), probe.size() * sizeof(probe[0]))) return {};
    const auto plausible = std::count_if(probe.begin(), probe.end(), plausible_entity_pointer);
    if (plausible < 8) return {};

    // A real manager can contain sparse slot tables. Read the complete bounded
    // table after the cheap probe and retain a broad live sample instead of
    // assuming that player/transform entities occur in the first 256 slots.
    std::vector<std::uintptr_t> all(static_cast<std::size_t>(count));
    if (!read_bytes(table, all.data(), all.size() * sizeof(all[0]))) return {};
    std::vector<std::uintptr_t> result;
    result.reserve(16'384);
    const auto bucket_width = (std::max)(std::size_t{1}, all.size() / std::size_t{16'384});
    std::size_t last_bucket = SIZE_MAX;
    for (std::size_t index = 0; index < all.size(); ++index) {
        const auto pointer = all[index];
        if (!plausible_entity_pointer(pointer)) continue;
        const auto bucket = index / bucket_width;
        if (bucket == last_bucket) continue;
        result.push_back(pointer);
        last_bucket = bucket;
        if (result.size() == 16'384) break;
    }
    return result;
}

bool infer_entity_layout(const std::vector<std::uintptr_t>& entities, const ComponentType& anchor,
                         ResolvedLayout& result) {
    if (entities.size() < 3 || anchor.index >= max_components || !anchor.size) return false;
    const auto word = static_cast<std::size_t>(anchor.index / 64) * 8;
    const auto mask = std::uint64_t{1} << (anchor.index % 64);
    for (std::size_t layout_field = 0; layout_field + 8 <= entity_search_bytes; layout_field += 8) {
        std::vector<std::pair<std::uintptr_t, std::uintptr_t>> samples;
        for (const auto entity : entities) {
            std::uintptr_t layout_pointer{};
            if (read(entity + layout_field, layout_pointer) && readable(layout_pointer, layout_search_bytes))
                samples.emplace_back(entity, layout_pointer);
        }
        if (samples.size() < 3) continue;
        for (std::size_t bits_base = 0; bits_base + word + 8 <= bitset_search_bytes; bits_base += 8) {
            std::vector<std::pair<std::uintptr_t, std::uintptr_t>> component_samples;
            for (const auto& sample : samples) {
                std::uint64_t bits{};
                if (read(sample.second + bits_base + word, bits) && (bits & mask)) component_samples.push_back(sample);
            }
            if (component_samples.size() < 3) continue;
            for (std::size_t strides_base = 0;
                 strides_base + static_cast<std::size_t>(anchor.index) * 2 + 2 <= layout_search_bytes;
                 strides_base += 2) {
                std::size_t stride_matches{};
                for (const auto& sample : component_samples) {
                    std::uint16_t stride{};
                    if (read(sample.second + strides_base + anchor.index * 2, stride) && stride == anchor.size) ++stride_matches;
                }
                if (stride_matches < 3) continue;
                for (std::size_t storage_field = 0; storage_field + 8 <= entity_search_bytes; storage_field += 8) {
                    if (storage_field == layout_field) continue;
                    for (std::size_t row_field = 0; row_field + 4 <= entity_search_bytes; row_field += 4) {
                        for (std::size_t offsets_base = 0;
                             offsets_base + static_cast<std::size_t>(anchor.index) * 2 + 2 <= layout_search_bytes;
                             offsets_base += 2) {
                            if (offsets_base == strides_base) continue;
                            std::size_t valid{};
                            for (const auto& sample : component_samples) {
                                std::uintptr_t storage{};
                                std::uint32_t row{};
                                std::uint16_t component_offset{};
                                if (!read(sample.first + storage_field, storage) || !storage ||
                                    !read(sample.first + row_field, row) || row > (1u << 24) ||
                                    !read(sample.second + offsets_base + anchor.index * 2, component_offset)) continue;
                                const auto address = storage + component_offset + static_cast<std::uintptr_t>(row) * anchor.size;
                                if (plausible_transform(address, anchor.size)) ++valid;
                            }
                            if (valid < 3) continue;
                            result.entity_layout = layout_field;
                            result.entity_storage = storage_field;
                            result.entity_row = row_field;
                            result.component_bits = bits_base;
                            result.component_offsets = offsets_base;
                            result.component_strides = strides_base;
                            result.confidence = valid;
                            return true;
                        }
                    }
                }
            }
        }
    }
    return false;
}

bool valid_identity_field(const std::vector<std::uintptr_t>& entities, std::size_t offset) {
    std::unordered_map<std::uint32_t, bool> seen;
    std::size_t valid{};
    for (const auto entity : entities) {
        std::uint32_t id{}, repeated{};
        if (!read(entity + offset, id) || !read(entity + offset, repeated) || id != repeated || !id)
            continue;
        seen.emplace(id, true);
        ++valid;
    }
    return valid >= 4 && seen.size() == valid;
}

bool infer_entity_identity(const std::vector<std::uintptr_t>& entities, ResolvedLayout& result) {
    // Keen keeps the stable entity id immediately before the layout pointer in
    // the currently validated runtime.  Verify that relation from live data;
    // never accept the offset merely because an older build used it.
    if (result.entity_layout >= 8) {
        const auto preferred = result.entity_layout - 8;
        if (valid_identity_field(entities, preferred)) {
            result.entity_id = preferred;
            result.entity_generation = preferred + sizeof(std::uint32_t);
            return true;
        }
    }
    for (std::size_t offset = 0; offset + 8 <= entity_search_bytes; offset += 4) {
        if (offset == result.entity_row || offset + 4 == result.entity_row ||
            offset == result.entity_layout || offset == result.entity_storage) continue;
        if (!valid_identity_field(entities, offset)) continue;
        result.entity_id = offset;
        result.entity_generation = offset + sizeof(std::uint32_t);
        return true;
    }
    return false;
}

bool discover_layout() {
    ComponentType anchor{};
    {
        std::scoped_lock lock(state_mutex);
        const auto found = types.find("keen::ecs::CurrentTransform");
        if (found == types.end()) return false;
        anchor = found->second;
    }
    constexpr std::size_t chunk_size = 16 * 1024 * 1024;
    std::vector<std::uint8_t> copy(chunk_size + manager_neighbourhood);
    std::uintptr_t cursor{};
    MEMORY_BASIC_INFORMATION memory{};
    while (VirtualQuery(reinterpret_cast<const void*>(cursor), &memory, sizeof(memory)) == sizeof(memory)) {
        if (stop_requested.load(std::memory_order_acquire)) return false;
        const auto region = reinterpret_cast<std::uintptr_t>(memory.BaseAddress);
        const auto end = region + memory.RegionSize;
        const bool writable_private = memory.State == MEM_COMMIT && memory.Type == MEM_PRIVATE &&
            !(memory.Protect & (PAGE_GUARD | PAGE_NOACCESS)) &&
            (memory.Protect & (PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY));
        if (writable_private && memory.RegionSize >= manager_neighbourhood) {
            for (std::size_t offset = 0; offset < memory.RegionSize; offset += chunk_size) {
                const auto wanted = (std::min)(copy.size(), memory.RegionSize - offset);
                SIZE_T received{};
                if (!ReadProcessMemory(GetCurrentProcess(), reinterpret_cast<const void*>(region + offset), copy.data(), wanted, &received)) continue;
                for (std::size_t local = 0; local + 8 <= received; local += 8) {
                    std::uint64_t count{};
                    std::memcpy(&count, copy.data() + local, 8);
                    if (count < 16 || count > (1u << 20)) continue;
                    const auto begin = local > manager_neighbourhood ? local - manager_neighbourhood : 0;
                    const auto finish = (std::min)(received - 8, local + manager_neighbourhood);
                    for (std::size_t pointer_at = begin; pointer_at <= finish; pointer_at += 8) {
                        std::uintptr_t table{};
                        std::memcpy(&table, copy.data() + pointer_at, 8);
                        if (table < 0x10000 || (table & 7)) continue;
                        auto entities = sample_pointer_table(table, count);
                        if (entities.size() < 8) continue;
                        ResolvedLayout candidate{};
                        candidate.count_address = region + offset + local;
                        candidate.table_address = region + offset + pointer_at;
                        if (!infer_entity_layout(entities, anchor, candidate) ||
                            !infer_entity_identity(entities, candidate)) continue;
                        std::scoped_lock lock(state_mutex);
                        live_layout = candidate;
                        layout_ready = true;
                        ++layout_epoch;
                        handles.clear();
                        reverse_handles.clear();
                        next_handle = 1;
                        return true;
                    }
                }
            }
        }
        if (end <= cursor) break;
        cursor = end;
    }
    return false;
}

bool layout_snapshot(ResolvedLayout& result) {
    std::scoped_lock lock(state_mutex);
    if (!layout_ready) return false;
    result = live_layout;
    return true;
}
bool entity_pointers(const ResolvedLayout& layout, std::vector<std::uintptr_t>& pointers) {
    std::uint64_t count{};
    std::uintptr_t table{};
    if (!read(layout.count_address, count) || !count || count > (1u << 20) ||
        !read(layout.table_address, table) || !table) {
        std::scoped_lock lock(state_mutex);
        layout_ready = false;
        ++layout_epoch;
        handles.clear();
        reverse_handles.clear();
        return false;
    }
    pointers.resize(static_cast<std::size_t>(count));
    return read_bytes(table, pointers.data(), pointers.size() * sizeof(pointers[0]));
}
bool entity_view(std::uintptr_t pointer, const ResolvedLayout& layout, EntityView& entity) {
    entity.pointer = pointer;
    return pointer && read(pointer + layout.entity_layout, entity.layout) && entity.layout &&
        read(pointer + layout.entity_storage, entity.storage) && entity.storage &&
        read(pointer + layout.entity_row, entity.row) && entity.row <= (1u << 24) &&
        read(pointer + layout.entity_id, entity.id) && entity.id &&
        read(pointer + layout.entity_generation, entity.generation);
}
bool component_address(const EntityView& entity, const ResolvedLayout& layout,
                       const ComponentType& component, std::uintptr_t& address) {
    std::uint64_t bits{};
    std::uint16_t offset{}, stride{};
    if (component.index >= max_components ||
        !read(entity.layout + layout.component_bits + (component.index / 64) * 8, bits) ||
        !(bits & (std::uint64_t{1} << (component.index % 64))) ||
        !read(entity.layout + ShroudforgeCompatibility::EnshroudedClient::component_offsets + component.index * 2, offset) ||
        !read(entity.layout + ShroudforgeCompatibility::EnshroudedClient::component_strides + component.index * 2, stride) || stride != component.size) return false;
    address = entity.storage + offset + static_cast<std::uintptr_t>(entity.row) * stride;
    return readable(address, component.size);
}
bool resolve_component(const char* name, ComponentType& component) {
    if (!name) return false;
    std::scoped_lock lock(state_mutex);
    const auto found = types.find(name);
    if (found == types.end()) return false;
    component = found->second;
    return true;
}
std::uint64_t identity_key(std::uint32_t id, std::uint32_t generation) {
    return static_cast<std::uint64_t>(generation) << 32 | id;
}
std::uint32_t handle_for(const EntityView& entity) {
    std::scoped_lock lock(state_mutex);
    const auto key = identity_key(entity.id, entity.generation);
    if (const auto found = reverse_handles.find(key); found != reverse_handles.end()) return found->second;
    while (!next_handle || handles.contains(next_handle)) ++next_handle;
    const auto handle = next_handle++;
    handles.emplace(handle, HandleRecord{entity.id, entity.generation, layout_epoch});
    reverse_handles.emplace(key, handle);
    return handle;
}
bool entity_for_handle(std::uint32_t handle, const ResolvedLayout& layout, EntityView& entity) {
    HandleRecord record{};
    {
        std::scoped_lock lock(state_mutex);
        const auto found = handles.find(handle);
        if (found == handles.end() || found->second.epoch != layout_epoch) return false;
        record = found->second;
    }
    std::vector<std::uintptr_t> pointers;
    if (!entity_pointers(layout, pointers)) return false;
    bool matched{};
    for (const auto pointer : pointers) {
        EntityView candidate{};
        if (!entity_view(pointer, layout, candidate) || candidate.id != record.id ||
            candidate.generation != record.generation) continue;
        if (matched) return false; // identity must resolve uniquely
        entity = candidate;
        matched = true;
    }
    return matched;
}

struct QueryOperation {
    const char* const* names{};
    std::size_t count{};
    std::uint32_t* entities{};
    std::size_t capacity{};
    std::size_t result{};
};
void query_on_game_thread(void* opaque) {
    auto& operation = *static_cast<QueryOperation*>(opaque);
    std::vector<ComponentType> components(operation.count);
    for (std::size_t index = 0; index < operation.count; ++index)
        if (!resolve_component(operation.names[index], components[index])) return;
    ResolvedLayout layout{};
    std::vector<std::uintptr_t> pointers;
    if (!layout_snapshot(layout) || !entity_pointers(layout, pointers)) return;
    for (const auto pointer : pointers) {
        EntityView entity{};
        if (!entity_view(pointer, layout, entity)) continue;
        bool include = true;
        for (const auto& component : components) {
            std::uintptr_t address{};
            if (!component_address(entity, layout, component, address)) { include = false; break; }
        }
        if (!include) continue;
        if (operation.entities && operation.result < operation.capacity)
            operation.entities[operation.result] = handle_for(entity);
        ++operation.result;
    }
}

struct ResolveOperation { std::uint32_t entity_id{}, result{}; };
void resolve_on_game_thread(void* opaque) {
    auto& operation = *static_cast<ResolveOperation*>(opaque);
    ResolvedLayout layout{};
    std::vector<std::uintptr_t> pointers;
    if (!layout_snapshot(layout) || !entity_pointers(layout, pointers)) return;
    EntityView matched{};
    bool found{};
    for (const auto pointer : pointers) {
        EntityView candidate{};
        if (!entity_view(pointer, layout, candidate) || candidate.id != operation.entity_id) continue;
        if (found) return;
        matched = candidate;
        found = true;
    }
    if (found) operation.result = handle_for(matched);
}

struct ReadOperation {
    std::uint32_t handle{};
    const char* name{};
    void* value{};
    std::size_t size{};
    bool result{};
};
void read_on_game_thread(void* opaque) {
    auto& operation = *static_cast<ReadOperation*>(opaque);
    ComponentType component{};
    ResolvedLayout layout{};
    EntityView entity{};
    std::uintptr_t address{};
    operation.result = operation.value && resolve_component(operation.name, component) &&
        component.size == operation.size && layout_snapshot(layout) &&
        entity_for_handle(operation.handle, layout, entity) &&
        component_address(entity, layout, component, address) &&
        read_bytes(address, operation.value, operation.size);
}

struct WriteOperation {
    std::uint32_t handle{};
    const char* name{};
    const void* expected{};
    const void* value{};
    std::size_t size{};
    bool result{};
};
void write_on_game_thread(void* opaque) {
    auto& operation = *static_cast<WriteOperation*>(opaque);
    ComponentType component{};
    ResolvedLayout layout{};
    EntityView entity{};
    std::uintptr_t address{};
    if (!operation.expected || !operation.value || !resolve_component(operation.name, component) ||
        component.size != operation.size || !layout_snapshot(layout) ||
        !entity_for_handle(operation.handle, layout, entity) ||
        !component_address(entity, layout, component, address)) return;
    std::scoped_lock transaction(write_mutex);
    std::vector<std::uint8_t> before(operation.size), verified(operation.size);
    if (!read_bytes(address, before.data(), operation.size) ||
        std::memcmp(before.data(), operation.expected, operation.size)) return;
    SIZE_T written{};
    if (!WriteProcessMemory(GetCurrentProcess(), reinterpret_cast<void*>(address), operation.value,
            operation.size, &written) || written != operation.size ||
        !read_bytes(address, verified.data(), operation.size) ||
        std::memcmp(verified.data(), operation.value, operation.size)) {
        SIZE_T restored{};
        WriteProcessMemory(GetCurrentProcess(), reinterpret_cast<void*>(address), before.data(),
            operation.size, &restored);
        return;
    }
    operation.result = true;
}
}

namespace EcsRuntime {
bool Initialize() {
    image_base = reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
    if (!image_base) return false;
    const auto dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(image_base);
    if (dos->e_magic != IMAGE_DOS_SIGNATURE) return false;
    const auto nt = reinterpret_cast<const IMAGE_NT_HEADERS64*>(image_base + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE) return false;
    stop_requested.store(false, std::memory_order_release);
    return GameThreadDispatcher::Initialize();
}
void Tick() {
    const auto manager = GameThreadDispatcher::EntityManager();
    if (!manager) return;
    std::scoped_lock lock(state_mutex);
    const auto count_address = manager + ShroudforgeCompatibility::EnshroudedClient::entity_manager_count;
    const auto table_address = manager + ShroudforgeCompatibility::EnshroudedClient::entity_manager_table;
    if (layout_ready && live_layout.count_address == count_address &&
        live_layout.table_address == table_address) return;
    live_layout = {};
    live_layout.count_address = count_address;
    live_layout.table_address = table_address;
    live_layout.entity_id = 0x10;
    live_layout.entity_generation = 0x14;
    live_layout.entity_layout = 0x18;
    live_layout.entity_storage = 0x20;
    live_layout.entity_row = 0x30;
    live_layout.component_bits = 0;
    live_layout.component_offsets = ShroudforgeCompatibility::EnshroudedClient::component_offsets;
    live_layout.component_strides = ShroudforgeCompatibility::EnshroudedClient::component_strides;
    live_layout.confidence = 8;
    layout_ready = true;
    ++layout_epoch;
    handles.clear();
    reverse_handles.clear();
    next_handle = 1;
}
std::string Status() {
    std::scoped_lock lock(state_mutex);
    std::ostringstream text;
    text << "types=" << types.size() << '/' << configured_types.size()
         << " type_attempts=" << type_resolution_attempts.load(std::memory_order_relaxed)
         << " layout_attempts=" << layout_resolution_attempts.load(std::memory_order_relaxed)
         << " game_thread=" << GameThreadDispatcher::Status();
    if (types.empty()) return text.str() + " registry=unresolved layout=unavailable";
    if (!layout_ready) return text.str() + " registry=ready layout=discovering";
    text << " layout=ready"
         << " entity{id=0x" << std::hex << live_layout.entity_id
         << ",generation=0x" << live_layout.entity_generation
         << ",layout=0x" << live_layout.entity_layout
         << ",storage=0x" << live_layout.entity_storage
         << ",row=0x" << live_layout.entity_row << std::dec << '}'
         << " components{bits=0x" << std::hex << live_layout.component_bits
         << ",offsets=0x" << live_layout.component_offsets
         << ",strides=0x" << live_layout.component_strides << std::dec << '}'
         << " confidence=" << live_layout.confidence
         << " epoch=" << layout_epoch;
    return text.str();
}
void Shutdown() {
    stop_requested.store(true, std::memory_order_release);
    GameThreadDispatcher::Shutdown();
    std::scoped_lock lock(state_mutex);
    types.clear();
    configured_types.clear();
    live_layout = {};
    layout_ready = false;
    handles.clear();
    reverse_handles.clear();
    type_resolution_attempts.store(0, std::memory_order_relaxed);
    layout_resolution_attempts.store(0, std::memory_order_relaxed);
}
}

extern "C" bool __cdecl ShroudforgeEcsConfigure(const char* const* names,
                                                const std::uint32_t* sizes,
                                                std::size_t count) {
    if (!names || !sizes || count < 2 || count > 20'000) return false;
    std::unordered_map<std::string, std::uint32_t> contract;
    for (std::size_t index = 0; index < count; ++index) {
        if (!names[index] || !sizes[index]) return false;
        const std::string name{names[index]};
        if (!name.starts_with("keen::ecs::")) return false;
        contract.emplace(name, sizes[index]);
    }
    if (!contract.contains("keen::ecs::CurrentTransform") ||
        !contract.contains("keen::ecs::DynamicActiveNpcState")) return false;
    std::scoped_lock lock(state_mutex);
    if (configured_types == contract) return true;
    configured_types = std::move(contract);
    types.clear();
    for (const auto& component : ShroudforgeCompatibility::EnshroudedClient::runtime_components) {
        const auto configured = configured_types.find(std::string(component.qualified_name));
        if (configured != configured_types.end() && configured->second == component.size)
            types.emplace(configured->first, ComponentType{component.index, component.size});
    }
    live_layout = {};
    layout_ready = false;
    handles.clear();
    reverse_handles.clear();
    return true;
}

extern "C" bool __cdecl ShroudforgeEcsReady() {
    std::scoped_lock lock(state_mutex);
    return GameThreadDispatcher::Ready() && layout_ready && !types.empty() && live_layout.confidence >= 3;
}
extern "C" bool __cdecl ShroudforgeEcsCanWrite() {
    std::scoped_lock lock(state_mutex);
    return GameThreadDispatcher::Ready() && layout_ready && live_layout.confidence >= 8;
}
extern "C" bool __cdecl ShroudforgeEcsDescribe(const char* name, std::uint32_t* size) {
    ComponentType component{};
    if (!size || !resolve_component(name, component)) return false;
    *size = component.size;
    return true;
}
extern "C" std::size_t __cdecl ShroudforgeEcsQuery(const char* const* names, std::size_t count,
                                                    std::uint32_t* entities, std::size_t capacity) {
    if (!names || !count || !ShroudforgeEcsReady()) return 0;
    QueryOperation operation{names, count, entities, capacity};
    return GameThreadDispatcher::Invoke(query_on_game_thread, &operation) ? operation.result : 0;
}
extern "C" std::uint32_t __cdecl ShroudforgeEcsResolve(std::uint32_t entity_id) {
    if (!entity_id || !ShroudforgeEcsReady()) return 0;
    ResolveOperation operation{entity_id};
    return GameThreadDispatcher::Invoke(resolve_on_game_thread, &operation) ? operation.result : 0;
}
extern "C" bool __cdecl ShroudforgeEcsRead(std::uint32_t handle, const char* name,
                                           void* value, std::size_t size) {
    if (!ShroudforgeEcsReady()) return false;
    ReadOperation operation{handle, name, value, size};
    return GameThreadDispatcher::Invoke(read_on_game_thread, &operation) && operation.result;
}
extern "C" bool __cdecl ShroudforgeEcsWrite(std::uint32_t handle, const char* name,
                                            const void* expected, const void* value,
                                            std::size_t size) {
    if (!ShroudforgeEcsCanWrite()) return false;
    WriteOperation operation{handle, name, expected, value, size};
    return GameThreadDispatcher::Invoke(write_on_game_thread, &operation) && operation.result;
}
