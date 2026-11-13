#include "abi/runtime_api.h"

// These lifecycle fixtures have an empty validation registry. Code reads and
// held-entry registration are unavailable; the real registry remains linked.
extern "C" KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeFunctionCode(
        uint32_t, unsigned char*, size_t) {
    return 0;
}
