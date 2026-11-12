#pragma once

#include <string>
#include "../../game-runtime/native/host/selected_startup.h"

namespace EcsRuntime {
bool Initialize();
bool PrepareSelected(const SelectedStartup::PlanV2&,const EngineHooks::HeldEntryProof&);
bool ActivateSelected(const SelectedStartup::PlanV2&);
bool RequireOriginalOwners();
bool FinalizeOriginalOwners();
void Tick();
std::string Status();
void Shutdown();
}
