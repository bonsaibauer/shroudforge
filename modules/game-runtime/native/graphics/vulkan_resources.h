// Resource mechanics adapted from pinned test4; MIT, Copyright (c) 2024 Miguel Oppermann.
// Included only by vulkan_overlay.cpp after its private host state declarations.

    bool IsExecutableAddress(uintptr_t address)
    {
        if (address == 0)
            return false;

        MEMORY_BASIC_INFORMATION info{};
        if (VirtualQuery(reinterpret_cast<const void*>(address), &info, sizeof(info)) != sizeof(info))
            return false;

        if (info.State != MEM_COMMIT)
            return false;

        const DWORD protect = info.Protect & 0xFF;
        return protect == PAGE_EXECUTE ||
            protect == PAGE_EXECUTE_READ ||
            protect == PAGE_EXECUTE_READWRITE ||
            protect == PAGE_EXECUTE_WRITECOPY;
    }

    GetDeviceProcAddrFn GetExportedVulkanGetDeviceProcAddr()
    {
        HMODULE vulkanModule = GetModuleHandleW(L"vulkan-1.dll");

        if (vulkanModule == nullptr)
            return nullptr;

        auto* proc = reinterpret_cast<GetDeviceProcAddrFn>(GetProcAddress(vulkanModule, "vkGetDeviceProcAddr"));
        return IsExecutableAddress(reinterpret_cast<uintptr_t>(proc)) ? proc : nullptr;
    }

    void* ResolveDeviceFunctionPtr(GetDeviceProcAddrFn getDeviceProcAddr, void* device, const char* name)
    {
        if (getDeviceProcAddr == nullptr || device == nullptr || name == nullptr)
            return nullptr;

        __try
        {
            void* pointer = getDeviceProcAddr(device, name);
            return IsExecutableAddress(reinterpret_cast<uintptr_t>(pointer)) ? pointer : nullptr;
        }
        __except (EXCEPTION_EXECUTE_HANDLER)
        {
            return nullptr;
        }
    }

    template<typename T> T ResolveDeviceFunction(GetDeviceProcAddrFn resolver,void* device,const char* name) { return reinterpret_cast<T>(ResolveDeviceFunctionPtr(resolver,device,name)); }

    bool LoadVulkanRendererFnsWithResolver(VulkanRendererFns& fns, void* device, GetDeviceProcAddrFn getDeviceProcAddr)
    {
        if (getDeviceProcAddr == nullptr)
            return false;

        fns.getSwapchainImages = ResolveDeviceFunction<GetSwapchainImagesFn>(getDeviceProcAddr, device, "vkGetSwapchainImagesKHR");

        fns.createImageView = ResolveDeviceFunction<CreateImageViewFn>(getDeviceProcAddr, device, "vkCreateImageView");
        fns.destroyImageView = ResolveDeviceFunction<DestroyImageViewFn>(getDeviceProcAddr, device, "vkDestroyImageView");
        fns.createRenderPass = ResolveDeviceFunction<CreateRenderPassFn>(getDeviceProcAddr, device, "vkCreateRenderPass");
        fns.destroyRenderPass = ResolveDeviceFunction<DestroyRenderPassFn>(getDeviceProcAddr, device, "vkDestroyRenderPass");
        fns.createFramebuffer = ResolveDeviceFunction<CreateFramebufferFn>(getDeviceProcAddr, device, "vkCreateFramebuffer");
        fns.destroyFramebuffer = ResolveDeviceFunction<DestroyFramebufferFn>(getDeviceProcAddr, device, "vkDestroyFramebuffer");
        fns.createCommandPool = ResolveDeviceFunction<CreateCommandPoolFn>(getDeviceProcAddr, device, "vkCreateCommandPool");
        fns.destroyCommandPool = ResolveDeviceFunction<DestroyCommandPoolFn>(getDeviceProcAddr, device, "vkDestroyCommandPool");
        fns.allocateCommandBuffers = ResolveDeviceFunction<AllocateCommandBuffersFn>(getDeviceProcAddr, device, "vkAllocateCommandBuffers");
        fns.resetCommandBuffer = ResolveDeviceFunction<ResetCommandBufferFn>(getDeviceProcAddr, device, "vkResetCommandBuffer");
        fns.beginCommandBuffer = ResolveDeviceFunction<BeginCommandBufferFn>(getDeviceProcAddr, device, "vkBeginCommandBuffer");
        fns.endCommandBuffer = ResolveDeviceFunction<EndCommandBufferFn>(getDeviceProcAddr, device, "vkEndCommandBuffer");
        fns.cmdBeginRenderPass = ResolveDeviceFunction<CmdBeginRenderPassFn>(getDeviceProcAddr, device, "vkCmdBeginRenderPass");
        fns.cmdEndRenderPass = ResolveDeviceFunction<CmdEndRenderPassFn>(getDeviceProcAddr, device, "vkCmdEndRenderPass");
        fns.cmdClearAttachments = ResolveDeviceFunction<CmdClearAttachmentsFn>(getDeviceProcAddr, device, "vkCmdClearAttachments");
        fns.createBuffer = ResolveDeviceFunction<CreateBufferFn>(getDeviceProcAddr, device, "vkCreateBuffer");
        fns.destroyBuffer = ResolveDeviceFunction<DestroyBufferFn>(getDeviceProcAddr, device, "vkDestroyBuffer");
        fns.getBufferMemoryRequirements = ResolveDeviceFunction<GetBufferMemoryRequirementsFn>(getDeviceProcAddr, device, "vkGetBufferMemoryRequirements");
        fns.createImage = ResolveDeviceFunction<CreateImageFn>(getDeviceProcAddr, device, "vkCreateImage");
        fns.destroyImage = ResolveDeviceFunction<DestroyImageFn>(getDeviceProcAddr, device, "vkDestroyImage");
        fns.getImageMemoryRequirements = ResolveDeviceFunction<GetImageMemoryRequirementsFn>(getDeviceProcAddr, device, "vkGetImageMemoryRequirements");
        fns.allocateMemory = ResolveDeviceFunction<AllocateMemoryFn>(getDeviceProcAddr, device, "vkAllocateMemory");
        fns.freeMemory = ResolveDeviceFunction<FreeMemoryFn>(getDeviceProcAddr, device, "vkFreeMemory");
        fns.bindBufferMemory = ResolveDeviceFunction<BindBufferMemoryFn>(getDeviceProcAddr, device, "vkBindBufferMemory");
        fns.bindImageMemory = ResolveDeviceFunction<BindImageMemoryFn>(getDeviceProcAddr, device, "vkBindImageMemory");
        fns.mapMemory = ResolveDeviceFunction<MapMemoryFn>(getDeviceProcAddr, device, "vkMapMemory");
        fns.unmapMemory = ResolveDeviceFunction<UnmapMemoryFn>(getDeviceProcAddr, device, "vkUnmapMemory");
        fns.createSampler = ResolveDeviceFunction<CreateSamplerFn>(getDeviceProcAddr, device, "vkCreateSampler");
        fns.destroySampler = ResolveDeviceFunction<DestroySamplerFn>(getDeviceProcAddr, device, "vkDestroySampler");
        fns.createShaderModule = ResolveDeviceFunction<CreateShaderModuleFn>(getDeviceProcAddr, device, "vkCreateShaderModule");
        fns.destroyShaderModule = ResolveDeviceFunction<DestroyShaderModuleFn>(getDeviceProcAddr, device, "vkDestroyShaderModule");
        fns.createDescriptorSetLayout = ResolveDeviceFunction<CreateDescriptorSetLayoutFn>(getDeviceProcAddr, device, "vkCreateDescriptorSetLayout");
        fns.destroyDescriptorSetLayout = ResolveDeviceFunction<DestroyDescriptorSetLayoutFn>(getDeviceProcAddr, device, "vkDestroyDescriptorSetLayout");
        fns.createDescriptorPool = ResolveDeviceFunction<CreateDescriptorPoolFn>(getDeviceProcAddr, device, "vkCreateDescriptorPool");
        fns.destroyDescriptorPool = ResolveDeviceFunction<DestroyDescriptorPoolFn>(getDeviceProcAddr, device, "vkDestroyDescriptorPool");
        fns.allocateDescriptorSets = ResolveDeviceFunction<AllocateDescriptorSetsFn>(getDeviceProcAddr, device, "vkAllocateDescriptorSets");
        fns.updateDescriptorSets = ResolveDeviceFunction<UpdateDescriptorSetsFn>(getDeviceProcAddr, device, "vkUpdateDescriptorSets");
        fns.createPipelineLayout = ResolveDeviceFunction<CreatePipelineLayoutFn>(getDeviceProcAddr, device, "vkCreatePipelineLayout");
        fns.destroyPipelineLayout = ResolveDeviceFunction<DestroyPipelineLayoutFn>(getDeviceProcAddr, device, "vkDestroyPipelineLayout");
        fns.createGraphicsPipelines = ResolveDeviceFunction<CreateGraphicsPipelinesFn>(getDeviceProcAddr, device, "vkCreateGraphicsPipelines");
        fns.destroyPipeline = ResolveDeviceFunction<DestroyPipelineFn>(getDeviceProcAddr, device, "vkDestroyPipeline");
        fns.cmdPipelineBarrier = ResolveDeviceFunction<CmdPipelineBarrierFn>(getDeviceProcAddr, device, "vkCmdPipelineBarrier");
        fns.cmdCopyBufferToImage = ResolveDeviceFunction<CmdCopyBufferToImageFn>(getDeviceProcAddr, device, "vkCmdCopyBufferToImage");
        fns.cmdBindPipeline = ResolveDeviceFunction<CmdBindPipelineFn>(getDeviceProcAddr, device, "vkCmdBindPipeline");
        fns.cmdBindDescriptorSets = ResolveDeviceFunction<CmdBindDescriptorSetsFn>(getDeviceProcAddr, device, "vkCmdBindDescriptorSets");
        fns.cmdPushConstants = ResolveDeviceFunction<CmdPushConstantsFn>(getDeviceProcAddr, device, "vkCmdPushConstants");
        fns.cmdDraw = ResolveDeviceFunction<CmdDrawFn>(getDeviceProcAddr, device, "vkCmdDraw");
        fns.flushMappedMemoryRanges = ResolveDeviceFunction<FlushMappedMemoryRangesFn>(getDeviceProcAddr, device, "vkFlushMappedMemoryRanges");
        fns.createSemaphore = ResolveDeviceFunction<CreateSemaphoreFn>(getDeviceProcAddr, device, "vkCreateSemaphore");
        fns.destroySemaphore = ResolveDeviceFunction<DestroySemaphoreFn>(getDeviceProcAddr, device, "vkDestroySemaphore");
        fns.createFence = ResolveDeviceFunction<CreateFenceFn>(getDeviceProcAddr, device, "vkCreateFence");
        fns.destroyFence = ResolveDeviceFunction<DestroyFenceFn>(getDeviceProcAddr, device, "vkDestroyFence");
        fns.getFenceStatus = ResolveDeviceFunction<GetFenceStatusFn>(getDeviceProcAddr, device, "vkGetFenceStatus");
        fns.resetFences = ResolveDeviceFunction<ResetFencesFn>(getDeviceProcAddr, device, "vkResetFences");
        fns.queueSubmit = ResolveDeviceFunction<QueueSubmitFn>(getDeviceProcAddr, device, "vkQueueSubmit");
        fns.deviceWaitIdle = ResolveDeviceFunction<DeviceWaitIdleFn>(getDeviceProcAddr, device, "vkDeviceWaitIdle");
        return fns.Ready();
    }

    bool TryCreateShaderModule(Renderer& renderer, const std::vector<std::uint8_t>& bytes, void** shaderModule)
    {
        *shaderModule = nullptr;
        if (bytes.empty() || (bytes.size() % sizeof(std::uint32_t)) != 0)
            return false;

        std::vector<std::uint32_t> words(bytes.size() / sizeof(std::uint32_t));
        std::memcpy(words.data(), bytes.data(), bytes.size());

        VkShaderModuleCreateInfo shaderInfo{};
        shaderInfo.codeSize = bytes.size();
        shaderInfo.pCode = words.data();
        return renderer.fns.createShaderModule(
            reinterpret_cast<void*>(renderer.device),
            &shaderInfo,
            nullptr,
            shaderModule) == VK_SUCCESS && *shaderModule != nullptr;
    }

    bool TryAllocateMemoryByTrial(Renderer& renderer, const VkMemoryRequirements& requirements, bool requireMappable, void** memory, void** mapped)
    {
        *memory = nullptr;
        if (mapped != nullptr)
            *mapped = nullptr;

        void* device = reinterpret_cast<void*>(renderer.device);
        for (std::uint32_t index = 0; index < 32; ++index)
        {
            if ((requirements.memoryTypeBits & (1u << index)) == 0)
                continue;

            VkMemoryAllocateInfo alloc{};
            alloc.allocationSize = requirements.size;
            alloc.memoryTypeIndex = index;

            void* candidate = nullptr;
            if (renderer.fns.allocateMemory(device, &alloc, nullptr, &candidate) != VK_SUCCESS || candidate == nullptr)
                continue;

            void* mappedCandidate = nullptr;
            if (requireMappable)
            {
                if (renderer.fns.mapMemory(device, candidate, 0, requirements.size, 0, &mappedCandidate) != VK_SUCCESS || mappedCandidate == nullptr)
                {
                    renderer.fns.freeMemory(device, candidate, nullptr);
                    continue;
                }
            }

            *memory = candidate;
            if (mapped != nullptr)
                *mapped = mappedCandidate;
            return true;
        }

        return false;
    }

    bool IsHdrColorSpace(std::uint32_t space)
    {
        return space == 1000104002 || space == 1000104008;
    }

    void EncodeOverlayColor(std::uint32_t space, float& r, float& g, float& b)
    {
        if (!IsHdrColorSpace(space)) return;
        const auto linear = [](float c) {
            c = (std::max)(0.0f, (std::min)(1.0f, c));
            return c <= 0.04045f ? c / 12.92f : std::pow((c + 0.055f) / 1.055f, 2.4f);
        };
        r=linear(r); g=linear(g); b=linear(b);
        // 203-nit diffuse white; scRGB defines 1.0 as 80 nits.
        if (space == 1000104002) { r*=203.0f/80.0f; g*=203.0f/80.0f; b*=203.0f/80.0f; return; }
        const float rr=0.627404f*r+0.329283f*g+0.043313f*b;
        const float gg=0.069097f*r+0.919540f*g+0.011362f*b;
        const float bb=0.016391f*r+0.088013f*g+0.895595f*b;
        const auto pq = [](float c) {
            const float y=std::pow((std::max)(0.0f,c)*203.0f/10000.0f,2610.0f/16384.0f);
            return std::pow((3424.0f/4096.0f+(2413.0f/128.0f)*y)/(1.0f+(2392.0f/128.0f)*y),2523.0f/32.0f);
        };
        r=pq(rr); g=pq(gg); b=pq(bb);
    }

    bool TryCreateTexturedResourcesLocked(Renderer& renderer, TexturedResources& resource,
        const std::vector<std::uint8_t>& rgba, int textureWidth, int textureHeight, bool terrain)
    {
        if (resource.frameTextureReady) return true;
        if (textureWidth <= 0 || textureHeight <= 0 || textureWidth > 4096 || textureHeight > 4096 ||
            rgba.size() != std::size_t(textureWidth) * textureHeight * 4) return false;
        if (!renderer.fns.TextureReady() || !renderer.fns.flushMappedMemoryRanges) return false;
        void* device = reinterpret_cast<void*>(renderer.device);
        std::vector<float> hdrPixels;
        const bool hdr = false; // Generic shader performs HDR encoding after tint.
        if (hdr) {
            hdrPixels.resize(rgba.size());
            for (std::size_t i=0; i<rgba.size(); i+=4) {
                float r=rgba[i]/255.0f, g=rgba[i+1]/255.0f, b=rgba[i+2]/255.0f;
                EncodeOverlayColor(renderer.colorSpace,r,g,b);
                hdrPixels[i]=r; hdrPixels[i+1]=g; hdrPixels[i+2]=b; hdrPixels[i+3]=rgba[i+3]/255.0f;
            }
        }
        const std::uint32_t textureFormat = hdr ? 109u : VK_FORMAT_R8G8B8A8_UNORM; // R32G32B32A32_SFLOAT
        const std::uint64_t textureBytes = hdr ? hdrPixels.size()*sizeof(float) : rgba.size();

        void* stagingBuffer = nullptr;
        void* stagingMemory = nullptr;
        void* mapped = nullptr;
        void* textureImage = nullptr;
        void* textureMemory = nullptr;
        void* textureImageView = nullptr;
        void* sampler = nullptr;
        void* setLayout = nullptr;
        void* descriptorPool = nullptr;
        void* vertexModule = nullptr;
        void* fragmentModule = nullptr;
        void* pipelineLayout = nullptr;
        void* pipeline = nullptr;
        // Until publication, every return or exception must release the local resources.
        const auto cleanup = [&](void*) {
            if (pipeline) renderer.fns.destroyPipeline(device, pipeline, nullptr);
            if (pipelineLayout) renderer.fns.destroyPipelineLayout(device, pipelineLayout, nullptr);
            if (vertexModule) renderer.fns.destroyShaderModule(device, vertexModule, nullptr);
            if (fragmentModule) renderer.fns.destroyShaderModule(device, fragmentModule, nullptr);
            if (descriptorPool) renderer.fns.destroyDescriptorPool(device, descriptorPool, nullptr);
            if (setLayout) renderer.fns.destroyDescriptorSetLayout(device, setLayout, nullptr);
            if (sampler) renderer.fns.destroySampler(device, sampler, nullptr);
            if (textureImageView) renderer.fns.destroyImageView(device, textureImageView, nullptr);
            if (textureImage) renderer.fns.destroyImage(device, textureImage, nullptr);
            if (textureMemory) renderer.fns.freeMemory(device, textureMemory, nullptr);
            if (stagingBuffer) renderer.fns.destroyBuffer(device, stagingBuffer, nullptr);
            if (mapped) renderer.fns.unmapMemory(device, stagingMemory);
            if (stagingMemory) renderer.fns.freeMemory(device, stagingMemory, nullptr);
        };
        std::unique_ptr<void, decltype(cleanup)> pending(device, cleanup);

        VkBufferCreateInfo bufferInfo{};
        bufferInfo.size = textureBytes;
        bufferInfo.usage = VK_BUFFER_USAGE_TRANSFER_SRC_BIT;
        if (renderer.fns.createBuffer(device, &bufferInfo, nullptr, &stagingBuffer) != VK_SUCCESS || stagingBuffer == nullptr)
            return false;

        VkMemoryRequirements bufferRequirements{};
        renderer.fns.getBufferMemoryRequirements(device, stagingBuffer, &bufferRequirements);
        if (!TryAllocateMemoryByTrial(renderer, bufferRequirements, true, &stagingMemory, &mapped))
            return false;
        std::memcpy(mapped, hdr ? static_cast<const void*>(hdrPixels.data()) : rgba.data(), static_cast<std::size_t>(textureBytes));
        VkMappedMemoryRange range{};
        range.memory = stagingMemory;
        if (renderer.fns.flushMappedMemoryRanges(device, 1, &range) != VK_SUCCESS) return false;
        renderer.fns.unmapMemory(device, stagingMemory);
        mapped = nullptr;
        if (renderer.fns.bindBufferMemory(device, stagingBuffer, stagingMemory, 0) != VK_SUCCESS)
            return false;

        VkImageCreateInfo imageInfo{};
        imageInfo.format = textureFormat;
        imageInfo.extent.width = static_cast<std::uint32_t>(textureWidth);
        imageInfo.extent.height = static_cast<std::uint32_t>(textureHeight);
        imageInfo.extent.depth = 1;
        if (renderer.fns.createImage(device, &imageInfo, nullptr, &textureImage) != VK_SUCCESS || textureImage == nullptr)
            return false;

        VkMemoryRequirements imageRequirements{};
        renderer.fns.getImageMemoryRequirements(device, textureImage, &imageRequirements);
        if (!TryAllocateMemoryByTrial(renderer, imageRequirements, false, &textureMemory, nullptr) ||
            renderer.fns.bindImageMemory(device, textureImage, textureMemory, 0) != VK_SUCCESS)
            return false;

        VkImageViewCreateInfo textureViewInfo{};
        textureViewInfo.image = textureImage;
        textureViewInfo.format = textureFormat;
        textureViewInfo.subresourceRange.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
        textureViewInfo.subresourceRange.levelCount = 1;
        textureViewInfo.subresourceRange.layerCount = 1;
        if (renderer.fns.createImageView(device, &textureViewInfo, nullptr, &textureImageView) != VK_SUCCESS || textureImageView == nullptr)
            return false;

        VkSamplerCreateInfo samplerInfo{};
        if (renderer.fns.createSampler(device, &samplerInfo, nullptr, &sampler) != VK_SUCCESS || sampler == nullptr)
            return false;

        VkDescriptorSetLayoutBinding binding{};
        VkDescriptorSetLayoutCreateInfo setLayoutInfo{};
        setLayoutInfo.bindingCount = 1;
        setLayoutInfo.pBindings = &binding;
        if (renderer.fns.createDescriptorSetLayout(device, &setLayoutInfo, nullptr, &setLayout) != VK_SUCCESS || setLayout == nullptr)
            return false;

        VkDescriptorPoolSize poolSize{};
        VkDescriptorPoolCreateInfo poolInfo{};
        poolInfo.maxSets = 1;
        poolInfo.poolSizeCount = 1;
        poolInfo.pPoolSizes = &poolSize;
        if (renderer.fns.createDescriptorPool(device, &poolInfo, nullptr, &descriptorPool) != VK_SUCCESS || descriptorPool == nullptr)
            return false;

        void* descriptorSet = nullptr;
        void* setLayoutForAlloc = setLayout;
        VkDescriptorSetAllocateInfo setAlloc{};
        setAlloc.descriptorPool = descriptorPool;
        setAlloc.descriptorSetCount = 1;
        setAlloc.pSetLayouts = &setLayoutForAlloc;
        if (renderer.fns.allocateDescriptorSets(device, &setAlloc, &descriptorSet) != VK_SUCCESS || descriptorSet == nullptr)
            return false;

        VkDescriptorImageInfo imageDescriptor{};
        imageDescriptor.sampler = sampler;
        imageDescriptor.imageView = textureImageView;
        imageDescriptor.imageLayout = VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL;
        VkWriteDescriptorSet write{};
        write.dstSet = descriptorSet;
        write.descriptorCount = 1;
        write.pImageInfo = &imageDescriptor;
        renderer.fns.updateDescriptorSets(device, 1, &write, 0, nullptr);

        std::vector<std::uint8_t> vertexShader;
        std::vector<std::uint8_t> fragmentShader;
        if (!TryReadFrameShader(terrain ? "terrain.vert" : "texture.vert", vertexShader) ||
            !TryReadFrameShader(terrain ? "terrain.frag" : "texture.frag", fragmentShader))
        {
            return false;
        }

        if (!TryCreateShaderModule(renderer, vertexShader, &vertexModule) ||
            !TryCreateShaderModule(renderer, fragmentShader, &fragmentModule))
            return false;

        VkPushConstantRange pushRange{};
        pushRange.stageFlags = VK_SHADER_STAGE_VERTEX_BIT | VK_SHADER_STAGE_FRAGMENT_BIT;
        pushRange.size = sizeof(float) * 32;
        void* setLayoutForPipeline = setLayout;
        VkPipelineLayoutCreateInfo pipelineLayoutInfo{};
        pipelineLayoutInfo.setLayoutCount = 1;
        pipelineLayoutInfo.pSetLayouts = &setLayoutForPipeline;
        pipelineLayoutInfo.pushConstantRangeCount = 1;
        pipelineLayoutInfo.pPushConstantRanges = &pushRange;
        if (renderer.fns.createPipelineLayout(device, &pipelineLayoutInfo, nullptr, &pipelineLayout) != VK_SUCCESS || pipelineLayout == nullptr)
            return false;

        VkPipelineShaderStageCreateInfo stages[2]{};
        stages[0].stage = VK_SHADER_STAGE_VERTEX_BIT;
        stages[0].module = vertexModule;
        stages[1].stage = VK_SHADER_STAGE_FRAGMENT_BIT;
        stages[1].module = fragmentModule;

        VkPipelineVertexInputStateCreateInfo vertexInput{};
        VkPipelineInputAssemblyStateCreateInfo inputAssembly{};
        VkViewport viewport{};
        viewport.width = static_cast<float>(renderer.width);
        viewport.height = static_cast<float>(renderer.height);
        VkRect2D scissor{};
        scissor.extent.width = renderer.width;
        scissor.extent.height = renderer.height;
        VkPipelineViewportStateCreateInfo viewportState{};
        viewportState.viewportCount = 1;
        viewportState.pViewports = &viewport;
        viewportState.scissorCount = 1;
        viewportState.pScissors = &scissor;
        VkPipelineRasterizationStateCreateInfo raster{};
        VkPipelineMultisampleStateCreateInfo multisample{};
        VkPipelineColorBlendAttachmentState blendAttachment{};
        blendAttachment.blendEnable = terrain ? 0 : 1;
        blendAttachment.srcColorBlendFactor = VK_BLEND_FACTOR_SRC_ALPHA;
        blendAttachment.dstColorBlendFactor = VK_BLEND_FACTOR_ONE_MINUS_SRC_ALPHA;
        blendAttachment.srcAlphaBlendFactor = VK_BLEND_FACTOR_ONE;
        blendAttachment.dstAlphaBlendFactor = VK_BLEND_FACTOR_ZERO;
        VkPipelineColorBlendStateCreateInfo blend{};
        blend.attachmentCount = 1;
        blend.pAttachments = &blendAttachment;

        VkGraphicsPipelineCreateInfo pipelineInfo{};
        pipelineInfo.stageCount = 2;
        pipelineInfo.pStages = stages;
        pipelineInfo.pVertexInputState = &vertexInput;
        pipelineInfo.pInputAssemblyState = &inputAssembly;
        pipelineInfo.pViewportState = &viewportState;
        pipelineInfo.pRasterizationState = &raster;
        pipelineInfo.pMultisampleState = &multisample;
        pipelineInfo.pColorBlendState = &blend;
        pipelineInfo.layout = pipelineLayout;
        pipelineInfo.renderPass = reinterpret_cast<void*>(renderer.renderPass);

        const bool pipelineOk = renderer.fns.createGraphicsPipelines(device, nullptr, 1, &pipelineInfo, nullptr, &pipeline) == VK_SUCCESS && pipeline != nullptr;
        renderer.fns.destroyShaderModule(device, vertexModule, nullptr);
        renderer.fns.destroyShaderModule(device, fragmentModule, nullptr);
        vertexModule = fragmentModule = nullptr;
        if (!pipelineOk)
            return false;

        resource.frameStagingBuffer = reinterpret_cast<uintptr_t>(stagingBuffer);
        resource.frameStagingMemory = reinterpret_cast<uintptr_t>(stagingMemory);
        resource.frameTextureImage = reinterpret_cast<uintptr_t>(textureImage);
        resource.frameTextureMemory = reinterpret_cast<uintptr_t>(textureMemory);
        resource.frameTextureImageView = reinterpret_cast<uintptr_t>(textureImageView);
        resource.frameSampler = reinterpret_cast<uintptr_t>(sampler);
        resource.frameDescriptorSetLayout = reinterpret_cast<uintptr_t>(setLayout);
        resource.frameDescriptorPool = reinterpret_cast<uintptr_t>(descriptorPool);
        resource.frameDescriptorSet = reinterpret_cast<uintptr_t>(descriptorSet);
        resource.framePipelineLayout = reinterpret_cast<uintptr_t>(pipelineLayout);
        resource.framePipeline = reinterpret_cast<uintptr_t>(pipeline);
        resource.frameTextureWidth = static_cast<std::uint32_t>(textureWidth);
        resource.frameTextureHeight = static_cast<std::uint32_t>(textureHeight);
        resource.frameTextureUploadPending = true;
        resource.frameTextureReady = true;
        pending.release();
        return true;
    }

    void DestroyTexturedResources(Renderer& renderer, TexturedResources& resource)
    {
        void* device = reinterpret_cast<void*>(renderer.device);
        const auto& fns = renderer.fns;
            if (resource.framePipeline != 0 && fns.destroyPipeline != nullptr)
                fns.destroyPipeline(device, reinterpret_cast<void*>(resource.framePipeline), nullptr);
            if (resource.framePipelineLayout != 0 && fns.destroyPipelineLayout != nullptr)
                fns.destroyPipelineLayout(device, reinterpret_cast<void*>(resource.framePipelineLayout), nullptr);
            if (resource.frameDescriptorPool != 0 && fns.destroyDescriptorPool != nullptr)
                fns.destroyDescriptorPool(device, reinterpret_cast<void*>(resource.frameDescriptorPool), nullptr);
            if (resource.frameDescriptorSetLayout != 0 && fns.destroyDescriptorSetLayout != nullptr)
                fns.destroyDescriptorSetLayout(device, reinterpret_cast<void*>(resource.frameDescriptorSetLayout), nullptr);
            if (resource.frameSampler != 0 && fns.destroySampler != nullptr)
                fns.destroySampler(device, reinterpret_cast<void*>(resource.frameSampler), nullptr);
            if (resource.frameTextureImageView != 0 && fns.destroyImageView != nullptr)
                fns.destroyImageView(device, reinterpret_cast<void*>(resource.frameTextureImageView), nullptr);
            if (resource.frameTextureImage != 0 && fns.destroyImage != nullptr)
                fns.destroyImage(device, reinterpret_cast<void*>(resource.frameTextureImage), nullptr);
            if (resource.frameTextureMemory != 0 && fns.freeMemory != nullptr)
                fns.freeMemory(device, reinterpret_cast<void*>(resource.frameTextureMemory), nullptr);
            if (resource.frameStagingBuffer != 0 && fns.destroyBuffer != nullptr)
                fns.destroyBuffer(device, reinterpret_cast<void*>(resource.frameStagingBuffer), nullptr);
            if (resource.frameStagingMemory != 0 && fns.freeMemory != nullptr)
                fns.freeMemory(device, reinterpret_cast<void*>(resource.frameStagingMemory), nullptr);

        resource = {};
    }

    bool FetchRendererSwapchainImages(Renderer& renderer, const SwapchainRuntimeInfo& snapshot)
    {
        if (!snapshot.images.empty())
        {
            renderer.images = snapshot.images;
            return true;
        }

        std::uint32_t count = snapshot.imageCount;
        void* device = reinterpret_cast<void*>(renderer.device);
        void* swapchain = reinterpret_cast<void*>(renderer.swapchain);
        if (count == 0)
        {
            if (renderer.fns.getSwapchainImages(device, swapchain, &count, nullptr) != VK_SUCCESS)
                return false;
        }

        if (count == 0 || count > 16)
            return false;

        renderer.images.assign(count, 0);
        return renderer.fns.getSwapchainImages(device, swapchain, &count, renderer.images.data()) == VK_SUCCESS;
    }

    bool CreateRendererCommandPoolLocked(std::uint32_t preferredFamily)
    {
        const std::uint32_t candidates[]{preferredFamily}; // Never submit a pool from a guessed queue family.

        void* device = reinterpret_cast<void*>(g_renderer.device);
        for (std::uint32_t family : candidates)
        {
            VkCommandPoolCreateInfo commandPoolInfo{};
            commandPoolInfo.flags = VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT;
            commandPoolInfo.queueFamilyIndex = family;

            void* commandPool = nullptr;
            if (g_renderer.fns.createCommandPool(device, &commandPoolInfo, nullptr, &commandPool) == VK_SUCCESS && commandPool != nullptr)
            {
                g_renderer.commandPool = reinterpret_cast<uintptr_t>(commandPool);
                g_renderer.queueFamilyIndex = family;
                return true;
            }
        }

        return false;
    }

    bool RendererMatchesSwapchain(const Renderer& renderer,
        const SwapchainRuntimeInfo& snapshot, std::uint32_t width, std::uint32_t height, std::uint32_t format, std::uint32_t queueFamily)
    {
        // Handles and dimensions can be reused after scene/resolution changes.
        // A known creation or changed image set must invalidate cached resources.
        return renderer.ready && renderer.device == snapshot.device &&
            renderer.swapchain == snapshot.handle && renderer.format == format &&
            renderer.colorSpace == snapshot.colorSpace && renderer.queueFamilyIndex == queueFamily &&
            renderer.width == width && renderer.height == height &&
            renderer.swapchainGeneration == snapshot.generation &&
            (snapshot.images.empty() || renderer.images == snapshot.images);
    }

    bool BuildRendererLocked(const SwapchainRuntimeInfo& snapshot, std::uint32_t queueFamily)
    {
        if (!snapshot.width || !snapshot.height || !snapshot.format) return false;
        const auto width=snapshot.width, height=snapshot.height, format=snapshot.format;

        if (RendererMatchesSwapchain(g_renderer, snapshot, width, height, format, queueFamily))
        {
            return true;
        }

        RetireCurrentRenderer();

        g_renderer.device = snapshot.device;
        g_renderer.swapchain = snapshot.handle;
        g_renderer.swapchainGeneration = snapshot.generation;
        g_renderer.format = format;
        g_renderer.colorSpace = snapshot.colorSpace;
        g_renderer.width = width;
        g_renderer.height = height;

        void* device = reinterpret_cast<void*>(g_renderer.device);
        if (device == nullptr || g_renderer.swapchain == 0 || !TryLoadVulkanRendererFns(g_renderer.fns, device))
        {
            LogRendererThrottled("[Minimap] Vulkan minimap renderer not ready: missing device functions");
            RetireCurrentRenderer();
            return false;
        }

        if (!FetchRendererSwapchainImages(g_renderer, snapshot))
        {
            LogRendererThrottled("[Minimap] Vulkan minimap renderer not ready: swapchain images unavailable");
            RetireCurrentRenderer();
            return false;
        }

        if (g_renderer.images.empty())
        {
            RetireCurrentRenderer();
            return false;
        }

        if (!CreateRendererCommandPoolLocked(queueFamily))
        {
            LogRendererThrottled("[Minimap] Vulkan minimap renderer not ready: command pool creation failed");
            RetireCurrentRenderer();
            return false;
        }

        VkAttachmentDescription colorAttachment{};
        colorAttachment.format = g_renderer.format;
        VkAttachmentReference colorReference{};
        VkSubpassDescription subpass{};
        subpass.colorAttachmentCount = 1;
        subpass.pColorAttachments = &colorReference;
        VkRenderPassCreateInfo renderPassInfo{};
        renderPassInfo.attachmentCount = 1;
        renderPassInfo.pAttachments = &colorAttachment;
        renderPassInfo.subpassCount = 1;
        renderPassInfo.pSubpasses = &subpass;

        void* renderPass = nullptr;
        if (g_renderer.fns.createRenderPass(device, &renderPassInfo, nullptr, &renderPass) != VK_SUCCESS || renderPass == nullptr)
        {
            LogRendererThrottled("[Minimap] Vulkan minimap renderer not ready: render pass creation failed");
            RetireCurrentRenderer();
            return false;
        }
        g_renderer.renderPass = reinterpret_cast<uintptr_t>(renderPass);

        g_renderer.imageViews.assign(g_renderer.images.size(), 0);
        g_renderer.framebuffers.assign(g_renderer.images.size(), 0);
        for (std::size_t index = 0; index < g_renderer.images.size(); ++index)
        {
            VkImageViewCreateInfo imageViewInfo{};
            imageViewInfo.image = reinterpret_cast<void*>(g_renderer.images[index]);
            imageViewInfo.format = g_renderer.format;
            imageViewInfo.subresourceRange.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
            imageViewInfo.subresourceRange.levelCount = 1;
            imageViewInfo.subresourceRange.layerCount = 1;

            void* imageView = nullptr;
            if (g_renderer.fns.createImageView(device, &imageViewInfo, nullptr, &imageView) != VK_SUCCESS || imageView == nullptr)
            {
                LogRendererThrottled("[Minimap] Vulkan minimap renderer not ready: image view creation failed");
                RetireCurrentRenderer();
                return false;
            }
            g_renderer.imageViews[index] = reinterpret_cast<uintptr_t>(imageView);

            void* attachment = imageView;
            VkFramebufferCreateInfo framebufferInfo{};
            framebufferInfo.renderPass = renderPass;
            framebufferInfo.attachmentCount = 1;
            framebufferInfo.pAttachments = &attachment;
            framebufferInfo.width = g_renderer.width;
            framebufferInfo.height = g_renderer.height;
            framebufferInfo.layers = 1;

            void* framebuffer = nullptr;
            if (g_renderer.fns.createFramebuffer(device, &framebufferInfo, nullptr, &framebuffer) != VK_SUCCESS || framebuffer == nullptr)
            {
                LogRendererThrottled("[Minimap] Vulkan minimap renderer not ready: framebuffer creation failed");
                RetireCurrentRenderer();
                return false;
            }
            g_renderer.framebuffers[index] = reinterpret_cast<uintptr_t>(framebuffer);
        }

        g_renderer.commandBuffers.assign(g_renderer.images.size(), 0);
        VkCommandBufferAllocateInfo allocateInfo{};
        allocateInfo.commandPool = reinterpret_cast<void*>(g_renderer.commandPool);
        allocateInfo.commandBufferCount = static_cast<std::uint32_t>(g_renderer.commandBuffers.size());
        if (g_renderer.fns.allocateCommandBuffers(device, &allocateInfo, reinterpret_cast<void**>(g_renderer.commandBuffers.data())) != VK_SUCCESS)
        {
            LogRendererThrottled("[Minimap] Vulkan minimap renderer not ready: command buffer allocation failed");
            RetireCurrentRenderer();
            return false;
        }

        g_renderer.renderCompleteSemaphores.assign(g_renderer.images.size(), 0);
        VkSemaphoreCreateInfo semaphoreInfo{};
        for (std::size_t index = 0; index < g_renderer.renderCompleteSemaphores.size(); ++index)
        {
            void* semaphore = nullptr;
            if (g_renderer.fns.createSemaphore(device, &semaphoreInfo, nullptr, &semaphore) != VK_SUCCESS || semaphore == nullptr)
            {
                LogRendererThrottled("[Minimap] Vulkan minimap renderer not ready: semaphore creation failed");
                RetireCurrentRenderer();
                return false;
            }
            g_renderer.renderCompleteSemaphores[index] = reinterpret_cast<uintptr_t>(semaphore);
        }

        g_renderer.submissionFences.assign(g_renderer.images.size(), 0);
        VkFenceCreateInfo fenceInfo{};
        fenceInfo.flags = VK_FENCE_CREATE_SIGNALED_BIT;
        for (std::size_t index = 0; index < g_renderer.submissionFences.size(); ++index)
        {
            void* fence = nullptr;
            if (g_renderer.fns.createFence(device, &fenceInfo, nullptr, &fence) != VK_SUCCESS || fence == nullptr)
            {
                LogRendererThrottled("[Minimap] Vulkan minimap renderer not ready: fence creation failed");
                RetireCurrentRenderer();
                return false;
            }
            g_renderer.submissionFences[index] = reinterpret_cast<uintptr_t>(fence);
        }

        g_renderer.presentFences.assign(g_renderer.images.size(),0);
        g_renderer.submissionPending.assign(g_renderer.images.size(),false);
        g_renderer.presentPending.assign(g_renderer.images.size(),false);
        fenceInfo.flags=0;
        for(auto& entry:g_renderer.presentFences) {
            void* fence{};
            if(g_renderer.fns.createFence(device,&fenceInfo,nullptr,&fence)!=0 || !fence){RetireCurrentRenderer();return false;}
            entry=reinterpret_cast<uintptr_t>(fence);
        }
        g_renderer.ready = true;

        std::ostringstream oss;
        oss << "[Minimap] Vulkan minimap renderer ready"
            << " | swapchain=" << Hex(g_renderer.swapchain)
            << " | extent=" << g_renderer.width << "x" << g_renderer.height
            << " | colorSpace=" << g_renderer.colorSpace
            << " | format=" << g_renderer.format
            << " | images=" << g_renderer.images.size()
            << " | queue_family=" << g_renderer.queueFamilyIndex
            << " | command_sync=per-image-fence"
            << " | draw=clear-attachment-widget";
        Log(oss.str());
        return true;
    }

    void RecordTextureUpload(Renderer& renderer, TexturedResources& resource, void* commandBuffer, bool full = true, std::uint32_t offsetY = 0,
        const std::vector<VkBufferImageCopy>* regions = nullptr)
    {
        if (!resource.frameTextureReady)
            return;

        void* image = reinterpret_cast<void*>(resource.frameTextureImage);
        void* buffer = reinterpret_cast<void*>(resource.frameStagingBuffer);
        if (image == nullptr || buffer == nullptr)
            return;

        VkImageMemoryBarrier toTransfer{};
        toTransfer.srcAccessMask = full ? 0 : VK_ACCESS_SHADER_READ_BIT;
        toTransfer.dstAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT;
        toTransfer.oldLayout = full ? VK_IMAGE_LAYOUT_UNDEFINED : VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL;
        toTransfer.newLayout = VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
        toTransfer.image = image;
        toTransfer.subresourceRange.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
        toTransfer.subresourceRange.levelCount = 1;
        toTransfer.subresourceRange.layerCount = 1;
        renderer.fns.cmdPipelineBarrier(
            commandBuffer,
            full ? VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT : VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT,
            VK_PIPELINE_STAGE_TRANSFER_BIT,
            0,
            0,
            nullptr,
            0,
            nullptr,
            1,
            &toTransfer);

        VkBufferImageCopy copy{};
        copy.imageSubresource.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
        copy.imageSubresource.layerCount = 1;
        copy.bufferOffset = std::uint64_t(offsetY) * resource.frameTextureWidth * 4;
        copy.imageOffset.y = static_cast<std::int32_t>(offsetY);
        copy.imageExtent.width = resource.frameTextureWidth;
        copy.imageExtent.height = resource.frameTextureHeight - offsetY;
        copy.imageExtent.depth = 1;
        renderer.fns.cmdCopyBufferToImage(
            commandBuffer,
            buffer,
            image,
            VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
            regions ? static_cast<std::uint32_t>(regions->size()) : 1,
            regions ? regions->data() : &copy);

        VkImageMemoryBarrier toShader{};
        toShader.srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT;
        toShader.dstAccessMask = VK_ACCESS_SHADER_READ_BIT;
        toShader.oldLayout = VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
        toShader.newLayout = VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL;
        toShader.image = image;
        toShader.subresourceRange.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
        toShader.subresourceRange.levelCount = 1;
        toShader.subresourceRange.layerCount = 1;
        renderer.fns.cmdPipelineBarrier(
            commandBuffer,
            VK_PIPELINE_STAGE_TRANSFER_BIT,
            VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT,
            0,
            0,
            nullptr,
            0,
            nullptr,
            1,
            &toShader);

        resource.frameTextureUploadPending = false;
    }
