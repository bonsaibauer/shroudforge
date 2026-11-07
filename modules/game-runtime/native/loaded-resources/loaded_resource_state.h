#pragma once

#include "loaded-resources/loaded_resource_access.h"
#include "engine-context/engine_owners.h"
#include "engine-context/hold_resource_lifetime.h"
#include "engine-context/lock_scene_access.h"
#include "ecs/entity_session.h"
#include "ecs/component_access.h"
#include "ecs/snapshot_entity_ids.h"
#include "ecs/read_building_prop.h"
#include "profiles/game_profile.h"
#include <windows.h>
#include <algorithm>
#include <array>
#include <cstring>
#include <limits>
#include <map>
#include <mutex>
#include <set>
#include <stdexcept>
#include <type_traits>
#include <vector>
#include <nlohmann/json.hpp>

// Private implementation contract. State is defined once, in the owning unit.

namespace ResourceImplementation {
using nlohmann::json;

using KfcRuntimeCompatibility::EnshroudedClient::image_sha256;

using KfcRuntimeCompatibility::EnshroudedClient::image_size;

using KfcRuntimeCompatibility::EnshroudedClient::image_target;

using KfcRuntimeCompatibility::EnshroudedClient::image_timestamp;

using KfcRuntimeCompatibility::EnshroudedClient::attribute_calculation_model;

struct ItemMapLayout {
    bool enabled{};
    std::uint32_t type_hash{}, part_index{};
    std::size_t root_size{}, item_id_offset{}, guid_offset{}, map_offset{}, header_size{};
    std::size_t bits_offset{}, capacity_offset{}, keys_offset{}, values_offset{}, count_offset{};
    std::size_t maximum_capacity{}, maximum_probes{};
};

struct Layout {
    std::string profile_id;
    bool dedicated_server{};
    std::size_t main_active_context_offset{}, context_item_registry_offset{};
    std::size_t item_registry_service_offset{}, item_registry_handle_offset{};
    std::size_t item_registry_resource_pointer_offset{};
    std::size_t resource_service_reader_offset{}, resource_service_chunk_table_offset{};
    std::size_t resource_service_id_callback_offset{}, resource_service_handle_callback_offset{};
    std::uintptr_t resource_service_id_callback_rva{}, resource_service_handle_callback_rva{};
    std::size_t resource_service_chunk_count_offset{}, reader_metadata_offset{}, metadata_map_offset{};
    std::size_t bucket_rel32_offset{}, bucket_count_offset{}, keys_rel32_offset{}, key_count_offset{};
    std::size_t values_rel32_offset{}, value_count_offset{}, map_header_size{};
    std::size_t bucket_stride{}, key_stride{}, resource_key_size{}, value_stride{};
    std::size_t resource_offset_offset{}, packed_size_offset{}, chunk_index_offset{}, size_mask{};
    std::size_t chunk_stride{}, chunk_base_pointer_offset{}, chunk_size_offset{};
    std::size_t maximum_entries{}, maximum_chunks{}, maximum_resource_bytes{};
    ItemMapLayout item_map;
};

struct View {
    std::uintptr_t image{}, root{}, client{}, controller{}, main_owner{}, context{};
    std::uintptr_t registry{}, registry_resource{}, service{}, reader{}, metadata{}, map{};
    std::uintptr_t buckets{}, keys{}, values{}, chunks{};
    std::uint32_t registry_handle{}, bucket_count{}, key_count{}, value_count{}, chunk_count{};
};

struct Span {
    std::uintptr_t data{};
    std::size_t size{};
    std::uint32_t handle{};
    std::uint8_t chunk{};
};

extern std::mutex state_mutex;

extern Layout active_layout;

extern bool profile_loaded;

extern bool owns_attribute_model;

extern std::string profile_status;

extern const char* scalar_write_fault;

void clear_owned_attribute_model();

void publish_attribute_model(const json& exact_profile);

using RegionCache = std::map<std::uintptr_t, MEMORY_BASIC_INFORMATION>;

bool readable(std::uintptr_t address, std::size_t size, RegionCache& regions);

bool copy_memory(std::uintptr_t address, void* destination, std::size_t size);

bool set_batch_count(std::size_t* actual, std::size_t count) noexcept;

bool publish_scalar_batch(const std::uint32_t* values, std::size_t count,
    std::uint32_t* output, std::size_t* actual) noexcept;

bool publish_resource_id(const KfcRuntimeResourceId& value, KfcRuntimeResourceId* output) noexcept;

template <typename T>
bool read_value(std::uintptr_t address, T& value) {
    static_assert(std::is_trivially_copyable_v<T>);
    return copy_memory(address, &value, sizeof(value));
}

bool add(std::uintptr_t base, std::size_t offset, std::uintptr_t& result);

bool relative_pointer(std::uintptr_t field, std::uintptr_t& pointer);

std::size_t json_size(const json& value, const char* key, std::size_t maximum);

Layout parse_layout(const json& candidate);

bool resolve_view(const Layout& layout, View& view, std::string* reason = nullptr,
    const EngineContext::Owners* guarded_owners = nullptr);

bool same_view(const View& left, const View& right);

bool span_from_entry(const Layout& layout, const View& view, std::size_t index,
    const unsigned char* entry, Span& span);

bool registry_is_bound(const Layout& layout, const View& view);

bool locate(const Layout& layout, const View& view, const KfcRuntimeResourceId& id,
    std::uint32_t static_hash, Span& span);

std::uint32_t canonical_resource_hash(const KfcRuntimeResourceId& id);

struct ItemMapView {
    std::uintptr_t address{}, bits{}, keys{}, values{};
    std::uint64_t capacity{};
    std::uint32_t count{};
    std::array<unsigned char, 128> header{};
};

bool item_map_view(const ItemMapLayout& layout, const View& owner, ItemMapView& view);

std::uint32_t item_map_lookup(const ItemMapLayout& layout, const ItemMapView& view,
    std::uint32_t wanted, std::uintptr_t& payload);

bool snapshot_arrays(const Layout& layout, const View& view,
    std::vector<unsigned char>& keys, std::vector<unsigned char>& values,
    std::vector<unsigned char>& chunks);

bool list_type(const Layout& layout, const View& view, std::uint32_t type_hash,
    std::vector<KfcRuntimeResourceId>& result);

bool compare_exchange_bits(std::uintptr_t address, std::size_t width, std::uint64_t expected,
    std::uint64_t desired, std::uint64_t& observed) noexcept;

bool copy_expected_snapshot(const void* source, unsigned char* destination, std::size_t size) noexcept;

bool snapshot_matches(std::uintptr_t address, const unsigned char* expected, std::size_t size) noexcept;
}
