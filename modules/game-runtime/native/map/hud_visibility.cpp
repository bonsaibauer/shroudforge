// Engine HUD visibility capture adapted from Minimap; see SOURCE-NOTICE.txt.
#include "map/hud_visibility.h"
#include "map/capture_game_map.h"
#include "map/current_client_map_contract.h"
#include "map/current_client_hud_guards.h"
#include "profiles/game_profile.h"
#include "map/menu_visibility_1_0.h"
#include <atomic>
#include <windows.h>
namespace MapRuntime {
namespace {
std::atomic<std::uintptr_t> hud_image{};std::atomic<std::size_t> hud_size{};
std::atomic<bool> hud_active{};std::atomic<std::uint64_t> hud_epoch{},observation{};
bool hud_prepared{};
const char* hud_preparation_reason="not-started";
bool current_hud() noexcept{return Current5d9ef2f1::Selected(KfcRuntimeCompatibility::EnshroudedClient::image_sha256);}
std::size_t call_rva() noexcept{return current_hud()?Current5d9ef2f1::HudCall:0xbc24ba;}
std::size_t renderer_rva() noexcept{return current_hud()?Current5d9ef2f1::Renderer:0xc4fc40;}
bool GuardRead(const std::uint8_t* source,void* output,std::size_t size) {return source && output && size<=64 && ReadMemory(reinterpret_cast<std::uintptr_t>(source),output,size);}
void HudRender();
bool NativeOwnership(bool retained) {
    const auto base=hud_image.load(),size=hud_size.load();
    if(!base || size<=renderer_rva()+18 || size<=call_rva()+10)return false;
    std::uint8_t original_call[5]{0xe8};const auto displacement=std::int32_t(renderer_rva()-call_rva()-5);std::memcpy(original_call+1,&displacement,4);
    if(retained && !SharedCallOwned(call_rva(),renderer_rva(),reinterpret_cast<void*>(&HudRender)))return false;
    // Normalize only our independently verified CALL. The engine renderer
    // entry and every remaining dependency must match original game bytes.
    if(current_hud())return CurrentHud5d9ef2f1::Match(reinterpret_cast<const std::uint8_t*>(base),size,
        retained?original_call:nullptr,retained?5:0,GuardRead);
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
    const bool staged=hud_active && NativeOwnership(true) && Screen(screen,first);
    if(!staged || screen!=1 || !HudGameplay(GetTickCount64()))observation=0;
    const auto original=reinterpret_cast<void(*)()>(hud_image.load()+renderer_rva());original();
    if(!staged || !hud_active || epoch!=hud_epoch.load()){observation=0;return;}
    std::array<std::uintptr_t,4> second{};
    if(!Screen(screen,second) || first!=second || !NativeOwnership(true) || !hud_active || epoch!=hud_epoch.load()){observation=0;return;}
    observation=(GetTickCount64()<<8)|screen;
    if(!hud_active || epoch!=hud_epoch.load())observation=0;
}
}
bool PrepareHudVisibility(std::uintptr_t image,std::size_t size) {
    hud_preparation_reason="preconditions";
    if(!current_hud() || hud_prepared)return false;
    hud_image=image;hud_size=size;observation=0;++hud_epoch;hud_active=false;
    hud_preparation_reason="engine-guards";
    if(!NativeOwnership(false))return false;
    hud_preparation_reason="shared-call";
    if(!InstallSharedCall(call_rva(),renderer_rva(),reinterpret_cast<void*>(&HudRender)))return false;
    hud_prepared=true;hud_preparation_reason="ready";return true;
}
std::string HudPreparationStatus() {return hud_preparation_reason;}
bool EnableHudVisibility(std::uintptr_t image,std::size_t size) {
    if(current_hud()){
        if(!hud_prepared || image!=hud_image || size!=hud_size || !NativeOwnership(true))return false;
        observation=0;++hud_epoch;hud_active=true;return true;
    }
    hud_image=image;hud_size=size;observation=0;++hud_epoch;
    if(!NativeOwnership(false) || !InstallSharedCall(call_rva(),renderer_rva(),reinterpret_cast<void*>(&HudRender)))return false;
    hud_active=true;return true;
}
void DisableHudVisibility() {hud_active=false;++hud_epoch;observation=0;}
bool HudKnown(std::uint64_t now) {const auto value=observation.load(),at=value>>8;return hud_active && (value&255) && now>=at && now-at<=EngineMenu10::MaxAgeMs;}
bool HudInputAllowed(std::uint64_t now) {return !hud_active || HudGameplay(now);}
bool HudGameplay(std::uint64_t now) {return HudKnown(now) && (observation.load()&255)==1;}
}
