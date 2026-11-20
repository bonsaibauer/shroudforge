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
 * arbitrary game callback threads. They must be thread-safe, must not call any host callback/service except an explicitly arbitrary-thread
 * read-only proof lease defined below,
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

/* V2 keeps V1 registration/callback layouts. Query and acquisition remain
 * owner-thread, held-entry Stage operations. Only a copied proof lease may be
 * retained and used on arbitrary threads, including entry_restored and a
 * validation callback. It contains no SFNativeHostV1/Rust owner context.
 *
 * A binding must exactly match this generation's existing registration;
 * owned_prefix_byte_count selects a nonempty prefix. Acquisition can precede
 * framework publication. Validation compares the entire registered sample,
 * requires a canonical suffix, and proves only an internally published native
 * prefix. It never grants patch authority or returns addresses/code bytes.
 *
 * The first verified publication is permanently adopted. Retirement, native
 * session closure, package revocation, or replacement invalidates the lease;
 * restoring canonical bytes cannot bypass an adopted witness. UNCHANGED has
 * zero identity; VERIFIED_HOST_OWNED has a stable nonzero opaque identity.
 * Classification values are 1 and 2 respectively (zero is unset).
 *
 * Errors leave outputs untouched. All unused array tails/reserved fields must
 * be zero. Revocation/closure and success share a commit ordering: success
 * committed before closure may return afterward. This is not quiescence.
 * Drain includes proof calls; callable code and native opaque tombstones stay
 * resident until process exit. Exhaustion never recycles a stale context.
 * Other services and host callbacks retain their original thread restrictions.
 */
#define SF_EXECUTABLE_SPAN_VALIDATION_VERSION_V2 2u
#define SF_EXECUTABLE_SPAN_PROOF_VERSION_V1 1u
#define SF_EXECUTABLE_SPAN_PROOF_UNCHANGED 1u
#define SF_EXECUTABLE_SPAN_PROOF_VERIFIED_HOST_OWNED 2u

typedef struct SFExecutableSpanProofBindingV1 {
    uint32_t size, version, registered_rva, registered_byte_count;
    uint32_t owned_prefix_byte_count, reserved;
    uint8_t expected[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES];
} SFExecutableSpanProofBindingV1;

typedef struct SFExecutableSpanProofQueryV1 {
    uint32_t size, version, phase, byte_count;
    uint8_t observed_current[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES];
} SFExecutableSpanProofQueryV1;

typedef struct SFExecutableSpanProofResultV1 {
    uint32_t size, version, classification, reserved;
    uint8_t owner_identity[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES];
} SFExecutableSpanProofResultV1;

typedef struct SFExecutableSpanProofLeaseV1 {
    uint32_t size, version;
    void* context;
    int32_t (SF_EXECUTABLE_SPAN_VALIDATION_CALL *validate)(
        void*, const SFExecutableSpanProofQueryV1*, uint32_t,
        SFExecutableSpanProofResultV1*);
} SFExecutableSpanProofLeaseV1;

typedef struct SFExecutableSpanValidationServiceV2 {
    SFExecutableSpanValidationServiceV1 base;
    int32_t (SF_EXECUTABLE_SPAN_VALIDATION_CALL *acquire_owned_prefix_lease)(
        void*, const SFExecutableSpanProofBindingV1*, uint32_t,
        SFExecutableSpanProofLeaseV1*);
} SFExecutableSpanValidationServiceV2;
