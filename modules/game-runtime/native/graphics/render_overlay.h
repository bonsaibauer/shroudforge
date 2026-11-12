#pragma once
#include "map/read_map_frame.h"
#include <condition_variable>
#include <string>
#include <unordered_map>

namespace OverlayRuntime {
class Registry {
public:
    explicit Registry(MapRuntime::SnapshotStore& frames) : frames_(frames) {}
    std::uint64_t Open(const std::string& package,std::uint64_t generation);
    std::int32_t Query(std::uint64_t owner,const std::string& name,std::uint32_t version,std::uint32_t capacity,void* output);
    std::int32_t Revoke(std::uint64_t owner);
    std::int32_t Drain(std::uint64_t owner,std::uint32_t timeout_ms);
    bool Dispatch(std::uint32_t width,std::uint32_t height,std::uint64_t now,std::uint64_t device_generation,const SFOverlayEncoderV1& encoder,const SFMapFrameV1* frame=nullptr);
private:
    struct Owner;
    MapRuntime::SnapshotStore& frames_;
    std::mutex mutex_,dispatch_mutex_;
    std::unordered_map<std::uint64_t,std::shared_ptr<Owner>> owners_;
    std::uint64_t next_owner_{},next_registration_{};
    std::shared_ptr<Owner> Find(std::uint64_t owner);
    static std::int32_t Acquire(void*,std::uint64_t,std::uint32_t,SFMapFrameV1*,SFMapFrameLeaseV1*);
    static std::int32_t Release(void*,SFMapFrameLeaseV1);
    static std::int32_t Keys(void*,const std::uint32_t*,std::uint32_t,std::uint32_t,SFInputKeyStateV1*);
    static std::int32_t Register(void*,SFOverlayFrameCallbackV1,void*,SFOverlayRegistrationV1*);
    static std::int32_t Unregister(void*,SFOverlayRegistrationV1);
    static std::int32_t Status(void*,std::uint32_t,SFOverlayStatusV1*);
};
Registry& Services();
std::int32_t SampleKeys(const std::uint32_t* keys,std::uint32_t count,std::uint32_t capacity,SFInputKeyStateV1* output);
bool PrepareGraphicsBoot(bool demand);
bool StartGraphics();
void StopGraphics();
void TickServices();
void ShutdownServices();
SFOverlayStatusV1 GraphicsStatus();
}
