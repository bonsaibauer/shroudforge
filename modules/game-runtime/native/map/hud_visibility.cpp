// Engine HUD visibility capture adapted from Minimap; see SOURCE-NOTICE.txt.
#include "map/hud_visibility.h"
#include "map/capture_game_map.h"
#include "map/current_client_map_contract.h"
#include "map/current_client_hud_guards.h"
#include "host/executable_span_validation.h"
#include "profiles/game_profile.h"
#include "map/menu_visibility_1_0.h"
#include <array>
#include <atomic>
#include <cstring>
#include <windows.h>
namespace MapRuntime {
namespace {
std::atomic<std::uintptr_t> hud_image{};std::atomic<std::size_t> hud_size{};
std::atomic<bool> hud_active{};std::atomic<std::uint64_t> hud_epoch{},observation{};
std::atomic<bool> hud_prepared{},hud_preflighted{};
std::atomic<const char*> hud_preparation_reason{"not-started"};
bool current_hud() noexcept{return Current5d9ef2f1::Selected(KfcRuntimeCompatibility::EnshroudedClient::image_sha256);}
std::size_t call_rva() noexcept{return current_hud()?Current5d9ef2f1::HudCall:0xbc24ba;}
std::size_t renderer_rva() noexcept{return current_hud()?Current5d9ef2f1::Renderer:0xc4fc40;}
bool GuardRead(const std::uint8_t* source,void* output,std::size_t size) {return source && output && size<=64 && ReadMemory(reinterpret_cast<std::uintptr_t>(source),output,size);}
bool NormalizeGuard(std::uint32_t rva,const std::uint8_t* expected,const std::uint8_t* observed,std::size_t size) {
    return ExecutableSpanValidation::Services().ValidateGuard(rva,expected,observed,size,
        SF_EXECUTABLE_SPAN_PHASE_NORMALIZE);
}
bool RendererValidated(std::uint32_t phase,std::uint8_t identity[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES]) {
    if(identity)std::memset(identity,0,SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES);
    if(!current_hud())return true;
    const auto& pin=CurrentHud5d9ef2f1::Pins[0];
    if(pin.rva>UINT32_MAX || pin.bytes.empty() || pin.bytes.size()>64)return false;
    std::array<std::uint8_t,64> actual{};
    if(!GuardRead(reinterpret_cast<const std::uint8_t*>(hud_image.load()+pin.rva),actual.data(),pin.bytes.size()))return false;
    return ExecutableSpanValidation::Services().ValidateGuard(static_cast<std::uint32_t>(pin.rva),
        reinterpret_cast<const std::uint8_t*>(pin.bytes.data()),actual.data(),pin.bytes.size(),phase,identity);
}
void HudRender();
bool NativeOwnership(bool retained) {
    const auto base=hud_image.load(),size=hud_size.load();
    if(!base || size<=renderer_rva()+18 || size<=call_rva()+10)return false;
    std::uint8_t original_call[5]{0xe8};const auto displacement=std::int32_t(renderer_rva()-call_rva()-5);std::memcpy(original_call+1,&displacement,4);
    if(retained && !SharedCallOwned(call_rva(),renderer_rva(),reinterpret_cast<void*>(&HudRender)))return false;
    // Normalize only our independently verified CALL. The engine renderer
    // entry and every remaining dependency must match, except a registered
    // owner may validate only its exact entry prefix.
    if(current_hud())return CurrentHud5d9ef2f1::Match(reinterpret_cast<const std::uint8_t*>(base),size,
        retained?original_call:nullptr,retained?5:0,GuardRead,NormalizeGuard);
    return EngineMenu10::MatchGuardsWithOwnedCall(reinterpret_cast<const std::uint8_t*>(base),size,
        retained?original_call:nullptr,retained?5:0,GuardRead,true);
}
bool Screen(std::uint8_t& screen,std::array<std::uintptr_t,4>& chain) {
    const auto Copy=[](std::uintptr_t at,void* out,std::size_t n){return ReadMemory(at,out,n);};
    const auto Range=[](std::uintptr_t at,std::size_t n){return at>=0x10000 && at<=UINTPTR_MAX-n;};
    std::uintptr_t context{},holder{},state{},play{};const auto base=hud_image.load();
    if(!base || !Copy(base+(current_hud()?Current5d9ef2f1::UiGlobal:0x281e498),&context,8) || !Range(context,0xd8) || !Copy(context+0xd0,&holder,8) ||
        !Copy(holder,&state,8) || !Range(state,0x40) || !Copy(state+0x38,&play,8) || !Range(play,0x1cd9) ||
        !Copy(play+0x1cd8,&screen,1) || !screen || screen>53)return false;
    chain={context,holder,state,play};return true;
}
void HudRender() {
    const auto epoch=hud_epoch.load();std::array<std::uintptr_t,4> first{};std::uint8_t screen{};
    std::array<std::uint8_t,SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES> identity_before{};
    const bool owner_before=NativeOwnership(true);
    const bool entry_before=RendererValidated(SF_EXECUTABLE_SPAN_PHASE_BEFORE_ORIGINAL,identity_before.data());
    const bool staged=hud_active && owner_before && entry_before && Screen(screen,first);
    if(!staged || screen!=1 || !HudGameplay(GetTickCount64()))observation=0;
    const auto original=reinterpret_cast<void(*)()>(hud_image.load()+renderer_rva());original();
    const bool owner_after=NativeOwnership(true);
    std::array<std::uint8_t,SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES> identity_after{};
    const bool entry_after=RendererValidated(SF_EXECUTABLE_SPAN_PHASE_AFTER_ORIGINAL,identity_after.data());
    if(!staged || !hud_active || epoch!=hud_epoch.load()){observation=0;return;}
    std::array<std::uintptr_t,4> second{};
    if(!owner_after || !entry_after || identity_before!=identity_after || !Screen(screen,second) ||
       first!=second || !hud_active || epoch!=hud_epoch.load()){observation=0;return;}
    observation=(GetTickCount64()<<8)|screen;
    if(!hud_active || epoch!=hud_epoch.load())observation=0;
}
}
bool PreflightHudPreparation(std::uintptr_t image,std::size_t size) {
    hud_preparation_reason="preconditions";
    if(!current_hud() || hud_prepared.load(std::memory_order_acquire))return false;
    hud_image=image;hud_size=size;observation=0;++hud_epoch;hud_active=false;
    hud_preflighted=false;
    hud_preparation_reason="engine-guards";
    if(!NativeOwnership(false))return false;
    hud_preflighted=true;return true;
}
bool InstallPreparedHudHooks() {
    hud_preparation_reason="preconditions";
    if(!current_hud() || !hud_preflighted.load(std::memory_order_acquire) ||
       hud_prepared.load(std::memory_order_acquire))return false;
    hud_preparation_reason="shared-call";
    if(!InstallSharedCall(call_rva(),renderer_rva(),reinterpret_cast<void*>(&HudRender)))return false;
    hud_prepared=true;hud_preflighted=false;hud_preparation_reason="ready";return true;
}
std::string HudPreparationStatus() {
    const auto* reason=hud_preparation_reason.load(std::memory_order_acquire);
    return reason ? reason : "unknown";
}
bool ValidatePreparedHudActivation() {
    return current_hud() && hud_prepared.load(std::memory_order_acquire) && NativeOwnership(true);
}
bool ActivatePreparedHudVisibility() {
    if(!current_hud() || !hud_prepared.load(std::memory_order_acquire))return false;
    observation=0;++hud_epoch;hud_active=true;return true;
}
bool PreflightLegacyHudVisibility(std::uintptr_t image,std::size_t size) {
    if(current_hud() || hud_preflighted.load(std::memory_order_acquire))return false;
    hud_image=image;hud_size=size;observation=0;++hud_epoch;
    hud_active=false;hud_preflighted=false;
    if(!NativeOwnership(false))return false;
    hud_preflighted=true;return true;
}
bool InstallPreflightedLegacyHudVisibility() {
    if(current_hud() || !hud_preflighted.load(std::memory_order_acquire))return false;
    if(!InstallSharedCall(call_rva(),renderer_rva(),reinterpret_cast<void*>(&HudRender)))return false;
    hud_preflighted=false;hud_active=true;return true;
}
void DisableHudVisibility() {hud_active=false;hud_preflighted=false;++hud_epoch;observation=0;}
bool HudKnown(std::uint64_t now) {const auto value=observation.load(),at=value>>8;return hud_active && (value&255) && now>=at && now-at<=EngineMenu10::MaxAgeMs;}
bool HudInputAllowed(std::uint64_t now) {return !hud_active || HudGameplay(now);}
bool HudGameplay(std::uint64_t now) {return HudKnown(now) && (observation.load()&255)==1;}
}
