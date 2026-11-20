#include "engine-hooks/held_entry_preparation.h"

namespace EngineHooks {
bool HeldEntryPreparationActiveOnCurrentThread() noexcept {
    return false;
}
}
