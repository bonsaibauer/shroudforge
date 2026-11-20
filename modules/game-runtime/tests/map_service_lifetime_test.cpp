#include "map/serve_map_overlay.cpp"
#include "engine-hooks/owned_executable_span.h"
#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <cstdint>
#include <array>
#include <cstring>
#include <thread>
namespace {unsigned checks{},enabled{},disabled{},callbacks{},graphics_started{};MapRuntime::SnapshotStore fixture;
bool expect_disabled=true;
std::array<std::uint8_t,8> data_span_anchor{0x10,0x21,0x32,0x43,0x54,0x65,0x76,0x87};
void check(bool value){++checks;if(!value){std::fprintf(stderr,"control lifetime check %u failed\n",checks);std::exit(1);}}
std::uint32_t valid_verifier_rva{};
std::array<std::uint8_t,8> valid_verifier_original{},valid_verifier_replacement{};
unsigned valid_verifier_calls{};
std::int32_t callback(void*,const SFOverlayFrameV1* frame,const SFOverlayEncoderV1*){
    check(frame->map_frame && frame->map_frame->status==(expect_disabled?SF_MAP_DISABLED:SF_MAP_READY));
    ++callbacks;return 0;}}
__declspec(noinline) int span_anchor_unmodified(){return 0x1357;}
__declspec(noinline) int span_anchor_owned(){return 0x2468;}
__declspec(noinline) int span_anchor_unregistered_foreign(){return 0x579B;}
__declspec(noinline) int span_anchor_invalid_relay(){return 0x369A;}
__declspec(noinline) int span_anchor_stale_owner(){return 0x48AC;}
bool SF_EXECUTABLE_SPAN_CALL verify_fixture_owned(std::uint32_t rva,const std::uint8_t* original,
    const std::uint8_t* replacement,std::uint32_t count) noexcept {
    ++valid_verifier_calls;
    return rva==valid_verifier_rva && count==valid_verifier_original.size() && original && replacement &&
        !std::memcmp(original,valid_verifier_original.data(),count) &&
        !std::memcmp(replacement,valid_verifier_replacement.data(),count);
}
bool SF_EXECUTABLE_SPAN_CALL verify_fixture_unavailable(std::uint32_t,const std::uint8_t*,const std::uint8_t*,std::uint32_t) noexcept {
    return false;
}
namespace MapRuntime {
SnapshotStore& CapturedFrames(){return fixture;}
std::int32_t EnableCapture(std::uint64_t,bool){++enabled;return SF_MAP_READY;}
void DisableCapture(std::uint64_t){++disabled;}
void UpdateCaptureSession(std::uint64_t,bool,bool,bool){}
bool HudKnown(std::uint64_t){return false;}bool HudGameplay(std::uint64_t){return false;}bool HudInputAllowed(std::uint64_t){return true;}
}
namespace OverlayRuntime {bool StartGraphics(){++graphics_started;return true;}void StopGraphics(){}void MaintainGraphics(){}SFOverlayStatusV1 GraphicsStatus(){return {sizeof(SFOverlayStatusV1),1,3};}}
extern "C" std::uint64_t __cdecl KfcRuntimeWorldSessionId(){return 0;}
int main(){
    const std::uint8_t package[]{'l','a','t','e'},name[]{'s','h','r','o','u','d','f','o','r','g','e','.','o','v','e','r','l','a','y'};
    SFExecutableSpanServiceV1 span_service{};
    check(KfcRuntimeQueryMapOverlayService(package,sizeof(package),8,
        reinterpret_cast<const std::uint8_t*>(SF_EXECUTABLE_SPAN_SERVICE_NAME),
        static_cast<std::uint32_t>(std::strlen(SF_EXECUTABLE_SPAN_SERVICE_NAME)),
        SF_EXECUTABLE_SPAN_SERVICE_VERSION,sizeof(span_service),
        reinterpret_cast<SFExecutableSpanServiceV1*>(1))==SF_EXECUTABLE_SPAN_STATUS_INVALID_ARGUMENT);
    check(KfcRuntimeQueryMapOverlayService(package,sizeof(package),8,
        reinterpret_cast<const std::uint8_t*>(SF_EXECUTABLE_SPAN_VALIDATION_SERVICE_V1),
        static_cast<std::uint32_t>(std::strlen(SF_EXECUTABLE_SPAN_VALIDATION_SERVICE_V1)),
        SF_EXECUTABLE_SPAN_VALIDATION_VERSION_V1,sizeof(SFExecutableSpanValidationServiceV1),
        reinterpret_cast<SFExecutableSpanValidationServiceV1*>(1))==1);
    check(KfcRuntimeQueryMapOverlayService(package,sizeof(package),8,
        reinterpret_cast<const std::uint8_t*>(SF_EXECUTABLE_SPAN_SERVICE_NAME),
        static_cast<std::uint32_t>(std::strlen(SF_EXECUTABLE_SPAN_SERVICE_NAME)),
        SF_EXECUTABLE_SPAN_SERVICE_VERSION,sizeof(span_service),&span_service)==SF_EXECUTABLE_SPAN_STATUS_OK);
    check(span_service.size==sizeof(span_service) && span_service.version==SF_EXECUTABLE_SPAN_SERVICE_VERSION &&
        span_service.context && span_service.query_main_image_span && enabled==0 && graphics_started==0);
    const auto make_request=[](const void* address,const std::array<std::uint8_t,8>& expected,
        const std::array<std::uint8_t,8>& observed) {
        SFExecutableSpanQueryV1 request{};request.size=sizeof(request);request.version=SF_EXECUTABLE_SPAN_SERVICE_VERSION;
        const auto base=reinterpret_cast<std::uintptr_t>(GetModuleHandleW(nullptr));
        const auto target=reinterpret_cast<std::uintptr_t>(address);
        check(target>=base && target-base<=UINT32_MAX);
        request.rva=static_cast<std::uint32_t>(target-base);request.byte_count=8;
        std::copy(expected.begin(),expected.end(),request.expected_original);
        std::copy(observed.begin(),observed.end(),request.observed_current);return request;
    };
    const auto sample=[](const void* address){std::array<std::uint8_t,8> bytes{};std::memcpy(bytes.data(),address,bytes.size());return bytes;};
    const auto query_span=[&](const SFExecutableSpanQueryV1& request,SFExecutableSpanResultV1& result) {
        return span_service.query_main_image_span(span_service.context,&request,sizeof(result),&result);
    };

    const auto original_live=sample(reinterpret_cast<const void*>(&span_anchor_unmodified));
    auto unchanged_request=make_request(reinterpret_cast<const void*>(&span_anchor_unmodified),original_live,original_live);
    SFExecutableSpanResultV1 span_result{sizeof(span_result),0xA5A5A5A5};
    check(query_span(unchanged_request,span_result)==SF_EXECUTABLE_SPAN_STATUS_OK &&
        span_result.size==sizeof(span_result) && span_result.classification==SF_EXECUTABLE_SPAN_UNCHANGED);

    auto maximum_request=unchanged_request;maximum_request.byte_count=SF_EXECUTABLE_SPAN_MAX_BYTES;
    std::memcpy(maximum_request.expected_original,reinterpret_cast<const void*>(&span_anchor_unmodified),
        SF_EXECUTABLE_SPAN_MAX_BYTES);
    std::memcpy(maximum_request.observed_current,maximum_request.expected_original,SF_EXECUTABLE_SPAN_MAX_BYTES);
    span_result={sizeof(span_result),0xA5A5A5A5};
    check(query_span(maximum_request,span_result)==SF_EXECUTABLE_SPAN_STATUS_OK &&
        span_result.size==sizeof(span_result) && span_result.classification==SF_EXECUTABLE_SPAN_UNCHANGED);

    auto oversized_request=maximum_request;oversized_request.byte_count=SF_EXECUTABLE_SPAN_MAX_BYTES+1;
    span_result={sizeof(span_result),0xA5A5A5A5};
    check(query_span(oversized_request,span_result)==SF_EXECUTABLE_SPAN_STATUS_INVALID_ARGUMENT &&
        span_result.size==sizeof(span_result) && span_result.classification==0xA5A5A5A5);

    valid_verifier_rva=make_request(reinterpret_cast<const void*>(&span_anchor_owned),{},{}).rva;
    valid_verifier_replacement=sample(reinterpret_cast<const void*>(&span_anchor_owned));
    valid_verifier_original=valid_verifier_replacement;valid_verifier_original[0]^=0xFF;
    check(EngineHooks::PublishOwnedExecutableSpan(valid_verifier_rva,valid_verifier_original,
        valid_verifier_replacement,verify_fixture_owned));
    check(EngineHooks::PublishOwnedExecutableSpan(valid_verifier_rva,valid_verifier_original,
        valid_verifier_replacement,verify_fixture_owned));
    auto overlapping_original=valid_verifier_original,overlapping_replacement=valid_verifier_replacement;
    check(!EngineHooks::PublishOwnedExecutableSpan(valid_verifier_rva+1,overlapping_original,overlapping_replacement,
        verify_fixture_owned));
    auto owned_request=make_request(reinterpret_cast<const void*>(&span_anchor_owned),valid_verifier_original,valid_verifier_replacement);
    span_result={sizeof(span_result),0xA5A5A5A5};
    check(query_span(owned_request,span_result)==SF_EXECUTABLE_SPAN_STATUS_OK &&
        span_result.classification==SF_EXECUTABLE_SPAN_VERIFIED_HOST_OWNED && valid_verifier_calls==1);

    auto stale_observation=owned_request;stale_observation.observed_current[0]^=0x01;
    span_result={sizeof(span_result),0xA5A5A5A5};
    check(query_span(stale_observation,span_result)==SF_EXECUTABLE_SPAN_STATUS_UNAVAILABLE &&
        span_result.size==sizeof(span_result) && span_result.classification==0xA5A5A5A5);

    auto partial_overlap=owned_request;partial_overlap.rva+=1;partial_overlap.byte_count=4;
    std::copy(valid_verifier_original.begin()+1,valid_verifier_original.begin()+5,partial_overlap.expected_original);
    std::copy(valid_verifier_replacement.begin()+1,valid_verifier_replacement.begin()+5,partial_overlap.observed_current);
    span_result={sizeof(span_result),0};
    check(query_span(partial_overlap,span_result)==SF_EXECUTABLE_SPAN_STATUS_OK &&
        span_result.classification==SF_EXECUTABLE_SPAN_FOREIGN_OR_UNAVAILABLE);

    const auto foreign_live=sample(reinterpret_cast<const void*>(&span_anchor_invalid_relay));
    auto foreign_expected=foreign_live;foreign_expected[0]^=0xFF;
    const auto foreign_request=make_request(reinterpret_cast<const void*>(&span_anchor_invalid_relay),foreign_expected,foreign_live);
    check(EngineHooks::PublishOwnedExecutableSpan(foreign_request.rva,foreign_expected,foreign_live,
        verify_fixture_unavailable));
    span_result={sizeof(span_result),0};
    check(query_span(foreign_request,span_result)==SF_EXECUTABLE_SPAN_STATUS_OK &&
        span_result.classification==SF_EXECUTABLE_SPAN_FOREIGN_OR_UNAVAILABLE);

    const auto unregistered_live=sample(reinterpret_cast<const void*>(&span_anchor_unregistered_foreign));
    auto unregistered_expected=unregistered_live;unregistered_expected[0]^=0xFF;
    const auto unregistered_foreign=make_request(reinterpret_cast<const void*>(&span_anchor_unregistered_foreign),
        unregistered_expected,unregistered_live);
    span_result={sizeof(span_result),0};
    check(query_span(unregistered_foreign,span_result)==SF_EXECUTABLE_SPAN_STATUS_OK &&
        span_result.classification==SF_EXECUTABLE_SPAN_FOREIGN_OR_UNAVAILABLE);

    const auto stale_live=sample(reinterpret_cast<const void*>(&span_anchor_stale_owner));
    auto stale_expected=stale_live;stale_expected[0]^=0xFF;
    const auto stale_owner_request=make_request(reinterpret_cast<const void*>(&span_anchor_stale_owner),stale_expected,stale_live);
    check(EngineHooks::PublishOwnedExecutableSpan(stale_owner_request.rva,stale_expected,stale_live,
        verify_fixture_unavailable));
    span_result={sizeof(span_result),0};
    check(query_span(stale_owner_request,span_result)==SF_EXECUTABLE_SPAN_STATUS_OK &&
        span_result.classification==SF_EXECUTABLE_SPAN_FOREIGN_OR_UNAVAILABLE);

    auto invalid_request=unchanged_request;invalid_request.byte_count=0;
    span_result={sizeof(span_result),0xA5A5A5A5};
    check(query_span(invalid_request,span_result)==SF_EXECUTABLE_SPAN_STATUS_INVALID_ARGUMENT &&
        span_result.size==sizeof(span_result) && span_result.classification==0xA5A5A5A5);
    invalid_request=unchanged_request;invalid_request.rva=UINT32_MAX;
    check(query_span(invalid_request,span_result)==SF_EXECUTABLE_SPAN_STATUS_INVALID_ARGUMENT &&
        span_result.size==sizeof(span_result) && span_result.classification==0xA5A5A5A5);
    IMAGE_DOS_HEADER dos{};IMAGE_NT_HEADERS64 nt{};SIZE_T header_bytes{};
    const auto main_image=GetModuleHandleW(nullptr);
    check(main_image && ReadProcessMemory(GetCurrentProcess(),main_image,&dos,sizeof(dos),&header_bytes) &&
        header_bytes==sizeof(dos) && dos.e_magic==IMAGE_DOS_SIGNATURE && dos.e_lfanew>0);
    const auto nt_address=reinterpret_cast<const std::uint8_t*>(main_image)+dos.e_lfanew;
    header_bytes=0;
    check(ReadProcessMemory(GetCurrentProcess(),nt_address,&nt,sizeof(nt),&header_bytes) &&
        header_bytes==sizeof(nt) && nt.Signature==IMAGE_NT_SIGNATURE && nt.OptionalHeader.SizeOfImage>0);
    invalid_request=unchanged_request;invalid_request.rva=nt.OptionalHeader.SizeOfImage;
    span_result={sizeof(span_result),0xA5A5A5A5};
    check(query_span(invalid_request,span_result)==SF_EXECUTABLE_SPAN_STATUS_INVALID_ARGUMENT &&
        span_result.classification==0xA5A5A5A5);
    const auto data_live=sample(data_span_anchor.data());
    const auto data_request=make_request(data_span_anchor.data(),data_live,data_live);
    check(query_span(data_request,span_result)==SF_EXECUTABLE_SPAN_STATUS_UNAVAILABLE &&
        span_result.size==sizeof(span_result) && span_result.classification==0xA5A5A5A5);
    check(span_service.query_main_image_span(span_service.context,nullptr,sizeof(span_result),&span_result)==
        SF_EXECUTABLE_SPAN_STATUS_INVALID_ARGUMENT && span_result.classification==0xA5A5A5A5);
    check(span_service.query_main_image_span(span_service.context,&unchanged_request,sizeof(span_result)-1,&span_result)==
        SF_EXECUTABLE_SPAN_STATUS_BUFFER_TOO_SMALL && span_result.classification==0xA5A5A5A5);
    check(span_service.query_main_image_span(span_service.context,&unchanged_request,sizeof(span_result),
        reinterpret_cast<SFExecutableSpanResultV1*>(1))==SF_EXECUTABLE_SPAN_STATUS_INVALID_ARGUMENT);

    SFExecutableSpanServiceV1 short_table{0x11111111,0x22222222,reinterpret_cast<void*>(3),nullptr};
    check(KfcRuntimeQueryMapOverlayService(package,sizeof(package),9,
        reinterpret_cast<const std::uint8_t*>(SF_EXECUTABLE_SPAN_SERVICE_NAME),
        static_cast<std::uint32_t>(std::strlen(SF_EXECUTABLE_SPAN_SERVICE_NAME)),
        SF_EXECUTABLE_SPAN_SERVICE_VERSION,sizeof(short_table)-1,&short_table)==SF_EXECUTABLE_SPAN_STATUS_BUFFER_TOO_SMALL &&
        short_table.size==0x11111111 && short_table.version==0x22222222 && short_table.context==reinterpret_cast<void*>(3));
    auto wrong_thread_status=SF_EXECUTABLE_SPAN_STATUS_OK;span_result={sizeof(span_result),0xA5A5A5A5};
    std::thread worker([&]{wrong_thread_status=query_span(owned_request,span_result);});worker.join();
    check(wrong_thread_status==SF_EXECUTABLE_SPAN_STATUS_WRONG_THREAD && span_result.classification==0xA5A5A5A5);
    check(KfcRuntimeRevokeMapOverlayServices(package,sizeof(package),8)==SF_EXECUTABLE_SPAN_STATUS_OK);
    span_result={sizeof(span_result),0xA5A5A5A5};
    check(query_span(owned_request,span_result)==SF_EXECUTABLE_SPAN_STATUS_UNAVAILABLE && span_result.classification==0xA5A5A5A5);

    SFOverlayServiceV1 table{};table.version=123;
    check(KfcRuntimeRevokeMapOverlayServices(package,sizeof(package),1)==0);
    const auto late_query=KfcRuntimeQueryMapOverlayService(package,sizeof(package),1,name,sizeof(name),1,sizeof(table),&table);
    if(late_query==0){SFOverlayRegistrationV1 late{};table.register_frame(table.context,callback,nullptr,&late);}
    check(KfcRuntimeDrainMapOverlayServices(package,sizeof(package),1,0)==0);
    OverlayRuntime::Services().Dispatch(800,600,100,1,SFOverlayEncoderV1{});
    check(late_query==3 && table.version==123 && enabled==0 && callbacks==0);
    check(KfcRuntimeQueryMapOverlayService(package,sizeof(package),2,name,sizeof(name),1,sizeof(table),&table)==0 && enabled==0);
    SFOverlayRegistrationV1 registration{};check(table.register_frame(table.context,callback,nullptr,&registration)==0);
    OverlayRuntime::Services().Dispatch(800,600,100,1,SFOverlayEncoderV1{});check(callbacks==1 && enabled==0);
    check(KfcRuntimeRevokeMapOverlayServices(package,sizeof(package),2)==0);
    SFOverlayRegistrationV1 rejected{};check(table.register_frame(table.context,callback,nullptr,&rejected)==3);
    OverlayRuntime::Services().Dispatch(800,600,100,1,SFOverlayEncoderV1{});check(callbacks==1);
    check(KfcRuntimeDrainMapOverlayServices(package,sizeof(package),2,0)==0 && disabled==0);
    check(KfcRuntimeQueryMapOverlayService(package,sizeof(package),2,name,sizeof(name),1,sizeof(table),&table)==3);
    check(KfcRuntimeQueryMapOverlayService(package,sizeof(package),3,name,sizeof(name),1,sizeof(table),&table)==0 && enabled==0);
    const std::string map_name="shroudforge.map";SFMapServiceV1 map{};
    check(KfcRuntimeQueryMapOverlayService(package,sizeof(package),3,reinterpret_cast<const std::uint8_t*>(map_name.data()),unsigned(map_name.size()),1,sizeof(map),&map)==0 && enabled==1);
    fixture.SetAdmission(SF_MAP_FEATURE_PLAYER);fixture.SetSession(99,true,false,true,GetTickCount64());expect_disabled=false;
    SFMapFrameV1 borrowed{};SFMapFrameLeaseV1 lease{};
    check(map.acquire(map.context,99,sizeof(borrowed),&borrowed,&lease)==0 && borrowed.status==SF_MAP_READY && borrowed.session_generation==99);
    check(table.register_frame(table.context,callback,nullptr,&registration)==0);
    OverlayRuntime::Services().Dispatch(800,600,100,1,SFOverlayEncoderV1{});check(callbacks==2);
    check(KfcRuntimeRevokeMapOverlayServices(package,sizeof(package),3)==0);
    check(KfcRuntimeDrainMapOverlayServices(package,sizeof(package),3,0)==6 && disabled==0);
    check(borrowed.session_generation==99 && map.release(map.context,lease)==0);
    check(KfcRuntimeDrainMapOverlayServices(package,sizeof(package),3,0)==0 && disabled==1);
    // Full bounded state must reject, rather than falsely acknowledge, an
    // unknown revocation it cannot durably record.
    for(std::uint64_t generation=100;OverlayRuntime::activations.size()<4096;++generation)
        OverlayRuntime::activations.emplace(std::make_pair(std::string("capacity"),generation),OverlayRuntime::Activation{});
    check(KfcRuntimeRevokeMapOverlayServices(package,sizeof(package),4)!=0);
    check(KfcRuntimeRevokeMapOverlayServices(package,sizeof(package),1)==0);
    std::printf("PASS: %u actual native service terminal-revocation checks\n",checks);
}
