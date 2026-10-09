#include "runtime.h"
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <iterator>
#include <string>
#include <windows.h>

namespace {
constexpr int32_t k_ready = 1;
constexpr int32_t k_steam_identity_type = 16;
constexpr int32_t k_steam_identity_size = 8;
constexpr int32_t k_steam_send_reliable = 8;
constexpr size_t k_max_message_bytes = 512u * 1024u;

struct SteamNetworkingIdentity {
    int32_t type;
    int32_t size;
    union {
        uint64_t steam_id;
        uint32_t reserved[32];
    } data;
};
static_assert(sizeof(SteamNetworkingIdentity) == 136);
static_assert(offsetof(SteamNetworkingIdentity, data) == 8);

struct SteamNetworkingMessagePrefix {
    void* data;
    int32_t size;
    uint32_t connection;
    SteamNetworkingIdentity peer;
    int64_t connection_user_data;
    int64_t received_at;
    int64_t message_number;
    void* free_data;
    void* release;
    int32_t channel;
    int32_t flags;
};
static_assert(offsetof(SteamNetworkingMessagePrefix, peer) == 16);
static_assert(offsetof(SteamNetworkingMessagePrefix, flags) == 196);

using InterfaceAccessor = void* (__cdecl*)();
using SendMessage = int32_t (__cdecl*)(void*, const SteamNetworkingIdentity&, const void*, uint32_t, int32_t, int32_t);
using ReceiveMessages = int32_t (__cdecl*)(void*, int32_t, SteamNetworkingMessagePrefix**, int32_t);
using AcceptSession = bool (__cdecl*)(void*, const SteamNetworkingIdentity&);
using ReleaseMessage = void (__cdecl*)(SteamNetworkingMessagePrefix*);
using GetIdentity = bool (__cdecl*)(void*, SteamNetworkingIdentity*);

HMODULE steam_api_module() {
    return GetModuleHandleW(L"steam_api64.dll");
}

bool is_dedicated_server() {
    wchar_t path[32768]{};
    const auto length = GetModuleFileNameW(nullptr, path, static_cast<DWORD>(std::size(path)));
    if (!length || length >= std::size(path)) return false;
    const wchar_t* filename = path + length;
    while (filename > path && filename[-1] != L'\\' && filename[-1] != L'/') --filename;
    return _wcsicmp(filename, L"enshrouded_server.exe") == 0;
}

template <typename T>
T steam_export(const char* name) {
    const auto module = steam_api_module();
    return module ? reinterpret_cast<T>(GetProcAddress(module, name)) : nullptr;
}

struct Interfaces {
    void* messages{};
    SendMessage send{};
    ReceiveMessages receive{};
    AcceptSession accept{};
    ReleaseMessage release{};
    InterfaceAccessor sockets_accessor{};
    GetIdentity get_identity{};
};

Interfaces resolve() {
    const bool server = is_dedicated_server();
    const auto messages_accessor = steam_export<InterfaceAccessor>(server
        ? "SteamAPI_SteamGameServerNetworkingMessages_SteamAPI_v002"
        : "SteamAPI_SteamNetworkingMessages_SteamAPI_v002");
    const auto sockets_accessor = steam_export<InterfaceAccessor>(server
        ? "SteamAPI_SteamGameServerNetworkingSockets_SteamAPI_v012"
        : "SteamAPI_SteamNetworkingSockets_SteamAPI_v012");
    Interfaces result{};
    if (messages_accessor) result.messages = messages_accessor();
    result.send = steam_export<SendMessage>("SteamAPI_ISteamNetworkingMessages_SendMessageToUser");
    result.receive = steam_export<ReceiveMessages>("SteamAPI_ISteamNetworkingMessages_ReceiveMessagesOnChannel");
    result.accept = steam_export<AcceptSession>("SteamAPI_ISteamNetworkingMessages_AcceptSessionWithUser");
    result.release = steam_export<ReleaseMessage>("SteamAPI_SteamNetworkingMessage_t_Release");
    result.sockets_accessor = sockets_accessor;
    result.get_identity = steam_export<GetIdentity>("SteamAPI_ISteamNetworkingSockets_GetIdentity");
    return result;
}

bool ready(const Interfaces& api) {
    return api.messages && api.send && api.receive && api.accept && api.release;
}

SteamNetworkingIdentity identity(uint64_t steam_id) {
    SteamNetworkingIdentity result{};
    result.type = k_steam_identity_type;
    result.size = k_steam_identity_size;
    result.data.steam_id = steam_id;
    return result;
}
}

extern "C" {
KFC_RUNTIME_API int32_t __cdecl KfcRuntimeNetworkStatus(uint64_t* local_steam_id) {
    if (local_steam_id) *local_steam_id = 0;
    try {
        const auto api = resolve();
        if (!ready(api)) return 0;
        if (local_steam_id && api.sockets_accessor && api.get_identity) {
            if (auto* sockets = api.sockets_accessor()) {
                SteamNetworkingIdentity local{};
                if (api.get_identity(sockets, &local) && local.type == k_steam_identity_type &&
                    local.size == k_steam_identity_size) {
                    *local_steam_id = local.data.steam_id;
                }
            }
        }
        return k_ready;
    } catch (...) { return 0; }
}

KFC_RUNTIME_API int32_t __cdecl KfcRuntimeNetworkSend(
    uint64_t peer_steam_id, const unsigned char* payload, size_t size,
    int32_t channel, int32_t reliable) {
    if (!peer_steam_id || size > k_max_message_bytes || (size && !payload) ||
        channel < 0 || channel > 65535) return -2;
    try {
        const auto api = resolve();
        if (!ready(api)) return -1;
        const auto peer = identity(peer_steam_id);
        return api.send(api.messages, peer, payload, static_cast<uint32_t>(size),
            reliable ? k_steam_send_reliable : 0, channel);
    } catch (...) { return -1; }
}

KFC_RUNTIME_API int32_t __cdecl KfcRuntimeNetworkAccept(uint64_t peer_steam_id) {
    if (!peer_steam_id) return -2;
    try {
        const auto api = resolve();
        if (!ready(api)) return -1;
        const auto peer = identity(peer_steam_id);
        return api.accept(api.messages, peer) ? 1 : 0;
    } catch (...) { return -1; }
}

KFC_RUNTIME_API int32_t __cdecl KfcRuntimeNetworkReceive(
    int32_t channel, unsigned char* payload, size_t capacity,
    uint64_t* peer_steam_id, size_t* actual, uint32_t* reliable) {
    if (channel < 0 || channel > 65535 || !payload || !peer_steam_id || !actual ||
        !reliable || capacity > k_max_message_bytes) return -2;
    *peer_steam_id = 0;
    *actual = 0;
    *reliable = 0;
    try {
        const auto api = resolve();
        if (!ready(api)) return -1;
        SteamNetworkingMessagePrefix* message{};
        const auto count = api.receive(api.messages, channel, &message, 1);
        if (count == 0) return 0;
        if (count != 1 || !message) {
            if (message) api.release(message);
            return -3;
        }
        const auto release = [&] { api.release(message); };
        if (message->size < 0 || static_cast<size_t>(message->size) > capacity ||
            (message->size && !message->data) || message->peer.type != k_steam_identity_type ||
            message->peer.size != k_steam_identity_size || !message->peer.data.steam_id) {
            release();
            return -2;
        }
        if (message->size) std::memcpy(payload, message->data, static_cast<size_t>(message->size));
        *peer_steam_id = message->peer.data.steam_id;
        *actual = static_cast<size_t>(message->size);
        *reliable = (message->flags & k_steam_send_reliable) ? 1u : 0u;
        release();
        return 1;
    } catch (...) { return -3; }
}
}
