#pragma once

#include <stddef.h>
#include <stdint.h>

#if defined(__cplusplus)
#include <stdbool.h>
extern "C" {
#else
#include <stdbool.h>
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
/* Optional ABI-12 code inventory extension. Same complete-buffer convention as
 * EcsRegistry. Code entries explicitly have no inferred calling ABI. */
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeFunctions(char* buffer, size_t capacity);
/* Optional ABI-12 bounded, read-only code evidence. Never calls the address.
 * At most 512 bytes, only from executable sections; zero means unavailable. */
KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeFunctionCode(uint32_t rva, unsigned char* buffer, size_t capacity);

/* ECS values are opaque byte layouts described by the active game profile. */
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsConfigure(
    const char* const* qualified_names, const uint32_t* sizes, size_t count);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsReady(void);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsPropQueryReady(void);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsCanWrite(void);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsDescribe(
    const char* qualified_name, uint32_t* size);
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
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldEntitySetScale(
    uint32_t entity_handle, const double* scale);
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeEcsResolve(uint32_t entity_id);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsRead(
    uint32_t entity, const char* qualified_name, void* value, size_t size);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeEcsWrite(
    uint32_t entity, const char* qualified_name, const void* mask,
    const void* value, size_t size);
/* Optional ABI 12 extension: 0 rejected/failed, 1 written, 2 snapshot changed.
   Comparison and masked write run in the same game-thread dispatch. */
KFC_RUNTIME_API uint32_t KFC_RUNTIME_CALL KfcRuntimeEcsCompareExchange(
    uint32_t entity, const char* qualified_name, const void* expected,
    const void* mask, const void* value, size_t size);

/* World operations are profile-backed and dispatched with runtime checks. */
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldOperationAvailable(const char* name);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldContextActive(void);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldEntityContextReady(void);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldCursorRead(
    uint8_t* cursor, size_t capacity, uint64_t* sequence);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldVoxelRead(
    const int32_t* origin, const uint32_t* dimensions, uint16_t* values,
    size_t capacity, size_t* actual);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldVoxelWrite(
    const int32_t* origin, const uint32_t* dimensions, const uint16_t* values,
    size_t count, uint32_t* outcome);
KFC_RUNTIME_API bool KFC_RUNTIME_CALL KfcRuntimeWorldGridGetSpec(
    const char* grid_id, KfcRuntimeGridSpec* spec);
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
