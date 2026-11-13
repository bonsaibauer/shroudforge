#include "abi/runtime_api.h"
#include "graphics/render_overlay.h"
#include "graphics/notification_overlay.h"
#include "host/executable_span_validation.h"
#include "map/capture_game_map.h"
#include "map/hud_visibility.h"
#include <windows.h>
#include <algorithm>
#include <array>
#include <chrono>
#include <condition_variable>
#include <cstring>
#include <map>
#include <mutex>
#include <string>

namespace OverlayRuntime {
void MaintainGraphics();
namespace {
struct Activation {std::uint64_t owner{};bool captured{},graphics{},revoked{},capture_pending{},graphics_pending{};unsigned in_flight{};};
std::mutex service_mutex;
std::condition_variable service_changed;
std::map<std::pair<std::string,std::uint64_t>,Activation> activations;
enum class ActivationOperation {None,Capture,Graphics};
struct ActivationLease {
    Activation* item{};
    ActivationOperation operation{ActivationOperation::None};
    ~ActivationLease() noexcept {
        if(!item)return;
        std::lock_guard lock(service_mutex);
        if(operation==ActivationOperation::Capture)item->capture_pending=false;
        if(operation==ActivationOperation::Graphics)item->graphics_pending=false;
        if(item->in_flight)--item->in_flight;
        service_changed.notify_all();
    }
};
bool Text(const std::uint8_t* text,std::uint32_t size,std::uint32_t limit,std::string& out) {
    if(!text || !size || size>limit)return false;out.resize(size);
    if(!MapRuntime::ReadMemory(reinterpret_cast<std::uintptr_t>(text),out.data(),size))return false;
    for(const auto c:out)if(static_cast<unsigned char>(c)<32 || c==127)return false;
    return true;
}
}
void TickServices() {
    DWORD process{};GetWindowThreadProcessId(GetForegroundWindow(),&process);
    const auto session=KfcRuntimeWorldSessionId();
    const auto now=GetTickCount64();
    MapRuntime::UpdateCaptureSession(session,session && MapRuntime::HudGameplay(now),session && MapRuntime::HudKnown(now) && !MapRuntime::HudGameplay(now),process==GetCurrentProcessId());
    MaintainGraphics();
}
void ShutdownServices() {
    NotificationOverlay::Stop();
    std::lock_guard lock(service_mutex);
    for(auto& [_,item]:activations){Services().Revoke(item.owner);item.revoked=true;}
    StopGraphics();
    for(auto& [_,item]:activations)if(item.captured){MapRuntime::DisableCapture(item.owner);item.captured=false;}
}
}
extern "C" {
KFC_RUNTIME_API std::int32_t __cdecl KfcRuntimeReadInputKeys(const std::uint32_t* keys,std::uint32_t count,std::uint32_t capacity,void* output) {
    try{return OverlayRuntime::SampleKeys(keys,count,capacity,static_cast<SFInputKeyStateV1*>(output));}catch(...){return 7;}
}
KFC_RUNTIME_API std::int32_t __cdecl KfcRuntimeQueryMapOverlayService(const std::uint8_t* package,std::uint32_t package_size,
    std::uint64_t generation,const std::uint8_t* service,std::uint32_t service_size,std::uint32_t version,
    std::uint32_t capacity,void* output) {
    using namespace OverlayRuntime;try {
        std::string id,name;if(!generation || !output || !Text(package,package_size,256,id) || !Text(service,service_size,64,name))return 1;
        if(name==SF_EXECUTABLE_SPAN_VALIDATION_SERVICE_V1) {
            if(version!=SF_EXECUTABLE_SPAN_VALIDATION_VERSION_V1)return 2;
            if(capacity<sizeof(SFExecutableSpanValidationServiceV1))return 5;
            if(!WritableServiceOutput(output,sizeof(SFExecutableSpanValidationServiceV1)))return 1;
            return ExecutableSpanValidation::Services().Query(id,generation,version,capacity,output);
        }
        const bool map_service=name=="shroudforge.map";
        const bool executable_span_service=name==SF_EXECUTABLE_SPAN_SERVICE_NAME;
        const bool overlay_service=name=="shroudforge.overlay";
        if(!map_service && !executable_span_service && !overlay_service)return 4;
        if((map_service && version!=SF_MAP_SERVICE_VERSION) ||
            (executable_span_service && version!=SF_EXECUTABLE_SPAN_SERVICE_VERSION) ||
            (overlay_service && version!=SF_OVERLAY_SERVICE_VERSION &&
                version!=SF_OVERLAY_EXTENDED_SERVICE_VERSION && version!=SF_OVERLAY_PROGRAMMABLE_SERVICE_VERSION))return 2;
        const auto required=map_service ? sizeof(SFMapServiceV1) :
            executable_span_service ? sizeof(SFExecutableSpanServiceV1) : sizeof(SFOverlayServiceV1);
        if(capacity<required)return 5;
        if(!WritableServiceOutput(output,required))
            return executable_span_service ? SF_EXECUTABLE_SPAN_STATUS_INVALID_ARGUMENT : 1;
        constexpr auto table_size=(std::max)((std::max)(sizeof(SFMapServiceV1),sizeof(SFExecutableSpanServiceV1)),sizeof(SFOverlayServiceV1));
        std::array<std::uint8_t,table_size> table{};
        const auto key=std::make_pair(id,generation);Activation* item{};std::uint64_t owner{};
        ActivationOperation operation=ActivationOperation::None;
        {
            std::lock_guard lock(service_mutex);
            if(!activations.contains(key) && activations.size()>=4096)return 6;
            auto& current=activations[key];if(current.revoked)return 3;
            if(current.in_flight)return 6;
            if(!current.owner)current.owner=Services().Open(id,generation);if(!current.owner)return 6;
            item=&current;owner=current.owner;++current.in_flight;
            if(map_service && !current.captured){current.capture_pending=true;operation=ActivationOperation::Capture;}
            if(overlay_service && !current.graphics){current.graphics_pending=true;operation=ActivationOperation::Graphics;}
        }
        ActivationLease lease{item,operation};
        const auto service_result=Services().Query(owner,name,version,capacity,table.data());
        if(service_result)return service_result;
        std::int32_t operation_result=0;
        if(operation==ActivationOperation::Capture){
            operation_result=MapRuntime::EnableCapture(owner,true);
            if(operation_result!=SF_MAP_READY)operation_result=operation_result==SF_MAP_FAILED ? 7:3;
        } else if(operation==ActivationOperation::Graphics && !StartGraphics())operation_result=3;
        if(operation_result)return operation_result;
        bool cleanup_capture{},cleanup_graphics{},rejected{};
        {
            std::lock_guard lock(service_mutex);
            rejected=item->revoked || item->owner!=owner;
            if(!rejected){
                if(operation==ActivationOperation::Capture)item->captured=true;
                if(operation==ActivationOperation::Graphics)item->graphics=true;
                const auto output_size=map_service ? sizeof(SFMapServiceV1) :
                    executable_span_service ? sizeof(SFExecutableSpanServiceV1) : sizeof(SFOverlayServiceV1);
                std::memcpy(output,table.data(),output_size);
            } else {
                cleanup_capture=operation==ActivationOperation::Capture;
                cleanup_graphics=operation==ActivationOperation::Graphics;
            }
        }
        if(cleanup_capture)MapRuntime::DisableCapture(owner);
        if(cleanup_graphics){
            bool another{};
            {std::lock_guard lock(service_mutex);another=std::any_of(activations.begin(),activations.end(),[&](const auto& entry){return &entry.second!=item && (entry.second.graphics || entry.second.graphics_pending);});}
            if(!another)StopGraphics();
        }
        return rejected ? 3 : 0;
    }catch(...){return 7;}
}
KFC_RUNTIME_API std::int32_t __cdecl KfcRuntimeRevokeMapOverlayServices(const std::uint8_t* package,std::uint32_t size,std::uint64_t generation) {
    using namespace OverlayRuntime;try {
        std::string id;if(!generation || !Text(package,size,256,id))return 1;
        const auto span_result=ExecutableSpanValidation::Services().Revoke(id,generation);
        std::lock_guard lock(service_mutex);
        const auto key=std::make_pair(id,generation);auto found=activations.find(key);
        if(found==activations.end()) {
            // A never-queried generation must stay closed during stop/shutdown.
            if(activations.size()>=4096)return 6;
            found=activations.try_emplace(key).first;
        }
        found->second.revoked=true;const auto map_result=Services().Revoke(found->second.owner);
        return map_result ? map_result : span_result;
    }catch(...){return 7;}
}
KFC_RUNTIME_API std::int32_t __cdecl KfcRuntimeDrainMapOverlayServices(const std::uint8_t* package,std::uint32_t size,std::uint64_t generation,std::uint32_t timeout) {
    using namespace OverlayRuntime;try {
        std::string id;if(!generation || timeout>250 || !Text(package,size,256,id))return 1;
        const auto started=GetTickCount64();
        const auto span_result=ExecutableSpanValidation::Services().Drain(id,generation,timeout);
        if(span_result)return span_result;
        const auto elapsed=static_cast<std::uint32_t>((std::min)(GetTickCount64()-started,static_cast<ULONGLONG>(timeout)));
        const auto remaining=timeout-elapsed;
        Activation* item{};std::uint64_t owner{};
        {
            std::unique_lock lock(service_mutex);
            const auto found=activations.find({id,generation});if(found==activations.end())return 0;
            item=&found->second;owner=item->owner;
            if(!service_changed.wait_for(lock,std::chrono::milliseconds(remaining),[&]{return item->in_flight==0;}))return 6;
        }
        const auto result=Services().Drain(owner,remaining);
        if(result)return result;
        bool disable_capture{},stop_graphics{};
        {
            std::lock_guard lock(service_mutex);
            if(item->revoked){
                disable_capture=item->captured;item->captured=false;
                if(item->graphics){
                    item->graphics=false;
                    const bool another=std::any_of(activations.begin(),activations.end(),[&](const auto& entry){
                        return &entry.second!=item && (entry.second.graphics || entry.second.graphics_pending);
                    });
                    stop_graphics=!another;
                }
            }
        }
        if(disable_capture)MapRuntime::DisableCapture(owner);
        if(stop_graphics)StopGraphics();
        return 0;
    }catch(...){return 7;}
}
}
