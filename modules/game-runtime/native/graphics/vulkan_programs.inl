// Private renderer implementation. All caches belong to one swapchain image;
// Record's fence boundary precedes eviction or descriptor-pool destruction.
void DestroyProgram(Renderer& r,ProgramResources& p) {
    auto* device=reinterpret_cast<void*>(r.device);
    if(p.pipeline)r.fns.destroyPipeline(device,reinterpret_cast<void*>(p.pipeline),nullptr);
    if(p.pipeline_layout)r.fns.destroyPipelineLayout(device,reinterpret_cast<void*>(p.pipeline_layout),nullptr);
    if(p.set_layout)r.fns.destroyDescriptorSetLayout(device,reinterpret_cast<void*>(p.set_layout),nullptr);
    p={};
}
void ResetProgramDescriptors(Renderer& r,ImageResources& image) {
    for(auto pool:image.program_pools)r.fns.destroyDescriptorPool(reinterpret_cast<void*>(r.device),reinterpret_cast<void*>(pool),nullptr);
    image.program_pools.clear();image.program_sets=0;
}
bool CreateProgram(Renderer& r,const std::vector<std::uint32_t>& words,ProgramResources& p) {
    auto* device=reinterpret_cast<void*>(r.device);void *vertex{},*fragment{};
    const auto cleanup=[&](void*){
        if(vertex)r.fns.destroyShaderModule(device,vertex,nullptr);
        if(fragment)r.fns.destroyShaderModule(device,fragment,nullptr);
        DestroyProgram(r,p);
    };
    std::unique_ptr<void,decltype(cleanup)> pending(device,cleanup);
    p.words=words;
    VkDescriptorSetLayoutBinding bindings[4]{};for(unsigned i=0;i<4;++i)bindings[i].binding=i;
    VkDescriptorSetLayoutCreateInfo set{};set.bindingCount=4;set.pBindings=bindings;void* handle{};
    if(r.fns.createDescriptorSetLayout(device,&set,nullptr,&handle)!=0 || !handle)return false;
    p.set_layout=reinterpret_cast<std::uintptr_t>(handle);
    VkPushConstantRange push{};push.stageFlags=VK_SHADER_STAGE_FRAGMENT_BIT;push.size=128;
    VkPipelineLayoutCreateInfo layout{};layout.setLayoutCount=1;layout.pSetLayouts=&handle;
    layout.pushConstantRangeCount=1;layout.pPushConstantRanges=&push;
    if(r.fns.createPipelineLayout(device,&layout,nullptr,&handle)!=0 || !handle)return false;
    p.pipeline_layout=reinterpret_cast<std::uintptr_t>(handle);
    VkShaderModuleCreateInfo shader{};shader.codeSize=sizeof(Shaders::program_vert);shader.pCode=Shaders::program_vert;
    if(r.fns.createShaderModule(device,&shader,nullptr,&vertex)!=0 || !vertex)return false;
    shader.codeSize=words.size()*4;shader.pCode=words.data();
    if(r.fns.createShaderModule(device,&shader,nullptr,&fragment)!=0 || !fragment)return false;
    VkPipelineShaderStageCreateInfo stages[2]{};stages[0].stage=VK_SHADER_STAGE_VERTEX_BIT;stages[0].module=vertex;
    stages[1].stage=VK_SHADER_STAGE_FRAGMENT_BIT;stages[1].module=fragment;
    VkPipelineVertexInputStateCreateInfo input{};VkPipelineInputAssemblyStateCreateInfo assembly{};
    VkPipelineViewportStateCreateInfo viewport{};viewport.viewportCount=viewport.scissorCount=1;
    const std::uint32_t dynamic_values[]{0,1};VkPipelineDynamicStateCreateInfo dynamic{};
    dynamic.dynamicStateCount=2;dynamic.pDynamicStates=dynamic_values;
    VkPipelineRasterizationStateCreateInfo raster{};VkPipelineMultisampleStateCreateInfo samples{};
    VkPipelineColorBlendAttachmentState attachment{};attachment.blendEnable=1;
    attachment.srcColorBlendFactor=VK_BLEND_FACTOR_SRC_ALPHA;attachment.dstColorBlendFactor=VK_BLEND_FACTOR_ONE_MINUS_SRC_ALPHA;
    attachment.srcAlphaBlendFactor=VK_BLEND_FACTOR_ONE;attachment.dstAlphaBlendFactor=VK_BLEND_FACTOR_ZERO;
    VkPipelineColorBlendStateCreateInfo blend{};blend.attachmentCount=1;blend.pAttachments=&attachment;
    VkGraphicsPipelineCreateInfo info{};info.stageCount=2;info.pStages=stages;info.pVertexInputState=&input;
    info.pInputAssemblyState=&assembly;info.pViewportState=&viewport;info.pRasterizationState=&raster;
    info.pMultisampleState=&samples;info.pColorBlendState=&blend;info.pDynamicState=&dynamic;
    info.layout=reinterpret_cast<void*>(p.pipeline_layout);info.renderPass=reinterpret_cast<void*>(r.renderPass);
    handle=nullptr;
    const auto result=r.fns.createGraphicsPipelines(device,nullptr,1,&info,nullptr,&handle);
    p.pipeline=reinterpret_cast<std::uintptr_t>(handle);
    // Vulkan may return a created pipeline alongside an error. It remains
    // locally owned until publication and must be retired on the failure path.
    if(result!=0 || !handle)return false;
    r.fns.destroyShaderModule(device,vertex,nullptr);r.fns.destroyShaderModule(device,fragment,nullptr);
    vertex=fragment=nullptr;pending.release();return true;
}
std::int32_t Program(RecordContext& context,const ProgramKey& key,const std::vector<std::uint32_t>& words,ProgramResources*& out) {
    auto& cache=context.image->programs;
    if(auto found=cache.find(key);found!=cache.end()){out=&found->second;context.used_programs.insert(key);return 0;}
    for(auto it=cache.begin();it!=cache.end();){
        if(it->first.owner==key.owner && it->first.identity==key.identity && !context.used_programs.contains(it->first)){
            DestroyProgram(*context.renderer,it->second);it=cache.erase(it);
        }else ++it;
    }
    const auto owned=std::count_if(cache.begin(),cache.end(),[&](const auto& p){return p.first.owner==key.owner;});
    if(cache.size()>=SF_OVERLAY_MAX_PROGRAMS_PER_IMAGE || owned>=SF_OVERLAY_MAX_PROGRAMS_PER_OWNER)return 6;
    ProgramResources resource;
    const auto cleanup=[&](void*){DestroyProgram(*context.renderer,resource);};
    std::unique_ptr<void,decltype(cleanup)> pending(reinterpret_cast<void*>(1),cleanup);
    if(!CreateProgram(*context.renderer,words,resource)){context.failed=true;return 7;}
    auto [found,inserted]=cache.emplace(key,std::move(resource));resource={};
    context.used_programs.insert(key);out=&found->second;return 0;
}
bool ProgramDescriptor(RecordContext& context,ProgramResources& program,
    const std::array<TexturedResources*,4>& textures,std::uintptr_t& output) {
    auto& image=*context.image;auto& r=*context.renderer;auto* device=reinterpret_cast<void*>(r.device);
    constexpr unsigned chunk=64;
    if(image.program_sets>=SF_OVERLAY_MAX_RECTANGLES)return false;
    if(image.program_sets/chunk==image.program_pools.size()){
        VkDescriptorPoolSize size{};size.descriptorCount=chunk*4;VkDescriptorPoolCreateInfo info{};
        info.maxSets=chunk;info.poolSizeCount=1;info.pPoolSizes=&size;void* pool{};
        if(r.fns.createDescriptorPool(device,&info,nullptr,&pool)!=0 || !pool)return false;
        try{image.program_pools.push_back(reinterpret_cast<std::uintptr_t>(pool));}
        catch(...){r.fns.destroyDescriptorPool(device,pool,nullptr);throw;}
    }
    void* layout=reinterpret_cast<void*>(program.set_layout);VkDescriptorSetAllocateInfo allocation{};
    allocation.descriptorPool=reinterpret_cast<void*>(image.program_pools[image.program_sets/chunk]);
    allocation.descriptorSetCount=1;allocation.pSetLayouts=&layout;void* set{};
    if(r.fns.allocateDescriptorSets(device,&allocation,&set)!=0 || !set)return false;
    ++image.program_sets;
    VkDescriptorImageInfo images[4]{};VkWriteDescriptorSet writes[4]{};
    for(unsigned i=0;i<4;++i){
        images[i].sampler=reinterpret_cast<void*>(textures[i]->frameSampler);
        images[i].imageView=reinterpret_cast<void*>(textures[i]->frameTextureImageView);images[i].imageLayout=VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL;
        writes[i].dstSet=set;writes[i].dstBinding=i;writes[i].descriptorCount=1;writes[i].pImageInfo=&images[i];
    }
    r.fns.updateDescriptorSets(device,4,writes,0,nullptr);output=reinterpret_cast<std::uintptr_t>(set);return true;
}
std::int32_t EncodePrograms(void* opaque,std::uint64_t owner,const SFOverlayProgramCommandV1* commands,std::uint32_t count) {
    auto& context=*static_cast<RecordContext*>(opaque);
    if(context.thread!=GetCurrentThreadId() || !owner || !commands || !context.renderer || !context.image ||
        context.generation!=context.renderer->device_generation)return 1;
    if(count>SF_OVERLAY_MAX_PROGRAM_DRAWS || context.draws.size()+count>SF_OVERLAY_MAX_RECTANGLES)return 10;
    const auto command_address=reinterpret_cast<std::uintptr_t>(commands);
    if(command_address>UINTPTR_MAX-std::size_t(count)*sizeof(*commands))return 1;
    auto& r=*context.renderer;if(!r.fns.TextureReady() || !r.fns.cmdSetViewport || !r.fns.cmdSetScissor)return 3;
    static constexpr std::uint8_t black[]{0,0,0,0};
    const SFOverlayTextureV1 missing{UINT64_MAX-1,1,1,1,black,4,nullptr,0};
    for(unsigned index=0;index<count;++index){
        SFOverlayProgramCommandV1 c{};
        if(!read(command_address+std::size_t(index)*sizeof(c),c) || c.shader.size!=sizeof(c.shader) ||
            c.shader.version!=1 || c.shader.reserved || !c.shader.program_id || !c.shader.revision || !c.shader.spirv ||
            c.shader.spirv_bytes<20 || c.shader.spirv_bytes>SF_OVERLAY_MAX_SHADER_BYTES || c.shader.spirv_bytes%4 ||
            c.width<=0 || c.height<=0 || c.width>16384 || c.height>16384 || c.x<0 || c.y<0 ||
            std::uint64_t(c.x)+unsigned(c.width)>r.width || std::uint64_t(c.y)+unsigned(c.height)>r.height ||
            c.texture_count>4 || (c.texture_count && !c.textures) || c.constant_bytes>128 || c.constant_bytes%4 ||
            (c.constant_bytes && !c.constants) ||
            (c.texture_count && reinterpret_cast<std::uintptr_t>(c.textures)>UINTPTR_MAX-std::size_t(c.texture_count)*sizeof(*c.textures)))return 1;
        const ProgramKey key{owner,c.shader.program_id,c.shader.revision,context.generation};
        const auto existing=context.image->programs.find(key);std::vector<std::uint32_t> words;
        if(existing==context.image->programs.end()){
            words.resize(c.shader.spirv_bytes/4);
            if(!MapRuntime::ReadMemory(reinterpret_cast<std::uintptr_t>(c.shader.spirv),words.data(),c.shader.spirv_bytes) || !ValidateOverlayFragment(words))return 1;
        }else if(existing->second.words.size()*4!=c.shader.spirv_bytes)return 1;
        Draw draw{};
        if(c.constant_bytes && !MapRuntime::ReadMemory(reinterpret_cast<std::uintptr_t>(c.constants),draw.push.data(),c.constant_bytes))return 1;
        std::array<SFOverlayTextureV1,4> inputs;std::array<std::vector<std::uint8_t>,4> copies;
        for(unsigned i=0;i<4;++i){
            inputs[i]=missing;
            if(i<c.texture_count && !read(reinterpret_cast<std::uintptr_t>(c.textures)+std::size_t(i)*sizeof(inputs[i]),inputs[i]))return 1;
            if(!TextureValid(inputs[i]))return 1;
            const auto found=context.image->textures.find(TextureKey{i<c.texture_count?owner:0,inputs[i].resource_id,inputs[i].revision,true});
            if(found==context.image->textures.end()){if(!CopyTexture(inputs[i],copies[i]))return 1;}
            else if(found->second.frameTextureWidth!=inputs[i].width || found->second.frameTextureHeight!=inputs[i].height)return 1;
        }
        ProgramResources* program{};if(const auto status=Program(context,key,words,program))return status;
        std::array<TexturedResources*,4> textures{};
        for(unsigned i=0;i<4;++i){
            textures[i]=Texture(context,i<c.texture_count?owner:0,inputs[i],true,&copies[i]);
            if(!textures[i]){context.failed=true;return 7;}
        }
        draw.program=program;
        if(!ProgramDescriptor(context,*program,textures,draw.descriptor)){context.failed=true;return 7;}
        draw.viewport={float(c.x),float(c.y),float(c.width),float(c.height),0,1};
        draw.scissor={{c.x,c.y},{unsigned(c.width),unsigned(c.height)}};context.draws.push_back(draw);
    }return 0;
}
SFOverlayFrameV3 TargetFrame(const Renderer& r) {
    SFOverlayFrameV3 target{};
    target.target_encoding=r.colorSpace==1000104002?SF_OVERLAY_TARGET_SCRGB_LINEAR:
        r.colorSpace==1000104008?SF_OVERLAY_TARGET_HDR10_PQ:SF_OVERLAY_TARGET_SRGB_VALUES;
    target.diffuse_white_nits=203;target.linear_unit_nits=target.target_encoding==SF_OVERLAY_TARGET_SCRGB_LINEAR?80.f:0.f;
    target.pq_peak_nits=target.target_encoding==SF_OVERLAY_TARGET_HDR10_PQ?10000.f:0.f;return target;
}
