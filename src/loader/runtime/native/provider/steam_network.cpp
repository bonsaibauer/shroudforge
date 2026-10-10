#include "runtime.h"
#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <atomic>
#include <charconv>
#include <fstream>
#include <iterator>
#include <map>
#include <mutex>
#include <set>
#include <string>
#include <string_view>
#include <vector>
#include <windows.h>

namespace {
constexpr int32_t k_ready = 1;
constexpr int32_t k_steam_identity_type = 16;
constexpr int32_t k_steam_identity_size = 8;
constexpr int32_t k_steam_send_reliable = 8;
constexpr int32_t k_steam_messages_session_request_callback = 1251;
constexpr int32_t k_network_service_channel = 65535;
constexpr size_t k_max_message_bytes = 512u * 1024u;
constexpr ULONGLONG k_network_probe_interval_ms = 5000;
constexpr ULONGLONG k_remote_server_probe_max_age_ms = 15000;
constexpr wchar_t k_server_identity_mapping[] = L"Local\\ShroudForge.Network.ServerIdentity.v1";
constexpr uint64_t k_server_identity_magic = 0x5346574544494431ull;
constexpr ULONGLONG k_server_identity_max_age_ms = 10000;

struct LocalServerIdentity {
    uint64_t magic{};
    uint64_t steam_id{};
    ULONGLONG updated_at_ms{};
    DWORD process_id{};
    DWORD size{};
};
static_assert(sizeof(LocalServerIdentity) == 32);

HANDLE g_server_identity_mapping{};

void publish_local_server_identity(uint64_t steam_id) {
    if (!steam_id) return;
    if (!g_server_identity_mapping) {
        g_server_identity_mapping = CreateFileMappingW(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE,
            0, sizeof(LocalServerIdentity), k_server_identity_mapping);
    }
    if (!g_server_identity_mapping) return;
    auto* identity = static_cast<LocalServerIdentity*>(MapViewOfFile(g_server_identity_mapping,
        FILE_MAP_WRITE, 0, 0, sizeof(LocalServerIdentity)));
    if (!identity) return;
    InterlockedExchange64(reinterpret_cast<volatile LONG64*>(&identity->magic), 0);
    InterlockedExchange64(reinterpret_cast<volatile LONG64*>(&identity->steam_id), static_cast<LONG64>(steam_id));
    InterlockedExchange64(reinterpret_cast<volatile LONG64*>(&identity->updated_at_ms),
        static_cast<LONG64>(GetTickCount64()));
    InterlockedExchange(reinterpret_cast<volatile LONG*>(&identity->process_id),
        static_cast<LONG>(GetCurrentProcessId()));
    InterlockedExchange(reinterpret_cast<volatile LONG*>(&identity->size), static_cast<LONG>(sizeof(LocalServerIdentity)));
    InterlockedExchange64(reinterpret_cast<volatile LONG64*>(&identity->magic), static_cast<LONG64>(k_server_identity_magic));
    FlushViewOfFile(identity, sizeof(LocalServerIdentity));
    UnmapViewOfFile(identity);
}

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
using RegisterCallback = void (__cdecl*)(void*, int32_t);
using UnregisterCallback = void (__cdecl*)(void*);

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

bool server_log_path(std::wstring& path) {
    if (!is_dedicated_server()) return false;
    wchar_t executable[32768]{};
    const auto length = GetModuleFileNameW(nullptr, executable, static_cast<DWORD>(std::size(executable)));
    if (!length || length >= std::size(executable)) return false;
    path.assign(executable, length);
    const auto separator = path.find_last_of(L"\\/");
    if (separator == std::wstring::npos) return false;
    path.resize(separator + 1);
    path += L"logs\\enshrouded_server.log";
    return true;
}

bool parse_peer_key(const std::string& line, const char* marker, std::string& key) {
    const auto start = line.find(marker);
    if (start == std::string::npos) return false;
    const auto value = start + std::strlen(marker);
    const auto end = line.find(' ', value);
    if (end == std::string::npos || end == value) return false;
    key.assign(line, value, end - value);
    return true;
}

bool parse_steam_id(const std::string& value, size_t start, uint64_t& id) {
    while (start < value.size() && (value[start] < '0' || value[start] > '9')) ++start;
    const auto end = start;
    while (start < value.size() && value[start] >= '0' && value[start] <= '9') ++start;
    if (end == start) return false;
    try { id = std::stoull(value.substr(end, start - end)); }
    catch (...) { return false; }
    return id != 0;
}

bool read_authenticated_server_peers(std::vector<uint64_t>& peers) {
    std::wstring path;
    if (!server_log_path(path)) return false;
    std::ifstream log(path);
    if (!log) return false;
    std::map<std::string, uint64_t> live;
    std::set<uint64_t> authenticated;
    std::string line;
    while (std::getline(log, line)) {
        std::string key;
        if (parse_peer_key(line, "[online] Added peer ", key)) {
            const auto steam = line.find("(steamid:");
            uint64_t id{};
            if (steam != std::string::npos && parse_steam_id(line, steam + 9, id)) live[key] = id;
        } else if (line.find("authenticated by steam") != std::string::npos &&
                   line.find("not authenticated by steam") == std::string::npos) {
            const auto client = line.find("Client '");
            uint64_t id{};
            if (client != std::string::npos && parse_steam_id(line, client + 8, id)) authenticated.insert(id);
        } else if (line.find("not authenticated by steam") != std::string::npos) {
            const auto client = line.find("Client '");
            uint64_t id{};
            if (client != std::string::npos && parse_steam_id(line, client + 8, id)) authenticated.erase(id);
        } else if (parse_peer_key(line, "[online] Removed peer ", key)) {
            live.erase(key);
        }
    }
    std::set<uint64_t> active;
    for (const auto& [key, id] : live) {
        (void)key;
        if (authenticated.contains(id)) active.insert(id);
    }
    peers.assign(active.begin(), active.end());
    return true;
}

SteamNetworkingIdentity identity(uint64_t steam_id) {
    SteamNetworkingIdentity result{};
    result.type = k_steam_identity_type;
    result.size = k_steam_identity_size;
    result.data.steam_id = steam_id;
    return result;
}

std::set<uint64_t> g_authorized_server_peers;
std::mutex g_authorized_server_peers_mutex;

// This is the stable CCallbackBase ABI used by SteamAPI_RegisterCallback.
// Steam dispatches it from the game's own SteamAPI_RunCallbacks call; this
// module never pumps Steam callbacks itself.
class SessionRequestCallback {
public:
    virtual void Run(void* parameter) {
        if (!parameter) return;
        const auto* request = static_cast<const SteamNetworkingIdentity*>(parameter);
        if (request->type != k_steam_identity_type || request->size != k_steam_identity_size ||
            !request->data.steam_id) return;
        if (is_dedicated_server()) {
            std::lock_guard lock(g_authorized_server_peers_mutex);
            if (!g_authorized_server_peers.contains(request->data.steam_id)) return;
        }
        const auto api = resolve();
        if (!api.messages || !api.accept) return;
        api.accept(api.messages, *request);
    }
    virtual void Run(void*, bool, uint64_t) {}
    virtual int32_t GetCallbackSizeBytes() { return sizeof(SteamNetworkingIdentity); }
    uint8_t callback_flags{};
    int32_t callback_id{k_steam_messages_session_request_callback};
};

std::atomic<bool> g_session_callback_registered{};
std::atomic<uint64_t> g_remote_server_steam_id{};
std::atomic<ULONGLONG> g_remote_server_probe_at_ms{};
std::atomic<uint64_t> g_probes_sent{};
std::atomic<uint64_t> g_probe_send_failures{};
std::atomic<uint64_t> g_probe_timeouts{};
std::atomic<uint64_t> g_probes_acknowledged{};
std::atomic<uint64_t> g_last_acknowledged_peer{};
std::atomic<uint64_t> g_last_probe_round_trip_ms{};
std::atomic<int32_t> g_last_send_result{};
std::map<uint64_t, std::pair<uint64_t, ULONGLONG>> g_pending_probes;
SessionRequestCallback g_session_request_callback;
ULONGLONG g_last_network_probe_ms{};
uint64_t g_network_probe_nonce{};

bool register_session_callback() {
    if (g_session_callback_registered.load(std::memory_order_acquire)) return true;
    const auto register_callback = steam_export<RegisterCallback>("SteamAPI_RegisterCallback");
    if (!register_callback) return false;
    bool expected = false;
    if (!g_session_callback_registered.compare_exchange_strong(expected, true,
            std::memory_order_acq_rel)) return true;
    if (is_dedicated_server()) g_session_request_callback.callback_flags |= 0x02; // k_ECallbackFlagsGameServer
    register_callback(&g_session_request_callback, k_steam_messages_session_request_callback);
    return true;
}

int32_t send_service_message(const Interfaces& api, uint64_t peer, const std::string& text) {
    if (!peer || !api.messages || !api.send) return -1;
    const auto target = identity(peer);
    const auto result = api.send(api.messages, target, text.data(), static_cast<uint32_t>(text.size()),
        k_steam_send_reliable, k_network_service_channel);
    g_last_send_result.store(result, std::memory_order_release);
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
                    if (is_dedicated_server()) publish_local_server_identity(local.data.steam_id);
                }
            }
        }
        return k_ready;
    } catch (...) { return 0; }
}

KFC_RUNTIME_API int32_t __cdecl KfcRuntimeNetworkServiceReady() {
    return g_session_callback_registered.load(std::memory_order_acquire) ? 1 : 0;
}

KFC_RUNTIME_API void __cdecl KfcRuntimeNetworkServiceTick(
    const uint64_t* authorized_clients, size_t authorized_count, int32_t restrict_clients) {
    try {
        if (authorized_count > 256 || (authorized_count && !authorized_clients)) return;
        const auto api = resolve();
        if (!ready(api)) return;
        const auto now = GetTickCount64();
        std::vector<uint64_t> peers;
        if (is_dedicated_server() && api.sockets_accessor && api.get_identity) {
            if (auto* sockets = api.sockets_accessor()) {
                SteamNetworkingIdentity local{};
                if (api.get_identity(sockets, &local) && local.type == k_steam_identity_type &&
                    local.size == k_steam_identity_size)
                    publish_local_server_identity(local.data.steam_id);
            }
        }
        if (is_dedicated_server()) {
            if (!read_authenticated_server_peers(peers)) peers.clear();
            if (restrict_clients) {
                std::set<uint64_t> allowed;
                if (authorized_count) allowed.insert(authorized_clients, authorized_clients + authorized_count);
                peers.erase(std::remove_if(peers.begin(), peers.end(), [&](uint64_t peer) {
                    return !allowed.contains(peer);
                }), peers.end());
            }
            {
                std::lock_guard lock(g_authorized_server_peers_mutex);
                g_authorized_server_peers = std::set<uint64_t>(peers.begin(), peers.end());
            }
        }
        if (!register_session_callback()) return;
        // Handle probes on both roles. The callback above accepts an inbound
        // Steam session before its messages become readable.
        for (size_t count = 0; count < 16; ++count) {
            SteamNetworkingMessagePrefix* message{};
            if (api.receive(api.messages, k_network_service_channel, &message, 1) != 1 || !message)
                break;
            if (message->size > 0 && message->size <= 128 && message->data &&
                message->peer.type == k_steam_identity_type && message->peer.size == k_steam_identity_size) {
                const std::string_view payload(static_cast<const char*>(message->data),
                    static_cast<size_t>(message->size));
                constexpr std::string_view prefix = "SFN1:PING:";
                constexpr std::string_view pong_prefix = "SFN1:PONG:";
                if (payload.starts_with(prefix)) {
                    if (!is_dedicated_server()) {
                        g_remote_server_steam_id.store(message->peer.data.steam_id, std::memory_order_release);
                        g_remote_server_probe_at_ms.store(now, std::memory_order_release);
                    }
                    const auto response = "SFN1:PONG:" + std::string(payload.substr(prefix.size()));
                    send_service_message(api, message->peer.data.steam_id, response);
                } else if (payload.starts_with(pong_prefix) && is_dedicated_server()) {
                    uint64_t nonce{};
                    const auto nonce_text = payload.substr(pong_prefix.size());
                    const auto parsed = std::from_chars(nonce_text.data(), nonce_text.data() + nonce_text.size(), nonce);
                    const auto pending = g_pending_probes.find(nonce);
                    if (parsed.ec == std::errc{} && parsed.ptr == nonce_text.data() + nonce_text.size() &&
                        pending != g_pending_probes.end() && pending->second.first == message->peer.data.steam_id) {
                        g_probes_acknowledged.fetch_add(1, std::memory_order_relaxed);
                        g_last_acknowledged_peer.store(message->peer.data.steam_id, std::memory_order_release);
                        g_last_probe_round_trip_ms.store(now - pending->second.second, std::memory_order_release);
                        g_pending_probes.erase(pending);
                    }
                }
            }
            api.release(message);
        }

        for (auto it = g_pending_probes.begin(); it != g_pending_probes.end();) {
            if (now >= it->second.second && now - it->second.second > k_remote_server_probe_max_age_ms) {
                g_probe_timeouts.fetch_add(1, std::memory_order_relaxed);
                it = g_pending_probes.erase(it);
            } else ++it;
        }
        if (!is_dedicated_server() || now - g_last_network_probe_ms < k_network_probe_interval_ms) return;
        g_last_network_probe_ms = now;
        for (const auto peer : peers) {
            const auto nonce = ++g_network_probe_nonce;
            if (send_service_message(api, peer, "SFN1:PING:" + std::to_string(nonce)) == 1) {
                g_probes_sent.fetch_add(1, std::memory_order_relaxed);
                g_pending_probes.emplace(nonce, std::make_pair(peer, now));
            } else {
                g_probe_send_failures.fetch_add(1, std::memory_order_relaxed);
            }
        }
    } catch (...) {}
}

KFC_RUNTIME_API void __cdecl KfcRuntimeNetworkServiceStats(
    uint64_t* probes_sent, uint64_t* probe_send_failures,
    uint64_t* probes_acknowledged, uint64_t* last_acknowledged_peer,
    uint64_t* last_round_trip_ms, int32_t* last_send_result, uint64_t* probe_timeouts) {
    if (probes_sent) *probes_sent = g_probes_sent.load(std::memory_order_relaxed);
    if (probe_send_failures) *probe_send_failures = g_probe_send_failures.load(std::memory_order_relaxed);
    if (probes_acknowledged) *probes_acknowledged = g_probes_acknowledged.load(std::memory_order_relaxed);
    if (last_acknowledged_peer) *last_acknowledged_peer = g_last_acknowledged_peer.load(std::memory_order_acquire);
    if (last_round_trip_ms) *last_round_trip_ms = g_last_probe_round_trip_ms.load(std::memory_order_acquire);
    if (last_send_result) *last_send_result = g_last_send_result.load(std::memory_order_acquire);
    if (probe_timeouts) *probe_timeouts = g_probe_timeouts.load(std::memory_order_relaxed);
}

KFC_RUNTIME_API void __cdecl KfcRuntimeNetworkServiceShutdown() {
    if (!g_session_callback_registered.exchange(false, std::memory_order_acq_rel)) return;
    const auto unregister_callback = steam_export<UnregisterCallback>("SteamAPI_UnregisterCallback");
    if (unregister_callback) unregister_callback(&g_session_request_callback);
}

KFC_RUNTIME_API int32_t __cdecl KfcRuntimeNetworkLocalServer(uint64_t* server_steam_id) {
    if (!server_steam_id || is_dedicated_server()) return 0;
    *server_steam_id = 0;
    HANDLE mapping = OpenFileMappingW(FILE_MAP_READ, FALSE, k_server_identity_mapping);
    if (!mapping) return 0;
    const auto* identity = static_cast<const LocalServerIdentity*>(MapViewOfFile(mapping,
        FILE_MAP_READ, 0, 0, sizeof(LocalServerIdentity)));
    if (!identity) {
        CloseHandle(mapping);
        return 0;
    }
    // This view is deliberately opened with FILE_MAP_READ only. Interlocked
    // compare/exchange is a read-modify-write operation, even when exchange is
    // zero, so using it here faults on the read-only view. On x64, aligned
    // 64-bit loads are atomic; the publisher clears/reinstates the magic around
    // its writes, and the barriers plus second magic read reject torn snapshots.
    const volatile auto* fields = reinterpret_cast<const volatile LocalServerIdentity*>(identity);
    const auto magic_before = fields->magic;
    MemoryBarrier();
    const auto steam_id = fields->steam_id;
    const auto updated_at = fields->updated_at_ms;
    const auto process_id = fields->process_id;
    const auto size = fields->size;
    MemoryBarrier();
    const auto magic_after = fields->magic;
    UnmapViewOfFile(identity);
    CloseHandle(mapping);
    const auto now = GetTickCount64();
    if (magic_before != magic_after || magic_after != k_server_identity_magic || size != sizeof(LocalServerIdentity) ||
        !steam_id || !process_id || now < updated_at || now - updated_at > k_server_identity_max_age_ms) return 0;
    *server_steam_id = steam_id;
    return 1;
}

KFC_RUNTIME_API int32_t __cdecl KfcRuntimeNetworkRemoteServer(uint64_t* server_steam_id) {
    if (!server_steam_id || is_dedicated_server()) return 0;
    *server_steam_id = g_remote_server_steam_id.load(std::memory_order_acquire);
    const auto received_at = g_remote_server_probe_at_ms.load(std::memory_order_acquire);
    const auto now = GetTickCount64();
    if (!received_at || now < received_at || now - received_at > k_remote_server_probe_max_age_ms) {
        *server_steam_id = 0;
        return 0;
    }
    return *server_steam_id ? 1 : 0;
}

KFC_RUNTIME_API int32_t __cdecl KfcRuntimeNetworkPeerAuthorized(uint64_t peer_steam_id) {
    if (!peer_steam_id) return 0;
    if (!is_dedicated_server()) return 1;
    std::lock_guard lock(g_authorized_server_peers_mutex);
    return g_authorized_server_peers.contains(peer_steam_id) ? 1 : 0;
}

KFC_RUNTIME_API int32_t __cdecl KfcRuntimeNetworkConnectedPeers(
    uint64_t* peer_steam_ids, size_t capacity, size_t* count) {
    if (!count || (capacity && !peer_steam_ids) || capacity > 256 || !is_dedicated_server()) return 0;
    *count = 0;
    try {
        std::vector<uint64_t> peers;
        if (!read_authenticated_server_peers(peers) || peers.size() > capacity) return 0;
        for (size_t index = 0; index < peers.size(); ++index) peer_steam_ids[index] = peers[index];
        *count = peers.size();
        return 1;
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
