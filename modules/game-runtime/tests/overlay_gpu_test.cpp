// Offline fake dispatch exercises the actual private renderer implementation.
// It neither loads Vulkan nor executes any game address.
#include "graphics/vulkan_overlay.cpp"
#include <cstdio>
#include <cstdlib>
#include <future>
using namespace OverlayRuntime;
namespace {
unsigned checks{},queries{},destroys{},copies{},barriers{},idle_waits{};int fence_result{};bool device_destroyed{};
std::vector<std::uintptr_t> lifetime_events;
int recovery_fence_result=1,idle_result{},cold_wait_result{};bool unknown_present_seen{},caller_queue_synchronized{},mutate_queue{};std::vector<std::uintptr_t> idle_queues;
void check_named(const char* name,bool value){++checks;if(!value){std::fprintf(stderr,"FAIL: %s (GPU check %u)\n",name,checks);std::exit(1);}}
std::vector<std::uint8_t> mapped(32*34*4);std::vector<VkBufferImageCopy> copied;
void check(bool value){++checks;if(!value){std::fprintf(stderr,"GPU check %u failed\n",checks);std::exit(1);}}
std::int32_t __fastcall FakeFence(void*,void* fence) {check(!device_destroyed);++queries;const auto handle=reinterpret_cast<std::uintptr_t>(fence);if(handle>=100)return 1;if(unknown_present_seen && handle==13)return recovery_fence_result;return fence_result;}
std::int32_t __fastcall FakeIdle(void* queue){check_named("exceptional idle occurs before Present returns under caller queue synchronization",caller_queue_synchronized);++idle_waits;idle_queues.push_back(reinterpret_cast<std::uintptr_t>(queue));lifetime_events.push_back(30000);return idle_result;}
void* __fastcall RecoveryResolver(void*,const char* name){return std::strcmp(name,"vkQueueWaitIdle")==0?reinterpret_cast<void*>(&FakeIdle):nullptr;}
void __fastcall FakeDestroy(void*,void* handle,const void*) {check(!device_destroyed);lifetime_events.push_back(1000+reinterpret_cast<std::uintptr_t>(handle));++destroys;}
std::int32_t __fastcall FakeMap(void*,void*,std::uint64_t,std::uint64_t,std::uint32_t,void** output){*output=mapped.data();return 0;}
void __fastcall FakeUnmap(void*,void*){}
std::int32_t __fastcall FakeFlush(void*,std::uint32_t,const VkMappedMemoryRange*){return 0;}
void __fastcall FakeBarrier(void*,std::uint32_t,std::uint32_t,std::uint32_t,std::uint32_t,const void*,std::uint32_t,const void*,std::uint32_t,const VkImageMemoryBarrier* image){check(image && image->sType==45 && (image->newLayout==7 || image->newLayout==5));++barriers;}
void __fastcall FakeCopy(void*,void*,void*,std::uint32_t,std::uint32_t count,const VkBufferImageCopy* regions){++copies;copied.assign(regions,regions+count);}
std::int32_t __fastcall FakeCreate(void*,const VkSwapchainCreateInfoKHR*,const void*,void** output){*output=reinterpret_cast<void*>(88);return 0;}
std::int32_t reset_result{},submit_result{},record_result{},present_result{};std::uintptr_t seen_wait{};unsigned submits{},draws{},present_calls{},cold_waits{};std::vector<std::uintptr_t> waited;
const void* seen_present_input{};const void* seen_next{};std::uintptr_t seen_present_fence{};
std::int32_t __fastcall FakeWait(void*,std::uint32_t n,void*const* fences,std::uint32_t all,std::uint64_t timeout){check(n==1 && all==1 && timeout==UINT64_MAX && !device_destroyed);++cold_waits;waited.push_back(reinterpret_cast<std::uintptr_t>(*fences));lifetime_events.push_back(reinterpret_cast<std::uintptr_t>(*fences));return cold_wait_result;}
std::int32_t __fastcall FakePresent(void*,const void* input){++present_calls;seen_present_input=input;unknown_present_seen=present_result==-13;if(unknown_present_seen && mutate_queue)devices.at(477)->queue=999;const auto& info=*static_cast<const VkPresentInfoKHR*>(input);seen_next=info.pNext;seen_present_fence=0;if(info.pNext && static_cast<const BaseIn*>(info.pNext)->sType==PresentFenceType){const auto* node=static_cast<const PresentFences*>(info.pNext);seen_present_fence=reinterpret_cast<std::uintptr_t>(node->fences[0]);seen_next=node->pNext;}if(info.pResults)info.pResults[0]=present_result;seen_wait=info.waitSemaphoreCount?reinterpret_cast<std::uintptr_t>(info.pWaitSemaphores[0]):0;return present_result;}
unsigned family_query_calls{};std::uint32_t family_count=3;
void __fastcall FakeQueueFamilies(void* physical,std::uint32_t* count,QueueFamilyProperties* properties) {
    check(physical==reinterpret_cast<void*>(901));++family_query_calls;
    if(!properties){*count=family_count;return;}
    check(*count==3);properties[0]={QueueGraphicsBit,1,0,{}};
    properties[1]={QueueGraphicsBit,1,0,{}};properties[2]={0,1,0,{}};
}
void* __fastcall FakeQueueGipa(void* instance,const char* name) {
    check(instance==reinterpret_cast<void*>(900) && !std::strcmp(name,"vkGetPhysicalDeviceQueueFamilyProperties"));
    return reinterpret_cast<void*>(&FakeQueueFamilies);
}
Renderer ReadyRenderer(std::uintptr_t handle);
void PreparePresent(std::uint64_t owner,std::uintptr_t handle=77,std::uintptr_t chain=88) {
    auto& info=swapchains[chain];g_renderer=ReadyRenderer(handle);g_renderer.queueFamilyIndex=devices.at(handle)->family;g_renderer.swapchain=chain;g_renderer.width=info.width;g_renderer.height=info.height;g_renderer.format=info.format;g_renderer.colorSpace=info.colorSpace;g_renderer.swapchainGeneration=info.generation;g_renderer.commandBuffers={20};g_renderer.resources.resize(1);
    TexturedResources white;white.frameTextureReady=true;white.frameTextureWidth=white.frameTextureHeight=1;
    g_renderer.resources[0].textures.emplace(TextureKey{0,UINT64_MAX,1},white);g_renderer.resources[0].allocated_bytes=4;
    auto& f=g_renderer.fns;f.resetCommandBuffer=[](void*,std::uint32_t)->std::int32_t{return record_result;};f.beginCommandBuffer=[](void*,const VkCommandBufferBeginInfo*)->std::int32_t{return 0;};f.endCommandBuffer=[](void*)->std::int32_t{return 0;};
    f.cmdBeginRenderPass=[](void*,const VkRenderPassBeginInfo*,std::uint32_t){};f.cmdEndRenderPass=[](void*){};
    f.cmdBindPipeline=[](void*,std::uint32_t,void*){};f.cmdBindDescriptorSets=[](void*,std::uint32_t,void*,std::uint32_t,std::uint32_t,void*const*,std::uint32_t,const std::uint32_t*){};
    f.cmdPushConstants=[](void*,void*,std::uint32_t,std::uint32_t,std::uint32_t size,const void*){check(size==128);};f.cmdDraw=[](void*,std::uint32_t,std::uint32_t,std::uint32_t,std::uint32_t){++draws;};
    f.resetFences=[](void*,std::uint32_t,void*const*)->std::int32_t{return reset_result;};
    f.queueSubmit=[](void*,std::uint32_t,const VkSubmitInfo*,void*)->std::int32_t{++submits;return submit_result;};
}
Renderer ReadyRenderer(std::uintptr_t handle) {
    Renderer r;r.device=handle;r.swapchain=88;r.ready=true;r.width=800;r.height=600;r.submissionFences={10};r.framebuffers={11};r.renderCompleteSemaphores={12};r.presentFences={13};r.submissionPending={true};r.presentPending={false};
    r.fns.destroySemaphore=FakeDestroy;r.fns.getFenceStatus=FakeFence;r.fns.destroyFramebuffer=FakeDestroy;r.fns.destroyFence=FakeDestroy;
    r.fns.mapMemory=FakeMap;r.fns.unmapMemory=FakeUnmap;r.fns.flushMappedMemoryRanges=FakeFlush;r.fns.cmdCopyBufferToImage=FakeCopy;r.fns.cmdPipelineBarrier=FakeBarrier;return r;
}
void PresentShutdownRegression(std::uint64_t owner,int fence_status,int idle_status,bool via_swap,bool lost,std::int32_t present_status=-13) {
    auto device=std::make_shared<Device>();device->handle=477;device->queue=407;device->family=5;device->queue_families.resize(6);device->queue_families[5].flags=QueueGraphicsBit;device->generation=3;device->broker=device->maintenance=true;
    device->present=FakePresent;device->wait_fences=FakeWait;device->resolver=RecoveryResolver;
    device->destroy_swapchain=[](void*,void*,const void*){lifetime_events.push_back(9000);};
    device->destroy=[](void*,const void*){lifetime_events.push_back(10000);};devices[477]=device;
    auto info=swapchains.at(88);info.device=477;info.handle=488;swapchains[488]=info;
    PreparePresent(owner,477,488);fence_result=0;recovery_fence_result=fence_status;idle_result=idle_status;unknown_present_seen=false;
    // Busy work from a different queue on the same device, and another device,
    // must retain its own completion evidence across current-queue recovery.
    auto previous=std::make_unique<Renderer>(ReadyRenderer(477));previous->swapchain=489;previous->submissionFences={110};previous->presentFences={113};previous->renderCompleteSemaphores={112};previous->framebuffers={111};previous->presentPending={true};
    auto* previous_ptr=previous.get();retired.push_back(std::move(previous));
    auto foreign=std::make_shared<Device>();foreign->handle=555;foreign->wait_fences=FakeWait;devices[555]=foreign;
    auto unrelated=std::make_unique<Renderer>(ReadyRenderer(555));unrelated->submissionFences={210};unrelated->presentFences={213};unrelated->presentPending={true};
    auto* unrelated_ptr=unrelated.get();retired.push_back(std::move(unrelated));
    const void* chain=reinterpret_cast<void*>(488);const void* semaphore=reinterpret_cast<void*>(99);std::uint32_t index=0;std::int32_t individual=0;
    BaseIn game_next{123,nullptr};VkPresentInfoKHR present{};present.pNext=&game_next;present.swapchainCount=1;present.pSwapchains=&chain;present.pImageIndices=&index;present.waitSemaphoreCount=1;present.pWaitSemaphores=&semaphore;present.pResults=&individual;
    present_result=present_status;lifetime_events.clear();idle_queues.clear();const auto calls=present_calls,waits=cold_waits;
    boot_queues[reinterpret_cast<void*>(407)]={477,5};mutate_queue=caller_queue_synchronized=true;
    const auto result=Present(reinterpret_cast<void*>(407),&present);
    mutate_queue=caller_queue_synchronized=false;const bool observed_lost=device->lost;
    check_named("present preserves caller result/pResults/pNext and exactly one original call",
        result==present_status && individual==present_status && present.pNext==&game_next && seen_next==&game_next && present_calls==calls+1);
    if(!lost)check_named("current present recovery never clears other-queue pending evidence",previous_ptr->submissionPending[0] && previous_ptr->presentPending[0]);
    check_named("current present recovery never clears other-device pending evidence",unrelated_ptr->submissionPending[0] && unrelated_ptr->presentPending[0]);
    const bool unknown=present_status==-13;
    if(unknown){const auto after_present=submits;
        check_named("fatal wrapper stops the next overlay without a second original present",Present(reinterpret_cast<void*>(407),&present)==-4 && individual==-4 && submits==after_present && present_calls==calls+1);}
    if(via_swap)DestroySwap(reinterpret_cast<void*>(477),reinterpret_cast<void*>(488),nullptr);
    else Destroy(reinterpret_cast<void*>(477),nullptr);
    std::vector<std::uintptr_t> expected;
    if(unknown && fence_status!=0 && fence_status!=-4)expected.push_back(30000);
    if(!lost){expected.push_back(10);expected.push_back(110);if(!unknown)expected.push_back(13);expected.push_back(113);}
    if(lost)expected.insert(expected.end(),{1112,1113,1110,1111,1012,1013,1010,1011});
    else expected.insert(expected.end(),{1012,1013,1010,1011,1112,1113,1110,1111});
    expected.push_back(via_swap?9000u:10000u);
    check_named("actual DestroySwap/Destroy disposes active and retired children before forwarding",lifetime_events==expected);
    check_named("recovery uses exact present queue once, affirmative fence uses no idle, real loss uses no cold wait",
        idle_queues==(unknown && fence_status!=0 && fence_status!=-4?std::vector<std::uintptr_t>({407}):std::vector<std::uintptr_t>{}) &&
        cold_waits==waits+(lost?0:unknown?3:4) && device->fatal==unknown && !device->uncertain && observed_lost==lost);
    check_named("other device survives actual destruction callback",FindDevice(reinterpret_cast<void*>(555)) && retired.size()==1 && retired[0].get()==unrelated_ptr);
    if(via_swap)Destroy(reinterpret_cast<void*>(477),nullptr);
    check_named("recovered actual Destroy forwards after owned disposal",lifetime_events.back()==10000 && !FindDevice(reinterpret_cast<void*>(477)));
    // Finish the unrelated fixture through its actual callback too.
    foreign->destroy=[](void*,const void*){lifetime_events.push_back(20000);};Destroy(reinterpret_cast<void*>(555),nullptr);
    check_named("other-device destruction drains its own fences",waited[waited.size()-2]==210 && waited.back()==213 && retired.empty() && lifetime_events.back()==20000);
    present_result=0;unknown_present_seen=false;
}
void UnrecoverablePresentPolicySeam(std::uint64_t owner) {
    auto device=devices.at(77);device->resolver=RecoveryResolver;
    PreparePresent(owner);fence_result=0;recovery_fence_result=1;idle_result=-1;unknown_present_seen=false;present_result=-13;
    const void* chain=reinterpret_cast<void*>(88);std::uint32_t index=0;VkPresentInfoKHR present{};present.swapchainCount=1;present.pSwapchains=&chain;present.pImageIndices=&index;
    const auto idle_before=idle_waits,waits=cold_waits;
    caller_queue_synchronized=true;
    check_named("non-device-lost recovery error remains uncertain",Present(reinterpret_cast<void*>(7),&present)==-13 && device->uncertain && device->fatal && !device->lost && idle_waits==idle_before+1);
    caller_queue_synchronized=false;
    check_named("unrecoverable drain still waits the known submission, never the unproven present",!QuiesceForDestruction(*device,true) && cold_waits==waits+1 && waited.back()==10 && !g_renderer.submissionPending[0] && !g_renderer.presentPending[0]);
    // Policy decision pending: do not execute the existing unsafe destructor
    // branch or introduce a process kill/suppressed original call in this test.
    g_renderer={};device->uncertain=device->fatal=false;present_result=0;unknown_present_seen=false;idle_result=0;
}
struct ShutdownDiagnostic {const char* operation;};
void WINAPI CaptureShutdownDiagnostic(PEXCEPTION_RECORD record,PCONTEXT,DWORD) {
    check_named("terminal diagnostic carries fail-fast code and exact unsafe parent operation",record && record->ExceptionCode==0xc0000602 && record->NumberParameters==1);
    lifetime_events.push_back(40000);throw ShutdownDiagnostic{reinterpret_cast<const char*>(record->ExceptionInformation[0])};
}
void UnrecoverableWrapperRegression(bool swap,bool failed_submission) {
    const auto original_raise=raise_shutdown_failure;raise_shutdown_failure=CaptureShutdownDiagnostic;
    auto device=std::make_shared<Device>();device->handle=677;device->queue=607;device->wait_fences=FakeWait;device->uncertain=!failed_submission;devices[677]=device;
    device->destroy_swapchain=[](void*,void*,const void*){lifetime_events.push_back(9000);};device->destroy=[](void*,const void*){lifetime_events.push_back(10000);};
    g_renderer=ReadyRenderer(677);g_renderer.swapchain=688;swapchains[688].device=677;
    cold_wait_result=failed_submission?-1:0;lifetime_events.clear();const auto waits=cold_waits,freed=destroys;bool diagnosed=false;
    try {if(swap)DestroySwap(reinterpret_cast<void*>(677),reinterpret_cast<void*>(688),nullptr);else Destroy(reinterpret_cast<void*>(677),nullptr);}
    catch(const ShutdownDiagnostic& diagnostic){diagnosed=std::strcmp(diagnostic.operation,swap?"vkDestroySwapchainKHR":"vkDestroyDevice")==0;}
    check_named(swap?"unresolved actual DestroySwap fails fast before children/original parent":"unresolved actual Destroy fails fast before children/original parent",
        diagnosed && lifetime_events==std::vector<std::uintptr_t>({10,40000}) && cold_waits==waits+1 && destroys==freed && g_renderer.device==677 && devices.contains(677) && swapchains.contains(688));
    check_named("failed known-submit drain does not loop or fabricate completion/loss",g_renderer.submissionPending[0]==failed_submission && !device->lost);
    // The substituted OS fail-fast throws only in this offline fixture. The
    // production function cannot return; isolated fixture state is discarded.
    raise_shutdown_failure=original_raise;cold_wait_result=0;g_renderer={};devices.erase(677);swapchains.erase(688);
}
}
namespace {
unsigned program_pipelines{},program_destroyed{},program_pool_destroyed{},program_images_destroyed{},program_writes{};
bool fail_program_pipeline{};
std::uintptr_t next_program_handle=2000;
void* ProgramHandle(){return reinterpret_cast<void*>(++next_program_handle);}
void ProgramFns(Renderer& r){
    auto& f=r.fns;
    f.createImageView=[](void*,const VkImageViewCreateInfo*,const void*,void** out)->std::int32_t{*out=ProgramHandle();return 0;};
    f.destroyImageView=[](void*,void*,const void*){};
    f.createBuffer=[](void*,const VkBufferCreateInfo*,const void*,void** out)->std::int32_t{*out=ProgramHandle();return 0;};
    f.destroyBuffer=[](void*,void*,const void*){};
    f.getBufferMemoryRequirements=[](void*,void*,VkMemoryRequirements* out){out->size=4096;out->memoryTypeBits=1;};
    f.createImage=[](void*,const VkImageCreateInfo*,const void*,void** out)->std::int32_t{*out=ProgramHandle();return 0;};
    f.destroyImage=[](void*,void*,const void*){++program_images_destroyed;};
    f.getImageMemoryRequirements=f.getBufferMemoryRequirements;
    f.allocateMemory=[](void*,const VkMemoryAllocateInfo*,const void*,void** out)->std::int32_t{*out=ProgramHandle();return 0;};
    f.freeMemory=[](void*,void*,const void*){};
    f.bindBufferMemory=[](void*,void*,void*,std::uint64_t)->std::int32_t{return 0;};f.bindImageMemory=f.bindBufferMemory;
    f.createSampler=[](void*,const VkSamplerCreateInfo*,const void*,void** out)->std::int32_t{*out=ProgramHandle();return 0;};
    f.destroySampler=[](void*,void*,const void*){};
    f.createShaderModule=[](void*,const VkShaderModuleCreateInfo* info,const void*,void** out)->std::int32_t{
        check(info->pCode && info->codeSize>=20 && info->pCode[0]==0x07230203);*out=ProgramHandle();return 0;
    };
    f.destroyShaderModule=[](void*,void*,const void*){};
    f.createDescriptorSetLayout=[](void*,const VkDescriptorSetLayoutCreateInfo* info,const void*,void** out)->std::int32_t{
        check(info->bindingCount==4);*out=ProgramHandle();return 0;
    };
    f.destroyDescriptorSetLayout=[](void*,void*,const void*){};
    f.createDescriptorPool=[](void*,const VkDescriptorPoolCreateInfo* info,const void*,void** out)->std::int32_t{
        check(info->maxSets==64 && info->pPoolSizes[0].descriptorCount==256);*out=ProgramHandle();return 0;
    };
    f.destroyDescriptorPool=[](void*,void*,const void*){++program_pool_destroyed;};
    f.allocateDescriptorSets=[](void*,const VkDescriptorSetAllocateInfo*,void** out)->std::int32_t{*out=ProgramHandle();return 0;};
    f.updateDescriptorSets=[](void*,std::uint32_t count,const VkWriteDescriptorSet* writes,std::uint32_t,const void*){
        check(count==4);for(unsigned i=0;i<4;++i)check(writes[i].dstBinding==i && writes[i].pImageInfo->imageView && writes[i].pImageInfo->sampler);++program_writes;
    };
    f.createPipelineLayout=[](void*,const VkPipelineLayoutCreateInfo* info,const void*,void** out)->std::int32_t{
        check(info->pushConstantRangeCount==1 && info->pPushConstantRanges[0].stageFlags==VK_SHADER_STAGE_FRAGMENT_BIT && info->pPushConstantRanges[0].size==128);*out=ProgramHandle();return 0;
    };
    f.destroyPipelineLayout=[](void*,void*,const void*){};
    f.createGraphicsPipelines=[](void*,void*,std::uint32_t,const VkGraphicsPipelineCreateInfo* info,const void*,void** out)->std::int32_t{
        check(info->pDynamicState && info->pViewportState->pViewports==nullptr && info->pColorBlendState->pAttachments[0].blendEnable);
        ++program_pipelines;*out=ProgramHandle();return fail_program_pipeline?-2:0;
    };
    f.destroyPipeline=[](void*,void*,const void*){++program_destroyed;};
    f.cmdSetViewport=[](void*,std::uint32_t,std::uint32_t,const VkViewport*){};
    f.cmdSetScissor=[](void*,std::uint32_t,std::uint32_t,const VkRect2D*){};
    f.cmdBindPipeline=[](void*,std::uint32_t,void*){};
    f.cmdBindDescriptorSets=[](void*,std::uint32_t,void*,std::uint32_t,std::uint32_t,void*const*,std::uint32_t,const std::uint32_t*){};
    f.cmdPushConstants=[](void*,void*,std::uint32_t,std::uint32_t,std::uint32_t,const void*){};
    f.cmdDraw=[](void*,std::uint32_t,std::uint32_t,std::uint32_t,std::uint32_t){++draws;};
    f.resetCommandBuffer=[](void*,std::uint32_t)->std::int32_t{return 0;};
    f.beginCommandBuffer=[](void*,const VkCommandBufferBeginInfo*)->std::int32_t{return 0;};
    f.endCommandBuffer=[](void*)->std::int32_t{return 0;};
}
void GenericPrograms(){
    static_assert(sizeof(SFOverlayShaderV1)==40 && sizeof(SFOverlayProgramCommandV1)==80);
    static_assert(sizeof(SFOverlayEncoderV1)==48 && sizeof(SFOverlayEncoderV2)==56 && sizeof(SFOverlayEncoderV3)==64);
    static_assert(sizeof(SFOverlayFrameV1)==32 && sizeof(SFOverlayFrameV3)==64);
    std::vector<std::uint32_t> shader(std::begin(Shaders::texture_frag),std::end(Shaders::texture_frag));
    check_named("complete validator accepts the fixed fragment interface",ValidateOverlayFragment(shader));
    auto invalid=shader;invalid.pop_back();check(!ValidateOverlayFragment(invalid));
    invalid=shader;invalid[5]=0;check(!ValidateOverlayFragment(invalid));
    invalid=shader;invalid[3]=65537;check(!ValidateOverlayFragment(invalid));
    invalid=shader;
    for(std::size_t at=5;at<invalid.size();at+=invalid[at]>>16)if((invalid[at]&65535)==71 && invalid[at+2]==34){invalid[at+3]=1;break;}
    check_named("a valid Vulkan descriptor set outside the fixed contract is refused",!ValidateOverlayFragment(invalid));
    Renderer renderer=ReadyRenderer(977);renderer.device_generation=42;renderer.commandBuffers={20};renderer.resources.resize(1);ProgramFns(renderer);
    RecordContext context;context.renderer=&renderer;context.image=&renderer.resources[0];context.thread=GetCurrentThreadId();context.generation=42;
    std::uint8_t pixel[]{1,2,3,255};SFOverlayTextureV1 image{1,1,1,1,pixel,4,nullptr,0};std::uint32_t constant=0x12345678;
    SFOverlayProgramCommandV1 command{};command.shader={sizeof(SFOverlayShaderV1),1,1,1,shader.data(),unsigned(shader.size()*4),0};
    command.x=10;command.y=20;command.width=30;command.height=40;command.texture_count=1;command.textures=&image;command.constant_bytes=4;command.constants=&constant;
    auto bad=command;bad.shader.spirv=invalid.data();
    check(EncodePrograms(&context,7,reinterpret_cast<const SFOverlayProgramCommandV1*>(UINTPTR_MAX-16),2)==1 && program_pipelines==0);
    check(EncodePrograms(&context,7,&bad,1)==1 && program_pipelines==0 && context.image->textures.empty());
    bad=command;bad.width=2000;check(EncodePrograms(&context,7,&bad,1)==1 && program_pipelines==0);
    bad=command;bad.constant_bytes=129;check(EncodePrograms(&context,7,&bad,1)==1 && program_pipelines==0);
    bad=command;bad.texture_count=5;check(EncodePrograms(&context,7,&bad,1)==1 && program_pipelines==0);
    bad=command;bad.textures=reinterpret_cast<const SFOverlayTextureV1*>(UINTPTR_MAX-8);check(EncodePrograms(&context,7,&bad,1)==1 && program_pipelines==0);
    bad=command;bad.constants=reinterpret_cast<const void*>(1);check(EncodePrograms(&context,7,&bad,1)==1 && program_pipelines==0);
    device_destroyed=false;
    check_named("V3 creates an independent program and texture-only resources",EncodePrograms(&context,7,&command,1)==0 && program_pipelines==1 && context.image->programs.size()==1 && context.image->textures.size()==2);
    check(context.draws.back().viewport.x==10 && context.draws.back().scissor.extent.height==40 && context.draws.back().push[1]==0);
    check(!std::memcmp(context.draws.back().push.data(),&constant,4));
    const auto pipeline=context.draws.back().program->pipeline,descriptor=context.draws.back().descriptor;
    ++image.revision;
    check_named("texture revision changes do not create another pipeline",EncodePrograms(&context,7,&command,1)==0 && program_pipelines==1 && context.draws.back().program->pipeline==pipeline && context.draws.back().descriptor!=descriptor);
    const auto allocations=next_program_handle;
    check_named("same immutable program and textures reuse their caches",EncodePrograms(&context,7,&command,1)==0 && program_pipelines==1 && next_program_handle==allocations+1);
    context.generation=41;check(EncodePrograms(&context,7,&command,1)==1);context.generation=42;
    bad=command;bad.shader.program_id=99;
    for(unsigned i=2;i<=32;++i)context.image->programs.emplace(ProgramKey{7,i,1,42},ProgramResources{});
    check_named("owner program cap refuses without creating a pipeline",EncodePrograms(&context,7,&bad,1)==6 && program_pipelines==1);
    for(unsigned i=0;context.image->programs.size()<128;++i)context.image->programs.emplace(ProgramKey{100+i,1,1,42},ProgramResources{});
    check(EncodePrograms(&context,8,&bad,1)==6 && program_pipelines==1);
    fail_program_pipeline=true;const auto before_cleanup=program_destroyed;ProgramResources failed_program;
    check_named("partial pipeline creation failure releases local handles",!CreateProgram(renderer,shader,failed_program) && !failed_program.pipeline && !failed_program.pipeline_layout && program_destroyed==before_cleanup+1);
    fail_program_pipeline=false;
    const auto probe=Services().Open("program-probe",1);SFOverlayServiceV1 service{};
    const auto legacy_owner=Services().Open("legacy-program-probe",1);SFOverlayServiceV1 legacy_service{};
    check(Services().Query(legacy_owner,"shroudforge.overlay",1,sizeof(legacy_service),&legacy_service)==0);
    SFOverlayRegistrationV1 legacy_registration{};
    check(legacy_service.register_frame(legacy_service.context,+[](void*,const SFOverlayFrameV1* frame,const SFOverlayEncoderV1* encoder)->std::int32_t{
        check(frame->version==1 && frame->size==32 && encoder->size==48);
        check(encoder->draw_rectangles(encoder->context,encoder->owner_id,nullptr,4097)==0);
        const auto& extended=*reinterpret_cast<const SFOverlayEncoderV3*>(encoder);
        check_named("legacy owners cannot bypass V3 negotiation",extended.draw_programs(encoder->context,encoder->owner_id,nullptr,1)==3);return 0;
    },nullptr,&legacy_registration)==0);
    check(Services().Query(probe,"shroudforge.overlay",3,sizeof(service),&service)==0 && service.version==3);
    SFOverlayEncoderV3 saved{};SFOverlayRegistrationV1 registration{};
    struct Probe {SFOverlayEncoderV3* saved;unsigned calls{};} probe_state{&saved};
    check(service.register_frame(service.context,+[](void* data,const SFOverlayFrameV1* base,const SFOverlayEncoderV1* encoder)->std::int32_t{
        auto& probe=*static_cast<Probe*>(data);const auto& frame=*reinterpret_cast<const SFOverlayFrameV3*>(base);const auto& v3=*reinterpret_cast<const SFOverlayEncoderV3*>(encoder);
        check(base->version==3 && base->size==64 && encoder->version==3 && encoder->size==64 && frame.target_encoding==SF_OVERLAY_TARGET_SCRGB_LINEAR && frame.linear_unit_nits==80 && frame.diffuse_white_nits==203);
        *probe.saved=v3;
        check(encoder->draw_rectangles(encoder->context,encoder->owner_id,nullptr,4097)==10);
        check(encoder->draw_rectangles(encoder->context,encoder->owner_id,nullptr,4095)==0);
        check(v3.draw_programs(encoder->context,encoder->owner_id,nullptr,1)==0);
        check(v3.base.draw_textures_v2(encoder->context,encoder->owner_id,nullptr,1)==10);++probe.calls;return 0;
    },&probe_state,&registration)==0);
    const SFOverlayEncoderV3 backend{{{sizeof(backend),3,nullptr,0,42,
        +[](void*,std::uint64_t,const SFOverlayRectangleV1*,std::uint32_t)->std::int32_t{return 0;},
        +[](void*,std::uint64_t,const SFOverlayTextureCommandV1*,std::uint32_t)->std::int32_t{return 0;}},
        +[](void*,std::uint64_t,const SFOverlayTextureCommandV2*,std::uint32_t)->std::int32_t{return 0;}},
        +[](void*,std::uint64_t,const SFOverlayProgramCommandV1*,std::uint32_t)->std::int32_t{return 0;}};
    renderer.colorSpace=1000104002;check(Services().Dispatch(800,600,100,42,backend.base.base,nullptr,TargetFrame(renderer)) && probe_state.calls==1);
    check(Services().Revoke(legacy_owner)==0 && Services().Drain(legacy_owner,0)==0);
    check(saved.draw_programs(saved.base.base.context,saved.base.base.owner_id,nullptr,1)==3);
    check(service.unregister_frame(service.context,registration)==0);
    check(service.register_frame(service.context,+[](void* data,const SFOverlayFrameV1*,const SFOverlayEncoderV1* encoder)->std::int32_t{
        const auto& v3=*reinterpret_cast<const SFOverlayEncoderV3*>(encoder);
        check(v3.draw_programs(encoder->context,encoder->owner_id,static_cast<SFOverlayProgramCommandV1*>(data),1)==0);return 7;
    },&command,&registration)==0);
    // Clear inert cap rows; keep the real cached program. A failed owner must
    // prevent the entire recorded command batch from entering a render pass.
    std::erase_if(context.image->programs,[](const auto& p){return !p.second.pipeline;});
    renderer.colorSpace=1000104008;check(TargetFrame(renderer).target_encoding==SF_OVERLAY_TARGET_HDR10_PQ && TargetFrame(renderer).pq_peak_nits==10000);
    renderer.colorSpace=0;check(TargetFrame(renderer).target_encoding==SF_OVERLAY_TARGET_SRGB_VALUES);
    const auto drawn=draws;Submission submission;submission.renderer=&renderer;submission.index=0;
    check_named("failed V3 callback cannot submit its partial commands",!Record(&submission) && submission.record.failed && draws==drawn);
    check(Services().Revoke(probe)==0 && Services().Drain(probe,0)==0);
    auto device=std::make_shared<Device>();device->handle=977;device->alive=true;devices[977]=device;
    g_renderer=std::move(renderer);fence_result=1;const auto destroyed=program_destroyed,pools=program_pool_destroyed,images=program_images_destroyed;
    RetireCurrentRenderer();ReapRetired();check(program_destroyed==destroyed && program_pool_destroyed==pools && program_images_destroyed==images);
    fence_result=0;device->uncertain=true;ReapRetired();check(program_destroyed==destroyed && program_pool_destroyed==pools && program_images_destroyed==images);
    device->uncertain=false;ReapRetired();check_named("programs descriptors and images retire only after their image fences",program_destroyed>destroyed && program_pool_destroyed>pools && program_images_destroyed>images);
    devices.erase(977);
}
}
int main(int argc,char** argv){
    check_named("graphics refuses activation without guarded startup admission",!StartGraphics());
    const auto extended_owner=Services().Open("overlay-v2-probe",2);
    SFOverlayServiceV1 extended_service{};
    check_named("extended overlay service is negotiated before registration",
        Services().Query(extended_owner,"shroudforge.overlay",2,sizeof(extended_service),&extended_service)==0 && extended_service.version==2);
    check(Services().Revoke(extended_owner)==0);
    if(argc==2){UnrecoverableWrapperRegression(std::strcmp(argv[1],"--terminal-swap")==0,false);return 0;}
    boot_gipa=FakeQueueGipa;
    const auto families=ReadQueueFamilies(reinterpret_cast<void*>(900),reinterpret_cast<void*>(901));
    check(families.size()==3 && families[0].flags==QueueGraphicsBit && families[2].flags==0 && family_query_calls==2);
    family_count=257;check(ReadQueueFamilies(reinterpret_cast<void*>(900),reinterpret_cast<void*>(901)).empty());
    check(ReadQueueFamilies(nullptr,reinterpret_cast<void*>(901)).empty());boot_gipa=nullptr;
    check(ReadQueueFamilies(reinterpret_cast<void*>(900),reinterpret_cast<void*>(901)).empty());
    // No observed enabled maintenance feature: never allocate/submit overlay work.

    check(VK_IMAGE_TYPE_2D==1 && VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL==7 && VK_BORDER_COLOR_INT_OPAQUE_BLACK==3 && VK_FRONT_FACE_COUNTER_CLOCKWISE==0);
    VkImageCreateInfo image_descriptor{};VkSamplerCreateInfo sampler_descriptor{};VkPipelineRasterizationStateCreateInfo raster_descriptor{};
    check(image_descriptor.sType==14 && image_descriptor.imageType==1 && image_descriptor.extent.depth==1);
    check(sampler_descriptor.borderColor==3 && raster_descriptor.frontFace==0);
    std::uint8_t pixels[]{10,20,30,255};SFOverlayTextureV1 texture{1,1,1,1,pixels,4,nullptr,0};
    check(TextureValid(texture));auto invalid=texture;invalid.rgba_bytes=3;check(!TextureValid(invalid));
    invalid=texture;invalid.width=UINT32_MAX;check(!TextureValid(invalid));std::vector<std::uint8_t> bytes;
    invalid=texture;invalid.rgba=reinterpret_cast<const std::uint8_t*>(1);check(!CopyTexture(invalid,bytes));
    const std::uint8_t cutout=0;invalid=texture;invalid.coverage_mask=&cutout;invalid.coverage_mask_bytes=1;
    check(CopyTexture(invalid,bytes) && bytes[3]==0 && bytes[0]==10);
    pixels[3]=128;check(CopyTexture(texture,bytes) && bytes[3]==128);pixels[3]=255;
    const std::uint8_t bad_mask=2;invalid=texture;invalid.coverage_mask=&bad_mask;invalid.coverage_mask_bytes=1;check(!CopyTexture(invalid,bytes));
    auto device=std::make_shared<Device>();device->handle=77;device->generation=1;device->create=FakeCreate;devices[77]=device;
    g_renderer=ReadyRenderer(77);fence_result=1;RetireCurrentRenderer();check(!g_renderer.device && retired.size()==1 && destroys==0);
    ReapRetired();check(retired.size()==1 && destroys==0);
    fence_result=0;ReapRetired();check(retired.empty() && destroys==4);
    g_renderer=ReadyRenderer(77);fence_result=-4;const auto before=destroys;const auto before_queries=queries;RetireCurrentRenderer();ReapRetired();
    check(device->lost && retired.empty() && destroys==before+4 && queries==before_queries+1);
    device->lost=false;device->wait_fences=FakeWait;
    g_renderer=ReadyRenderer(77);g_renderer.presentPending={true};fence_result=1;
    device->destroy=[](void*,const void*){device_destroyed=true;};Destroy(reinterpret_cast<void*>(77),nullptr);
    const auto prior_queries=queries;ReapRetired();
    check(device_destroyed && !device->alive && retired.empty() && queries==prior_queries && cold_waits==2 && waited==std::vector<std::uintptr_t>({10,13}) && idle_waits==0);
    device_destroyed=false;auto recreated=std::make_shared<Device>();recreated->handle=77;recreated->generation=2;recreated->create=FakeCreate;devices[77]=recreated;
    VkSwapchainCreateInfoKHR create{};create.imageUsage=VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT;create.imageExtent={1920,1080};create.imageFormat=97;create.imageColorSpace=1000104008;void* output{};
    check(Create(reinterpret_cast<void*>(77),&create,nullptr,&output)==0);check(swapchains[88].format==97 && swapchains[88].width==1920 && swapchains[88].colorSpace==1000104008);
    const auto generation=swapchains[88].generation;create.imageExtent={1280,720};Create(reinterpret_cast<void*>(77),&create,nullptr,&output);
    check(swapchains[88].generation>generation && swapchains[88].height==720);
    Renderer renderer=ReadyRenderer(77);renderer.resources.resize(1);
    RecordContext context;context.renderer=&renderer;context.image=&renderer.resources[0];context.thread=GetCurrentThreadId();
    TexturedResources white;white.frameTextureReady=true;white.frameTextureWidth=white.frameTextureHeight=1;
    context.image->textures.emplace(TextureKey{0,UINT64_MAX,1},white);
    static constexpr std::uint8_t solid[]={255,255,255,255};
    SFOverlayTextureCommandV2 command{};command.base.texture={UINT64_MAX,1,1,1,solid,4,nullptr,0};
    command.base.x=10;command.base.y=20;command.base.width=command.base.height=36;command.base.clip_radius=100;
    command.base.tint_rgba=0x43e7fff2;command.square_clip=1;
    check_named("generic V2 texture keeps tint and square clipping",EncodeTexturesV2(&context,0,&command,1)==0 &&
        context.draws.back().push[15]==1 && context.draws.back().push[21]==0 && context.draws.back().push[24]==0);
    command.square_clip=2;check(EncodeTexturesV2(&context,0,&command,1)==1);command.square_clip=0;
    command.base.rotation_radians=std::numeric_limits<float>::quiet_NaN();check(EncodeTexturesV2(&context,0,&command,1)==1);
    command.base.rotation_radians=0;
    check_named("V1 texture keeps image semantics",EncodeTextures(&context,0,&command.base,1)==0 && context.draws.back().push[21]==0);
    check(EncodeTextures(&context,2,reinterpret_cast<const SFOverlayTextureCommandV1*>(1),1)==1);
    // The real Present interceptor must preserve original waits on every
    // incomplete path and publish its own signal only after successful submit.
    auto owner=Services().Open("fixture-present",1);SFOverlayServiceV1 service{};check(Services().Query(owner,"shroudforge.overlay",1,sizeof(service),&service)==0);
    SFOverlayRegistrationV1 registration{};
    check(service.register_frame(service.context,[](void*,const SFOverlayFrameV1*,const SFOverlayEncoderV1* encoder)->std::int32_t {
        SFOverlayRectangleV1 rectangle{0,0,10,10,0xff000080};return encoder->draw_rectangles(encoder->context,encoder->owner_id,&rectangle,1);
    },nullptr,&registration)==0);
    recreated->queue=7;recreated->present=FakePresent;recreated->wait_fences=FakeWait;active=true;
    const void* game_wait=reinterpret_cast<void*>(99);const void* swapchain=reinterpret_cast<void*>(88);std::uint32_t image_index=0;
    VkPresentInfoKHR present{};present.waitSemaphoreCount=1;present.pWaitSemaphores=&game_wait;present.swapchainCount=1;present.pSwapchains=&swapchain;present.pImageIndices=&image_index;
    PreparePresent(owner);fence_result=0;check(Present(reinterpret_cast<void*>(7),&present)==0 && seen_wait==99 && submits==0);
    original_present_fallback=reinterpret_cast<std::uintptr_t>(&FakePresent);
    const auto foreign_submits=submits,foreign_calls=present_calls;
    check(Present(reinterpret_cast<void*>(777),&present)==0 && submits==foreign_submits && present_calls==foreign_calls+1 && seen_wait==99);
    recreated->broker=true;recreated->maintenance=true;
    recreated->queue_families={{QueueGraphicsBit,1,0,{}},{QueueGraphicsBit,1,0,{}},{0,1,0,{}}};
    boot_queues[reinterpret_cast<void*>(7)]={77,0,0};
    PreparePresent(owner);
    // Refusal preserves the exact caller packet and emits no overlay work.
    const auto refused=[&]{
        const auto calls=present_calls,submitted=submits,drawn=draws;
        check(Present(reinterpret_cast<void*>(7),&present)==0 && seen_present_input==&present &&
            seen_wait==99 && present_calls==calls+1 && submits==submitted && draws==drawn);
    };
    boot_queues.erase(reinterpret_cast<void*>(7));refused(); // Unknown queue origin.
    for(const auto family:{2u,99u}){boot_queues[reinterpret_cast<void*>(7)]={77,family,0};refused();}
    boot_queues[reinterpret_cast<void*>(7)]={77,0,1};refused(); // Protected queue.
    boot_queues[reinterpret_cast<void*>(7)]={77,0,0};
    create.imageUsage=VK_IMAGE_USAGE_TRANSFER_DST_BIT;
    check(Create(reinterpret_cast<void*>(77),&create,nullptr,&output)==0 &&
        swapchains[88].imageUsage==VK_IMAGE_USAGE_TRANSFER_DST_BIT && create.imageUsage==VK_IMAGE_USAGE_TRANSFER_DST_BIT);
    refused();create.imageUsage=VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT;
    check(Create(reinterpret_cast<void*>(77),&create,nullptr,&output)==0);
    recreated->family=0;PreparePresent(owner);
    // A different graphics family cannot reuse old command buffers. Busy old
    // work remains retained even when replacement construction is unavailable.
    recreated->resolver=RecoveryResolver;fence_result=1;
    boot_queues[reinterpret_cast<void*>(7)]={77,1,0};
    const auto freed_before=destroys;refused();
    check(retired.size()==1 && retired.front()->queueFamilyIndex==0 &&
        retired.front()->submissionPending[0] && destroys==freed_before && !g_renderer.ready);
    fence_result=0;ReapRetired();check(retired.empty());
    recreated->family=0;boot_queues[reinterpret_cast<void*>(7)]={77,0,0};
    PreparePresent(owner);fence_result=0;
    const auto first_calls=present_calls;void* game_fence=reinterpret_cast<void*>(91);PresentFences collision;collision.count=1;collision.fences=&game_fence;present.pNext=&collision;
    check(Present(reinterpret_cast<void*>(7),&present)==0 && submits==0 && present_calls==first_calls+1 && seen_wait==99);
    present.pNext=nullptr;present.swapchainCount=2;
    check(Present(reinterpret_cast<void*>(7),&present)==0 && submits==0 && present_calls==first_calls+2 && seen_wait==99);
    present.swapchainCount=1;
    PreparePresent(owner);fence_result=1;check(Present(reinterpret_cast<void*>(7),&present)==0 && seen_wait==99 && submits==0);
    PreparePresent(owner);fence_result=0;record_result=-1;check(Present(reinterpret_cast<void*>(7),&present)==0 && seen_wait==99 && submits==0 && g_renderer.submissionFailed);
    PreparePresent(owner);record_result=0;reset_result=-1;check(Present(reinterpret_cast<void*>(7),&present)==0 && seen_wait==99 && submits==0 && g_renderer.submissionFailed);
    PreparePresent(owner);reset_result=0;submit_result=-1;check(Present(reinterpret_cast<void*>(7),&present)==0 && seen_wait==99 && submits==1 && g_renderer.submissionFailed);
    PreparePresent(owner);submit_result=0;BaseIn game_next{123,nullptr};present.pNext=&game_next;
    check(Present(reinterpret_cast<void*>(7),&present)==0 && seen_wait==12 && submits==2 && seen_present_fence==13 && seen_next==&game_next && g_renderer.presentPending[0]);
    const auto submitted_count=submits;const auto waits_before=cold_waits;fence_result=1;
    check(Present(reinterpret_cast<void*>(7),&present)==0 && submits==submitted_count && seen_wait==99 && cold_waits==waits_before);
    present.pNext=nullptr;fence_result=0;
    PreparePresent(owner);present_result=-1;std::int32_t individual=0;present.pResults=&individual;
    const auto oom_calls=present_calls;
    check(Present(reinterpret_cast<void*>(7),&present)==-4 && individual==-4 && present_calls==oom_calls+1 && recreated->fatal && !recreated->lost && !g_renderer.presentPending[0] && g_renderer.submissionPending[0]);
    const auto fatal_submits=submits;individual=0;
    check(Present(reinterpret_cast<void*>(7),&present)==-4 && individual==-4 && present_calls==oom_calls+1 && submits==fatal_submits);
    present.pResults=nullptr;
    const auto oom_waits=cold_waits;QuiesceForDestruction(*recreated,false);check(cold_waits==oom_waits+1 && waited.back()==10);present_result=0;recreated->fatal=false;
    PreparePresent(owner);present_result=-1000001004;
    check(Present(reinterpret_cast<void*>(7),&present)==-1000001004 && g_renderer.presentPending[0] && !recreated->fatal);
    const auto rejected_waits=cold_waits;QuiesceForDestruction(*recreated,false);
    check(cold_waits==rejected_waits+2);present_result=0;
    // VK_EXT_present_timing rejection is defined to enqueue waits/fences.
    constexpr std::int32_t timing_queue_full=-1000208000;
    PreparePresent(owner);present_result=timing_queue_full;present.pNext=&game_next;
    std::int32_t timing_individual=0;present.pResults=&timing_individual;
    const auto timing_calls=present_calls,timing_submits=submits,timing_waits=cold_waits,timing_destroys=destroys;
    lifetime_events.clear();
    check(Present(reinterpret_cast<void*>(7),&present)==timing_queue_full && timing_individual==timing_queue_full &&
        seen_next==&game_next && present.pNext==&game_next && seen_present_fence==13 &&
        g_renderer.presentPending[0] && g_renderer.submissionPending[0] &&
        !recreated->fatal && !recreated->uncertain && !recreated->lost && !g_renderer.submissionFailed &&
        present_calls==timing_calls+1 && submits==timing_submits+1 && cold_waits==timing_waits);
    check(QuiesceForDestruction(*recreated,true));
    check(cold_waits==timing_waits+2 && destroys==timing_destroys+4 && lifetime_events.size()==6 &&
        lifetime_events[0]==10 && lifetime_events[1]==13 &&
        std::all_of(lifetime_events.begin()+2,lifetime_events.end(),[](auto event){return event>=1000;}));
    present.pNext=nullptr;present.pResults=nullptr;present_result=0;
    auto failed_owner=Services().Open("failed-consumer",1);SFOverlayServiceV1 failed_service{};
    check(Services().Query(failed_owner,"shroudforge.overlay",1,sizeof(failed_service),&failed_service)==0);
    SFOverlayRegistrationV1 failed_registration{};
    check(failed_service.register_frame(failed_service.context,[](void*,const SFOverlayFrameV1*,const SFOverlayEncoderV1* encoder)->std::int32_t {
        SFOverlayRectangleV1 rect{0,0,4,4,0xffffffff};encoder->draw_rectangles(encoder->context,encoder->owner_id,&rect,1);return 7;
    },nullptr,&failed_registration)==0);
    PreparePresent(owner);const auto old_submits=submits;check(Present(reinterpret_cast<void*>(7),&present)==0 && seen_wait==99 && submits==old_submits && g_renderer.submissionFailed);
    check(Services().Query(failed_owner,"shroudforge.overlay",1,sizeof(failed_service),&failed_service)==3 && Services().Drain(failed_owner,0)==0);
    check_named("ordinary frame and spec-defined rejection paths never call QueueWaitIdle",idle_waits==0);
    PresentShutdownRegression(owner,1,0,true,false);
    PresentShutdownRegression(owner,0,0,false,false);
    PresentShutdownRegression(owner,-4,0,true,true);
    PresentShutdownRegression(owner,1,-4,false,true);
    PresentShutdownRegression(owner,0,0,true,false,0);
    PresentShutdownRegression(owner,0,0,false,false,-1000208000);
    UnrecoverablePresentPolicySeam(owner);
    UnrecoverableWrapperRegression(true,false);
    UnrecoverableWrapperRegression(false,false);
    UnrecoverableWrapperRegression(true,true);
    UnrecoverableWrapperRegression(false,true);
    PreparePresent(owner);fence_result=-4;check(Present(reinterpret_cast<void*>(7),&present)==0 && seen_wait==99 && recreated->lost && !g_renderer.device);
    recreated->lost=false;PreparePresent(owner);fence_result=0;present_result=-4;check(Present(reinterpret_cast<void*>(7),&present)==-4 && recreated->lost && !g_renderer.device);
    const auto idle_before=idle_waits;
    recreated->destroy=[](void*,const void*){device_destroyed=true;};Destroy(reinterpret_cast<void*>(77),nullptr);
    check(idle_waits==idle_before && device_destroyed); // Already lost: no wait/query; child destruction precedes original destroy.
    check(Services().Revoke(owner)==0 && Services().Drain(owner,0)==0);
    std::promise<void> stopped;auto stopped_result=stopped.get_future();
    render_mutex.lock();std::thread stopper([&]{StopGraphics();stopped.set_value();});
    check(stopped_result.wait_for(std::chrono::seconds(1))==std::future_status::ready && retirement_pending && !active);
    render_mutex.unlock();stopper.join();MaintainGraphics();check(!retirement_pending);
    GenericPrograms();
    std::printf("PASS: %u actual native GPU validation/retirement/generation/drawing checks\n",checks);
}
