#include "player-input/world_cursor_state.h"
#include "abi/runtime_api.h"

#include <algorithm>
#include <cstring>
#include <mutex>
#include <vector>

// The local client's camera, and its building cursor, for a caller that
// draws every frame (the game overlay): read from the same ClientPlayerInput
// sample as the local cursor. The mod runtime knows the reflected field
// offsets and gives them once.
namespace WorldRuntime::Implementation {
std::uint32_t ReadLocalCursorInput(std::size_t component_size, std::uint64_t expected_session,
                                  std::uint8_t* buffer, std::size_t capacity, std::uint64_t* sequence);
namespace {
// Camera: position x, y, z (Q32.32 int64); orientation x, y, z, w; fovY;
// aspectRatio (float). Cursor: position x, y, z (Q32.32 int64); orientation
// x, y, z, w (float); flags (unsigned, `flags_width` bytes).
constexpr std::size_t CameraFields = 9;
constexpr std::size_t ViewFields = 17;
struct ViewLayout {
    std::size_t component_size{};
    std::size_t fields{};  // CameraFields, or ViewFields with the cursor.
    std::uint32_t offsets[ViewFields]{};
    std::uint32_t flags_width{};
    bool ready{};
};
std::mutex view_layout_mutex;
ViewLayout view_layout;

bool fixed_point(std::size_t index) { return index < 3 || (index >= 9 && index < 12); }
}
}

using namespace WorldRuntime::Implementation;

extern "C" std::uint32_t __cdecl KfcRuntimeSetLocalCameraLayout(std::size_t component_size,
    const std::uint32_t* offsets, std::size_t count) {
    // 9 offsets (camera), or 17 offsets and the cursor flags' width (18).
    if (!component_size || component_size > 65536 || !offsets || (count != CameraFields && count != ViewFields + 1))
        return 2;
    ViewLayout layout;
    layout.component_size = component_size;
    layout.fields = count == CameraFields ? CameraFields : ViewFields;
    if (layout.fields == ViewFields) {
        layout.flags_width = offsets[ViewFields];
        if (layout.flags_width != 1 && layout.flags_width != 2 && layout.flags_width != 4) return 2;
    }
    for (std::size_t index = 0; index < layout.fields; ++index) {
        const std::size_t width = fixed_point(index) ? 8 : (index == ViewFields - 1 ? layout.flags_width : 4);
        if (offsets[index] > component_size - width) return 2;
        layout.offsets[index] = offsets[index];
    }
    layout.ready = true;
    std::scoped_lock lock(view_layout_mutex);
    view_layout = layout;
    return 1;
}

// Writes the camera (9 doubles: position in meters, orientation, fovY,
// aspect) and, when count allows and the layout has it, the cursor (8 more:
// position in meters, orientation, flags; flags -1 without the cursor).
extern "C" std::uint32_t __cdecl KfcRuntimeReadLocalCamera(double* output, std::size_t count) {
    if (!output || count < CameraFields) return 2;
    try {
        ViewLayout layout;
        { std::scoped_lock lock(view_layout_mutex); layout = view_layout; }
        if (!layout.ready) return 3;
        const auto session = KfcRuntimeWorldSessionId();
        if (!session) return 4;
        std::vector<std::uint8_t> bytes(layout.component_size);
        std::uint64_t sequence{};
        const auto result = ReadLocalCursorInput(layout.component_size, session, bytes.data(), bytes.size(), &sequence);
        if (result != 1) return result;
        const std::size_t fields = (std::min)(layout.fields, count);
        for (std::size_t index = 0; index < fields; ++index) {
            const auto* source = bytes.data() + layout.offsets[index];
            if (fixed_point(index)) {
                std::int64_t fixed{};
                std::memcpy(&fixed, source, sizeof(fixed));
                output[index] = static_cast<double>(fixed) / 4294967296.0;
            } else if (index == ViewFields - 1) {
                std::uint32_t flags{};
                std::memcpy(&flags, source, layout.flags_width);
                output[index] = flags;
            } else {
                float value{};
                std::memcpy(&value, source, sizeof(value));
                output[index] = value;
            }
        }
        if (count >= ViewFields && layout.fields < ViewFields) output[ViewFields - 1] = -1.0;
        return 1;
    } catch (...) {
        return 4;
    }
}
