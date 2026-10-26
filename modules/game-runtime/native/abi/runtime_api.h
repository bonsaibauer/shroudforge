#pragma once

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

#if defined(__cplusplus)
extern "C" {
#endif

#if defined(_WIN32)
#define KFC_RUNTIME_CALL __cdecl
#if defined(KFC_RUNTIME_BUILD)
#define KFC_RUNTIME_API __declspec(dllexport)
#else
#define KFC_RUNTIME_API __declspec(dllimport)
#endif
#else
#define KFC_RUNTIME_CALL
#define KFC_RUNTIME_API
#endif

/* Stable C ABI for native modloader hosts. Query KfcRuntimeAbi before use. */
#define KFC_RUNTIME_ABI_VERSION 12u
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeAbi(void);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeInitialize(void);
KFC_RUNTIME_API void KFC_RUNTIME_CALL KfcRuntimeTick(void);
KFC_RUNTIME_API void KFC_RUNTIME_CALL KfcRuntimeShutdown(void);
KFC_RUNTIME_API void KFC_RUNTIME_CALL KfcRuntimeStatus(char* buffer, size_t capacity);
KFC_RUNTIME_API void KFC_RUNTIME_CALL KfcRuntimeDiagnostics(char* buffer, size_t capacity);
/* Optional ABI-12 selected-character cache. Returns required UTF-8 bytes
 * including NUL, or zero when unavailable. Copies only a complete name.
 * This getter never reads game memory or refreshes the cached selection. */
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeSelectedCharacterName(char* buffer, size_t capacity);
/* Optional ABI-12 original GameFlow snapshot, refreshed by the existing native
 * lifecycle tick. 0 unknown/transition, 1 PreGame/menu, 2 validated other state.
 * This getter only copies the cache and never reads game memory. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeMainMenuState(void);
/* Optional Steam Networking Messages transport used by runtime.network. */
KFC_RUNTIME_API int32_t KFC_RUNTIME_CALL KfcRuntimeNetworkStatus(uint64_t* local_steam_id);
/* Registers an ISteamNetworkingMessages session-request callback without
 * pumping Steam callbacks. The game continues to own SteamAPI_RunCallbacks. */
KFC_RUNTIME_API int32_t KFC_RUNTIME_CALL KfcRuntimeNetworkServiceReady(void);
KFC_RUNTIME_API void KFC_RUNTIME_CALL KfcRuntimeNetworkServiceTick(
    const uint64_t* authorized_clients, size_t count, int32_t restrict_clients);
KFC_RUNTIME_API void KFC_RUNTIME_CALL KfcRuntimeNetworkServiceStats(
    uint64_t* probes_sent, uint64_t* probe_send_failures,
    uint64_t* probes_acknowledged, uint64_t* last_acknowledged_peer,
    uint64_t* last_round_trip_ms, int32_t* last_send_result,
    uint64_t* probe_timeouts);
KFC_RUNTIME_API void KFC_RUNTIME_CALL KfcRuntimeNetworkServiceShutdown(void);
/* Optional local-host discovery. A dedicated server publishes its current
 * Steam identity through a session-scoped mapping; clients read it here. */
KFC_RUNTIME_API int32_t KFC_RUNTIME_CALL KfcRuntimeNetworkLocalServer(uint64_t* server_steam_id);
/* SteamID64 learned from the Dedicated Server's automatic Network probe. */
KFC_RUNTIME_API int32_t KFC_RUNTIME_CALL KfcRuntimeNetworkRemoteServer(uint64_t* server_steam_id);
/* True only for a currently authenticated server peer inside the optional Network allowlist. */
KFC_RUNTIME_API int32_t KFC_RUNTIME_CALL KfcRuntimeNetworkPeerAuthorized(uint64_t peer_steam_id);
/* Authenticated, currently connected Dedicated Server clients from the server's
 * active Enshrouded session log. Returns 1 when the list was read. */
KFC_RUNTIME_API int32_t KFC_RUNTIME_CALL KfcRuntimeNetworkConnectedPeers(
    uint64_t* peer_steam_ids, size_t capacity, size_t* count);
KFC_RUNTIME_API int32_t KFC_RUNTIME_CALL KfcRuntimeNetworkSend(
    uint64_t peer_steam_id, const unsigned char* payload, size_t size,
    int32_t channel, int32_t reliable);
KFC_RUNTIME_API int32_t KFC_RUNTIME_CALL KfcRuntimeNetworkAccept(uint64_t peer_steam_id);
KFC_RUNTIME_API int32_t KFC_RUNTIME_CALL KfcRuntimeNetworkReceive(
    int32_t channel, unsigned char* payload, size_t capacity,
    uint64_t* peer_steam_id, size_t* actual, uint32_t* reliable);
/* Optional ABI-12 code inventory extension. Same complete-buffer convention as
 * EcsRegistry. Code entries explicitly have no inferred calling ABI. */
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeFunctions(char* buffer, size_t capacity);
/* Optional ABI-12 bounded, read-only code evidence. Never calls the address.
 * At most 512 bytes, only from executable sections; zero means unavailable. */
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeFunctionCode(uint32_t rva, unsigned char* buffer, size_t capacity);

/* Read-only access to ResourceService data. Resource IDs use the parser's
   canonical GUID/type/part fields and static_hash value. The resource backend
   is enabled only by an exact-image, read-validated profile. */
typedef struct KfcRuntimeResourceId {
    uint8_t guid[16];
    uint32_t type_hash;
    uint32_t part_index;
} KfcRuntimeResourceId;
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeResourceReady(void);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeResourcesByType(
    uint32_t type_hash, KfcRuntimeResourceId* resources, size_t capacity, size_t* count);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeResourceRead(
    const KfcRuntimeResourceId* resource, uint32_t static_hash,
    unsigned char* buffer, size_t capacity, size_t* actual);
/* Optional ABI-12 bounded projection of one fixed inline 32-bit field. The
   caller resolves root_size/offset through the active reflection. Requests use
   the parser's canonical static_hash and all must have the selected type_hash.
   Count is 1..64, capacity >= count, offset is 4-byte aligned and inside root.
   A nonzero ECS session and the original registered content-worker lifetime
   contract are required. No Scene mutation or engine function is performed.
   Output bits correspond exactly to request order. actual is count only on
   success, zero otherwise. No partial rows are valid after any failure.
   Each identity/span and key is rechecked within this bounded operation.
   Success is a sampled projection, not an immutable map generation or an
   atomic snapshot across resources. Never retain pointers across calls.
   Result: 1 success, 2 invalid argument, 3 unsupported lifetime contract,
   4 stale context/session, 5 retry incomplete (busy/unavailable/changed). */
typedef struct KfcRuntimeResourceScalarRequest {
    KfcRuntimeResourceId resource;
    uint32_t static_hash;
} KfcRuntimeResourceScalarRequest;
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeResourceReadScalarBatch(
    const KfcRuntimeResourceScalarRequest* requests, size_t count, uint32_t type_hash,
    size_t root_size, size_t offset, uint64_t expected_session,
    uint32_t* scalar_bits, size_t capacity, size_t* actual);
/* Optional ABI-12 original ItemRegistry relation. This selects the canonical
   entry in the validated ResourceService owner's registry, not a globally
   unique match among every loaded ItemInfo. The exact profile admits the map
   layout and target identity fields. Host reflection must supply matching
   type_hash/root_size/item_id_offset/guid_offset. A nonzero LocalAuthority
   session and original resource-worker lifetime are required. The returned
   full ResourceId is checked against the very same ResourceService payload.
   No pointers or engine calls are exposed. Result: 1 found, 2 invalid argument,
   3 unsupported contract, 4 stale context, 5 retry/incomplete, 6 not found.
   The output is valid only for result 1. Old exports retain their contracts. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeResourceResolveItem(
    uint32_t item_id, uint32_t type_hash, size_t root_size,
    size_t item_id_offset, size_t guid_offset, uint64_t expected_session,
    KfcRuntimeResourceId* resource);
/* Optional ABI-12 scalar data update. Exact-image profile and registered
   content-worker lifetime are required. Naturally aligned 1/2/4/8-byte values in the
   private resource bulk are supported. Never changes executable pages.
   Result: 0 rejected, 1 applied with protection restored, 2 CAS conflict,
   3 outcome uncertain. Result 3 must not trigger a blind retry. */
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeResourceCanWrite(void);
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeResourceCompareExchangeScalar(
    const KfcRuntimeResourceId* resource, uint32_t static_hash,
    size_t resource_size, size_t offset, const void* expected,
    const void* desired, size_t width);
/* Optional ABI-12 session-bound extension. The nonzero expected session is
   checked and pinned against the existing ECS binding while the original
   LocalAuthority Scene and ResourceService lifetime guards protect the CAS.
   expected points to resource_size bytes from the complete prior resource
   snapshot. desired points to width bytes (1, 2, 4 or 8). The full snapshot, including
   reflected descriptors, is compared under both guards before the scalar CAS.
   Results match KfcRuntimeResourceCompareExchangeScalar. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeResourceCompareExchangeScalarForSession(
    const KfcRuntimeResourceId* resource, uint32_t static_hash,
    size_t resource_size, size_t offset, const void* expected,
    const void* desired, size_t width, uint64_t expected_session);
/* Complete JSON status. Returns required bytes including NUL and copies only
   when the complete document fits. */
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeResourceStatus(char* buffer, size_t capacity);

/* Named plain-data DI service views use the same original authority/Scene
   lifetime gate. All accesses require the current world session. The provider
   checks the original full registry name, type, mutable binding and both service
   pointer selections. No engine function or ECS tracker is called.
   CompareExchange result: 0 rejected, 1 applied/read back, 2 conflict,
   3 uncertain. An uncertain result blocks service writes for that session. */
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeServiceRead(
    const char* key, uint32_t type_hash, uint32_t size, uint64_t expected_session,
    unsigned char* buffer, size_t capacity);
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeServiceCompareExchange(
    const char* key, uint32_t type_hash, uint32_t size, uint64_t expected_session,
    const void* expected, const void* mask, const void* desired);

/* Optional ABI-12 typed original Impact operation. Resolves isSameEntity by its
   complete registered name. Own VM/input/result buffers compare two uint32 IDs,
   without checking entity existence or reading/changing the game world. All
   original Scene/session gates are released before the callback. DI ownership
   still uses the existing profile-backed owner/lifetime infrastructure.
   equal is unchanged on failure and contains 0/1 only for result 1.
   Results: 1 confirmed, 2 invalid arguments, 3 registry unavailable,
   4 session changed, 5 unsupported contract, 6 native exception,
   7 invalid native result. No generic engine invocation is exposed. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeImpactCompareEntityIds(
    uint64_t expected_session, uint32_t left, uint32_t right, uint8_t* equal);

/* ECS values are opaque byte layouts described by the active game profile. */
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsConfigure(
    const char* const* qualified_names, const uint32_t* sizes, size_t count);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsReady(void);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsPropQueryReady(void);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsCanWrite(void);
// Current component policy, independent of a particular entity value.
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsCanWriteComponent(const char* name);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsDescribe(
    const char* qualified_name, uint32_t* size);
/* Optional ABI-12 compact registration metadata. Includes every matching
   original/runtime/template alias. Callback code bodies use the full registry
   export. Returns the required JSON buffer bytes including its terminator. */
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeEcsRegistration(
    const char* qualified_name, char* buffer, size_t capacity);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsEntityIdentity(
    uint32_t entity_handle, uint32_t* entity_id);
/* Optional ABI-12 entity-template owner link. Copies the original 16-byte GUID
   only for a current generation/membership-checked entity and validated layout. */
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsTemplateUuid(
    uint32_t entity_handle, uint8_t* guid);
/* Optional ABI-12 extensions. Registry JSON contains every engine registration,
 * including template-only entries. Returns required bytes including NUL; copies
 * only when the entire document fits. No partial JSON is returned. */
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeEcsRegistry(char* buffer, size_t capacity);
/* Resolve a registration name or storage type name to its entity storage layout.
 * Template-only registrations return false. This does not convert template bytes. */
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsResolveType(
    const char* name, char* runtime_name, size_t capacity);
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeEcsQuery(
    const char* const* qualified_names, size_t component_count,
    uint32_t* entities, size_t capacity);
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeEcsQueryBounds(
    const char* const* qualified_names, size_t component_count,
    const double* bounds, double padding, uint32_t* entities, size_t capacity);
typedef struct KfcRuntimePropRecord {
    uint32_t entity_handle;
    uint32_t item_id;
    int64_t position[3];
    float orientation[4];
    float scale[3];
    uint64_t template_uuid[2];
    uint32_t entity_id;
} KfcRuntimePropRecord;
typedef struct KfcRuntimePropRecipe {
    uint32_t item_id;
    float bounds[6];
    uint32_t feedback;
} KfcRuntimePropRecipe;
typedef struct KfcRuntimeGridSpec {
    char id[16];
    double origin[3];
    double cell_size[3];
    uint64_t maximum[3];
} KfcRuntimeGridSpec;
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeWorldEntityQueryProps(
    const double* bounds, double padding, KfcRuntimePropRecord* props, size_t capacity);
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeWorldEntityQueryPropsInBounds(
    const double* bounds, KfcRuntimePropRecord* props, size_t capacity);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldEntityRegisterPropRecipes(
    const KfcRuntimePropRecipe* recipes, size_t count);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldEntityGetPropRecipe(
    uint32_t item_id, KfcRuntimePropRecipe* recipe);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldEntityGetTransform(
    uint32_t entity_handle, KfcRuntimePropRecord* prop);
/* Optional ABI-12 session-bound prop read. Only original occupancy/key layouts
   can distinguish absence. The handle must have been issued in this session.
   Two matching hash lookups confirm absence or a changed entity generation.
   Unreadable/moving data never means absent. Output is unchanged unless result
   is 1. Results: 1 present, 2 invalid arguments, 3 unsupported contract,
   4 stale session/handle, 5 incomplete/changed (retry), 6 confirmed absent,
   7 session write outcome uncertain. The bool export retains its old contract. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeWorldEntityReadTransformForSession(
    uint32_t entity_handle, uint64_t expected_session, KfcRuntimePropRecord* prop);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldEntitySetScale(
    uint32_t entity_handle, const double* scale);
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeEcsResolve(uint32_t entity_id);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsRead(
    uint32_t entity, const char* qualified_name, void* value, size_t size);
/* Optional ABI-12 extension. One checked owner operation reads 1..64 component
   candidates into fixed slices in input order. capacity must equal sum(sizes)
   and be <=1 MiB. present[count] is 0 for absent or 1 for individually verified
   payloads. Failure publishes no output. Components are not an atomic snapshot. */
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsReadMany(
    uint32_t entity, const char* const* qualified_names, const uint32_t* sizes,
    size_t count, uint8_t* buffer, size_t capacity, uint8_t* present);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsWrite(
    uint32_t entity, const char* qualified_name, const void* mask,
    const void* value, size_t size);
/* Optional ABI 12 extension: 0 rejected/failed, 1 written, 2 snapshot changed,
   3 uncertain outcome after an unverified rollback. Comparison and masked write
   share the approved dispatcher phase or original ECS lifetime/execution gates. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeEcsCompareExchange(
    uint32_t entity, const char* qualified_name, const void* expected,
    const void* mask, const void* value, size_t size);

/* World operations are profile-backed and dispatched with runtime checks. */
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldOperationAvailable(const char* name);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldContextActive(void);
/* Optional ABI-12 extension: 0=unknown, 1=validated direct writable world,
   2=validated client cursor world that is read-only from this process. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeWorldContextKind(void);
KFC_RUNTIME_API uint64_t KFC_RUNTIME_CALL KfcRuntimeWorldSessionId(void);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldEntityContextReady(void);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldCursorRead(
    uint8_t* cursor, size_t capacity, uint64_t* sequence);
/* Optional ABI-12 patch-free local cursor source. Copies the complete original
   ClientPlayerInput component from the validated main-client ECS manager.
   component_size comes from the active reflection and must match the original
   runtime registration. capacity must equal component_size (at most 64 KiB).
   expected_session must be the current nonzero ECS world session. No pointers,
   engine calls or code changes are exposed. sequence counts successful samples,
   not engine ticks. Output is valid only for result 1. Results: 1 success,
   2 invalid arguments, 3 unsupported contract, 4 stale world/session,
   5 incomplete/changed/busy (retry next update), 6 ambiguous local input. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeWorldCursorReadInput(
    size_t component_size, uint64_t expected_session, uint8_t* buffer,
    size_t capacity, uint64_t* sequence);
/* Optional. How the last scan chose among several player inputs (multiplayer:
   one ClientPlayerInput per player), or what it tried when it could not.
   Returns the text length. */
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeWorldCursorInputDetail(char* buffer, size_t capacity);

/* Optional. The local client's camera, from the same ClientPlayerInput
   sample as the local cursor, for a caller drawing every frame. The mod
   runtime sets the reflected layout once: component size and 9 byte offsets
   (position x, y, z as Q32.32 int64; orientation x, y, z, w; fovY;
   aspectRatio as float). Read writes 9 doubles (position in meters). Results
   as KfcRuntimeWorldCursorReadInput, and 3 while no layout was set. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeSetLocalCameraLayout(
    size_t component_size, const uint32_t* offsets, size_t count);
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeReadLocalCamera(double* output, size_t count);
/* Optional ABI-12 client input extension. Exact build only. Dispatch is not a
   replicated-world acknowledgement. kind: select=0, place=1, remove=2, undo=3.
   Status: unknown=0, queued=1, pressed=2, dispatched=3, timeout=4, cancelled=5. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeWorldBuildingInput(
    uint32_t player, uint32_t kind, uint32_t item, uint32_t material, uint32_t slot,
    uint32_t target_entity_id, const double* position, const double* rotation, const double* scale);
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeWorldBuildingInputStatus(uint32_t id);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldBuildingInputCancel(uint32_t id);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldVoxelRead(
    const int32_t* origin, const uint32_t* dimensions, uint16_t* values,
    size_t capacity, size_t* actual);
/* Optional ABI-12 session-bound voxel reader. A pending original request is
   retried on a later update. No partial cell buffer is a successful result.
   Results: 1 complete, 2 pending/busy, 3 rejected input, 4 stale session,
   5 unavailable contract, 6 native reader fault. Only result1 publishes cells. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeWorldVoxelReadForSession(
    uint64_t expected_session, const int32_t* origin, const uint32_t* dimensions,
    uint16_t* values, size_t capacity, size_t* actual);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldVoxelWrite(
    const int32_t* origin, const uint32_t* dimensions, const uint16_t* values,
    size_t count, uint32_t* outcome);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldGridGetSpec(
    const char* grid_id, KfcRuntimeGridSpec* spec);
/* Optional ABI-12 registered grid metadata, bound to the caller's world.
   Same result codes as KfcRuntimeWorldVoxelReadForSession. Only success
   publishes metadata. Pending must be retried on a later update. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeWorldGridGetSpecForSession(
    uint64_t expected_session, const char* grid_id, KfcRuntimeGridSpec* spec);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldEntitySpawn(
    const uint64_t* template_uuid, const double* position, const double* rotation,
    uint32_t tracking, uint32_t flags, uint32_t* queue_token, uint32_t* outcome);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldEntityPlace(
    const double* position, const double* rotation, const float* bounds,
    uint32_t tracking, uint32_t feedback, uint32_t* outcome);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldEntityDestroy(
    const double* position, const double* rotation, const float* bounds,
    uint32_t tracking, uint32_t feedback, uint32_t* outcome);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldEntityDestroyHandle(
    uint32_t entity_handle, uint32_t* outcome);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldEntityFinishBuilding(
    bool complete, uint32_t* outcome);

/* Only patches registered in the approved build profile can be toggled. */
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimePatchAvailable(const char* name);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimePatchSetEnabled(
    const char* name, bool enabled, uint32_t* outcome);

#if defined(__cplusplus)
}
#endif
