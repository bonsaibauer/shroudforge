#include "abi/runtime_api.h"
#include "map/capture_game_map.h"
#include "graphics/render_overlay.h"
#include "abi/map_overlay_api.h"
#include <nlohmann/json.hpp>
#include <cstring>
#include <string>

namespace {
using nlohmann::json;
json Meta(const SFMapSnapshotMetaV1& value) {
    return {{"status",value.status},{"freshness_limit_ms",value.freshness_limit_ms},
        {"session_generation",std::to_string(value.session_generation)},{"revision",std::to_string(value.revision)},
        {"published_at_ms",std::to_string(value.published_at_ms)},{"clock_domain",std::to_string(value.clock_domain)}};
}
json Pose(const SFMapPoseV1& value) {
    return {{"position",value.position},{"heading_radians",value.heading_radians},{"flags",value.flags}};
}
json Markers(const SFMapMarkerViewV1& value) {
    auto items=json::array();
    for(std::uint32_t i=0;i<value.count;++i) {
        const auto& marker=value.data[i];auto icons=json::array();
        for(std::uint32_t j=0;j<marker.icon_candidate_count && j<10;++j)icons.push_back(marker.icon_candidates[j]);
        items.push_back({{"id",std::to_string(marker.id)},{"position",marker.position},
            {"heading_radians",marker.heading_radians},{"kind",marker.kind},{"flags",marker.flags},
            {"expires_at_ms",std::to_string(marker.expires_at_ms)},{"icon_candidates",std::move(icons)}});
    }
    return {{"meta",Meta(value.meta)},{"items",std::move(items)}};
}
}
extern "C" KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeMapOverlayStatusJson(char* output,size_t capacity) {
    try {
        const auto status=OverlayRuntime::GraphicsStatus();
        const auto target=OverlayRuntime::Services().LastTarget();
        const auto text=json{{"map_prepared",MapRuntime::CapturePrepared()},{"map_reason",MapRuntime::PreparationStatus()},
            {"overlay_prepared",OverlayRuntime::GraphicsPrepared()},{"overlay",{{"status",status.status},{"reason",status.reason},
                {"device_generation",std::to_string(status.device_generation)},{"rendered_frames",std::to_string(status.rendered_frames)},
                {"busy_frames",std::to_string(status.busy_frames)},{"failed_frames",std::to_string(status.failed_frames)},
                {"target",{{"valid",!status.status && target.base.width && target.base.device_generation==status.device_generation},
                    {"width",target.base.width},{"height",target.base.height},{"device_generation",std::to_string(target.base.device_generation)},
                    {"encoding",target.target_encoding},{"diffuse_white_nits",target.diffuse_white_nits},
                    {"linear_unit_nits",target.linear_unit_nits},{"pq_peak_nits",target.pq_peak_nits}}}}}}.dump();
        if(output && capacity>=text.size()+1)std::memcpy(output,text.c_str(),text.size()+1);return text.size()+1;
    }catch(...){return 0;}
}
extern "C" KFC_RUNTIME_API size_t KFC_RUNTIME_CALL KfcRuntimeMapSnapshotJson(
    const std::uint8_t* package,std::uint32_t size,std::uint64_t generation,
    std::uint64_t expected_session,std::uint32_t view,char* output,size_t capacity) {
    try {
        if(view>2)return 0;
        constexpr char name[]="shroudforge.map";
        SFMapServiceV1 service{};SFMapFrameV1 frame{};SFMapFrameLeaseV1 lease{};
        auto status=KfcRuntimeQueryMapOverlayService(package,size,generation,
            reinterpret_cast<const std::uint8_t*>(name),sizeof(name)-1,1,sizeof(service),&service);
        json result{{"service_status",status}};
        if(!status) {
            status=service.acquire(service.context,expected_session,sizeof(frame),&frame,&lease);
            struct Release {SFMapServiceV1& service;SFMapFrameLeaseV1& lease;
                ~Release(){if(lease)service.release(service.context,lease);}} release{service,lease};
            result["service_status"]=status;
            if(!status) {
                std::string tiles;
                if(view==0){
                    constexpr char hex[]="0123456789abcdef";tiles.resize(static_cast<std::size_t>(frame.fog.byte_count)*2);
                    for(std::size_t i=0;i<frame.fog.byte_count;++i){tiles[i*2]=hex[frame.fog.tiles[i]>>4];tiles[i*2+1]=hex[frame.fog.tiles[i]&15];}
                }
                result["frame"]={{"status",frame.status},{"state_flags",frame.state_flags},
                    {"session_generation",std::to_string(frame.session_generation)},{"clock_domain",std::to_string(frame.clock_domain)},
                    {"now_ms",std::to_string(frame.now_ms)},{"supported_features",frame.supported_features}};
                if(view==0)result["frame"].update({
                    {"player",{{"meta",Meta(frame.player.meta)},{"pose",Pose(frame.player.pose)},{"camera",Pose(frame.player.camera)}}},
                    {"world_markers",Markers(frame.world_markers)},{"remote_players",Markers(frame.remote_players)},
                    {"deaths",Markers(frame.deaths)},{"waypoints",Markers(frame.waypoints)},
                    {"pings",Markers(frame.pings)},{"discovery",Markers(frame.discovery)},
                    {"fog",{{"meta",Meta(frame.fog.meta)},{"world_width",frame.fog.world_width},{"world_height",frame.fog.world_height},
                        {"columns",frame.fog.columns},{"rows",frame.fog.rows},{"tiles_hex",std::move(tiles)},{"byte_count",frame.fog.byte_count}}}});
                if(view!=2)result["frame"]["environment"]={{"clock_meta",Meta(frame.environment.clock_meta)},{"weather_meta",Meta(frame.environment.weather_meta)},
                    {"minute_of_day",frame.environment.minute_of_day},{"daytime",frame.environment.daytime},{"weather_kind",frame.environment.weather_kind}};
            }
        }
        const auto text=result.dump();const auto required=text.size()+1;
        if(required>64u*1024u*1024u)return 0;
        // Same complete-buffer/NUL convention as other optional JSON exports.
        if(output && capacity>=required)std::memcpy(output,text.c_str(),required);
        return required;
    }catch(...){return 0;}
}
