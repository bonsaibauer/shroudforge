#include "player-input/world_cursor_state.h"
#include "abi/runtime_api.h"

#include <cstring>
#include <mutex>
#include <vector>

// The local client's camera for a caller that draws every frame (the game
// overlay): read from the same ClientPlayerInput sample as the local cursor.
// The mod runtime knows the reflected field offsets and gives them once.
namespace WorldRuntime::Implementation {
std::uint32_t ReadLocalCursorInput(std::size_t component_size, std::uint64_t expected_session,
                                  std::uint8_t* buffer, std::size_t capacity, std::uint64_t* sequence);
namespace {
constexpr std::size_t CameraFields = 9;  // position x, y, z; orientation x, y, z, w; fovY; aspectRatio.
struct CameraLayout {
    std::size_t component_size{};
    std::uint32_t offsets[CameraFields]{};
    bool ready{};
};
std::mutex camera_layout_mutex;
CameraLayout camera_layout;
}
}

using namespace WorldRuntime::Implementation;

extern "C" std::uint32_t __cdecl KfcRuntimeSetLocalCameraLayout(std::size_t component_size,
    const std::uint32_t* offsets, std::size_t count) {
    if (!component_size || component_size > 65536 || !offsets || count != CameraFields) return 2;
    CameraLayout layout;
    layout.component_size = component_size;
    for (std::size_t index = 0; index < CameraFields; ++index) {
        // Positions are 8-byte fixed-point integers, the others 4-byte floats.
        const std::size_t width = index < 3 ? 8 : 4;
        if (offsets[index] > component_size - width) return 2;
        layout.offsets[index] = offsets[index];
    }
    layout.ready = true;
    std::scoped_lock lock(camera_layout_mutex);
    camera_layout = layout;
    return 1;
}

extern "C" std::uint32_t __cdecl KfcRuntimeReadLocalCamera(double* output, std::size_t count) {
    if (!output || count < CameraFields) return 2;
    try {
        CameraLayout layout;
        { std::scoped_lock lock(camera_layout_mutex); layout = camera_layout; }
        if (!layout.ready) return 3;
        const auto session = KfcRuntimeWorldSessionId();
        if (!session) return 4;
        std::vector<std::uint8_t> bytes(layout.component_size);
        std::uint64_t sequence{};
        const auto result = ReadLocalCursorInput(layout.component_size, session, bytes.data(), bytes.size(), &sequence);
        if (result != 1) return result;
        for (std::size_t index = 0; index < 3; ++index) {
            std::int64_t fixed{};
            std::memcpy(&fixed, bytes.data() + layout.offsets[index], sizeof(fixed));
            output[index] = static_cast<double>(fixed) / 4294967296.0;
        }
        for (std::size_t index = 3; index < CameraFields; ++index) {
            float value{};
            std::memcpy(&value, bytes.data() + layout.offsets[index], sizeof(value));
            output[index] = value;
        }
        return 1;
    } catch (...) {
        return 4;
    }
}
