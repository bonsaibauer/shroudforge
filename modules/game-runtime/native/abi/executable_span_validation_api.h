#pragma once

#include <stdint.h>

/*
 * Read-only validation for a package-owned executable entry prefix. This
 * service exposes no arbitrary code-read API, installs no patch, and returns
 * no code bytes or addresses through its service table. Its callback receives
 * only the bounded sample for the span that the same package registered. The
 * package may register spans only during its
 * stage-before-entry callback, on its control/owner thread, while the host's
 * held-entry preparation proof is active. Query, revoke, and drain are also
 * owner/control-thread operations. The host binds each registration to
 * the package generation that queried this table and copies expected bytes.
 *
 * Registered spans are 1..32 bytes, must be within the loaded main image's
 * readable executable sections, and may not overlap any other registration.
 * The host admits at most 16 spans per package generation and 64 total.
 * Duplicate, overlapping, stale, revoked, malformed, or changed registrations
 * are rejected. The callback/context are borrowed until revoke-and-drain has
 * completed; the callback must remain valid for that lifetime.
 *
 * Validation callbacks are synchronous and may run on any thread, including
 * arbitrary game callback threads. They must be thread-safe, must not call any host callback/service,
 * must not retain request/response pointers, and must not mutate executable
 * memory. Request byte arrays contain only byte_count meaningful bytes; the
 * remainder is zero. Response reserved fields and unused identity bytes must
 * be zero where specified below. UNCHANGED is accepted only when the host has
 * independently confirmed observed == expected, and its identity must be all
 * zero. VERIFIED_OWNER carries only an opaque nonzero identity; it authorizes
 * the host to substitute its copied expected prefix in a local comparison
 * buffer. Consumers that compare before/after samples must require matching
 * owner identities. The suffix of any engine guard is still compared
 * byte-for-byte and is never normalized by this service.
 *
 * The host sets request size/version to this structure's size and version 1;
 * the request's expected/observed tails past byte_count are zero. A callback
 * returns zero only after filling a complete response with size
 * sizeof(SFExecutableSpanValidationResponseV1), version 1, a valid result, and
 * zero reserved fields. For UNCHANGED, every identity byte is zero. For
 * VERIFIED_OWNER, at least one identity byte is nonzero. A nonzero callback
 * return is refusal; the host ignores the response and rejects validation.
 *
 * Revocation prevents new callback entries. Drain returns BUSY while callbacks
 * are in flight; package code and callback context must remain loaded until a
 * successful drain. Errors leave caller output structures untouched.
 */
#define SF_EXECUTABLE_SPAN_VALIDATION_SERVICE_V1 \
    "shroudforge.executable-span-validation"
#define SF_EXECUTABLE_SPAN_VALIDATION_VERSION_V1 1u
#define SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES 32u
#define SF_EXECUTABLE_SPAN_VALIDATION_MAX_SPANS_PER_OWNER 16u
#define SF_EXECUTABLE_SPAN_VALIDATION_MAX_SPANS_TOTAL 64u

#if defined(_MSC_VER)
#define SF_EXECUTABLE_SPAN_VALIDATION_CALL __cdecl
#else
#define SF_EXECUTABLE_SPAN_VALIDATION_CALL
#endif

typedef enum SFExecutableSpanValidationPhaseV1 {
    SF_EXECUTABLE_SPAN_PHASE_NORMALIZE = 1u,
    SF_EXECUTABLE_SPAN_PHASE_BEFORE_ORIGINAL = 2u,
    SF_EXECUTABLE_SPAN_PHASE_AFTER_ORIGINAL = 3u
} SFExecutableSpanValidationPhaseV1;

typedef enum SFExecutableSpanValidationResultV1 {
    SF_EXECUTABLE_SPAN_VALIDATION_UNCHANGED = 0u,
    SF_EXECUTABLE_SPAN_VALIDATION_VERIFIED_OWNER = 1u,
    SF_EXECUTABLE_SPAN_VALIDATION_REJECT = 2u
} SFExecutableSpanValidationResultV1;

typedef struct SFExecutableSpanValidationRequestV1 {
    uint32_t size;
    uint32_t version;
    uint32_t phase;
    uint32_t byte_count;
    uint32_t rva;
    uint8_t expected[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES];
    uint8_t observed[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES];
} SFExecutableSpanValidationRequestV1;

typedef struct SFExecutableSpanValidationResponseV1 {
    uint32_t size;
    uint32_t version;
    uint32_t result;
    uint32_t reserved;
    uint8_t owner_identity[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES];
} SFExecutableSpanValidationResponseV1;

typedef int32_t (SF_EXECUTABLE_SPAN_VALIDATION_CALL *
    SFExecutableSpanValidationCallbackV1)(
        void* callback_context,
        const SFExecutableSpanValidationRequestV1* request,
        SFExecutableSpanValidationResponseV1* response);

typedef struct SFExecutableSpanRegistrationV1 {
    uint32_t size;
    uint32_t version;
    uint32_t rva;
    uint32_t byte_count;
    uint8_t expected[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES];
    SFExecutableSpanValidationCallbackV1 callback;
    void* callback_context;
} SFExecutableSpanRegistrationV1;

typedef struct SFExecutableSpanValidationServiceV1 {
    uint32_t size;
    uint32_t version;
    /* Opaque owner-generation context; not an engine address or handle. */
    void* context;
    int32_t (SF_EXECUTABLE_SPAN_VALIDATION_CALL *register_span)(
        void* context,
        const SFExecutableSpanRegistrationV1* registration);
} SFExecutableSpanValidationServiceV1;
