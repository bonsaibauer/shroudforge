#include "graphics/render_overlay.h"
#include "map/capture_game_map.h"
#include "map/hud_visibility.h"
#include <windows.h>
#include <chrono>
#include <atomic>
#include <cstring>
#include <vector>

namespace OverlayRuntime {
struct Registry::Owner {
    Registry* registry{};
    std::string package;
    std::uint64_t id{},generation{},registration{},next_lease{},next_registration{};
    std::mutex mutex;
    std::condition_variable changed;
    bool revoked{};
    unsigned in_flight{};
    std::uint32_t overlay_version=1;
    struct EncoderGate {
        std::atomic<bool> active{};
        std::atomic<DWORD> thread{};
        std::uint64_t owner{};
        SFOverlayEncoderV1 backend{};
        SFOverlayEncoderV2 extended{};
        SFOverlayEncoderV3 programmable{};
        bool limited{};std::uint32_t remaining{};
        bool Valid(std::uint64_t id) const {return thread.load()==GetCurrentThreadId() && active.load(std::memory_order_acquire) && id==owner;}
        bool Reserve(std::uint32_t count){if(!limited)return true;if(count>remaining)return false;remaining-=count;return true;}
        static std::int32_t Rectangles(void* opaque,std::uint64_t id,const SFOverlayRectangleV1* values,std::uint32_t count) {
            auto& gate=*static_cast<EncoderGate*>(opaque);if(!gate.Valid(id))return 3;
            if(!gate.Reserve(count))return 10;
            try {return gate.backend.draw_rectangles ? gate.backend.draw_rectangles(gate.backend.context,id,values,count) : 3;}catch(...){return 7;}
        }
        static std::int32_t Textures(void* opaque,std::uint64_t id,const SFOverlayTextureCommandV1* values,std::uint32_t count) {
            auto& gate=*static_cast<EncoderGate*>(opaque);if(!gate.Valid(id))return 3;
            if(!gate.Reserve(count))return 10;
            try {return gate.backend.draw_textures ? gate.backend.draw_textures(gate.backend.context,id,values,count) : 3;}catch(...){return 7;}
        }

        static std::int32_t TexturesV2(void* opaque,std::uint64_t id,const SFOverlayTextureCommandV2* values,std::uint32_t count) {
            auto& gate=*static_cast<EncoderGate*>(opaque);if(!gate.Valid(id))return 3;
            if(!gate.Reserve(count))return 10;
            try {return gate.extended.draw_textures_v2 ? gate.extended.draw_textures_v2(gate.backend.context,id,values,count) : 3;}catch(...){return 7;}
        }

        static std::int32_t Programs(void* opaque,std::uint64_t id,const SFOverlayProgramCommandV1* values,std::uint32_t count) {
            auto& gate=*static_cast<EncoderGate*>(opaque);if(!gate.Valid(id))return 3;
            if(!gate.limited)return 3; // V3 negotiation is required even if a caller copies beyond its base table.
            if(!gate.Reserve(count))return 10;
            try{return gate.programmable.draw_programs?gate.programmable.draw_programs(gate.backend.context,id,values,count):3;}catch(...){return 7;}
        }

    } gate;
    SFOverlayFrameCallbackV1 callback{};
    void* callback_context{};
    std::unordered_map<std::uint64_t,MapRuntime::FrameLease> leases;
};
std::shared_ptr<Registry::Owner> Registry::Find(std::uint64_t id) {
    std::lock_guard lock(mutex_); const auto found=owners_.find(id);
    return found==owners_.end() ? nullptr : found->second;
}
std::uint64_t Registry::Open(const std::string& package,std::uint64_t generation) {
    if(package.empty() || package.size()>256 || !generation) return 0;
    std::lock_guard lock(mutex_);
    for(const auto& [id,owner]:owners_) if(owner->package==package && owner->generation==generation) return id;
    if(owners_.size()>=4096) return 0;
    auto owner=std::make_shared<Owner>(); owner->registry=this; owner->package=package;
    owner->id=++next_owner_; owner->generation=generation;
    owners_.emplace(owner->id,owner); return owner->id;
}
std::int32_t Registry::Query(std::uint64_t id,const std::string& name,std::uint32_t version,std::uint32_t capacity,void* output) {
    if(version!=1 && !((version==2 || version==3) && name=="shroudforge.overlay")) return 2;
    if(!output) return 1;
    if(name!="shroudforge.map" && name!="shroudforge.overlay") return 4;
    if(capacity < (name=="shroudforge.map" ? sizeof(SFMapServiceV1) : sizeof(SFOverlayServiceV1))) return 5;
    auto owner=Find(id); if(!owner) return 1;
    std::lock_guard lock(owner->mutex); if(owner->revoked) return 3;
    if(name=="shroudforge.map") {
        SFMapServiceV1 table{sizeof(table),1,owner.get(),Acquire,Release,Keys}; std::memcpy(output,&table,sizeof(table));
    } else {
        if(owner->overlay_version!=version && (owner->callback || owner->in_flight))return 6;
        owner->overlay_version=version;
        SFOverlayServiceV1 table{sizeof(table),version,owner.get(),Register,Unregister,Status}; std::memcpy(output,&table,sizeof(table));
    }
    return 0;
}
std::int32_t Registry::Acquire(void* context,std::uint64_t generation,std::uint32_t capacity,SFMapFrameV1* frame,SFMapFrameLeaseV1* lease) {
    if(!context || !frame || !lease) return 1;
    if(capacity<sizeof(*frame)) return 5;
    auto& owner=*static_cast<Owner*>(context);
    try {
        std::lock_guard lock(owner.mutex); if(owner.revoked) return 3;
        if(owner.leases.size()>=32 || owner.next_lease>=UINT32_MAX) return 6;
        auto captured=owner.registry->frames_.Acquire(generation,GetTickCount64());
        const auto token=(owner.id<<32)|++owner.next_lease;
        auto inserted=owner.leases.emplace(token,std::move(captured));
        *frame=inserted.first->second.frame; *lease=token; return 0;
    } catch(...) { return 7; }
}
std::int32_t Registry::Release(void* context,SFMapFrameLeaseV1 lease) {
    if(!context || !lease) return 1; auto& owner=*static_cast<Owner*>(context);
    std::lock_guard lock(owner.mutex);
    if(!owner.leases.erase(lease)) return 1;
    owner.changed.notify_all(); return 0;
}
std::int32_t Registry::Register(void* context,SFOverlayFrameCallbackV1 callback,void* data,SFOverlayRegistrationV1* registration) {
    if(!context || !callback || !registration) return 1; auto& owner=*static_cast<Owner*>(context);
    std::lock_guard lock(owner.mutex); if(owner.revoked) return 3;
    if(owner.callback || owner.in_flight || owner.next_registration>=UINT32_MAX) return 1;
    owner.registration=(owner.id<<32)|++owner.next_registration; owner.callback=callback; owner.callback_context=data;
    *registration=owner.registration; return 0;
}
std::int32_t Registry::Unregister(void* context,SFOverlayRegistrationV1 registration) {
    if(!context || !registration) return 1; auto& owner=*static_cast<Owner*>(context);
    std::lock_guard lock(owner.mutex); if(owner.registration!=registration) return 1;
    owner.callback=nullptr; owner.callback_context=nullptr;
    return owner.in_flight ? 6 : 0;
}
std::int32_t Registry::Status(void* context,std::uint32_t capacity,SFOverlayStatusV1* status) {
    if(!context || !status) return 1; if(capacity<sizeof(*status)) return 5;
    auto& owner=*static_cast<Owner*>(context); std::lock_guard lock(owner.mutex);
    if(owner.revoked) return 3; *status=GraphicsStatus(); status->version=owner.overlay_version; return 0;
}
std::int32_t Registry::Keys(void* context,const std::uint32_t* keys,std::uint32_t count,std::uint32_t capacity,SFInputKeyStateV1* output) {
    if(!context) return 1; auto& owner=*static_cast<Owner*>(context);
    std::lock_guard lock(owner.mutex); if(owner.revoked) return 3; return SampleKeys(keys,count,capacity,output);
}
std::int32_t Registry::Revoke(std::uint64_t id) {
    auto owner=Find(id); if(!owner) return 0;
    std::lock_guard lock(owner->mutex); owner->revoked=true; owner->gate.active.store(false,std::memory_order_release); owner->callback=nullptr; owner->callback_context=nullptr;
    owner->changed.notify_all(); return 0;
}
std::int32_t Registry::Drain(std::uint64_t id,std::uint32_t timeout) {
    if(timeout>250) return 1; auto owner=Find(id); if(!owner) return 0;
    std::unique_lock lock(owner->mutex);
    const auto drained=[&]{return !owner->in_flight && owner->leases.empty();};
    if(!owner->changed.wait_for(lock,std::chrono::milliseconds(timeout),drained)) return 6; return 0;
}
bool Registry::Dispatch(std::uint32_t width,std::uint32_t height,std::uint64_t now,std::uint64_t generation,const SFOverlayEncoderV1& input,const SFMapFrameV1* supplied_frame,SFOverlayFrameV3 target) {
    std::unique_lock dispatch_lock(dispatch_mutex_,std::try_to_lock); if(!dispatch_lock.owns_lock())return false;
    bool success=true;
    std::vector<std::shared_ptr<Owner>> owners;
    {std::lock_guard lock(mutex_); for(const auto& [_,owner]:owners_) owners.push_back(owner);}
    auto lease=frames_.Acquire(0,now);
    auto& frame=target.base;frame={sizeof(frame),1,width,height,generation,supplied_frame ? supplied_frame : &lease.frame};
    for(const auto& owner:owners) {
        SFOverlayFrameCallbackV1 callback{}; void* context{};
        {std::lock_guard lock(owner->mutex); if(owner->revoked || !owner->callback) continue;
            ++owner->in_flight; callback=owner->callback; context=owner->callback_context;frame.version=owner->overlay_version;}
        auto& gate=owner->gate;gate.backend=input;gate.owner=owner->id;gate.thread=GetCurrentThreadId();gate.active.store(true,std::memory_order_release);
        gate.extended={};gate.programmable={};gate.limited=frame.version==3;gate.remaining=SF_OVERLAY_MAX_PROGRAM_DRAWS;
        if(input.version>=2 && input.size>=sizeof(SFOverlayEncoderV2))gate.extended=*reinterpret_cast<const SFOverlayEncoderV2*>(&input);
        if(input.version==3 && input.size>=sizeof(SFOverlayEncoderV3))gate.programmable=*reinterpret_cast<const SFOverlayEncoderV3*>(&input);
        const auto encoder_size=frame.version==3?sizeof(SFOverlayEncoderV3):frame.version==2?sizeof(SFOverlayEncoderV2):sizeof(SFOverlayEncoderV1);
        frame.size=frame.version==3?sizeof(target):sizeof(frame);
        const SFOverlayEncoderV3 encoder{{{std::uint32_t(encoder_size),frame.version,&gate,owner->id,generation,Owner::EncoderGate::Rectangles,Owner::EncoderGate::Textures},Owner::EncoderGate::TexturesV2},Owner::EncoderGate::Programs};
        std::int32_t result=7;try{
            if((frame.version==1 || gate.extended.draw_textures_v2) && (frame.version!=3 || gate.programmable.draw_programs))
                result=callback(context,&frame,&encoder.base.base);
        }catch(...){}
        gate.active.store(false,std::memory_order_release);
        {std::lock_guard lock(owner->mutex); --owner->in_flight;
            if(result!=0) {success=false;owner->revoked=true;owner->callback=nullptr;} owner->changed.notify_all();}
    }
    return success;
}
Registry& Services() { static auto* registry=new Registry(MapRuntime::CapturedFrames()); return *registry; }
std::int32_t SampleKeys(const std::uint32_t* keys,std::uint32_t count,std::uint32_t capacity,SFInputKeyStateV1* output) {
    if(!keys || !output || !count || count>32) return 1;
    if(capacity<count*sizeof(*output)) return 5;
    for(unsigned i=0;i<count;++i) if(!keys[i] || keys[i]>255) return 1;
    static std::mutex mutex; static SFInputKeyStateV1 states[256]{};
    static std::uint64_t sequence{}; std::lock_guard lock(mutex);
    const auto now=GetTickCount64(); DWORD process{}; GetWindowThreadProcessId(GetForegroundWindow(),&process);
    const bool focused=process==GetCurrentProcessId();
    for(unsigned i=0;i<count;++i) {
        const auto key=keys[i]; auto& state=states[key];
        if(state.sampled_at_ms!=now) {
            const bool down=focused && MapRuntime::HudInputAllowed(now) && (GetAsyncKeyState(int(key)) & 0x8000)!=0, was_down=(state.flags&1)!=0;
            state={key,std::uint32_t((down?1:0)|(down&&!was_down?2:0)|(!down&&was_down?4:0)|(focused?8:0)),++sequence,now};
        }
        output[i]=state;
    }
    return 0;
}
}
