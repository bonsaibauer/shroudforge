// Compile the real scan implementation with a private synthetic entity table.
// No game process, hooks, or game files are opened.
#include "ecs_runtime.cpp"
#include <cstdio>
#include <cstdlib>

namespace {
unsigned checks{};
void check(bool ok) {
    ++checks;
    if (!ok) { std::fprintf(stderr, "prop query check %u failed\n", checks); std::exit(1); }
}
}
int main() {
    std::vector<std::uintptr_t> table(32768, 0);
    std::uintptr_t table_address = reinterpret_cast<std::uintptr_t>(table.data());
    std::uint64_t count = table.size();
    live_layout.count_address = reinterpret_cast<std::uintptr_t>(&count);
    live_layout.table_address = reinterpret_cast<std::uintptr_t>(&table_address);
    layout_ready = true;
    types["keen::ecs::CurrentTransform"] = {0, 0x38};
    types["keen::ecs::UsedItem"] = {1, 4};
    prop_recipe_catalog_ready = true;
    double bounds[]{0,0,0,10,10,10};
    auto query = [&] { return KfcRuntimeWorldEntityQueryPropsInBounds(bounds, nullptr, 0); };
    check(query() == SIZE_MAX - 1);
    check(pending_prop_query && pending_prop_query->cursor > 0 && pending_prop_query->cursor <= 8192);
    const auto cursor = pending_prop_query->cursor;
    check(query() == SIZE_MAX - 1);
    check(pending_prop_query->cursor > cursor);
    std::size_t result = SIZE_MAX - 1;
    for (unsigned i=0; i<100 && result==SIZE_MAX-1; ++i) result=query();
    check(result == 0 && !pending_prop_query);

    check(query() == SIZE_MAX - 1);
    ++layout_epoch;
    check(query() == SIZE_MAX && !pending_prop_query);
    check(query() == SIZE_MAX - 1);
    // A slow but advancing scan must survive the old total-duration cutoff.
    pending_prop_query->started_ms = GetTickCount64() - 30001;
    const auto long_scan_cursor = pending_prop_query->cursor;
    check(query() == SIZE_MAX - 1);
    check(pending_prop_query && pending_prop_query->cursor > long_scan_cursor);
    pending_prop_query->started_ms = GetTickCount64() - 120001;
    check(query() == SIZE_MAX && !pending_prop_query);
    check(query() == SIZE_MAX - 1);
    // A genuinely stalled/abandoned operation still fails closed.
    pending_prop_query->progressed_ms = GetTickCount64() - 30001;
    check(query() == SIZE_MAX && !pending_prop_query);

    check(query() == SIZE_MAX - 1);
    // An abandoned region must not monopolize the next selection.
    pending_prop_query->touched_ms = GetTickCount64() - 2001;
    bounds[3] = 11;
    check(query() == SIZE_MAX - 1);
    check(pending_prop_query->bounds[3] == 11);

    pending_prop_query.reset();
    struct Layout { std::uint64_t bits{3}; std::uint16_t offsets[2]{0,56}, strides[2]{56,4}; } layout;
    struct Storage {
        std::int64_t position[3]{1ll<<32,1ll<<32,1ll<<32};
        float rotation[4]{0,0,0,1}, scale[3]{1,1,1};
        std::uint32_t padding{}, item{42};
    } storage;
    std::uint64_t uuid[2]{123,456};
    struct Entity { std::uintptr_t layout, storage, definition; std::uint32_t row{}, id{1}, generation{1}; } entity{
        reinterpret_cast<std::uintptr_t>(&layout), reinterpret_cast<std::uintptr_t>(&storage),
        reinterpret_cast<std::uintptr_t>(uuid)};
    live_layout.entity_layout = offsetof(Entity,layout);
    live_layout.entity_storage = offsetof(Entity,storage);
    live_layout.entity_row = offsetof(Entity,row);
    live_layout.entity_id = offsetof(Entity,id);
    live_layout.entity_generation = offsetof(Entity,generation);
    live_layout.component_bits = offsetof(Layout,bits);
    KfcRuntimeCompatibility::EnshroudedClient::entity_definition = offsetof(Entity,definition);
    KfcRuntimeCompatibility::EnshroudedClient::definition_uuid = 0;
    KfcRuntimeCompatibility::EnshroudedClient::component_offsets = offsetof(Layout,offsets);
    KfcRuntimeCompatibility::EnshroudedClient::component_strides = offsetof(Layout,strides);
    prop_recipes[42] = {{-.5f,-.5f,-.5f,.5f,.5f,.5f},0};
    table[0] = table[1500] = reinterpret_cast<std::uintptr_t>(&entity);
    check(query() == SIZE_MAX-1); // first match must not publish a partial result
    result = SIZE_MAX-1;
    for (unsigned i=0; i<100 && result==SIZE_MAX-1; ++i) result=query();
    check(result == 1); // duplicate entity pointers must not duplicate the prop
    KfcRuntimePropRecord prop{};
    check(KfcRuntimeWorldEntityQueryPropsInBounds(bounds,&prop,1) == 1);
    check(prop.item_id==42 && prop.entity_handle!=0 && prop.template_uuid[0]==123);
    check(!prop_query_result_cache.ready);
    KfcRuntimePropRecord observed{};
    check(KfcRuntimeWorldEntityGetTransform(prop.entity_handle, &observed));
    check(observed.item_id == 42);
    // A readable cached pointer outside the live table is not a live prop.
    table[0] = table[1500] = 0;
    check(!KfcRuntimeWorldEntityGetTransform(prop.entity_handle, &observed));
    table[0] = reinterpret_cast<std::uintptr_t>(&entity);
    ++entity.generation;
    check(!KfcRuntimeWorldEntityGetTransform(prop.entity_handle, &observed));
    --entity.generation;
    check(KfcRuntimeWorldEntityGetTransform(prop.entity_handle, &observed));
    // Real general ECS discovery over a sparse remote-world-sized table.
    table.resize(131072, 0);
    table[0] = 0;
    table.back() = reinterpret_cast<std::uintptr_t>(&entity);
    count = table.size(); table_address = reinterpret_cast<std::uintptr_t>(table.data());
    query_scans.clear(); query_cache.clear();
    QueryOperation discovery;
    const char* names[]{"keen::ecs::CurrentTransform", "keen::ecs::UsedItem"};
    discovery.names=names; discovery.count=2;
    discovery.owned_names={names[0], names[1]};
    unsigned ticks=0;
    do {
        discovery.result=SIZE_MAX;
        query_on_game_thread(&discovery);
        ++ticks;
    } while (discovery.result==SIZE_MAX-1 && ticks<128);
    check(discovery.result==1 && ticks<128);
    check(active_query_cursor.load()==table.size());
    // A world change discards any previous cached handles/results.
    table.back()=0; ++layout_epoch;
    do {
        discovery.result=SIZE_MAX;
        query_on_game_thread(&discovery);
    } while (discovery.result==SIZE_MAX-1);
    check(discovery.result==0);
    // Bounded retry must retain the live-byte guard: never overwrite another patch.
    auto code = static_cast<unsigned char*>(VirtualAlloc(nullptr, 4096,
        MEM_RESERVE | MEM_COMMIT, PAGE_EXECUTE_READWRITE));
    check(code != nullptr);
    const unsigned char original[]{0x90,0x90,0x90,0x90,0x90};
    const unsigned char changed[]{0xcc,0xcc,0xcc,0xcc,0xcc};
    std::memcpy(code, original, sizeof(original));
    check(!GameThreadDispatcher::WriteCode(reinterpret_cast<std::uintptr_t>(code), changed, original, sizeof(original)));
    check(std::memcmp(code, original, sizeof(original)) == 0);
    check(GameThreadDispatcher::WriteCode(reinterpret_cast<std::uintptr_t>(code), original, changed, sizeof(original)));
    check(std::memcmp(code, changed, sizeof(changed)) == 0);
    check(GameThreadDispatcher::WriteCode(reinterpret_cast<std::uintptr_t>(code), changed, original, sizeof(original)));
    VirtualFree(code, 0, MEM_RELEASE);
    std::printf("%u prop query checks passed; no game process opened\n", checks);
}
