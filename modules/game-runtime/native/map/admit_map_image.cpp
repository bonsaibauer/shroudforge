#include "map/capture_game_map.h"
#include "engine-context/engine_owners.h"
#include "host/executable_span_validation.h"
#include <windows.h>
#include <algorithm>
#include <cstring>

namespace MapRuntime {
namespace {
using Profile = KfcRuntimeCompatibility::MapCaptureProfile;
struct ImageReader {
    std::span<const std::uint8_t> bytes;
    std::uintptr_t Base() const { return reinterpret_cast<std::uintptr_t>(bytes.data()); }
    bool Read(std::size_t rva, void* output, std::size_t size) const {
        return rva <= bytes.size() && size <= bytes.size() - rva && ReadMemory(Base() + rva, output, size);
    }
    bool Match(std::uint32_t rva, const std::uint8_t* expected, std::size_t size, MapImageAdmission& result) const {
        std::array<std::uint8_t, 1024> actual{};
        if (size > actual.size() || !Read(rva, actual.data(), size)) {
            if (!result.failure_rva) { result.failure_reason = "guard-read"; result.failure_rva = rva; }
            return false;
        }
        if (!std::memcmp(actual.data(), expected, size)) return true;
        if (size <= 512 && ExecutableSpanValidation::Services().ValidateGuard(rva, expected, actual.data(), size,
                SF_EXECUTABLE_SPAN_PHASE_NORMALIZE)) return true;
        if (!result.failure_rva) {
            result.failure_reason = "guard-bytes";
            for (std::size_t i = 0; i < size; ++i) if (actual[i] != expected[i]) {
                result.failure_rva = rva + i; result.expected_byte = expected[i]; result.observed_byte = actual[i]; break;
            }
        }
        return false;
    }
    bool Named(const Profile::System& system) const {
        std::uintptr_t descriptor[3]{};
        std::array<char, 96> name{};
        const auto length = system.name.size() + 1;
        return Read(system.descriptor, descriptor, sizeof(descriptor)) && descriptor[0] >= Base() &&
            descriptor[1] == length - 1 && descriptor[2] == Base() + system.callback &&
            Read(descriptor[0] - Base(), name.data(), length) && !std::memcmp(name.data(), system.name.c_str(), length);
    }
};

bool Identity(const ImageReader& image, const Profile& profile, std::string_view hash) {
    if (!image.Base() || image.Base() > UINTPTR_MAX - image.bytes.size() ||
        image.bytes.size() != profile.image_size || hash != profile.image_sha256) return false;
    IMAGE_DOS_HEADER dos{};
    IMAGE_NT_HEADERS64 nt{};
    return image.Read(0, &dos, sizeof(dos)) && dos.e_magic == IMAGE_DOS_SIGNATURE && dos.e_lfanew >= 0 &&
        image.Read(static_cast<std::size_t>(dos.e_lfanew), &nt, sizeof(nt)) && nt.Signature == IMAGE_NT_SIGNATURE &&
        nt.FileHeader.Machine == IMAGE_FILE_MACHINE_AMD64 && nt.OptionalHeader.Magic == IMAGE_NT_OPTIONAL_HDR64_MAGIC &&
        nt.FileHeader.TimeDateStamp == profile.timestamp && nt.OptionalHeader.SizeOfImage == profile.image_size;
}
bool View(const ImageReader& image, const Profile& profile, const Profile::View& view, MapImageAdmission& result) {
    for (const auto& system : view.systems) if (!image.Named(system)) {
        if (!result.failure_rva) { result.failure_reason = "named-system"; result.failure_rva = system.descriptor; }
        return false;
    }
    for (const auto& guard : view.guards) {
        for (std::size_t offset = 0; offset < guard.bytes.size();) {
            const auto size = (std::min)(std::size_t(profile.guard_chunk_bytes), guard.bytes.size() - offset);
            if (!image.Match(guard.rva + static_cast<std::uint32_t>(offset), guard.bytes.data() + offset, size, result)) return false;
            offset += size;
        }
    }
    for (const auto& pointer : view.pointers) {
        const auto expected = image.Base() + pointer.target;
        if (!image.Match(pointer.address, reinterpret_cast<const std::uint8_t*>(&expected), sizeof(expected), result)) return false;
    }
    if (view.enumeration) {
        const auto& table = *view.enumeration;
        for (std::size_t i = 0; i < table.names.size(); ++i) {
            const auto entry = table.rva + i * table.stride;
            std::uintptr_t pointer{};
            std::uint64_t value{};
            std::array<char, 96> name{};
            const auto length = table.names[i].size() + 1;
            if (!image.Read(entry + table.name_offset, &pointer, sizeof(pointer)) || pointer < image.Base() ||
                !image.Read(entry + table.value_offset, &value, sizeof(value)) || value != i ||
                !image.Read(pointer - image.Base(), name.data(), length) ||
                std::memcmp(name.data(), table.names[i].c_str(), length)) return false;
        }
    }
    return true;
}
MapImageAdmission Inspect(std::span<const std::uint8_t> bytes, std::string_view hash, bool permitted, bool renderer_only) {
    MapImageAdmission result;
    if (!permitted) { result.failure_reason = "capability-denied"; result.status = SF_MAP_CAPABILITY_DENIED; return result; }
    const auto selected = EngineContext::ConfiguredMapCaptureProfile();
    if (!selected.profile) { result.failure_reason = "map-capture-profile-unavailable"; return result; }
    const auto& profile = *selected.profile;
    const ImageReader image{bytes};
    result.failure_reason = "image-header";
    if (!Identity(image, profile, hash)) return result;
    result.failure_reason = "none";
    if (renderer_only) {
        result.renderer = View(image, profile, profile.views[1], result);
        if (result.renderer) result.status = SF_MAP_READY;
        return result;
    }
    for (const auto& system : profile.required_systems) if (!image.Named(system)) {
        result.failure_reason = "named-system"; result.failure_rva = system.descriptor; return result;
    }
    bool* outputs[]{&result.player, &result.renderer, &result.markers, &result.clock, &result.weather,
                    &result.fog, &result.world, &result.deaths, &result.discovery};
    bool complete = true;
    for (std::size_t i = 0; i < profile.views.size(); ++i) {
        *outputs[i] = (i != 7 || !profile.deaths_require_world || result.world) && View(image, profile, profile.views[i], result);
        complete &= *outputs[i];
    }
    if (profile.all_views ? complete : result.player) result.status = SF_MAP_READY;
    return result;
}
}
MapImageAdmission InspectMapImage(std::span<const std::uint8_t> bytes, std::string_view hash, bool permitted) {
    return Inspect(bytes, hash, permitted, false);
}
MapImageAdmission InspectRendererImage(std::span<const std::uint8_t> bytes, std::string_view hash) {
    return Inspect(bytes, hash, true, true);
}
}
