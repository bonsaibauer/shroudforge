#include "loaded-resources/loaded_resource_state.h"

using namespace ResourceImplementation;

namespace ResourceImplementation {
std::mutex state_mutex;

Layout active_layout;

bool profile_loaded{};

bool owns_attribute_model{};

std::string profile_status{"not-initialized"};

// Protected by state_mutex. Literal pointers avoid allocation while the
// original lifetime guard is held. An uncertain operation disables every mod's
// subsequent writes until the resource runtime is explicitly reinitialized.
const char* scalar_write_fault{};

void clear_owned_attribute_model(){
    if (owns_attribute_model && attribute_calculation_model == "attribute-command-v1") attribute_calculation_model.clear();
    owns_attribute_model = false;
}

void publish_attribute_model(const json& exact_profile){
    const auto model = exact_profile.value("attributeCalculationModel", std::string{});
    const auto& provenance = exact_profile.at("provenance");
    const auto evidence = provenance.find("attributeCalculationEvidence");
    if (model != "attribute-command-v1" || evidence == provenance.end() ||
        !evidence->is_string() || evidence->get_ref<const std::string&>().empty()) return;
    attribute_calculation_model = model;
    owns_attribute_model = true;
}

}

namespace ResourceRuntime {
bool Initialize() {
    std::scoped_lock lock(state_mutex);
    clear_owned_attribute_model();
    EngineContext::Shutdown();
    profile_loaded = false;
    active_layout = {};
    scalar_write_fault = nullptr;
    profile_status = "no-exact-resource-read-profile";
    std::size_t exact_matches{};
    Layout selected{};
    json selected_profile;
    for (const auto resource_id : KfcRuntimeEmbedded::resource_read_profile_ids) {
        if (!resource_id) continue;
        HMODULE self{};
        if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                reinterpret_cast<LPCWSTR>(&Initialize), &self)) continue;
        const auto resource = FindResourceW(self, MAKEINTRESOURCEW(resource_id), MAKEINTRESOURCEW(10));
        if (!resource) continue;
        const auto loaded = LoadResource(self, resource);
        const auto size = SizeofResource(self, resource);
        const auto* data = static_cast<const char*>(LockResource(loaded));
        if (!loaded || !data || !size) continue;
        try {
            const auto candidate = json::parse(data, data + size);
            const auto version = candidate.value("schemaVersion", 0);
            if ((version != 1 && version != 2) ||
                candidate.value("target", std::string{}) != image_target ||
                candidate.value("kfcFormat", 0) != 4 ||
                !candidate.contains("provenance") ||
                candidate["provenance"].value("status", std::string{}) != "read-validated" ||
                !candidate["provenance"].value("resourceReadValidated", false)) continue;
            const auto& image = candidate.at("image");
            if (image.value("timestamp", 0u) != image_timestamp ||
                image.value("size", 0u) != image_size ||
                image.value("sha256", std::string{}) != image_sha256) continue;
            ++exact_matches;
            selected = parse_layout(candidate);
            selected_profile = candidate;
        } catch (const std::exception& error) {
            profile_status = std::string("resource-profile-error:") + error.what();
            return false;
        }
    }
    if (exact_matches != 1) {
        profile_status = exact_matches ? "ambiguous-exact-resource-read-profile" : "no-exact-resource-read-profile";
        return false;
    }
    std::string owner_error;
    if (!EngineContext::Configure(selected_profile, owner_error)) {
        profile_status = "engine-owner-profile-error:" + owner_error;
        return false;
    }
    publish_attribute_model(selected_profile);
    active_layout = std::move(selected);
    profile_loaded = true;
    profile_status = active_layout.profile_id + ":exact-image-resource-service";
    return true;
}

void Shutdown() {
    std::scoped_lock lock(state_mutex);
    clear_owned_attribute_model();
    EngineContext::Shutdown();
    profile_loaded = false;
    active_layout = {};
    scalar_write_fault = nullptr;
    profile_status = "shutdown";
}

bool Ready() {
    Layout layout;
    {
        std::scoped_lock lock(state_mutex);
        if (!profile_loaded) return false;
        layout = active_layout;
    }
    View view{};
    return resolve_view(layout, view) && registry_is_bound(layout, view);
}

bool CanWrite() {
    std::scoped_lock lock(state_mutex);
    if (!profile_loaded || scalar_write_fault || !EngineContext::ResourceWriteConfigured()) return false;
    View before{}, after{};
    if (!resolve_view(active_layout, before) || !registry_is_bound(active_layout, before)) return false;
    EngineContext::ResourceLifeGuard lifetime;
    EngineContext::Owners owners;
    return lifetime.TryAcquire(before.service, before.reader) && lifetime.Validate(owners) &&
        resolve_view(active_layout, after, nullptr, &owners) && same_view(before, after) &&
        registry_is_bound(active_layout, after);
}

std::string Status() {
    Layout layout;
    std::string detail;
    const char* write_fault{};
    {
        std::scoped_lock lock(state_mutex);
        detail = profile_status;
        write_fault = scalar_write_fault;
        if (!profile_loaded) return json({{"available", false}, {"profile", detail},
            {"mode", "resource-service"}, {"resourceWritesConfigured", false}, {"resourceWrites", false}}).dump();
        layout = active_layout;
    }
    View view{};
    std::string reason;
    const bool available = resolve_view(layout, view, &reason) && registry_is_bound(layout, view);
    if (!available && reason.empty()) reason = "item-registry-resource-handle-mismatch";
    const bool writes_configured = EngineContext::ResourceWriteConfigured();
    const bool writes_available = writes_configured && CanWrite();
    return json({{"available", available}, {"profile", detail},
        {"mode", "resource-service"}, {"reason", available ? "ready" : reason},
        {"bucketCount", view.bucket_count}, {"resourceIndexCount", view.key_count},
        {"chunkCount", view.chunk_count}, {"resourceEngineCalls", false},
        {"itemRegistryResolverConfigured", layout.item_map.enabled},
        {"itemRegistryResolverScope", "resource-service-owner-registry"},
        {"resourceWrites", writes_available}, {"resourceWritesConfigured", writes_configured},
        {"resourceWriteFault", write_fault != nullptr},
        {"resourceWriteReason", write_fault ? write_fault :
            (writes_available ? "ready" : (writes_configured ? "resource-write-gate-unavailable" : "not-configured"))},
        {"resourceWritePolicy", "atomic-naturally-aligned-private-bulk-scalar"},
        // Keep the original preferred-width field for existing status readers.
        // The supported-width list is authoritative for the expanded writer.
        {"resourceWriteWidth", 4}, {"resourceWriteWidths", {1, 2, 4, 8}},
        {"resourceWriteTransactions", false},
        {"resourceHooksInstalled", false}}).dump();
}
}

extern "C" bool __cdecl KfcRuntimeResourceReady() {
    try { return ResourceRuntime::Ready(); } catch (...) { return false; }
}

extern "C" bool __cdecl KfcRuntimeResourceCanWrite() {
    try { return ResourceRuntime::CanWrite(); } catch (...) { return false; }
}

extern "C" std::size_t __cdecl KfcRuntimeResourceStatus(char* buffer, std::size_t capacity) {
    try {
        const auto text = ResourceRuntime::Status();
        const auto required = text.size() + 1;
        if (buffer && capacity >= required) std::memcpy(buffer, text.c_str(), required);
        return required;
    } catch (...) {
        constexpr char error[] = "{\"available\":false,\"reason\":\"resource-status-exception\"}";
        if (buffer && capacity >= sizeof(error)) std::memcpy(buffer, error, sizeof(error));
        return sizeof(error);
    }
}
