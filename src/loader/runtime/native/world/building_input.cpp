#include "building_input.h"
#include "world_runtime.h"
#include "profile.h"
#include <windows.h>
#include <cmath>
#include <cstring>
#include <mutex>

namespace {
// client-player-input-v1: original reflected layouts, gated by the exact
// production profile and its client_cursor hook. No server-memory writes.
constexpr std::size_t CursorOffset = 624, DigitalOffset = 792;
constexpr std::size_t CreateOffset = 432, StockOffset = 164;
constexpr std::uint64_t BuildMask = (1ull << 0) | (1ull << 1) | (1ull << 2) | (1ull << 3) |
    (1ull << 4) | (1ull << 5) | (1ull << 7) | (1ull << 37);
constexpr std::uint64_t DismantleMask = (1ull << 5) | (1ull << 7); // ContextualAction + ContextualAction_Hold
constexpr std::uint64_t DismantleHoldMs = 1200;
struct Transform { std::int64_t position[3]; float rotation[4]; float scale[3]; std::uint32_t padding; };
static_assert(sizeof(Transform) == 56);
struct Command {
    std::uint32_t id{}, player{}, kind{}, item{}, material{}, slot{}, status{};
    Transform transform{};
    std::uint64_t queued_at{}, pressed_at{};
    std::uintptr_t input{};
    std::uint32_t pressed_version{}, terminal_status{};
    std::uint32_t target_entity_id{};
};
std::mutex mutex;
Command command;
std::uint32_t next_id{};

bool available() {
    using namespace KfcRuntimeCompatibility::EnshroudedClient;
    if (!exact_build_match || image_target != "enshrouded.exe") return false;
    bool input = false, version = false;
    for (const auto& op : runtime_operations) {
        if (op.name == "runtime.world.building.input") input = op.available && op.abi == "client-player-input-v1";
        if (op.name == "runtime.world.building.version") version = op.available;
    }
    return input && version && WorldRuntime::OperationAvailable("runtime.world.cursor.get");
}

bool apply(void* cursor, void* execution_view, Command& cmd) {
    auto input = static_cast<std::uint8_t*>(cursor) - CursorOffset;
    std::uintptr_t version_address{};
    for (const auto& op : KfcRuntimeCompatibility::EnshroudedClient::runtime_operations)
        if (op.name == "runtime.world.building.version" && op.available)
            version_address = reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr)) + op.function_rva;
    __try {
        if (!version_address || !execution_view) return false;
        using Version = std::uint32_t (__fastcall*)(void*);
        const auto version = reinterpret_cast<Version>(version_address)(execution_view);
        std::uint64_t bits{};
        std::memcpy(&bits, input + DigitalOffset, sizeof(bits));
        if (cmd.status == 2) {
            // Release only on the same local input object that accepted this
            // command. Never redirect an outstanding command to another player.
            if (reinterpret_cast<std::uintptr_t>(input) != cmd.input) return false;
            // A system may visit the same input more than once in a simulation
            // tick. Keep the press alive until a later input version.
            if (cmd.kind == 4 && GetTickCount64() - cmd.pressed_at < DismantleHoldMs) {
                std::memcpy(input + CursorOffset, &cmd.transform, sizeof(cmd.transform));
                std::memcpy(input + CursorOffset + 56, &cmd.transform, sizeof(cmd.transform));
                bits |= DismantleMask;
                std::memcpy(input + DigitalOffset, &bits, sizeof(bits));
                return true;
            }
            if (version == cmd.pressed_version) return false;
            if (cmd.kind != 3) {
                std::memcpy(input + CursorOffset, &cmd.transform, sizeof(cmd.transform));
                std::memcpy(input + CursorOffset + 56, &cmd.transform, sizeof(cmd.transform));
            }
            bits &= ~BuildMask;
            if (cmd.kind == 1) bits |= 1ull << 3;
            std::memcpy(input + DigitalOffset, &bits, sizeof(bits));
            cmd.status = cmd.terminal_status ? cmd.terminal_status : 3; // NOT a world-effect acknowledgement
            return true;
        }
        if (bits & BuildMask) return false; // let the user's physical action finish
        if (cmd.kind == 0) {
            // The game's own version producer is called by the original cursor
            // system with this execution view. It is separately profile guarded.
            input[CreateOffset + 4] = static_cast<std::uint8_t>(cmd.slot);
            std::memcpy(input + CreateOffset + 8, &cmd.item, 4);
            std::memcpy(input + CreateOffset, &version, 4);
            if (cmd.material) {
                // Select the normal default/terrain building material. Roof and
                // overgrowth selections remain controlled by the game.
                std::memcpy(input + StockOffset + 4, &cmd.material, 4);
                std::memcpy(input + StockOffset + 8, &cmd.material, 4);
                std::memcpy(input + StockOffset, &version, 4);
            }
            cmd.status = 3;
        } else {
            if (cmd.kind != 3) {
                std::memcpy(input + CursorOffset, &cmd.transform, sizeof(cmd.transform));
                std::memcpy(input + CursorOffset + 56, &cmd.transform, sizeof(cmd.transform));
                // Preserve client validity flags. The server still performs its
                // normal build permissions, range, recipe and placement checks.
            }
            if (cmd.kind == 4 && !cmd.target_entity_id) return false;
            bits |= cmd.kind == 1 ? 3ull : cmd.kind == 2 ? (1ull << 4) :
                cmd.kind == 3 ? (1ull << 37) : DismantleMask;
            std::memcpy(input + DigitalOffset, &bits, sizeof(bits));
            cmd.status = 2;
            cmd.pressed_version = version;
            cmd.pressed_at = GetTickCount64();
        }
        cmd.input = reinterpret_cast<std::uintptr_t>(input);
        return true;
    } __except(EXCEPTION_EXECUTE_HANDLER) { cmd.status = 5; return false; }
}
}

namespace BuildingInput {
bool Available() { return available(); }
void Observe(void* cursor, void* execution_view) {
    if (!cursor || !available()) return;
    std::lock_guard lock(mutex);
    if (command.status != 1 && command.status != 2) return;
    if (GetTickCount64() - command.queued_at > 5000) {
        if (command.status == 1) { command.status = 4; return; }
        command.terminal_status = 4; // still release an already pressed input
    }
    if (!EcsRuntime::MatchesComponentAddress(command.player, "keen::ecs::ClientPlayerInput",
            static_cast<std::uint8_t*>(cursor) - CursorOffset)) return;
    apply(cursor, execution_view, command);
}
void Reset() { std::lock_guard lock(mutex); if (command.status == 1 || command.status == 2) command.status = 5; }
}

// kind: 0 select, 1 primary, 2 secondary, 3 engine undo, 4 exact prop dismantle.
// Status: 1 queued, 2 pressed, 3 dispatched, 4 timeout, 5 cancelled/error.
extern "C" __declspec(dllexport) std::uint32_t __cdecl KfcRuntimeWorldBuildingInput(
    std::uint32_t player, std::uint32_t kind, std::uint32_t item, std::uint32_t material, std::uint32_t slot,
    std::uint32_t target_entity_id, const double* position, const double* rotation, const double* scale) {
    if (!player || !available() || kind > 4 || slot > 255 || (kind == 0 && !item) ||
        (kind == 4) != (target_entity_id != 0)) return 0;
    Command next{};
    next.player = player; next.kind = kind; next.item = item; next.material = material; next.slot = slot;
    next.target_entity_id = target_entity_id;
    if (kind == 1 || kind == 2 || kind == 4) {
        if (!position || !rotation || !scale) return 0;
        long double norm{};
        for (unsigned i = 0; i < 4; ++i) {
            if (!std::isfinite(rotation[i])) return 0;
            norm += static_cast<long double>(rotation[i]) * rotation[i];
        }
        if (!std::isfinite(norm) || norm < 1e-12L) return 0;
        for (unsigned i = 0; i < 4; ++i) next.transform.rotation[i] = static_cast<float>(rotation[i] / std::sqrt(norm));
        for (unsigned i = 0; i < 3; ++i) {
            const auto fixed = std::ldexp(static_cast<long double>(position[i]), 32);
            if (!std::isfinite(fixed) || fixed <= INT64_MIN || fixed >= INT64_MAX ||
                !std::isfinite(scale[i]) || scale[i] <= 0 || scale[i] > 1024) return 0;
            next.transform.position[i] = static_cast<std::int64_t>(std::llround(fixed));
            next.transform.scale[i] = static_cast<float>(scale[i]);
        }
    }
    std::lock_guard lock(mutex);
    if (command.status == 1 || command.status == 2) return 0;
    next.id = ++next_id; if (!next.id) next.id = ++next_id;
    next.status = 1; next.queued_at = GetTickCount64(); command = next;
    return next.id;
}
extern "C" __declspec(dllexport) std::uint32_t __cdecl KfcRuntimeWorldBuildingInputStatus(std::uint32_t id) {
    std::lock_guard lock(mutex);
    if (!id || id != command.id) return 0;
    if (GetTickCount64() - command.queued_at > 5000) {
        if (command.status == 1) command.status = 4;
        else if (command.status == 2) command.terminal_status = 4;
    }
    return command.terminal_status ? command.terminal_status : command.status;
}
extern "C" __declspec(dllexport) bool __cdecl KfcRuntimeWorldBuildingInputCancel(std::uint32_t id) {
    std::lock_guard lock(mutex);
    if (!id || id != command.id) return false;
    if (command.status == 1) command.status = 5;
    else if (command.status == 2) command.terminal_status = 5;
    return true;
}
