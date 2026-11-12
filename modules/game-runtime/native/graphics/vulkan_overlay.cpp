// Host-owned Vulkan mechanisms adapted from pinned Minimap test.4, MIT,
// Copyright (c) 2024 Miguel Oppermann. See ../map/SOURCE-NOTICE.txt.
#include "graphics/render_overlay.h"
#include "graphics/notification_overlay.h"
#include "graphics/vulkan_api.h"
#include "graphics/vulkan_boot_api.h"
#include "engine-hooks/import_pointer.h"
#include "profiles/game_profile.h"
#include "graphics/present_transaction.h"
#include "graphics/shaders/embedded_shaders.h"
#include "map/capture_game_map.h"
#include "map/current_client_map_contract.h"
#include "map/hud_visibility.h"
#include <windows.h>
#include <algorithm>
#include <array>
#include <atomic>
#include <cmath>
#include <cstring>
#include <cstdlib>
#include <sstream>
#include <thread>
#include <unordered_map>
#include <unordered_set>

namespace OverlayRuntime {
namespace {
using namespace Vulkan;
template<class T> T MaxValue(T a,T b) {return (std::max)(a,b);}
template<class T> T ClampValue(T v,T a,T b) {return std::clamp(v,a,b);}
std::string Hex(std::uintptr_t value) {std::ostringstream out;out<<std::hex<<value;return out.str();}
void Log(const std::string& value) {OutputDebugStringA((value+"\n").c_str());}
decltype(&::RaiseFailFastException) raise_shutdown_failure=::RaiseFailFastException;
[[noreturn]] void FailUnresolvedShutdown(const char* operation) {
    OutputDebugStringA("[ShroudForge overlay] fatal shutdown: ");OutputDebugStringA(operation);
    OutputDebugStringA(" cannot confirm GPU completion; refusing unsafe child/parent destruction; process will end (unsaved progress may be lost)\n");
    EXCEPTION_RECORD record{};record.ExceptionCode=0xc0000602;record.NumberParameters=1;
    record.ExceptionInformation[0]=reinterpret_cast<ULONG_PTR>(operation);
    raise_shutdown_failure(&record,nullptr,0);
    std::abort(); // Guaranteed non-return fallback if Windows fail-fast returns.
}
void LogRendererThrottled(const std::string& value) {static std::uint64_t last{};const auto now=GetTickCount64();if(now-last>=5000){last=now;Log(value);}}
template<class T> bool read(std::uintptr_t address,T& value) {return MapRuntime::ReadMemory(address,&value,sizeof(value));}
struct TextureKey {
    std::uint64_t owner{},identity{},revision{};
    bool operator==(const TextureKey&) const=default;
};
struct TextureHash {std::size_t operator()(const TextureKey& k) const {return std::size_t(k.owner^(k.identity*1099511628211ULL)^(k.revision*0x9e3779b97f4a7c15ULL));}};
struct TerrainSlot : TexturedResources {
    TextureKey key{};std::uint32_t map_width{},map_height{},fog_width{},fog_height{};
    std::shared_ptr<const MapRuntime::FogPublication> fog;
};
struct ImageResources {
    std::unordered_map<TextureKey,TexturedResources,TextureHash> textures;
    std::unordered_map<std::uint64_t,TerrainSlot> terrain;
    std::uint64_t allocated_bytes{},fallback_bytes{};
    std::unordered_map<TextureKey,std::shared_ptr<const std::vector<std::uint8_t>>,TextureHash> fallback_maps;
};
struct Renderer : TexturedResources {
    bool ready{},submissionFailed{};
    std::uintptr_t device{},swapchain{},renderPass{},commandPool{};
    std::uint64_t swapchainGeneration{},device_generation{};
    std::uint32_t format{},colorSpace{},width{},height{},queueFamilyIndex{};
    std::vector<std::uintptr_t> images,imageViews,framebuffers,commandBuffers,renderCompleteSemaphores,submissionFences;
    std::vector<ImageResources> resources;
    std::vector<std::uintptr_t> presentFences;
    std::vector<bool> submissionPending,presentPending;
    VulkanRendererFns fns;
};
using DestroyDevice=void(__fastcall*)(void*,const void*);
using DestroySwapchain=void(__fastcall*)(void*,void*,const void*);
using GetDeviceQueue=void(__fastcall*)(void*,std::uint32_t,std::uint32_t,void**);
struct Device {
    std::uintptr_t handle{},queue{};
    std::uint64_t generation{};
    bool alive{true},lost{},broker{},maintenance{},fatal{},uncertain{};
    std::uint32_t family{},capability_reason{};
    GetDeviceProcAddrFn resolver{};
    CreateSwapchainFn create{};GetSwapchainImagesFn images{};QueuePresentFn present{};
    DestroyDevice destroy{};DestroySwapchain destroy_swapchain{};
    WaitFencesFn wait_fences{};
};
std::recursive_mutex render_mutex;
std::unordered_map<std::uintptr_t,std::shared_ptr<Device>> devices;
std::unordered_map<std::uintptr_t,SwapchainRuntimeInfo> swapchains;
std::vector<std::unique_ptr<Renderer>> retired;
Renderer g_renderer;
std::atomic<bool> active{},retirement_pending{};
std::atomic<std::uint64_t> generation_counter{},frames_count{},busy_count{},failed_count{},upload_bytes{},upload_regions{},fallback_count{};
std::atomic<std::uint32_t> graphics_status{3},graphics_reason{SF_OVERLAY_REASON_NONE}; // 1: awaiting observed creation; 2: device lost.
std::atomic<std::uint64_t> published_generation{};
std::atomic<std::uint32_t> g_lastVulkanQueueFamilyIndex{};
std::uintptr_t original_present_fallback{};
void RetireCurrentRenderer();
void DestroyRenderer(Renderer&,bool);
bool TryLoadVulkanRendererFns(VulkanRendererFns&,void*);
bool TryReadFrameShader(const char* name,std::vector<std::uint8_t>& bytes) {
    const std::uint32_t* data{};std::size_t count{};const std::string_view key{name};
    if(key=="texture.vert"){data=Shaders::texture_vert;count=sizeof(Shaders::texture_vert);}
    else if(key=="texture.frag"){data=Shaders::texture_frag;count=sizeof(Shaders::texture_frag);}
    else if(key=="terrain.vert"){data=Shaders::terrain_vert;count=sizeof(Shaders::terrain_vert);}
    else if(key=="terrain.frag"){data=Shaders::terrain_frag;count=sizeof(Shaders::terrain_frag);}
    else return false;
    bytes.resize(count);std::memcpy(bytes.data(),data,count);return true;
}
#include "graphics/vulkan_resources.h"

bool TryLoadVulkanRendererFns(VulkanRendererFns& fns,void* device) {
    const auto found=devices.find(reinterpret_cast<std::uintptr_t>(device));
    return found!=devices.end() && found->second->alive && !found->second->lost &&
        LoadVulkanRendererFnsWithResolver(fns,device,found->second->resolver);
}
bool Completed(Renderer& renderer) {
    const auto found=devices.find(renderer.device);
    if(found==devices.end() || !found->second->alive || found->second->lost || found->second->uncertain)return false;
    const auto completed=[&](const auto& fences,auto& pending) {
        for(std::size_t i=0;i<fences.size();++i)if(i<pending.size() && pending[i]) {
            const auto result=renderer.fns.getFenceStatus(reinterpret_cast<void*>(renderer.device),reinterpret_cast<void*>(fences[i]));
            if(result==-4){found->second->lost=true;graphics_status=7;graphics_reason=SF_OVERLAY_REASON_DEVICE_LOST;return false;}
            if(result!=0)return false;pending[i]=false;
        }return true;
    };
    return completed(renderer.submissionFences,renderer.submissionPending) && completed(renderer.presentFences,renderer.presentPending);
}
void DestroyRenderer(Renderer& renderer,bool abandon) {
    if(!abandon) {
        auto* device=reinterpret_cast<void*>(renderer.device);auto& f=renderer.fns;
        for(auto& image:renderer.resources) {
            for(auto& [_,resource]:image.textures) DestroyTexturedResources(renderer,resource);
            for(auto& [_,resource]:image.terrain) DestroyTexturedResources(renderer,resource);
        }
        for(auto handle:renderer.renderCompleteSemaphores)if(handle && f.destroySemaphore)f.destroySemaphore(device,reinterpret_cast<void*>(handle),nullptr);
        for(auto handle:renderer.presentFences)if(handle && f.destroyFence)f.destroyFence(device,reinterpret_cast<void*>(handle),nullptr);
        for(auto handle:renderer.submissionFences) if(handle && f.destroyFence) f.destroyFence(device,reinterpret_cast<void*>(handle),nullptr);
        for(auto handle:renderer.framebuffers) if(handle && f.destroyFramebuffer) f.destroyFramebuffer(device,reinterpret_cast<void*>(handle),nullptr);
        if(renderer.renderPass && f.destroyRenderPass) f.destroyRenderPass(device,reinterpret_cast<void*>(renderer.renderPass),nullptr);
        for(auto handle:renderer.imageViews) if(handle && f.destroyImageView) f.destroyImageView(device,reinterpret_cast<void*>(handle),nullptr);
        if(renderer.commandPool && f.destroyCommandPool) f.destroyCommandPool(device,reinterpret_cast<void*>(renderer.commandPool),nullptr);
    }
    renderer={};
}
void ReapRetired() {
    std::erase_if(retired,[](auto& renderer){
        const auto found=devices.find(renderer->device);
        const bool invalid=found==devices.end() || !found->second->alive || found->second->lost;
        if(invalid){DestroyRenderer(*renderer,found==devices.end() || !found->second->alive);return true;}
        if(Completed(*renderer)){DestroyRenderer(*renderer,false);return true;} return false;
    });
}
void RetireCurrentRenderer() {
    if(!g_renderer.device) return;
    if(Completed(g_renderer)) DestroyRenderer(g_renderer,false);
    else {retired.push_back(std::make_unique<Renderer>(std::move(g_renderer)));g_renderer={};}
}
bool TextureValid(const SFOverlayTextureV1& image) {
    const auto pixels=std::uint64_t(image.width)*image.height;
    return image.resource_id && image.revision && image.width && image.height && image.width<=4096 && image.height<=4096 &&
        image.rgba && image.rgba_bytes==pixels*4 && image.rgba_bytes<=SF_OVERLAY_MAX_TEXTURE_BYTES &&
        (!image.coverage_mask || image.coverage_mask_bytes>=pixels);
}
bool CopyTexture(const SFOverlayTextureV1& image,std::vector<std::uint8_t>& bytes) {
    if(!TextureValid(image)) return false;
    bytes.resize(std::size_t(image.rgba_bytes));
    if(!MapRuntime::ReadMemory(reinterpret_cast<std::uintptr_t>(image.rgba),bytes.data(),bytes.size())) return false;
    if(image.coverage_mask) {
        std::vector<std::uint8_t> mask(std::size_t(image.width)*image.height);
        if(!MapRuntime::ReadMemory(reinterpret_cast<std::uintptr_t>(image.coverage_mask),mask.data(),mask.size())) return false;
        for(std::size_t i=0;i<mask.size();++i) {if(mask[i]>1) return false;if(!mask[i])bytes[i*4+3]=0;}
    }
    return true;
}
struct Draw {TexturedResources* texture{};std::array<float,32> push{};};
struct RecordContext {
    Renderer* renderer{};ImageResources* image{};void* command{};
    std::uint64_t generation{};DWORD thread{};
    MapRuntime::FrameLease map;
    std::vector<Draw> draws;
    std::unordered_set<TextureKey,TextureHash> used_textures;
    std::unordered_set<std::uint64_t> used_terrain;
    bool failed{},no_work{};
    std::uint64_t notification_frame{};
};
void DrawTextureResource(RecordContext& context,TexturedResources& texture,const float* push) {
    Draw draw;draw.texture=&texture;std::copy_n(push,32,draw.push.begin());context.draws.push_back(draw);
}
TexturedResources* Texture(RecordContext& context,std::uint64_t owner,const SFOverlayTextureV1& image) {
    if(!TextureValid(image)) return nullptr;
    const TextureKey key{owner,image.resource_id,image.revision};auto& cache=context.image->textures;
    if(auto found=cache.find(key);found!=cache.end()){context.used_textures.insert(key);return &found->second;}
    for(auto it=cache.begin();it!=cache.end();) {
        const auto& previous=it->first;
        if(previous.owner!=owner || previous.identity!=image.resource_id || context.used_textures.contains(previous)){++it;continue;}
        context.image->allocated_bytes-=std::uint64_t(it->second.frameTextureWidth)*it->second.frameTextureHeight*4;
        DestroyTexturedResources(*context.renderer,it->second);it=cache.erase(it);
    }
    if(cache.size()>=512 || context.image->allocated_bytes+image.rgba_bytes>UINT64_C(268435456))return nullptr;
    std::vector<std::uint8_t> bytes;if(!CopyTexture(image,bytes))return nullptr;
    TexturedResources resource;
    if(!TryCreateTexturedResourcesLocked(*context.renderer,resource,bytes,int(image.width),int(image.height),false))return nullptr;
    RecordTextureUpload(*context.renderer,resource,context.command);
    context.image->allocated_bytes+=image.rgba_bytes;
    context.used_textures.insert(key);return &cache.emplace(key,resource).first->second;
}
std::int32_t EncodeTexture(RecordContext& context,std::uint64_t owner,const SFOverlayTextureCommandV1& command,
    std::uint32_t square=0,std::uint32_t effect=0,float body=0,float halo=0) {
        if(!std::isfinite(command.rotation_radians) ||
            command.width<=0 || command.height<=0 || command.width>16384 || command.height>16384 || command.clip_radius<0)return 1;
        auto* texture=Texture(context,owner,command.texture);if(!texture){context.failed=true;return 7;}
        const auto sw=command.source_width ? command.source_width : command.texture.width;
        const auto sh=command.source_height ? command.source_height : command.texture.height;
        if(command.source_x>command.texture.width || sw>command.texture.width-command.source_x ||
            command.source_y>command.texture.height || sh>command.texture.height-command.source_y)return 1;
        float push[32]{};
        push[0]=float(command.x);push[1]=float(command.y);push[2]=float(double(command.x)+command.width);push[3]=float(double(command.y)+command.height);
        push[4]=float(command.source_x)/command.texture.width;push[5]=float(command.source_y)/command.texture.height;
        push[6]=float(command.source_x+sw)/command.texture.width;push[7]=float(command.source_y+sh)/command.texture.height;
        push[8]=std::cos(command.rotation_radians);push[9]=std::sin(command.rotation_radians);
        push[10]=float(context.renderer->width);push[11]=float(context.renderer->height);
        push[12]=float(command.clip_center_x);push[13]=float(command.clip_center_y);push[14]=float(command.clip_radius);
        push[15]=float(square);
        for(unsigned c=0;c<4;++c)push[16+c]=float((command.tint_rgba>>(24-8*c))&255)/255;
        push[20]=context.renderer->colorSpace==1000104002 ? 1.f : context.renderer->colorSpace==1000104008 ? 2.f : 0.f;
        push[21]=float(effect);push[24]=body;push[25]=halo;
        DrawTextureResource(context,*texture,push);
    return 0;
}
std::int32_t EncodeTextures(void* opaque,std::uint64_t owner,const SFOverlayTextureCommandV1* commands,std::uint32_t count) {
    auto& context=*static_cast<RecordContext*>(opaque);
    if(context.thread!=GetCurrentThreadId() || !commands || count>SF_OVERLAY_MAX_TEXTURE_COMMANDS || context.draws.size()+count>SF_OVERLAY_MAX_RECTANGLES)return 1;
    for(unsigned i=0;i<count;++i) {SFOverlayTextureCommandV1 c{};
        if(!read(reinterpret_cast<std::uintptr_t>(commands+i),c))return 1;
        if(const auto status=EncodeTexture(context,owner,c))return status;
    }return 0;
}
std::int32_t EncodeTexturesV2(void* opaque,std::uint64_t owner,const SFOverlayTextureCommandV2* commands,std::uint32_t count) {
    auto& context=*static_cast<RecordContext*>(opaque);
    if(context.thread!=GetCurrentThreadId() || !commands || count>SF_OVERLAY_MAX_TEXTURE_COMMANDS || context.draws.size()+count>SF_OVERLAY_MAX_RECTANGLES)return 1;
    for(unsigned i=0;i<count;++i) {SFOverlayTextureCommandV2 c{};
        if(!read(reinterpret_cast<std::uintptr_t>(commands+i),c) || c.square_clip>1 || c.effect>3 ||
            !std::isfinite(c.body_extent) || !std::isfinite(c.halo_radius) || c.base.clip_radius>16384 ||
            (c.effect==0 ? (c.body_extent!=0 || c.halo_radius!=0) :
                (!(c.body_extent>0) || c.body_extent>8192 || !(c.halo_radius>0) || c.halo_radius>64)))return 1;
        if(const auto status=EncodeTexture(context,owner,c.base,c.square_clip,c.effect,c.body_extent,c.halo_radius))return status;
    }return 0;
}
std::int32_t EncodeRectangles(void* opaque,std::uint64_t owner,const SFOverlayRectangleV1* commands,std::uint32_t count) {
    if(!commands || count>SF_OVERLAY_MAX_RECTANGLES)return 1;
    static constexpr std::uint8_t white[]{255,255,255,255};
    for(unsigned i=0;i<count;++i) {
        SFOverlayRectangleV1 rectangle{};if(!read(reinterpret_cast<std::uintptr_t>(commands+i),rectangle))return 1;
        SFOverlayTextureCommandV1 command{};command.texture={UINT64_MAX,1,1,1,white,sizeof(white),nullptr,0};
        command.x=rectangle.x;command.y=rectangle.y;command.width=rectangle.width;command.height=rectangle.height;command.tint_rgba=rectangle.rgba;
        const auto result=EncodeTextures(opaque,0,&command,1);if(result)return result;
    }
    return 0;
}
bool FogValid(const SFOverlayTerrainV1& terrain,const RecordContext& context) {
    const auto& fog=terrain.fog;
    return fog.meta.status==SF_MAP_READY && fog.meta.session_generation==context.map.frame.session_generation &&
        fog.columns && fog.rows && fog.columns<=128 && fog.rows<=128 && fog.tiles &&
        fog.byte_count==std::uint64_t(fog.columns)*fog.rows*1024 && fog.world_width>0 && fog.world_height>0 &&
        std::isfinite(fog.world_width) && std::isfinite(fog.world_height) && context.map.fog &&
        context.map.fog->value->tiles.data()==fog.tiles;
}
void FogTile(std::uint8_t* output,std::uint32_t stride,const MapRuntime::FogSnapshot& fog,std::size_t tile) {
    for(unsigned y=0;y<32;++y)for(unsigned x=0;x<32;++x)
        output[((tile/fog.columns*32+y)*stride+tile%fog.columns*32+x)*4]=fog.tiles[tile*1024+y*32+x];
}
bool PrepareTerrain(RecordContext& context,std::uint64_t owner,const SFOverlayTerrainV1& terrain,TerrainSlot*& output) {
    const bool valid=FogValid(terrain,context);auto& slots=context.image->terrain;
    if(!slots.contains(owner) && slots.size()>=16)return false;
    auto& slot=slots[owner];const auto& map=terrain.world_map;
    const std::uint32_t fw=valid ? terrain.fog.columns*32 : 1,fh=valid ? terrain.fog.rows*32 : 1;
    const auto width=(std::max)(map.width,fw),height=map.height+fh;
    if(width>4096 || height>4096)return false;
    const TextureKey key{owner,map.resource_id,map.revision};
    const bool rebuild=!slot.frameTextureReady || slot.key!=key || slot.map_width!=map.width || slot.map_height!=map.height ||
        slot.fog_width!=fw || slot.fog_height!=fh || (valid && slot.fog && slot.fog->meta.session_generation!=terrain.fog.meta.session_generation);
    if(rebuild) {
        const auto old_bytes=slot.frameTextureReady?std::uint64_t(slot.frameTextureWidth)*slot.frameTextureHeight*4:0;
        const auto new_bytes=std::uint64_t(width)*height*4;
        if(context.image->allocated_bytes-old_bytes+new_bytes>UINT64_C(268435456))return false;
        context.image->allocated_bytes-=old_bytes;
        DestroyTexturedResources(*context.renderer,slot);slot={};
        std::vector<std::uint8_t> pixels;if(!CopyTexture(map,pixels))return false;
        std::vector<std::uint8_t> atlas(std::size_t(width)*height*4);
        for(unsigned y=0;y<map.height;++y)std::memcpy(atlas.data()+std::size_t(y)*width*4,pixels.data()+std::size_t(y)*map.width*4,std::size_t(map.width)*4);
        if(valid)for(std::size_t tile=0;tile<std::size_t(fw/32)*(fh/32);++tile)FogTile(atlas.data()+std::size_t(map.height)*width*4,width,*context.map.fog->value,tile);
        if(!TryCreateTexturedResourcesLocked(*context.renderer,slot,atlas,int(width),int(height),true))return false;
        context.image->allocated_bytes+=new_bytes;
        slot.key=key;slot.map_width=map.width;slot.map_height=map.height;slot.fog_width=fw;slot.fog_height=fh;
        slot.fog=valid ? context.map.fog : nullptr;RecordTextureUpload(*context.renderer,slot,context.command);
        upload_bytes+=std::uint64_t(width)*height*4;upload_regions++;
    } else if(valid && slot.fog!=context.map.fog) {
        std::vector<VkBufferImageCopy> regions;
        for(std::size_t tile=0;tile<terrain.fog.byte_count/1024;++tile) {
            if(slot.fog && !std::memcmp(slot.fog->value->tiles.data()+tile*1024,terrain.fog.tiles+tile*1024,1024))continue;
            VkBufferImageCopy copy{};copy.bufferRowLength=width;copy.imageSubresource.aspectMask=VK_IMAGE_ASPECT_COLOR_BIT;
            copy.imageSubresource.layerCount=1;copy.imageOffset.x=std::int32_t(tile%terrain.fog.columns*32);
            copy.imageOffset.y=std::int32_t(map.height+tile/terrain.fog.columns*32);
            copy.bufferOffset=(std::uint64_t(copy.imageOffset.y)*width+copy.imageOffset.x)*4;copy.imageExtent={32,32,1};regions.push_back(copy);
        }
        if(!regions.empty()) {
            void* mapped{};auto* device=reinterpret_cast<void*>(context.renderer->device);auto* memory=reinterpret_cast<void*>(slot.frameStagingMemory);
            if(context.renderer->fns.mapMemory(device,memory,0,UINT64_MAX,0,&mapped)!=0 || !mapped)return false;
            for(const auto& region:regions) {
                const auto tile=std::size_t((region.imageOffset.y-map.height)/32)*terrain.fog.columns+region.imageOffset.x/32;
                FogTile(static_cast<std::uint8_t*>(mapped)+std::size_t(map.height)*width*4,width,*context.map.fog->value,tile);
            }
            VkMappedMemoryRange range{};range.memory=memory;const auto flushed=context.renderer->fns.flushMappedMemoryRanges(device,1,&range);
            context.renderer->fns.unmapMemory(device,memory);if(flushed!=0)return false;
            RecordTextureUpload(*context.renderer,slot,context.command,false,0,&regions);
            upload_bytes+=std::uint64_t(regions.size())*32*32*4;upload_regions+=regions.size();
        }
        slot.fog=context.map.fog;
    }
    output=&slot;return true;
}
// CPU fallback retains the pinned terrain sampling/lighting and cell-run
// behavior. It runs only when the combined map/fog atlas cannot be prepared.
bool DrawCpuTerrain(RecordContext& context,std::uint64_t owner,const SFOverlayTerrainV1& terrain,bool square=false) {
    auto& maps=context.image->fallback_maps;const TextureKey key{owner,terrain.world_map.resource_id,terrain.world_map.revision};
    auto found=maps.find(key);
    if(found==maps.end()) {
        for(auto it=maps.begin();it!=maps.end();)if(it->first.owner==owner && it->first.identity==key.identity){context.image->fallback_bytes-=it->second->size();it=maps.erase(it);}else ++it;
        if(maps.size()>=8 || context.image->fallback_bytes+terrain.world_map.rgba_bytes>SF_OVERLAY_MAX_TEXTURE_BYTES)return false;auto bytes=std::make_shared<std::vector<std::uint8_t>>();
        if(!CopyTexture(terrain.world_map,*bytes))return false;context.image->fallback_bytes+=bytes->size();found=maps.emplace(key,std::move(bytes)).first;
    }
    const auto& pixels=*found->second;const auto width=terrain.world_map.width,height=terrain.world_map.height;
    static constexpr std::uint8_t white[]{255,255,255,255};
    const SFOverlayTextureV1 white_view{UINT64_MAX,1,1,1,white,sizeof(white),nullptr,0};
    auto* texture=Texture(context,0,white_view);if(!texture)return false;
    const auto sample=[](float fx,float fy,unsigned w,unsigned h,const auto& channel) {
        fx=std::clamp(fx,0.f,float(w-1));fy=std::clamp(fy,0.f,float(h-1));
        const auto x=unsigned(std::floor(fx)),y=unsigned(std::floor(fy)),x1=(std::min)(x+1,w-1),y1=(std::min)(y+1,h-1);
        const auto tx=fx-x,ty=fy-y;
        return (channel(x,y)*(1-tx)+channel(x1,y)*tx)*(1-ty)+(channel(x,y1)*(1-tx)+channel(x1,y1)*tx)*ty;
    };
    const auto radius=terrain.radius,step=terrain.sample_step;const float cosine=std::cos(terrain.heading_radians),sine=std::sin(terrain.heading_radians);
    const bool valid_fog=FogValid(terrain,context);const auto levels=terrain.map_light==0 ? 34.f:48.f;
    const auto emit=[&](int x,int y,int w,int h,const std::array<float,3>& color) {
        if(context.draws.size()>=SF_OVERLAY_MAX_RECTANGLES)return false;float push[32]{};
        push[0]=float(double(terrain.center_pixel_x)+x);push[1]=float(double(terrain.center_pixel_y)+y);
        push[2]=push[0]+w;push[3]=push[1]+h;push[6]=push[7]=push[8]=1;
        push[10]=float(context.renderer->width);push[11]=float(context.renderer->height);
        push[16]=color[0];push[17]=color[1];push[18]=color[2];push[19]=1;
        push[20]=context.renderer->colorSpace==1000104002?1.f:context.renderer->colorSpace==1000104008?2.f:0.f;
        DrawTextureResource(context,*texture,push);return true;
    };
    for(int y=-radius;y<=radius;y+=step) {
        const int h=(std::min)(step,radius-y+1),sy=y+h/2;bool running=false;int start{};std::array<float,3> run{};
        for(int x=-radius;x<=radius;x+=step) {
            const int w=(std::min)(step,radius-x+1),sx=x+w/2,distance_sq=sx*sx+sy*sy;
            if(!square && distance_sq>radius*radius){if(running && !emit(start,y,x-start,h,run))return false;running=false;continue;}
            const float wx=terrain.center_world_x+(cosine*sx-sine*sy)*terrain.units_per_pixel;
            const float wz=terrain.center_world_z-(sine*sx+cosine*sy)*terrain.units_per_pixel;
            const float fx=std::clamp(wx/10240.f,0.f,1.f)*(width-1),fy=std::clamp(1-wz/10240.f,0.f,1.f)*(height-1);
            float revealed=0;
            if(valid_fog && wx>=0 && wz>=0 && wx<=terrain.fog.world_width && wz<=terrain.fog.world_height) {
                revealed=sample(wx*.125f-.5f,(terrain.fog.world_height-wz)*.125f-.5f,terrain.fog.columns*32,terrain.fog.rows*32,
                    [&](unsigned px,unsigned py){const auto tile=std::size_t(py/32)*terrain.fog.columns+px/32;return terrain.fog.tiles[tile*1024+py%32*32+px%32]/255.f;});
            }
            const float strength=terrain.map_light/100.f,style=(std::min)(1.f,strength/.55f);
            const float distance=std::sqrt(float(distance_sq))/radius,vignette=(1.02f-.01f*style)-(.34f-.24f*style)*distance*distance;
            std::array<float,3> color{};constexpr float contrast[]{1.20f,1.22f,1.16f},warmth[]{0,.035f,.115f},unknown[]{.30f,.31f,.28f};
            for(unsigned c=0;c<3;++c) {
                auto value=sample(fx,fy,width,height,[&](unsigned px,unsigned py){return pixels[(std::size_t(py)*width+px)*4+c]/255.f;});
                value=std::clamp((value-.42f)*contrast[c]+.42f,0.f,1.f);
                value=(value+(1-value)*strength*.62f)*(1-warmth[c]*style);value=std::clamp(value*vignette+.004f,0.f,1.f);
                if(terrain.fog_enabled)value=unknown[c]+(value-unknown[c])*revealed;
                color[c]=std::round(std::clamp(value,0.f,1.f)*levels)/levels;
            }
            if(running && run==color)continue;if(running && !emit(start,y,x-start,h,run))return false;
            start=x;run=color;running=true;
        }
        if(running && !emit(start,y,radius+1-start,h,run))return false;
    }
    ++fallback_count;return true;
}
std::int32_t EncodeTerrainInternal(void* opaque,std::uint64_t owner,const SFOverlayTerrainV1* input,bool square) {
    auto& context=*static_cast<RecordContext*>(opaque);SFOverlayTerrainV1 terrain{};
    if(context.thread!=GetCurrentThreadId() || !input || !read(reinterpret_cast<std::uintptr_t>(input),terrain) ||
        !TextureValid(terrain.world_map) || terrain.radius<=0 || terrain.radius>2048 || terrain.sample_step<1 || terrain.sample_step>4 ||
        terrain.map_light<0 || terrain.map_light>100 || !(terrain.units_per_pixel>0) || !std::isfinite(terrain.units_per_pixel) ||
        !std::isfinite(terrain.center_world_x) || !std::isfinite(terrain.center_world_z) || !std::isfinite(terrain.heading_radians))return 1;
    if(!context.used_terrain.insert(owner).second)return 1;
    TerrainSlot* slot{};
    if(!PrepareTerrain(context,owner,terrain,slot)) {if(DrawCpuTerrain(context,owner,terrain,square))return 0;context.failed=true;return 7;}
    const auto radius=terrain.radius;const float left=float(double(terrain.center_pixel_x)-radius),top=float(double(terrain.center_pixel_y)-radius),side=float(radius*2+1);
    const bool fog=FogValid(terrain,context);
    float push[32]{2*left/context.renderer->width-1,2*top/context.renderer->height-1,
        2*(left+side)/context.renderer->width-1,2*(top+side)/context.renderer->height-1,
        terrain.center_world_x,terrain.center_world_z,terrain.units_per_pixel,float(radius),
        std::cos(terrain.heading_radians),std::sin(terrain.heading_radians),float(terrain.sample_step),float(terrain.map_light),
        float(slot->map_width),float(slot->map_height),float(slot->frameTextureWidth),float(slot->frameTextureHeight),
        float(slot->fog_width),float(slot->fog_height),fog?terrain.fog.world_width:0,fog?terrain.fog.world_height:0,
        terrain.fog_enabled?1.f:0.f,fog?1.f:0.f,context.renderer->colorSpace==1000104002?1.f:context.renderer->colorSpace==1000104008?2.f:0.f,square?1.f:0.f,
        float(terrain.center_pixel_x),float(terrain.center_pixel_y),1,0,left,top,0,0};
    DrawTextureResource(context,*slot,push);return 0;
}
std::int32_t EncodeTerrain(void* opaque,std::uint64_t owner,const SFOverlayTerrainV1* input) {
    return EncodeTerrainInternal(opaque,owner,input,false);
}
std::int32_t EncodeTerrainV2(void* opaque,std::uint64_t owner,const SFOverlayTerrainV2* input) {
    SFOverlayTerrainV2 c{};
    if(!input || !read(reinterpret_cast<std::uintptr_t>(input),c) || c.square>1 || c.reserved || c.base.reserved || c.base.fog_enabled>1)return 1;
    return EncodeTerrainInternal(opaque,owner,&c.base,c.square!=0);
}
struct Submission {Renderer* renderer{};void* queue{};const VkPresentInfoKHR* present{};unsigned index{};RecordContext record;};
bool Record(void* opaque) {
    auto& submission=*static_cast<Submission*>(opaque);auto& renderer=*submission.renderer;
    auto* command=reinterpret_cast<void*>(renderer.commandBuffers[submission.index]);auto& f=renderer.fns;
    if(f.resetCommandBuffer(command,0)!=0){submission.record.failed=true;return false;}VkCommandBufferBeginInfo begin{};if(f.beginCommandBuffer(command,&begin)!=0){submission.record.failed=true;return false;}
    renderer.resources.resize(renderer.commandBuffers.size());
    auto& context=submission.record;context.renderer=&renderer;context.image=&renderer.resources[submission.index];context.command=command;
    context.thread=GetCurrentThreadId();context.generation=renderer.device_generation;
    context.notification_frame=NotificationOverlay::BeginFrame();
    context.map=MapRuntime::CapturedFrames().Acquire(0,GetTickCount64());
    if(context.map.frame.supported_features&SF_MAP_FEATURE_HUD){
        context.map.frame.state_flags&=~(SF_MAP_STATE_GAMEPLAY|SF_MAP_STATE_MENU);
        if(MapRuntime::HudKnown(context.map.frame.now_ms))context.map.frame.state_flags|=MapRuntime::HudGameplay(context.map.frame.now_ms)?SF_MAP_STATE_GAMEPLAY:SF_MAP_STATE_MENU;
    }
    const SFOverlayEncoderV2 encoder{{sizeof(encoder),2,&context,0,renderer.device_generation,EncodeRectangles,EncodeTextures,EncodeTerrain},EncodeTexturesV2,EncodeTerrainV2};
    if(!Services().Dispatch(renderer.width,renderer.height,context.map.frame.now_ms,renderer.device_generation,encoder.base,&context.map.frame))context.failed=true;
    if(context.failed || context.draws.empty()){context.no_work=!context.failed && context.draws.empty();return false;}
    VkRenderPassBeginInfo pass{};pass.renderPass=reinterpret_cast<void*>(renderer.renderPass);pass.framebuffer=reinterpret_cast<void*>(renderer.framebuffers[submission.index]);
    pass.renderArea.extent={renderer.width,renderer.height};f.cmdBeginRenderPass(command,&pass,0);
    for(const auto& draw:context.draws) {
        auto* set=reinterpret_cast<void*>(draw.texture->frameDescriptorSet);auto* layout=reinterpret_cast<void*>(draw.texture->framePipelineLayout);
        f.cmdBindPipeline(command,VK_PIPELINE_BIND_POINT_GRAPHICS,reinterpret_cast<void*>(draw.texture->framePipeline));
        f.cmdBindDescriptorSets(command,VK_PIPELINE_BIND_POINT_GRAPHICS,layout,0,1,&set,0,nullptr);
        f.cmdPushConstants(command,layout,VK_SHADER_STAGE_VERTEX_BIT|VK_SHADER_STAGE_FRAGMENT_BIT,0,128,draw.push.data());f.cmdDraw(command,6,1,0,0);
    }
    f.cmdEndRenderPass(command);const bool ended=f.endCommandBuffer(command)==0;if(!ended)context.failed=true;return ended;
}
std::int32_t Fence(void* opaque) {
    auto& s=*static_cast<Submission*>(opaque);auto& r=*s.renderer;auto* device=reinterpret_cast<void*>(r.device);
    if(s.index>=r.presentFences.size() || s.index>=r.presentPending.size())return 1;
    if(r.presentPending[s.index]) {
        const auto result=r.fns.getFenceStatus(device,reinterpret_cast<void*>(r.presentFences[s.index]));
        if(result!=0)return result;r.presentPending[s.index]=false;
    }
    if(s.index<r.submissionPending.size() && r.submissionPending[s.index]) {
        const auto result=r.fns.getFenceStatus(device,reinterpret_cast<void*>(r.submissionFences[s.index]));
        if(result!=0)return result;r.submissionPending[s.index]=false;
    }return 0;
}
std::int32_t Reset(void* opaque) {
    auto& s=*static_cast<Submission*>(opaque);auto& r=*s.renderer;
    void* fences[]{reinterpret_cast<void*>(r.submissionFences[s.index]),reinterpret_cast<void*>(r.presentFences[s.index])};
    return r.fns.resetFences(reinterpret_cast<void*>(r.device),2,fences);
}
std::int32_t Submit(void* opaque) {
    auto& s=*static_cast<Submission*>(opaque);const auto& info=*s.present;std::vector<std::uint32_t> stages(info.waitSemaphoreCount,VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT);
    const void* command=reinterpret_cast<void*>(s.renderer->commandBuffers[s.index]);const void* signal=reinterpret_cast<void*>(s.renderer->renderCompleteSemaphores[s.index]);
    VkSubmitInfo submit{};submit.waitSemaphoreCount=info.waitSemaphoreCount;submit.pWaitSemaphores=info.pWaitSemaphores;submit.pWaitDstStageMask=stages.data();
    submit.commandBufferCount=1;submit.pCommandBuffers=&command;submit.signalSemaphoreCount=1;submit.pSignalSemaphores=&signal;
    const auto result=s.renderer->fns.queueSubmit(s.queue,1,&submit,reinterpret_cast<void*>(s.renderer->submissionFences[s.index]));
    if(result==0)s.renderer->submissionPending[s.index]=true;return result;
}
std::shared_ptr<Device> FindDevice(void* handle) {const auto found=devices.find(reinterpret_cast<std::uintptr_t>(handle));return found==devices.end()?nullptr:found->second;}
std::int32_t __fastcall Present(void*,const void*);
std::int32_t __fastcall Create(void*,const VkSwapchainCreateInfoKHR*,const void*,void**);
std::int32_t __fastcall Images(void*,void*,std::uint32_t*,void*);
void __fastcall Destroy(void*,const void*);
void __fastcall DestroySwap(void*,void*,const void*);
#include "graphics/vulkan_broker.inl"
std::int32_t __fastcall Present(void* queue,const void* pointer) {
    std::lock_guard lock(render_mutex);std::shared_ptr<Device> device;
    if(const auto found=boot_queues.find(queue);found!=boot_queues.end()) {
        device=FindDevice(reinterpret_cast<void*>(found->second.first));
        if(device){device->queue=reinterpret_cast<std::uintptr_t>(queue);device->family=found->second.second;}
    }
    if(!device || !device->alive || device->queue!=reinterpret_cast<std::uintptr_t>(queue))
        for(const auto& [_,candidate]:devices)if(candidate->alive && candidate->queue==reinterpret_cast<std::uintptr_t>(queue)){device=candidate;break;}
    if(device && (!device->alive || device->queue!=reinterpret_cast<std::uintptr_t>(queue)))device.reset();
    const auto original=device ? device->present : reinterpret_cast<QueuePresentFn>(original_present_fallback);
    if(!original)return -1;
    if(device && device->fatal) {
        VkPresentInfoKHR failed{};
        if(read(reinterpret_cast<std::uintptr_t>(pointer),failed) && failed.pResults && failed.swapchainCount<=8)
            for(std::uint32_t i=0;i<failed.swapchainCount;++i)failed.pResults[i]=-4;
        return -4;
    }
    const auto forward=[&](const void* present_info) {
        const auto result=original(queue,present_info);
        if(result==-4 && device){device->lost=true;graphics_status=7;graphics_reason=2;failed_count++;if(g_renderer.device==device->handle)RetireCurrentRenderer();ReapRetired();}
        return result;
    };
    if(retirement_pending.exchange(false))RetireCurrentRenderer();
    ReapRetired();VkPresentInfoKHR info{};std::uintptr_t swapchain{};std::uint32_t index{};
    if(!active || !device || !device->broker || !device->maintenance || device->lost || !read(reinterpret_cast<std::uintptr_t>(pointer),info) ||
        info.waitSemaphoreCount>8 || (info.waitSemaphoreCount && !info.pWaitSemaphores) ||
        !read(reinterpret_cast<std::uintptr_t>(info.pSwapchains),swapchain) || !read(reinterpret_cast<std::uintptr_t>(info.pImageIndices),index))return forward(pointer);
    if(info.swapchainCount!=1){graphics_status=3;graphics_reason=SF_OVERLAY_REASON_PRESENT_CONFLICT;return forward(pointer);}
    const BaseIn* collision{};
    if(!Chain(info.pNext,PresentFenceType,collision) || collision){graphics_status=3;graphics_reason=SF_OVERLAY_REASON_PRESENT_CONFLICT;return forward(pointer);}
    const auto observed=swapchains.find(swapchain);
    if(observed==swapchains.end() || !observed->second.width || !observed->second.height || !observed->second.format) {
        graphics_status=3;graphics_reason=1;LogRendererThrottled("[ShroudForge overlay] waiting for an observed swapchain creation; recreate the swapchain/display mode before the overlay can draw");
        return forward(pointer);
    }
    g_lastVulkanQueueFamilyIndex=device->family;
    if(retired.size()>=4){graphics_status=7;LogRendererThrottled("[ESM overlay] retained presentation resources reached the bounded budget; device recreation required");return forward(pointer);}
    if(!BuildRendererLocked(observed->second) || index>=g_renderer.commandBuffers.size()){graphics_status=3;graphics_reason=SF_OVERLAY_REASON_NONE;return forward(pointer);}
    if(g_renderer.submissionFailed)return forward(pointer);
    g_renderer.device_generation=device->generation;published_generation=device->generation;
    Submission submission{&g_renderer,queue,&info,index};const PresentSteps steps{&submission,Fence,Record,Reset,Submit};
    const auto result=SubmitPresent(steps);
    if(result==PresentResult::Submitted) {
        frames_count++;graphics_status=0;graphics_reason=0;
        const void* signal=reinterpret_cast<void*>(g_renderer.renderCompleteSemaphores[index]);auto adjusted=info;
        void* fence=reinterpret_cast<void*>(g_renderer.presentFences[index]);PresentFences present_fence;
        present_fence.pNext=info.pNext;present_fence.count=1;present_fence.fences=&fence;
        adjusted.pNext=&present_fence;adjusted.waitSemaphoreCount=1;adjusted.pWaitSemaphores=&signal;
        const auto presented=forward(&adjusted);
        if(presented==0||presented==1000001003)
            NotificationOverlay::Presented(submission.record.notification_frame,GetTickCount64());
        // Defined rejection errors still enqueue waits and the present fence.
        const bool enqueued=presented==0 || presented==1000001003 || presented==-1000001004 ||
            presented==-1000000000 || presented==-1000255000 || presented==-1000208000; // VK_ERROR_PRESENT_TIMING_QUEUE_FULL_EXT
        if(enqueued && g_renderer.device==device->handle)g_renderer.presentPending[index]=true;
        if(presented==-1 || presented==-2) {
            // OOM did not enqueue presentation, but our successful submit has
            // changed caller waits. It cannot be reported as recoverable OOM.
            // This is logical failure, not evidence of real driver device loss.
            device->fatal=true;g_renderer.submissionFailed=true;failed_count++;
            graphics_status=7;graphics_reason=SF_OVERLAY_REASON_PRESENT_FATAL;
            if(info.pResults)info.pResults[0]=-4;
            return -4;
        }
        if(!enqueued && presented!=-4) {
            device->fatal=true;g_renderer.submissionFailed=true;failed_count++;
            // The caller still holds this exact queue's external synchronization.
            // Maintenance present fences are covered by vkQueueWaitIdle. Never
            // defer this exceptional recovery to device.queue (which can change).
            auto completion=g_renderer.fns.getFenceStatus(reinterpret_cast<void*>(device->handle),fence);
            if(completion!=0 && completion!=-4 && device->resolver) {
                using QueueWaitIdle=std::int32_t(__fastcall*)(void*);
                const auto idle=reinterpret_cast<QueueWaitIdle>(device->resolver(reinterpret_cast<void*>(device->handle),"vkQueueWaitIdle"));
                if(idle)completion=idle(queue);
            }
            device->uncertain=completion!=0 && completion!=-4;
            graphics_status=7;graphics_reason=device->uncertain?SF_OVERLAY_REASON_PRESENT_UNCERTAIN:SF_OVERLAY_REASON_PRESENT_FATAL;
            if(completion==-4) {
                device->lost=true;graphics_reason=SF_OVERLAY_REASON_DEVICE_LOST;RetireCurrentRenderer();ReapRetired();
            } else if(device->uncertain)LogRendererThrottled("[ESM overlay] unclassified/unsupported present result; ownership uncertain; teardown unresolved");
        }
        return presented;
    }
    if(result==PresentResult::Busy)busy_count++;
    else if(result!=PresentResult::DeviceLost && submission.record.no_work) { /* No consumer emitted work. */ }
    else {failed_count++;g_renderer.submissionFailed=true;graphics_status=7;
        if(result==PresentResult::DeviceLost){device->lost=true;graphics_reason=2;RetireCurrentRenderer();}}
    return forward(pointer);
}
std::int32_t __fastcall Create(void* handle,const VkSwapchainCreateInfoKHR* pointer,const void* allocator,void** output) {
    std::lock_guard lock(render_mutex);auto device=FindDevice(handle);if(!device)return -1;
    const auto result=device->create(handle,pointer,allocator,output);VkSwapchainCreateInfoKHR info{};void* swapchain{};
    if(result==0 && read(reinterpret_cast<std::uintptr_t>(pointer),info) && read(reinterpret_cast<std::uintptr_t>(output),swapchain) &&
        swapchain && info.imageExtent.width && info.imageExtent.height && info.imageExtent.width<=32768 && info.imageExtent.height<=32768 && info.imageFormat) {
        SwapchainRuntimeInfo observed;observed.handle=reinterpret_cast<std::uintptr_t>(swapchain);observed.device=device->handle;
        observed.generation=++generation_counter;observed.width=info.imageExtent.width;observed.height=info.imageExtent.height;
        observed.format=info.imageFormat;observed.colorSpace=info.imageColorSpace;observed.minImageCount=info.minImageCount;swapchains[observed.handle]=std::move(observed);
    }
    return result;
}
std::int32_t __fastcall Images(void* handle,void* swapchain,std::uint32_t* count,void* images) {
    std::lock_guard lock(render_mutex);auto device=FindDevice(handle);if(!device)return -1;
    const auto result=device->images(handle,swapchain,count,images);std::uint32_t size{};
    const auto found=swapchains.find(reinterpret_cast<std::uintptr_t>(swapchain));
    if((result==0 || result==5) && found!=swapchains.end() && count && images && read(reinterpret_cast<std::uintptr_t>(count),size) && size && size<=16) {
        std::vector<std::uintptr_t> values(size);if(MapRuntime::ReadMemory(reinterpret_cast<std::uintptr_t>(images),values.data(),values.size()*8)) {
            if(found->second.images!=values){found->second.images=std::move(values);found->second.generation=++generation_counter;}
        }
    }
    return result;
}
bool QuiesceForDestruction(Device& device,bool) {
    if(!device.alive)return false;
    const auto drain=[&](Renderer& renderer,bool submission) {
        if(renderer.device!=device.handle || device.lost)return true;
        const auto& fences=submission?renderer.submissionFences:renderer.presentFences;
        auto& pending=submission?renderer.submissionPending:renderer.presentPending;
        for(std::size_t i=0;i<fences.size() && !device.lost;++i)if(i<pending.size() && pending[i]) {
            // Only proven successful submissions/enqueued presents may be waited.
            void* fence=reinterpret_cast<void*>(fences[i]);
            if(!device.wait_fences)return false;
            const auto result=device.wait_fences(reinterpret_cast<void*>(device.handle),1,&fence,1,UINT64_MAX);
            if(result==-4){device.lost=true;break;}
            if(result!=0)return false;
            pending[i]=false;
        }
        return true;
    };
    // Submission completion remains provable even when presentation is uncertain.
    bool drained=drain(g_renderer,true);for(auto& renderer:retired)drained=drain(*renderer,true) && drained;
    if((device.uncertain || !drained) && !device.lost) {
        graphics_status=7;graphics_reason=SF_OVERLAY_REASON_PRESENT_UNCERTAIN;return false;
    }
    drained=drain(g_renderer,false);for(auto& renderer:retired)drained=drain(*renderer,false) && drained;
    if(!drained && !device.lost){graphics_status=7;graphics_reason=SF_OVERLAY_REASON_PRESENT_UNCERTAIN;return false;}
    // Lost-device destruction is permitted; no status query/wait follows loss.
    if(g_renderer.device==device.handle)DestroyRenderer(g_renderer,false);
    for(auto& renderer:retired)if(renderer->device==device.handle)DestroyRenderer(*renderer,false);
    std::erase_if(retired,[](const auto& renderer){return !renderer->device;});return true;
}
void __fastcall DestroySwap(void* handle,void* swapchain,const void* allocator) {
    std::lock_guard lock(render_mutex);auto device=FindDevice(handle);if(!device)return;
    if(!QuiesceForDestruction(*device,false))FailUnresolvedShutdown("vkDestroySwapchainKHR");
    if(g_renderer.swapchain==reinterpret_cast<std::uintptr_t>(swapchain))RetireCurrentRenderer();
    swapchains.erase(reinterpret_cast<std::uintptr_t>(swapchain));ReapRetired();device->destroy_swapchain(handle,swapchain,allocator);
}
void __fastcall Destroy(void* handle,const void* allocator) {
    std::lock_guard lock(render_mutex);auto device=FindDevice(handle);if(!device)return;
    if(!QuiesceForDestruction(*device,true))FailUnresolvedShutdown("vkDestroyDevice");
    if(g_renderer.device==device->handle)RetireCurrentRenderer();ReapRetired();
    // No host wait, fence query or resource use may occur after the original
    // device destructor. Owned children have already been drained or destroyed
    // under the lost-device rules before forwarding that destructor.
    device->alive=false;device->lost=true;
    std::erase_if(boot_queues,[&](const auto& p){return p.second.first==device->handle;});
    std::erase_if(swapchains,[&](const auto& entry){return entry.second.device==device->handle;});
    published_generation=++generation_counter;graphics_status=device->uncertain?7:3;graphics_reason=device->uncertain?SF_OVERLAY_REASON_PRESENT_UNCERTAIN:SF_OVERLAY_REASON_AWAIT_OBSERVED_CREATE;device->destroy(handle,allocator);devices.erase(device->handle);
}
}
bool PrepareGraphicsBoot(bool demand) {
    std::lock_guard lock(render_mutex);if(boot_armed)return true;boot_demand=demand;
    if(!demand){BootUnavailable(SF_OVERLAY_REASON_BOOT_NOT_ARMED,"native overlay was not requested at startup; enable its package/service and restart");return true;}
    namespace Profile=KfcRuntimeCompatibility::EnshroudedClient;
    auto* image=reinterpret_cast<std::uint8_t*>(GetModuleHandleW(nullptr));
    const bool renderer_admitted=MapRuntime::Current5d9ef2f1::Selected(Profile::image_sha256)
        ? MapRuntime::PreparedRendererAdmitted()
        : MapRuntime::InspectMapImage({image,Profile::image_size},Profile::image_sha256,true).renderer;
    if(!renderer_admitted) {
        BootUnavailable(SF_OVERLAY_REASON_BOOT_NOT_ARMED,"exact executable/renderer admission unavailable; boot resolver interception refused");return false;
    }
    auto* cell=EngineHooks::FindNamedImport({image,Profile::image_size},"KERNEL32.dll","GetProcAddress");
    HMODULE pinned{};
    if(!cell || !GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS|GET_MODULE_HANDLE_EX_FLAG_PIN,reinterpret_cast<LPCWSTR>(&PrepareGraphicsBoot),&pinned)) {
        BootUnavailable(SF_OVERLAY_REASON_BOOT_MISSED,"GetProcAddress import coverage unavailable; restart with a supported boot path");return false;
    }
    // Do not chain a foreign resolver whose callback/module lifetime we do not own.
    const auto canonical=reinterpret_cast<std::uintptr_t>(&::GetProcAddress);
    if(*cell!=canonical) {
        BootUnavailable(SF_OVERLAY_REASON_BOOT_MISSED,"GetProcAddress import is already owned by another interceptor; overlay boot refused");return false;
    }
    boot_get_proc=::GetProcAddress;
    if(!EngineHooks::ReplaceImportPointer(cell,reinterpret_cast<std::uintptr_t>(boot_get_proc),reinterpret_cast<std::uintptr_t>(&BrokerGetProc))) {
        BootUnavailable(SF_OVERLAY_REASON_BOOT_MISSED,"GetProcAddress import ownership conflict; overlay remains unavailable");return false;
    }
    boot_import=cell;boot_armed=true;graphics_reason=SF_OVERLAY_REASON_BOOT_MISSED;return true;
}
bool StartGraphics() {
    std::lock_guard lock(render_mutex);if(active)return true;
    if(!MapRuntime::RendererAdmitted())return false;
    active=true;
    if(!boot_armed)BootUnavailable(SF_OVERLAY_REASON_BOOT_NOT_ARMED,"overlay requested after boot; enable native overlay demand and restart");
    else if(devices.empty())BootUnavailable(SF_OVERLAY_REASON_BOOT_MISSED,"device creation was not observed; restart with the early Vulkan broker");
    else if(std::none_of(devices.begin(),devices.end(),[](const auto& item){return item.second->alive && item.second->maintenance && !item.second->fatal;}))
        BootUnavailable(devices.begin()->second->capability_reason,"no observed device has supported enabled presentation fences; inspect boot/feature status and restart after repair");
    return true;
}
void StopGraphics() {
    active=false;graphics_status=3;graphics_reason=SF_OVERLAY_REASON_NONE;retirement_pending=true;
    std::unique_lock lock(render_mutex,std::try_to_lock);if(!lock.owns_lock())return;
    retirement_pending=false;RetireCurrentRenderer();ReapRetired();
}
void MaintainGraphics() {std::unique_lock lock(render_mutex,std::try_to_lock);if(!lock.owns_lock())return;if(retirement_pending.exchange(false))RetireCurrentRenderer();ReapRetired();}
SFOverlayStatusV1 GraphicsStatus() {return {sizeof(SFOverlayStatusV1),1,graphics_status.load(),graphics_reason.load(),published_generation.load(),frames_count.load(),busy_count.load(),failed_count.load(),upload_bytes.load(),upload_regions.load(),fallback_count.load()};}
}
