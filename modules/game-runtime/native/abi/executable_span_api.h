#ifndef SHROUDFORGE_EXECUTABLE_SPAN_API_H
#define SHROUDFORGE_EXECUTABLE_SPAN_API_H

#include <stdint.h>

#if defined(_MSC_VER)
#define SF_EXECUTABLE_SPAN_CALL __cdecl
#else
#define SF_EXECUTABLE_SPAN_CALL
#endif

#define SF_EXECUTABLE_SPAN_SERVICE_NAME "shroudforge.executable-spans"
#define SF_EXECUTABLE_SPAN_SERVICE_VERSION 1u
#define SF_EXECUTABLE_SPAN_MAX_BYTES 32u

#define SF_EXECUTABLE_SPAN_UNSET 0u
#define SF_EXECUTABLE_SPAN_UNCHANGED 1u
#define SF_EXECUTABLE_SPAN_VERIFIED_HOST_OWNED 2u
#define SF_EXECUTABLE_SPAN_FOREIGN_OR_UNAVAILABLE 3u

typedef int32_t SFExecutableSpanStatusV1;
#define SF_EXECUTABLE_SPAN_STATUS_OK ((SFExecutableSpanStatusV1)0)
#define SF_EXECUTABLE_SPAN_STATUS_INVALID_ARGUMENT ((SFExecutableSpanStatusV1)1)
#define SF_EXECUTABLE_SPAN_STATUS_ABI_MISMATCH ((SFExecutableSpanStatusV1)2)
#define SF_EXECUTABLE_SPAN_STATUS_UNAVAILABLE ((SFExecutableSpanStatusV1)3)
#define SF_EXECUTABLE_SPAN_STATUS_UNKNOWN_SERVICE ((SFExecutableSpanStatusV1)4)
#define SF_EXECUTABLE_SPAN_STATUS_BUFFER_TOO_SMALL ((SFExecutableSpanStatusV1)5)
#define SF_EXECUTABLE_SPAN_STATUS_BUSY ((SFExecutableSpanStatusV1)6)
#define SF_EXECUTABLE_SPAN_STATUS_FAILED ((SFExecutableSpanStatusV1)7)
#define SF_EXECUTABLE_SPAN_STATUS_WRONG_THREAD ((SFExecutableSpanStatusV1)8)

/* Read-only comparison against a caller-observed executable range in the
 * current main image. byte_count is 1..SF_EXECUTABLE_SPAN_MAX_BYTES; only the
 * first byte_count bytes in each array are used. A successful result reports
 * whether the supplied observation still matches expected_original or a
 * verified resident framework-owned replacement. A changed observation
 * returns an error and leaves out_result untouched. No code bytes or addresses
 * are returned. This is evidence, not patch authorization or a quiescence
 * guarantee.
 */
typedef struct SFExecutableSpanQueryV1 {
    uint32_t size;
    uint32_t version;
    uint32_t rva;
    uint32_t byte_count;
    uint8_t expected_original[SF_EXECUTABLE_SPAN_MAX_BYTES];
    uint8_t observed_current[SF_EXECUTABLE_SPAN_MAX_BYTES];
} SFExecutableSpanQueryV1;

/* Zero is reserved as unset. Classification is written only on status OK. */
typedef struct SFExecutableSpanResultV1 {
    uint32_t size;
    uint32_t classification;
} SFExecutableSpanResultV1;

typedef SFExecutableSpanStatusV1(SF_EXECUTABLE_SPAN_CALL *SFExecutableSpanQueryFnV1)(
    void *context,
    const SFExecutableSpanQueryV1 *request,
    uint32_t out_size,
    SFExecutableSpanResultV1 *out_result);

/* Borrowed from one package generation. Query and callback use the owner
 * thread and are revoked/drained with that generation. */
typedef struct SFExecutableSpanServiceV1 {
    uint32_t size;
    uint32_t version;
    void *context;
    SFExecutableSpanQueryFnV1 query_main_image_span;
} SFExecutableSpanServiceV1;

#endif
