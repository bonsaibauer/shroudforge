// Narrow native capture mechanisms adapted from pinned ESM Minimap test.4.
// MIT, Copyright (c) 2024 Miguel Oppermann. See SOURCE-NOTICE.txt.
#include "map/capture_game_map.h"
#include "map/hud_visibility.h"
#include "profiles/game_profile.h"
#include "map/native_iterator_snapshot.h"
#include "engine-context/engine_owners.h"
#include "engine-hooks/publish_engine_hook.h"
#include "engine-hooks/held_entry_preparation.h"
#include "engine-hooks/owned_executable_span.h"
#include "engine-hooks/owned_span_proof.h"
#include "engine-hooks/shared_call_proof.h"
#include "host/executable_span_validation.h"
#include <windows.h>
#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstring>
#include <limits>
#include <unordered_set>

namespace MapRuntime {
std::uint32_t MapImageAdmission::Features() const {
    return (player ? SF_MAP_FEATURE_PLAYER : 0) | (markers ? SF_MAP_FEATURE_MARKERS : 0) |
        (fog ? SF_MAP_FEATURE_FOG : 0) | ((clock || weather) ? SF_MAP_FEATURE_ENVIRONMENT : 0) |
        (clock ? SF_MAP_FEATURE_CLOCK : 0) | (weather ? SF_MAP_FEATURE_WEATHER : 0) |
        (world ? SF_MAP_FEATURE_WORLD : 0) | (deaths ? SF_MAP_FEATURE_DEATHS : 0) |
        (discovery ? SF_MAP_FEATURE_DISCOVERY : 0);
}
bool ReadLocalCamera(const NativeCameraRecord& record, std::uint32_t entity, std::size_t local_id_offset,
    SFMapPoseV1& player, SFMapPoseV1& camera) {
    std::uint32_t local{}; NativeTransform p{},c{};
    return entity && entity <= 1023 && ReadMemory(record.local_player+local_id_offset,&local,4) && local == entity &&
        ReadMemory(record.player_transform,&p,sizeof(p)) && ReadMemory(record.camera_transform,&c,sizeof(c)) &&
        DecodeLocalTransforms(p,c,player,camera);
}
bool ReadPingEvents(std::uintptr_t list, std::uint32_t excluded, std::uint64_t now,
    std::vector<SFMapMarkerV1>& result) {
    result.clear(); std::uint32_t count{}; std::uint16_t index{}; std::uintptr_t arena{}; std::uint64_t capacity{};
    if (!list || list > UINTPTR_MAX-0x42 || !ReadMemory(list+0x18,&count,4) || count > 1024) return false;
    if (!count) return true;
    if (!ReadMemory(list+0x30,&arena,8) || !arena || !ReadMemory(list+0x38,&capacity,8) || !capacity ||
        capacity > (UINTPTR_MAX-arena)/0x800 || !ReadMemory(list+0x40,&index,2) || !index) return false;
    std::array<std::uint16_t,22> visited{}; std::size_t blocks{},offset{}; std::uintptr_t block{};
    const auto select=[&]() {
        if (!index || index > capacity || blocks >= visited.size() ||
            std::find(visited.begin(),visited.begin()+blocks,index) != visited.begin()+blocks) return false;
        visited[blocks++]=index; block=arena+std::uintptr_t(index-1)*0x800; offset=0; return true;
    };
    if (!select()) return false;
    std::vector<SFMapMarkerV1> pending;
    for (std::uint32_t i=0;i<count;++i) {
        if (offset+0x28 > 0x7f8 && (!ReadMemory(block+2,&index,2) || !select())) return false;
        std::uint8_t event[0x28]{}; SFMapMarkerV1 ping{}; NativeTransform transform{};
        if (!ReadMemory(block+8+offset,event,sizeof(event))) return false;
        offset+=sizeof(event); std::uint32_t id{}; std::memcpy(&id,event+8,4);
        std::memcpy(transform.xyz,event+0x10,sizeof(transform.xyz)); transform.quaternion[3]=1;
        SFMapPoseV1 pose{},camera{};
        if (!id || id > 1023 || id == excluded || !DecodeLocalTransforms(transform,transform,pose,camera)) continue;
        std::uint64_t identity_words[5]{};std::memcpy(identity_words,event,sizeof(identity_words));SetMarkerIdentity(ping,identity_words);
        ping.id=id; std::copy(std::begin(pose.position),std::end(pose.position),ping.position);
        ping.flags=SF_MAP_MARKER_VISIBLE; ping.expires_at_ms=now > UINT64_MAX-8000 ? UINT64_MAX : now+8000;
        pending.push_back(ping);
    }
    result=std::move(pending); return true;
}


namespace {
using IterInit=void(__fastcall*)(void*,void*,std::uint32_t);
using IterNext=bool(__fastcall*)(void*,void*,std::uint32_t);
using GetWorld=std::uintptr_t(__fastcall*)(std::uintptr_t);
using Lookup=void(__fastcall*)(std::uintptr_t*,std::uintptr_t,std::uintptr_t,std::uint32_t);
using WeatherSample=float*(__fastcall*)(float*,void*,const void*,std::int64_t);
using FogUpdate=void(__fastcall*)(void*,const float*,float);
SnapshotStore store;
std::recursive_mutex control_mutex, transient_mutex;
std::atomic<bool> accepting{};
std::atomic<bool> deaths_enabled{};
std::atomic<std::uint64_t> capture_generation{}, last_fog{}, last_remote{}, last_world{}, last_clock{}, last_weather{};
std::unordered_set<std::uint64_t> owners;
std::uintptr_t image_base{};
IterInit iter_init{}; IterNext iter_next{}; GetWorld get_world{}; Lookup lookup{};
WeatherSample weather_sample{}; FogUpdate fog_update{};
MapImageAdmission admission;
// Retained with published camera proofs. Never fetch profile files or allocate
// control-plane metadata while validating an executable replacement.
std::shared_ptr<const KfcRuntimeCompatibility::MapCaptureProfile> map_profile;
const KfcRuntimeCompatibility::MapCaptureProfile::Execution* execution() noexcept {
    return map_profile && map_profile->execution ? &*map_profile->execution : nullptr;
}
const KfcRuntimeCompatibility::MapCaptureProfile::Hook* CameraGuard() noexcept {
    const auto* contract=execution();return contract ? &contract->hooks[0] : nullptr;
}
std::uint64_t source_session{}, next_generation{};
std::uint64_t preparation_generation{};
bool call_proofs_published{};
std::vector<SFMapMarkerV1> pings;
std::vector<EngineHooks::PublishedEngineHook> hooks;
enum class Preparation { Unprepared,Preparing,Ready,Failed };
Preparation preparation{Preparation::Unprepared};
bool preparingCapture{};
const char* preparation_step="not-started";
const char* preparation_reason="none";
std::uintptr_t preparation_rva{};
unsigned preparation_writer_failure{};
DWORD preparation_windows_error{};
bool executable_span_published{};
std::int32_t preparation_result=SF_MAP_UNSUPPORTED_BUILD;
bool publication_result(std::size_t rva,const EngineHooks::HookPublicationResult& result,bool published) {
    preparation_rva=rva;preparation_writer_failure=result.writer_failure;
    preparation_windows_error=result.windows_error;
    if(!published)preparation_reason=result.reason;
    return published;
}

template<class T> bool read(std::uintptr_t address, T& out) { return ReadMemory(address,&out,sizeof(out)); }
bool call_initialize(void* context, void* record, std::uint32_t size) {
    __try { iter_init(context,record,size); return true; }
    __except(EXCEPTION_EXECUTE_HANDLER) { return false; }
}
bool call_advance(void* context, void* record, std::uint32_t size) {
    __try { return iter_next(context,record,size); }
    __except(EXCEPTION_EXECUTE_HANDLER) { return false; }
}
bool initialize(std::uintptr_t (&context)[2],void* record,std::uint32_t capacity,
    NativeIteratorExtent::IteratorExtentPlan* output=nullptr) {
    if(output)*output={};
    if(!iter_init)return false;
    const auto* contract=execution();if(!contract)return false;
    auto plan=NativeIteratorSnapshot::ReadPlan(context,record,capacity,ReadMemory,contract->iterator);
    if(!plan.valid)return false;
    if(output)*output=plan;
    return call_initialize(context,record,capacity);
}
bool advance(std::uintptr_t (&context)[2],void* record,std::uint32_t capacity,
    const NativeIteratorExtent::IteratorExtentPlan& plan,std::uint32_t cursorLimit) {
    if(!iter_next)return false;
    if(!plan.valid || plan.private_context_storage!=reinterpret_cast<std::uintptr_t>(context) ||
       plan.query!=context[0] || plan.destination!=reinterpret_cast<std::uintptr_t>(record) ||
       plan.destination_capacity!=capacity || capacity>NativeIteratorSnapshot::QualifiedMaximumCapacity)return false;
    const auto next=NativeIteratorExtent::PlanNext(plan,static_cast<std::uint32_t>(context[1]),cursorLimit);
    if(next.disposition!=NativeIteratorExtent::NextDisposition::Call)return false;
    const bool result=call_advance(context,record,capacity);
    return context[0]==plan.query && NativeIteratorExtent::ValidateNextAdvance(
        plan,next,result,static_cast<std::uint32_t>(context[1]));
}
bool component(std::uintptr_t context, std::uintptr_t lookup_context, std::uint32_t id, std::uintptr_t& result) {
    __try {
        const auto world=get_world(context); if (!world) return false;
        std::uintptr_t out[2]{}; lookup(out,world,lookup_context,id); result=out[0]; return result != 0;
    } __except(EXCEPTION_EXECUTE_HANDLER) { return false; }
}
bool throttle(std::atomic<std::uint64_t>& last, std::uint64_t now, unsigned interval) {
    auto previous=last.load(std::memory_order_relaxed);
    return now >= previous && now-previous >= interval && last.compare_exchange_strong(previous,now);
}
void __fastcall camera_callback(void* ctx, void*, void*, void*) noexcept {
    if (!accepting.load(std::memory_order_acquire)) return;
    try {
        const auto generation=capture_generation.load();
        std::uintptr_t context[2]{}; NativeCameraRecord record{};
        NativeIteratorExtent::IteratorExtentPlan plan{};
        if (!read(reinterpret_cast<std::uintptr_t>(ctx),context) || !initialize(context,&record,sizeof(record),&plan)) return;
        for (unsigned attempts=0;attempts<256 && advance(context,&record,sizeof(record),plan,256);++attempts) {
            const auto index=std::uint32_t(context[1]); std::uintptr_t entities{}; std::uint32_t entity{};
            if (!index || index > 256 || !read(context[0]+execution()->entity_array,entities) ||
                !read(entities+std::uintptr_t(index-1)*execution()->entity_stride,entity)) return;
            SFMapPoseV1 player{},camera{};
            if (ReadLocalCamera(record,entity,execution()->local_id,player,camera)) {
                store.PublishPlayer(generation,player,camera,GetTickCount64()); return;
            }
        }
    } catch (...) {}
}
void __fastcall remote_callback(void* ctx, void*, void*, void*) noexcept {
    if (!accepting.load(std::memory_order_acquire)) return;
    const auto generation=capture_generation.load(), now=GetTickCount64();
    if (!throttle(last_remote,now,100)) return;
    try {
        std::uintptr_t context[2]{},record[19]{}; std::uint32_t local{};
        if (!read(reinterpret_cast<std::uintptr_t>(ctx),context) || !initialize(context,record,sizeof(record)) ||
            !record[1] || !record[2] || !record[6] || !read(record[2]+execution()->local_id,local) || !local || local > 1023) return;
        std::vector<SFMapMarkerV1> markers;
        for (std::uint32_t id=1;id<=1023 && markers.size()<32;++id) {
            std::uint32_t slot{}; std::uintptr_t address{}; NativeTransform transform{};
            if (id==local || !read(record[1]+std::uintptr_t(id-1)*execution()->remote_stride,slot) || slot != id ||
                !component(context[0],record[6],id,address) || !read(address,transform)) continue;
            SFMapPoseV1 pose{},camera{};
            if (!DecodeLocalTransforms(transform,transform,pose,camera)) continue;
            SFMapMarkerV1 marker{}; marker.id=id; marker.heading_radians=pose.heading_radians;
            marker.flags=SF_MAP_MARKER_VISIBLE | SF_MAP_MARKER_HEADING;
            std::copy(std::begin(pose.position),std::end(pose.position),marker.position); markers.push_back(marker);
        }
        store.PublishMarkers(Collection::RemotePlayers,generation,std::move(markers),now);
    } catch (...) {}
}
void __fastcall waypoints_callback(void* ctx, void*, void*, void*) noexcept {
    if (!accepting.load(std::memory_order_acquire)) return;
    const auto generation=capture_generation.load(), now=GetTickCount64();
    try {
        std::uintptr_t context[2]{},record[6]{}; std::uint32_t local{};
        if (!read(reinterpret_cast<std::uintptr_t>(ctx),context) || !initialize(context,record,sizeof(record)) ||
            !read(record[1]+execution()->local_id,local) || !local || local>1023) return;
        std::vector<SFMapMarkerV1> input,received;
        const bool valid_received=ReadPingEvents(record[4],local,now,received);
        const bool valid_input=ReadPingEvents(record[5],0,now,input);
        if (valid_received && valid_input) {
            std::lock_guard lock(transient_mutex);
            if (capture_generation.load() == generation) {
                std::erase_if(pings,[&](const auto& ping){ return now >= ping.expires_at_ms; });
                for (const auto& list:{&received,&input}) for (const auto& ping:*list) {
                    const auto existing=std::find_if(pings.begin(),pings.end(),[&](const auto& value){return value.id==ping.id;});
                    if (existing != pings.end()) *existing=ping; else if (pings.size()<32) pings.push_back(ping);
                }
                store.PublishMarkers(Collection::Pings,generation,pings,now);
            }
        }
        std::vector<SFMapMarkerV1> waypoints; std::uintptr_t address{}; std::uint8_t enabled{};
        if (!component(context[0],record[3],local,address) || !read(address,enabled)) return;
        if (enabled) {
            NativeTransform transform{}; SFMapPoseV1 pose{},camera{}; transform.quaternion[3]=1;
            if (!ReadMemory(address+execution()->waypoint_position,transform.xyz,sizeof(transform.xyz)) || !DecodeLocalTransforms(transform,transform,pose,camera)) return;
            SFMapMarkerV1 marker{}; marker.id=local; marker.kind=10; marker.flags=SF_MAP_MARKER_VISIBLE;
            std::copy(std::begin(pose.position),std::end(pose.position),marker.position); waypoints.push_back(marker);
        }
        store.PublishMarkers(Collection::Waypoints,generation,std::move(waypoints),now);
    } catch (...) {}
}
void __fastcall clock_callback(void* ctx, void*, void*, void*) noexcept {
    if (!accepting.load(std::memory_order_acquire)) return;
    const auto generation=capture_generation.load(),now=GetTickCount64();
    if (!throttle(last_clock,now,100)) return;
    try {
        std::uintptr_t context[2]{},record[5]{}; std::int64_t fields[5]{}; SFMapEnvironmentV1 environment{};
        if (read(reinterpret_cast<std::uintptr_t>(ctx),context) && initialize(context,record,sizeof(record)) &&
            read(record[2]+execution()->clock_values,fields) && DecodeClock(fields,environment)) store.PublishEnvironment(generation,environment,true,now);
    } catch (...) {}
}
float* __fastcall weather_callback(float* output,void* weather,const void* position,std::int64_t time) noexcept {
    const auto generation=capture_generation.load();
    auto* result=weather_sample(output,weather,position,time);
    if (!accepting.load(std::memory_order_acquire)) return result;
    const auto now=GetTickCount64(); if (!throttle(last_weather,now,100)) return result;
    try { float weights[4]{}; SFMapEnvironmentV1 environment{};
        if (read(reinterpret_cast<std::uintptr_t>(result),weights) && DecodeWeather(weights,environment))
            store.PublishEnvironment(generation,environment,false,now);
    } catch (...) {}
    return result;
}
bool FogEntryValidated(std::uint32_t phase,std::uint8_t identity[SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES]) {
    constexpr std::size_t prefix_bytes=15; // checked fog callback ABI v1
    const std::uint8_t* expected{};
    if(!execution())return false;
    for(const auto& guard:map_profile->views[5].guards)
        if(guard.rva==execution()->hooks[5].target && guard.bytes.size()>=prefix_bytes)expected=guard.bytes.data();
    if(!expected)return false;
    std::array<std::uint8_t,prefix_bytes> actual{};
    if(!fog_update || !image_base || !ReadMemory(reinterpret_cast<std::uintptr_t>(fog_update),actual.data(),actual.size()))return false;
    const auto address=reinterpret_cast<std::uintptr_t>(fog_update);
    if(address<image_base || address-image_base>UINT32_MAX)return false;
    return ExecutableSpanValidation::Services().ValidateGuard(static_cast<std::uint32_t>(address-image_base),
        expected,actual.data(),actual.size(),phase,identity);
}
void __fastcall fog_callback(void* fog,const float* position,float radius) noexcept {
    const auto generation=capture_generation.load();
    const auto now=GetTickCount64();
    const bool sample=accepting.load(std::memory_order_acquire) && throttle(last_fog,now,500);
    std::array<std::uint8_t,SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES> identity_before{};
    const bool owner_before=FogEntryValidated(SF_EXECUTABLE_SPAN_PHASE_BEFORE_ORIGINAL,identity_before.data());
    fog_update(fog,position,radius);
    std::array<std::uint8_t,SF_EXECUTABLE_SPAN_VALIDATION_MAX_BYTES> identity_after{};
    const bool owner_after=FogEntryValidated(SF_EXECUTABLE_SPAN_PHASE_AFTER_ORIGINAL,identity_after.data());
    if (!sample || !accepting.load(std::memory_order_acquire) || generation!=capture_generation.load() ||
        !owner_before || !owner_after || identity_before!=identity_after) return;
    try { auto snapshot=std::make_shared<FogSnapshot>();
        if (ReadFog(reinterpret_cast<std::uintptr_t>(fog),*snapshot)) store.PublishFog(generation,std::move(snapshot),now);
    } catch (...) {}
}
bool world_array(std::uintptr_t state,std::size_t offset,bool custom,std::vector<SFMapMarkerV1>& markers,unsigned maximum=2048) {
    std::uintptr_t entries{}; std::uint64_t count{};
    if (!state || !read(state+offset,entries) || !read(state+offset+8,count) || count>maximum) return false;
    std::vector<std::uint8_t> bytes(std::size_t(count)*execution()->marker_stride);
    if (count && !ReadMemory(entries,bytes.data(),bytes.size())) return false;
    return DecodeWorldMarkers(bytes,std::size_t(count),custom,markers);
}
void __fastcall world_callback(void* ctx,void*,void*,void*) noexcept {
    if (!accepting.load(std::memory_order_acquire)) return;
    const auto generation=capture_generation.load(),now=GetTickCount64(); if (!throttle(last_world,now,100)) return;
    try {
        std::uintptr_t context[2]{},record[2]{};
        if (!read(reinterpret_cast<std::uintptr_t>(ctx),context) || !initialize(context,record,sizeof(record))) return;
        std::vector<SFMapMarkerV1> world,secondary,custom,deaths;
        if (world_array(record[1],execution()->custom_list,true,custom) &&
            world_array(record[1],execution()->world_list,false,world) &&
            (!execution()->secondary_list || world_array(record[1],execution()->secondary_list,false,secondary))) {
            // Each existing decoder remains capped at2048 records. Normal and
            // custom categories/identities remain intact for consumer dedupe.
            world.insert(world.end(),secondary.begin(),secondary.end());
            world.insert(world.end(),custom.begin(),custom.end()); store.PublishMarkers(Collection::World,generation,std::move(world),now);
        }
        if (deaths_enabled.load() && world_array(record[1],execution()->death_list,false,deaths,64))
            store.PublishMarkers(Collection::Deaths,generation,std::move(deaths),now);
    } catch (...) {}
}

bool entry_hook(std::size_t rva,std::span<const std::uint8_t> original,void* callback) {
    EngineHooks::HookPublicationResult result;
    const auto published=EngineHooks::PublishEntryHook(hooks,image_base+rva,original,callback,result);
    return publication_result(rva,result,published);
}
bool call_hook(std::size_t rva,std::size_t original_target,void* callback) {
    EngineHooks::HookPublicationResult result;
    const auto published=EngineHooks::PublishCallHook(hooks,image_base+rva,image_base+original_target,callback,result);
    return publication_result(rva,result,published);
}
bool ReadCallProof(std::uintptr_t source,void* output,std::size_t size) noexcept {
    return ReadMemory(source,output,size);
}
bool CallProofRegion(std::uintptr_t at,std::size_t size,std::uintptr_t allocation,bool image) noexcept {
    MEMORY_BASIC_INFORMATION region{};
    if(!at || VirtualQuery(reinterpret_cast<void*>(at),&region,sizeof(region))!=sizeof(region) ||
        region.State!=MEM_COMMIT || reinterpret_cast<std::uintptr_t>(region.AllocationBase)!=allocation ||
        (region.Protect&(PAGE_GUARD|PAGE_NOACCESS)) || at<reinterpret_cast<std::uintptr_t>(region.BaseAddress))return false;
    const auto offset=at-reinterpret_cast<std::uintptr_t>(region.BaseAddress);
    return offset<=region.RegionSize && size<=region.RegionSize-offset &&
        (region.Protect&0xff)==PAGE_EXECUTE_READ && (!image || region.Type==MEM_IMAGE);
}
bool CallProofResident(std::uintptr_t handler,std::uintptr_t expected) noexcept {
    HMODULE actual{};
    return GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS|GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        reinterpret_cast<LPCWSTR>(handler),&actual) && reinterpret_cast<std::uintptr_t>(actual)==expected;
}
bool VerifyCallProof(const void* opaque) noexcept {
    return opaque && EngineHooks::VerifySharedCallWitness(
        *static_cast<const EngineHooks::SharedCallWitness*>(opaque),ReadCallProof,CallProofRegion,CallProofResident);
}
bool PublishQualifiedCallProofs() {
    unsigned published{};
    for(const auto& hook:hooks) {
        if(hook.original.size()!=5 || hook.replacement.size()!=5 || hook.relay.size()!=14)continue;
        if(!hook.installed || !hook.publicationQualified || !hook.residentOwner ||
            hook.target<image_base || hook.target-image_base>UINT32_MAX)return false;
        auto witness=std::make_shared<EngineHooks::SharedCallWitness>();
        witness->image=image_base;witness->target=hook.target;witness->allocation=hook.allocation;
        witness->resident=reinterpret_cast<std::uintptr_t>(hook.residentOwner);
        std::memcpy(witness->original.data(),hook.original.data(),5);
        std::memcpy(witness->replacement.data(),hook.replacement.data(),5);
        std::memcpy(witness->relay.data(),hook.relay.data(),14);
        std::int32_t relative{};std::memcpy(&relative,witness->original.data()+1,4);
        witness->original_target=std::uintptr_t(std::int64_t(hook.target)+5+relative);
        std::memcpy(&witness->handler,witness->relay.data()+6,8);
        if(!VerifyCallProof(witness.get()) || !EngineHooks::ExecutableProofs().Publish(
            static_cast<std::uint32_t>(hook.target-image_base),hook.original,hook.replacement,
            witness,VerifyCallProof))return false;
        ++published;
    }
    return published!=0;
}

}
SnapshotStore& CapturedFrames() { return store; }
bool ReadOwnedCameraEntryReplacement(std::span<const std::uint8_t> original,
    std::span<std::uint8_t> replacement) noexcept {
    try {
        std::lock_guard lock(control_mutex);
        if(original.size()!=18 || replacement.size()!=18 || !original.data() || !replacement.data() ||
           !CameraGuard() || !image_base || image_base>UINTPTR_MAX-(CameraGuard()->rva+CameraGuard()->original.size()))return false;
        const auto target=image_base+CameraGuard()->rva;
        const auto* selected=CameraGuard();
        if(!selected || std::memcmp(original.data(),selected->original.data(),18))return false;
        for(const auto& hook:hooks)if(hook.target==target){
            if(!hook.installed || !hook.publicationQualified || !hook.residentOwner || !hook.allocation ||
               hook.original.size()!=18 || hook.replacement.size()!=18 || hook.relay.size()<57 || hook.relay.size()>128 ||
               std::memcmp(original.data(),hook.original.data(),18) || hook.replacement[0]!=0xe9 ||
               !std::all_of(hook.replacement.begin()+5,hook.replacement.end(),[](auto b){return b==0x90;}))return false;
            HMODULE actualOwner{};
            if(!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS|GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                reinterpret_cast<LPCWSTR>(&ReadOwnedCameraEntryReplacement),&actualOwner) || actualOwner!=hook.residentOwner)return false;
            MEMORY_BASIC_INFORMATION info{};
            if(VirtualQuery(reinterpret_cast<void*>(hook.allocation),&info,sizeof(info))!=sizeof(info) ||
               info.State!=MEM_COMMIT || info.AllocationBase!=reinterpret_cast<void*>(hook.allocation) ||
               (info.Protect&(PAGE_GUARD|PAGE_NOACCESS)) || info.RegionSize<hook.relay.size())return false;
            const auto protection=info.Protect&0xff;
            if(protection!=PAGE_EXECUTE_READ)return false;
            std::array<std::uint8_t,18> live{};std::array<std::uint8_t,128> bridge{};
            if(!ReadMemory(target,live.data(),live.size()) || std::memcmp(live.data(),hook.replacement.data(),18) ||
               !ReadMemory(hook.allocation,bridge.data(),hook.relay.size()) || std::memcmp(bridge.data(),hook.relay.data(),hook.relay.size()))return false;
            std::int32_t entryDelta{},returnDelta{};std::uintptr_t handler{};
            std::memcpy(&entryDelta,live.data()+1,4);std::memcpy(&handler,bridge.data()+26,8);
            const auto returnOffset=hook.relay.size()-5;
            if(bridge[returnOffset]!=0xe9 || std::memcmp(bridge.data()+returnOffset-18,original.data(),18))return false;
            std::memcpy(&returnDelta,bridge.data()+returnOffset+1,4);
            if(std::int64_t(target)+5+entryDelta!=std::int64_t(hook.allocation) || handler!=reinterpret_cast<std::uintptr_t>(&camera_callback) ||
               std::int64_t(hook.allocation)+hook.relay.size()+returnDelta!=std::int64_t(image_base+CameraGuard()->rva+CameraGuard()->original.size()))return false;
            std::memcpy(replacement.data(),live.data(),18);return true;
        }
    }catch(...){return false;}
    return false;
}
bool SF_EXECUTABLE_SPAN_CALL verify_owned_camera_entry_span(std::uint32_t rva,const std::uint8_t* original,
    const std::uint8_t* replacement,std::uint32_t byte_count) noexcept {
    if(!original || !replacement || byte_count!=18u)return false;
    {std::lock_guard lock(control_mutex);if(!CameraGuard() || rva!=CameraGuard()->rva)return false;}
    std::array<std::uint8_t,18> verified{};
    return ReadOwnedCameraEntryReplacement({original,byte_count},verified) &&
        !std::memcmp(verified.data(),replacement,byte_count);
}
void StopCapture() noexcept {
    try {
        std::lock_guard control(control_mutex);
        EngineHooks::ExecutableProofs().Close();
        ++preparation_generation;
        accepting.store(false,std::memory_order_release);capture_generation.store(0);
        preparation=Preparation::Failed;preparation_reason="stopped";preparation_result=SF_MAP_DISABLED;
        DisableHudVisibility();store.Disable();owners.clear();
        std::lock_guard transient(transient_mutex);pings.clear();source_session=0;
        // No restoration, relay freeing, module release, or ownership retirement.
    }catch(...){accepting.store(false,std::memory_order_release);capture_generation.store(0);}
}


bool InstallSharedCall(std::size_t rva,std::size_t target,void* callback) {
    std::lock_guard lock(control_mutex);return (accepting.load() || preparingCapture) && callback && call_hook(rva,target,callback);
}
bool SharedCallOwned(std::size_t rva,std::size_t target,void* callback) {
    std::lock_guard lock(control_mutex);
    for(const auto& hook:hooks)if(hook.target==image_base+rva && hook.installed && hook.relay.size()==14 && hook.original.size()==5) {
        std::uint8_t bytes[14]{},actual[5]{};std::int32_t relative{};std::uintptr_t handler{};
        std::memcpy(&relative,hook.original.data()+1,4);std::memcpy(&handler,hook.relay.data()+6,8);
        return hook.original[0]==0xe8 && std::int64_t(rva)+5+relative==std::int64_t(target) && handler==reinterpret_cast<std::uintptr_t>(callback) &&
            ReadMemory(hook.target,actual,5) && !std::memcmp(actual,hook.replacement.data(),5) &&
            ReadMemory(hook.allocation,bytes,14) && !std::memcmp(bytes,hook.relay.data(),14);
    }return false;
}
std::string PreparationStatus() {
    std::lock_guard lock(control_mutex);
    return std::string("step=")+preparation_step+",reason="+preparation_reason+
        ",result="+std::to_string(preparation_result)+",rva="+std::to_string(preparation_rva)+
        ",writer="+std::to_string(preparation_writer_failure)+",win32="+std::to_string(preparation_windows_error)+
        ",admission="+admission.failure_reason+",guard_rva="+std::to_string(admission.failure_rva)+
        ",expected="+std::to_string(admission.expected_byte)+",observed="+std::to_string(admission.observed_byte)+
        ",features="+std::to_string(admission.Features())+",renderer="+std::to_string(admission.renderer)+
        ",hud="+HudPreparationStatus()+",executable-span="+(executable_span_published?"published":"unavailable");
}
bool CapturePrepared(){std::lock_guard lock(control_mutex);return preparation==Preparation::Ready;}
std::int32_t PrepareCapture(const EngineHooks::HeldEntryToken& token) noexcept {
    bool proof_ready{};
    struct ProofPreparation {
        bool& ready;
        ~ProofPreparation(){if(!ready && !hooks.empty())EngineHooks::ExecutableProofs().Close();}
    } proof_preparation{proof_ready};
    try {
        auto selected_map = EngineContext::ConfiguredMapCaptureProfile();
        std::unique_lock control(control_mutex);
        preparation_step="preconditions";preparation_reason="none";preparation_result=SF_MAP_FAILED;
        if(!token.ValidForCurrentThread()){preparation_reason="invalid-token";return SF_MAP_FAILED;}
        if(preparation==Preparation::Ready){
            std::array<std::uint8_t,18> original{},replacement{};
            if(const auto* guard=CameraGuard())std::memcpy(original.data(),guard->original.data(),18);
            preparation_step="owned-camera-readback";
            const bool owned=ReadOwnedCameraEntryReplacement(original,replacement);
            preparation_reason=owned?(executable_span_published?"none":"owned-span-unavailable"):"owned-camera-rejected";
            proof_ready=owned && call_proofs_published;
            preparation_result=proof_ready?SF_MAP_READY:SF_MAP_FAILED;return preparation_result;
        }
        if(preparation!=Preparation::Unprepared || accepting || !hooks.empty()){preparation_reason="prior-preparation-or-publication";return SF_MAP_FAILED;}
        map_profile = std::move(selected_map.profile);
        if(!map_profile || !map_profile->execution){preparation_reason=map_profile ? map_profile->execution_status.c_str() : "map-capture-profile-unavailable";preparation_result=SF_MAP_UNSUPPORTED_BUILD;return SF_MAP_UNSUPPORTED_BUILD;}
        if(!CameraGuard()){preparation_reason="map-capture-profile-unavailable";preparation_result=SF_MAP_UNSUPPORTED_BUILD;return SF_MAP_UNSUPPORTED_BUILD;}
        preparation=Preparation::Preparing;
        executable_span_published=false;
        image_base=reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
        const auto attempt=++preparation_generation;
        if(!image_base){preparation=Preparation::Failed;preparation_reason="image-base";return SF_MAP_FAILED;}
        preparation_step="image-admission";
        const auto image=std::span<const std::uint8_t>(reinterpret_cast<const std::uint8_t*>(image_base),map_profile->image_size);
        control.unlock();
        auto candidate=InspectMapImage(image,map_profile->image_sha256,true);
        const bool hud_preflight=candidate.status==SF_MAP_READY && PreflightHudPreparation(image_base,map_profile->image_size);
        control.lock();
        if(attempt!=preparation_generation || preparation!=Preparation::Preparing){
            DisableHudVisibility();preparation=Preparation::Failed;preparation_reason="preparation-superseded";
            preparation_result=SF_MAP_FAILED;return preparation_result;
        }
        if(!token.ValidForCurrentThread()){preparation=Preparation::Failed;preparation_reason="invalid-token-after-validation";return SF_MAP_FAILED;}
        admission=std::move(candidate);
        admission.discovery=false; // No unproved position scanning or discovery hook.
        if(admission.status!=SF_MAP_READY){preparation=Preparation::Failed;preparation_reason=admission.failure_reason;preparation_result=admission.status;return admission.status;}
        if(!hud_preflight){preparation=Preparation::Failed;preparation_reason="hud-preflight";preparation_result=SF_MAP_UNSUPPORTED_BUILD;return preparation_result;}
        preparation_step="module-residency";
        HMODULE resident{};
        if(!token.ValidForCurrentThread() || !GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS|GET_MODULE_HANDLE_EX_FLAG_PIN,
            reinterpret_cast<LPCWSTR>(&PrepareCapture),&resident)){preparation=Preparation::Failed;preparation_reason="token-or-module-pin";preparation_windows_error=GetLastError();return SF_MAP_FAILED;}
        const auto& contract=*execution();
        iter_init=reinterpret_cast<IterInit>(image_base+contract.iter_init);iter_next=reinterpret_cast<IterNext>(image_base+contract.iter_next);
        get_world=reinterpret_cast<GetWorld>(image_base+contract.get_world);lookup=reinterpret_cast<Lookup>(image_base+contract.lookup);
        weather_sample=reinterpret_cast<WeatherSample>(image_base+contract.hooks[4].target);fog_update=reinterpret_cast<FogUpdate>(image_base+contract.hooks[5].target);
        struct Scope{Scope(){preparingCapture=true;}~Scope(){preparingCapture=false;}} scope;
        const char* steps[]{"camera","waypoints","remote","clock","weather","fog","world"};
        void* callbacks[]{reinterpret_cast<void*>(&camera_callback),reinterpret_cast<void*>(&waypoints_callback),
            reinterpret_cast<void*>(&remote_callback),reinterpret_cast<void*>(&clock_callback),reinterpret_cast<void*>(&weather_callback),
            reinterpret_cast<void*>(&fog_callback),reinterpret_cast<void*>(&world_callback)};
        bool complete=true;
        for(std::size_t i=0;i<contract.hooks.size() && complete;++i){
            preparation_step=steps[i];preparation_reason="operation-refused";preparation_rva=0;
            preparation_writer_failure=0;preparation_windows_error=0;
            const auto& hook=contract.hooks[i];
            complete=token.ValidForCurrentThread() && (i==4 || i==5 ?
                call_hook(hook.rva,hook.target,callbacks[i]) : entry_hook(hook.rva,hook.original,callbacks[i]));
        }
        if(complete){preparation_step="hud";complete=token.ValidForCurrentThread() && InstallPreparedHudHooks();}
        if(!complete){preparation=Preparation::Failed;accepting=false;capture_generation=0;return SF_MAP_FAILED;}
        // The pinned shared writer guarantees false=no publication/verified rollback;
        // its indeterminate branch fails fast. Never use the old79f writer here.
        for(auto& hook:hooks){hook.residentOwner=resident;hook.publicationQualified=true;}
        call_proofs_published=PublishQualifiedCallProofs();
        if(!call_proofs_published){preparation=Preparation::Failed;preparation_reason="call-proof-publication";return SF_MAP_FAILED;}
        const auto camera_hook=std::find_if(hooks.begin(),hooks.end(),[](const auto& hook){return hook.target==image_base+CameraGuard()->rva;});
        if(camera_hook!=hooks.end() && camera_hook->installed && camera_hook->publicationQualified && camera_hook->target>=image_base &&
           camera_hook->target-image_base<=UINT32_MAX && camera_hook->original.size()==18 && camera_hook->replacement.size()==18) {
            executable_span_published=EngineHooks::PublishOwnedExecutableSpan(
                static_cast<std::uint32_t>(camera_hook->target-image_base),camera_hook->original,camera_hook->replacement,
                verify_owned_camera_entry_span);
        }
        proof_ready=true;
        preparation=Preparation::Ready;preparation_step="ready";
        preparation_reason=executable_span_published?"none":"owned-span-unavailable";preparation_result=SF_MAP_READY;
        capture_generation=0;source_session=0;deaths_enabled=admission.deaths;
        return SF_MAP_READY;
    }catch(...){
        std::lock_guard control(control_mutex);
        ++preparation_generation;preparation_reason="exception";preparation_result=SF_MAP_FAILED;
        preparingCapture=false;preparation=Preparation::Failed;accepting=false;capture_generation=0;
        DisableHudVisibility();return SF_MAP_FAILED;
    }
}


std::int32_t EnableCapture(std::uint64_t owner,bool capability) {
    if(!owner || !capability)return SF_MAP_CAPABILITY_DENIED;
    std::lock_guard control(control_mutex);
    if(owners.contains(owner))return SF_MAP_INVALID_ARGUMENT;
    if(!owners.empty()){owners.insert(owner);return SF_MAP_READY;}
    if(preparation!=Preparation::Ready || !CameraGuard())return SF_MAP_UNSUPPORTED_BUILD;
    std::array<std::uint8_t,18> replacement{};
    if(!ReadOwnedCameraEntryReplacement(CameraGuard()->original,replacement) || !ActivatePreparedHudVisibility())return SF_MAP_FAILED;
    capture_generation=0;source_session=0;store.SetAdmission(admission.Features()|SF_MAP_FEATURE_HUD);
    owners.insert(owner);accepting.store(true,std::memory_order_release);return SF_MAP_READY;
}
void DisableCapture(std::uint64_t owner) {
    std::lock_guard control(control_mutex);
    if (!owners.erase(owner) || !owners.empty()) return;
    DisableHudVisibility();accepting.store(false,std::memory_order_release); capture_generation.store(0); store.Disable();
    std::lock_guard transient(transient_mutex); pings.clear(); source_session=0;
}
void UpdateCaptureSession(std::uint64_t session,bool gameplay,bool menu,bool focused) {
    std::lock_guard control(control_mutex);
    if (!accepting.load()) return;
    if (source_session != session) {
        capture_generation.store(0); source_session=session;
        const auto next=session ? ++next_generation : 0;
        store.SetSession(next,gameplay,menu,focused,GetTickCount64());
        std::lock_guard transient(transient_mutex); pings.clear();
        capture_generation.store(next);
        last_fog=last_remote=last_world=last_clock=last_weather=0;
    } else store.SetSession(capture_generation.load(),gameplay,menu,focused,GetTickCount64());
}
}
