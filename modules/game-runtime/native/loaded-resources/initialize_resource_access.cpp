#include "loaded-resources/loaded_resource_state.h"
#include "loaded-resources/select_resource_profile.h"
#include "profiles/read_installed_profiles.h"

using namespace ResourceImplementation;

namespace ResourceImplementation {
std::mutex state_mutex;

Layout active_layout;

bool profile_loaded{};

bool owns_attribute_model{};

std::string profile_status{"not-initialized"};

json profile_selection = json::object();

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
    profile_selection = {{"kfcFormat", KfcRuntimeCompatibility::EnshroudedClient::kfc_format}};
    Layout selected{};
    json selected_profile;
    try {
        wchar_t process_path[32768]{};
        if (!GetModuleFileNameW(nullptr, process_path, 32768))
            throw std::runtime_error("process-image-path-unavailable");
        const auto installed = KfcRuntimeCompatibility::ReadInstalledProfiles(process_path, image_target,
            {{"sha256", image_sha256}, {"timestamp", image_timestamp}, {"size", image_size},
             {"kfcFormat", KfcRuntimeCompatibility::EnshroudedClient::kfc_format}},
            "resource-profile-selection");
        const json image{{"target", image_target}, {"sha256", image_sha256},
            {"timestamp", image_timestamp}, {"size", image_size}};
        auto selection = select_resource_profile(image,
            KfcRuntimeCompatibility::EnshroudedClient::kfc_format, installed.profiles);
        profile_selection = std::move(selection.diagnostics);
        const auto directory = installed.directory.u8string();
        profile_selection["directory"] = std::string(directory.begin(), directory.end());
        profile_selection["discovery"] = installed.discovery;
        if (!installed.errors.empty()) profile_selection["catalogErrors"] = installed.errors;
        if (!selection.error.empty()) {
            profile_status = selection.error;
            return false;
        }
        selected_profile = std::move(selection.profile);
        selected = parse_layout(selected_profile);
    } catch (const std::exception& error) {
        profile_status = std::string("resource-profile-error:") + error.what();
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
    profile_selection = json::object();
}

bool ProfileLoaded() {
    std::scoped_lock lock(state_mutex);
    return profile_loaded;
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
    json selection;
    const char* write_fault{};
    {
        std::scoped_lock lock(state_mutex);
        detail = profile_status;
        selection = profile_selection;
        write_fault = scalar_write_fault;
        if (!profile_loaded) return json({{"available", false}, {"profile", detail},
            {"mode", "resource-service"}, {"profileSelection", std::move(selection)},
            {"resourceWritesConfigured", false}, {"resourceWrites", false}}).dump();
        layout = active_layout;
    }
    View view{};
    std::string reason;
    const bool available = resolve_view(layout, view, &reason) && registry_is_bound(layout, view);
    if (!available && reason.empty()) reason = "item-registry-resource-handle-mismatch";
    const bool writes_configured = EngineContext::ResourceWriteConfigured();
    const bool writes_available = writes_configured && CanWrite();
    return json({{"available", available}, {"profile", detail},
        {"profileId", layout.profile_id}, {"profileSelection", std::move(selection)},
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
