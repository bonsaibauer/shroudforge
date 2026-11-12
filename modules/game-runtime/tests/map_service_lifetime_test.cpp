#include "map/serve_map_overlay.cpp"
#include <cstdio>
#include <cstdlib>
namespace {unsigned checks{},enabled{},disabled{},callbacks{};MapRuntime::SnapshotStore fixture;
void check(bool value){++checks;if(!value){std::fprintf(stderr,"control lifetime check %u failed\n",checks);std::exit(1);}}
std::int32_t callback(void*,const SFOverlayFrameV1*,const SFOverlayEncoderV1*){++callbacks;return 0;}}
namespace MapRuntime {
SnapshotStore& CapturedFrames(){return fixture;}
std::int32_t EnableCapture(std::uint64_t,bool){++enabled;return SF_MAP_READY;}
void DisableCapture(std::uint64_t){++disabled;}
void UpdateCaptureSession(std::uint64_t,bool,bool,bool){}
bool HudKnown(std::uint64_t){return false;}bool HudGameplay(std::uint64_t){return false;}bool HudInputAllowed(std::uint64_t){return true;}
}
namespace OverlayRuntime {bool StartGraphics(){return true;}void StopGraphics(){}void MaintainGraphics(){}SFOverlayStatusV1 GraphicsStatus(){return {sizeof(SFOverlayStatusV1),1,3};}}
extern "C" std::uint64_t __cdecl KfcRuntimeWorldSessionId(){return 0;}
int main(){
    const std::uint8_t package[]{'l','a','t','e'},name[]{'s','h','r','o','u','d','f','o','r','g','e','.','o','v','e','r','l','a','y'};
    SFOverlayServiceV1 table{};table.version=123;
    check(KfcRuntimeRevokeMapOverlayServices(package,sizeof(package),1)==0);
    const auto late_query=KfcRuntimeQueryMapOverlayService(package,sizeof(package),1,name,sizeof(name),1,sizeof(table),&table);
    if(late_query==0){SFOverlayRegistrationV1 late{};table.register_frame(table.context,callback,nullptr,&late);}
    check(KfcRuntimeDrainMapOverlayServices(package,sizeof(package),1,0)==0);
    OverlayRuntime::Services().Dispatch(800,600,100,1,SFOverlayEncoderV1{});
    check(late_query==3 && table.version==123 && enabled==0 && callbacks==0);
    check(KfcRuntimeQueryMapOverlayService(package,sizeof(package),2,name,sizeof(name),1,sizeof(table),&table)==0 && enabled==1);
    SFOverlayRegistrationV1 registration{};check(table.register_frame(table.context,callback,nullptr,&registration)==0);
    check(KfcRuntimeRevokeMapOverlayServices(package,sizeof(package),2)==0);
    SFOverlayRegistrationV1 rejected{};check(table.register_frame(table.context,callback,nullptr,&rejected)==3);
    OverlayRuntime::Services().Dispatch(800,600,100,1,SFOverlayEncoderV1{});check(callbacks==0);
    check(KfcRuntimeDrainMapOverlayServices(package,sizeof(package),2,0)==0 && disabled==1);
    check(KfcRuntimeQueryMapOverlayService(package,sizeof(package),2,name,sizeof(name),1,sizeof(table),&table)==3);
    check(KfcRuntimeQueryMapOverlayService(package,sizeof(package),3,name,sizeof(name),1,sizeof(table),&table)==0 && enabled==2);
    check(table.register_frame(table.context,callback,nullptr,&registration)==0);
    OverlayRuntime::Services().Dispatch(800,600,100,1,SFOverlayEncoderV1{});check(callbacks==1);
    check(KfcRuntimeRevokeMapOverlayServices(package,sizeof(package),3)==0);
    check(KfcRuntimeDrainMapOverlayServices(package,sizeof(package),3,0)==0 && disabled==2);
    // Full bounded state must reject, rather than falsely acknowledge, an
    // unknown revocation it cannot durably record.
    for(std::uint64_t generation=100;OverlayRuntime::activations.size()<4096;++generation)
        OverlayRuntime::activations.emplace(std::make_pair(std::string("capacity"),generation),OverlayRuntime::Activation{});
    check(KfcRuntimeRevokeMapOverlayServices(package,sizeof(package),4)!=0);
    check(KfcRuntimeRevokeMapOverlayServices(package,sizeof(package),1)==0);
    std::printf("PASS: %u actual native service terminal-revocation checks\n",checks);
}
