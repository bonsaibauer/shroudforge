// HUD/public-Creative proofs adapted from pinned Minimap test.4; see SOURCE-NOTICE.txt.
#include "map/hud_visibility.h"
#include "map/capture_game_map.h"
#include "map/current_client_map_contract.h"
#include "map/current_client_hud_guards.h"
#include "profiles/game_profile.h"
#include "map/menu_visibility_1_0.h"
#include "map/public_creative_compat.h"
#include <algorithm>
namespace MapRuntime {
namespace {
std::atomic<std::uintptr_t> hud_image{};std::atomic<std::size_t> hud_size{};
std::atomic<bool> hud_active{};std::atomic<std::uint64_t> hud_epoch{},observation{};
bool hud_prepared{};
bool hud_finalization_attempted{};
const char* hud_preparation_reason="not-started";
PublicCreativeCompat::FiniteGraph::Diagnostic hud_preparation_graph;
PublicCreativeCompat::FiniteGraph::ApprovedProviderLeases provider_leases;
bool current_hud() noexcept{return Current5d9ef2f1::Selected(KfcRuntimeCompatibility::EnshroudedClient::image_sha256);}
std::size_t call_rva() noexcept{return current_hud()?Current5d9ef2f1::HudCall:0xbc24ba;}
std::size_t renderer_rva() noexcept{return current_hud()?Current5d9ef2f1::Renderer:0xc4fc40;}
struct Ownership {unsigned kind{};PublicCreativeCompat::Digest identity{};};
bool GuardRead(const std::uint8_t* source,void* output,std::size_t size) {return source && output && size<=64 && PublicCreativeCompat::GuardCopy(source,output,size);}
void HudRender();
bool OwnershipSample(Ownership& sample,bool retained,PublicCreativeCompat::ProofOrigin origin,bool acquireLeases=false,
    PublicCreativeCompat::FiniteGraph::Diagnostic* diagnostic=nullptr) {
    const auto refuse=[&](const char* reason){OriginalOwnerFailure::Record("hud-ownership",reason);
        if(current_hud() && provider_leases.Retained())PublicCreativeCompat::ForgetIdentity();return false;};
    const auto base=hud_image.load(),size=hud_size.load();if(!base || size<=renderer_rva()+18 || size<=call_rva()+10)return refuse("image-range");
    std::uint8_t original_call[5]{0xe8};const auto displacement=std::int32_t(renderer_rva()-call_rva()-5);std::memcpy(original_call+1,&displacement,4);
    if(retained && !SharedCallOwned(call_rva(),renderer_rva(),reinterpret_cast<void*>(&HudRender)))return refuse("shared-call");
    const auto guards=[&](const std::uint8_t* prefix,std::size_t prefix_size){
        if(current_hud())return CurrentHud5d9ef2f1::Match(reinterpret_cast<const std::uint8_t*>(base),size,
            prefix,prefix_size,retained?original_call:nullptr,retained?5:0,GuardRead);
        return MinimapMenu10::MatchGuardsWithRendererPrefix(reinterpret_cast<const std::uint8_t*>(base),size,
            prefix,prefix_size,retained?original_call:nullptr,retained?5:0,GuardRead,true);};
    std::array<std::uint8_t,18> entry{};if(!PublicCreativeCompat::Copy(base+renderer_rva(),entry.data(),entry.size(),true))return refuse("renderer-read");
    if(guards(nullptr,0)){if(diagnostic)diagnostic->reason="native";PublicCreativeCompat::ForgetIdentity();sample.kind=1;std::copy(entry.begin(),entry.end(),sample.identity.begin());return true;}
    PublicCreativeCompat::Proof proof{};
    if(!PublicCreativeCompat::Inspect(base+renderer_rva(),proof,origin,current_hud()?&provider_leases:nullptr,acquireLeases,diagnostic))return refuse("graph");
    if(!guards(PublicCreativeCompat::Original,sizeof(PublicCreativeCompat::Original))){
        if(diagnostic)diagnostic->reason="hud-guards";return refuse("normalized-guards");}
    sample.kind=proof.kind+2;sample.identity=proof.digest;return true;
}
bool Screen(std::uint8_t& screen,std::array<std::uintptr_t,4>& chain) {
    using PublicCreativeCompat::Copy;using PublicCreativeCompat::Range;
    std::uintptr_t context{},holder{},state{},play{};const auto base=hud_image.load();
    if(!base || !Copy(base+(current_hud()?Current5d9ef2f1::UiGlobal:0x281e498),&context,8) || !Range(context,0xd8) || !Copy(context+0xd0,&holder,8) ||
        !Copy(holder,&state,8) || !Range(state,0x40) || !Copy(state+0x38,&play,8) || !Range(play,0x1cd9) ||
        !Copy(play+0x1cd8,&screen,1) || !screen || screen>53)return false;
    chain={context,holder,state,play};return true;
}
void HudRender() {
    const auto epoch=hud_epoch.load();Ownership before{};std::array<std::uintptr_t,4> first{};std::uint8_t screen{};
    const bool staged=hud_active && OwnershipSample(before,true,PublicCreativeCompat::ProofOrigin::CallbackPre) && Screen(screen,first);
    if(!staged || screen!=1 || !HudGameplay(GetTickCount64()))observation=0;
    // A caller may already have fetched the relay during revoke. Forwarding
    // remains process resident even when capture is no longer admitted.
    const auto original=reinterpret_cast<void(*)()>(hud_image.load()+renderer_rva());original();
    if(!staged || !hud_active || epoch!=hud_epoch.load()){observation=0;return;}
    Ownership after{};std::array<std::uintptr_t,4> second{};
    if(!Screen(screen,second) || first!=second || !OwnershipSample(after,true,PublicCreativeCompat::ProofOrigin::CallbackPost) ||
        before.kind!=after.kind || before.identity!=after.identity || !hud_active || epoch!=hud_epoch.load()){observation=0;return;}
    observation=(GetTickCount64()<<8)|screen;
    if(!hud_active || epoch!=hud_epoch.load())observation=0;
}
}
bool PrepareHudVisibility(std::uintptr_t image,std::size_t size) {
    hud_preparation_reason="preconditions";hud_preparation_graph={};
    if(!current_hud() || hud_prepared)return false;
    hud_image=image;hud_size=size;observation=0;++hud_epoch;hud_active=false;
    // Root must run actual selected renderer-chain writers before this boundary.
    PublicCreativeCompat::AdmissionEpochScope admissionScope;
    Ownership sample{};
    hud_preparation_reason="ownership";
    if(!OwnershipSample(sample,false,PublicCreativeCompat::ProofOrigin::AdmissionUpdate,true,&hud_preparation_graph))return false;
    hud_preparation_reason="shared-call";
    if(!InstallSharedCall(call_rva(),renderer_rva(),reinterpret_cast<void*>(&HudRender)))return false;
    hud_prepared=true;hud_preparation_reason="ready";return true;
}
std::string HudPreparationStatus() {
    return std::string(hud_preparation_reason)+",graph="+hud_preparation_graph.reason+
        ",modules="+std::to_string(hud_preparation_graph.beforeModules)+"/"+std::to_string(hud_preparation_graph.afterModules)+
        ",leases="+std::to_string(hud_preparation_graph.retainedModules)+",reads="+std::to_string(hud_preparation_graph.reads);
}
bool FinalizeHudOriginalOwners() {
    hud_preparation_reason="post-scan-preconditions";hud_preparation_graph={};
    using namespace OriginalOwnerFailure;
    if(!Check(Hud,1,current_hud(),"not-current-profile"))return false;
    if(!Check(Hud,2,hud_prepared,"not-prepared"))return false;
    if(!Check(Hud,4,!hud_active,"already-active"))return false;
    if(!Check(Hud,8,!hud_finalization_attempted,"already-attempted"))return false;
    hud_finalization_attempted=true;hud_preparation_reason="post-scan-ownership";
    // Sole fixed post-original-scan boundary. Keep the original epoch and its
    // consumed identity budgets; no query, Tick or callback may enter this path.
    PublicCreativeCompat::ContinueAdmissionScope admissionScope;
    Ownership sample{};
    if(!OwnershipSample(sample,true,PublicCreativeCompat::ProofOrigin::AdmissionUpdate,true,&hud_preparation_graph))return false;
    if(!provider_leases.HasCurrentCohort()){hud_preparation_reason="post-scan-cohort";OriginalOwnerFailure::Record("hud","post-scan-cohort");return false;}
    hud_preparation_reason="post-scan-ready";return true;
}
bool EnableHudVisibility(std::uintptr_t image,std::size_t size) {
    if(current_hud()){
        if(!hud_prepared || image!=hud_image || size!=hud_size)return false;
        Ownership sample{};
        // Only warm verification. No new file attempt or epoch at service query.
        if(!OwnershipSample(sample,true,PublicCreativeCompat::ProofOrigin::AdmissionUpdate))return false;
        observation=0;++hud_epoch;hud_active=true;return true;
    }
    hud_image=image;hud_size=size;observation=0;++hud_epoch;
    // Explicit control-plane boundary only. No callback or Tick mints an epoch.
    PublicCreativeCompat::AdmissionEpochScope admissionScope;
    Ownership sample{};if(!OwnershipSample(sample,false,PublicCreativeCompat::ProofOrigin::AdmissionUpdate))return false;
    if(!InstallSharedCall(call_rva(),renderer_rva(),reinterpret_cast<void*>(&HudRender)))return false;
    hud_active=true;return true;
}
void DisableHudVisibility() {
    hud_active=false;++hud_epoch;observation=0;
    // Only the current profile's continuously referenced approved modules may
    // survive logical disable. Unknown/mutated observations still invalidate.
    if(!current_hud() || !provider_leases.Retained())PublicCreativeCompat::ForgetIdentity();
}
bool HudKnown(std::uint64_t now) {const auto value=observation.load(),at=value>>8;return hud_active && (value&255) && now>=at && now-at<=MinimapMenu10::MaxAgeMs;}
bool HudInputAllowed(std::uint64_t now) {return !hud_active || HudGameplay(now);}
bool HudGameplay(std::uint64_t now) {return HudKnown(now) && (observation.load()&255)==1;}
}
