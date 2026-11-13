#include "abi/runtime_api.h"
#include "graphics/render_overlay.h"
#include "graphics/notification_overlay.h"
#include "map/capture_game_map.h"
#include "map/hud_visibility.h"
#include <windows.h>
#include <algorithm>
#include <map>
#include <mutex>
#include <string>

namespace OverlayRuntime {
void MaintainGraphics();
namespace {
struct Activation {std::uint64_t owner{};bool captured{},graphics{},revoked{};};
std::mutex service_mutex;
std::map<std::pair<std::string,std::uint64_t>,Activation> activations;
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
        if(name!="shroudforge.map" && name!="shroudforge.overlay")return 4;
        const bool map_service=name=="shroudforge.map";
        if((map_service && version!=SF_MAP_SERVICE_VERSION) ||
            (!map_service && version!=SF_OVERLAY_SERVICE_VERSION && version!=SF_OVERLAY_EXTENDED_SERVICE_VERSION && version!=SF_OVERLAY_PROGRAMMABLE_SERVICE_VERSION))return 2;
        if(capacity<(name=="shroudforge.map" ? sizeof(SFMapServiceV1):sizeof(SFOverlayServiceV1)))return 5;
        std::lock_guard lock(service_mutex);const auto key=std::make_pair(id,generation);
        if(!activations.contains(key) && activations.size()>=4096)return 6;auto& item=activations[key];if(item.revoked)return 3;
        if(!item.owner)item.owner=Services().Open(id,generation);if(!item.owner)return 6;
        if(map_service && !item.captured){const auto result=MapRuntime::EnableCapture(item.owner,true);if(result!=SF_MAP_READY)return result==SF_MAP_FAILED ? 7:3;item.captured=true;}
        if(!map_service && !item.graphics){if(!StartGraphics())return 3;item.graphics=true;}
        return Services().Query(item.owner,name,version,capacity,output);
    }catch(...){return 7;}
}
KFC_RUNTIME_API std::int32_t __cdecl KfcRuntimeRevokeMapOverlayServices(const std::uint8_t* package,std::uint32_t size,std::uint64_t generation) {
    using namespace OverlayRuntime;try {
        std::string id;if(!generation || !Text(package,size,256,id))return 1;std::lock_guard lock(service_mutex);
        const auto key=std::make_pair(id,generation);auto found=activations.find(key);
        if(found==activations.end()) {
            // A never-queried generation must stay closed during stop/shutdown.
            if(activations.size()>=4096)return 6;
            found=activations.try_emplace(key).first;
        }
        found->second.revoked=true;return Services().Revoke(found->second.owner);
    }catch(...){return 7;}
}
KFC_RUNTIME_API std::int32_t __cdecl KfcRuntimeDrainMapOverlayServices(const std::uint8_t* package,std::uint32_t size,std::uint64_t generation,std::uint32_t timeout) {
    using namespace OverlayRuntime;try {
        std::string id;if(!generation || timeout>250 || !Text(package,size,256,id))return 1;std::lock_guard lock(service_mutex);
        const auto found=activations.find({id,generation});if(found==activations.end())return 0;
        auto& item=found->second;const auto result=Services().Drain(item.owner,timeout);
        if(result==0 && item.revoked){
            if(item.captured){MapRuntime::DisableCapture(item.owner);item.captured=false;}
            if(item.graphics){
                item.graphics=false;
                const bool another=std::any_of(activations.begin(),activations.end(),[](const auto& entry){return entry.second.graphics;});
                if(!another)StopGraphics();
            }
        }
        return result;
    }catch(...){return 7;}
}
}
