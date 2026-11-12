#pragma once
// Vulkan declarations/resource contracts adapted from pinned Minimap test.4.
// MIT, Copyright (c) 2024 Miguel Oppermann. See ../map/SOURCE-NOTICE.txt.
#include <cstdint>
#include <cstddef>
#include <vector>
namespace OverlayRuntime::Vulkan {
using std::uintptr_t;
    constexpr std::int32_t VK_SUCCESS = 0;
    constexpr std::int32_t VK_NOT_READY = 1;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_SUBMIT_INFO = 4;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_FENCE_CREATE_INFO = 8;
    constexpr std::uint32_t VK_FENCE_CREATE_SIGNALED_BIT = 0x1;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO = 9;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO = 15;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO = 37;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO = 38;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO = 39;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO = 40;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO = 42;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO = 43;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER = 45;
    constexpr std::uint32_t VK_IMAGE_VIEW_TYPE_2D = 1;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO = 5;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO = 12;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO = 14;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO = 16;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO = 18;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO = 19;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO = 20;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO = 22;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO = 23;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO = 24;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO = 26;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO = 28;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO = 30;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO = 31;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO = 32;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO = 33;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO = 34;
    constexpr std::uint32_t VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET = 35;
    constexpr std::uint32_t VK_COMPONENT_SWIZZLE_IDENTITY = 0;
    constexpr std::uint32_t VK_FORMAT_R8G8B8A8_UNORM = 37;
    constexpr std::uint32_t VK_IMAGE_ASPECT_COLOR_BIT = 0x1;
    constexpr std::uint32_t VK_IMAGE_TYPE_2D = 1;
    constexpr std::uint32_t VK_IMAGE_TILING_OPTIMAL = 0;
    constexpr std::uint32_t VK_IMAGE_USAGE_TRANSFER_DST_BIT = 0x2;
    constexpr std::uint32_t VK_IMAGE_USAGE_SAMPLED_BIT = 0x4;
    constexpr std::uint32_t VK_BUFFER_USAGE_TRANSFER_SRC_BIT = 0x1;
    constexpr std::uint32_t VK_SAMPLE_COUNT_1_BIT = 0x1;
    constexpr std::uint32_t VK_ATTACHMENT_LOAD_OP_LOAD = 0;
    constexpr std::uint32_t VK_ATTACHMENT_STORE_OP_STORE = 0;
    constexpr std::uint32_t VK_ATTACHMENT_LOAD_OP_DONT_CARE = 2;
    constexpr std::uint32_t VK_ATTACHMENT_STORE_OP_DONT_CARE = 1;
    constexpr std::uint32_t VK_IMAGE_LAYOUT_UNDEFINED = 0;
    constexpr std::uint32_t VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL = 2;
    constexpr std::uint32_t VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL = 5;
    constexpr std::uint32_t VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL = 7;
    constexpr std::uint32_t VK_IMAGE_LAYOUT_PRESENT_SRC_KHR = 1000001002;
    constexpr std::uint32_t VK_PIPELINE_BIND_POINT_GRAPHICS = 0;
    constexpr std::uint32_t VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT = 0x2;
    constexpr std::uint32_t VK_COMMAND_BUFFER_LEVEL_PRIMARY = 0;
    constexpr std::uint32_t VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT = 0x1;
    constexpr std::uint32_t VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT = 0x1;
    constexpr std::uint32_t VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT = 0x80;
    constexpr std::uint32_t VK_PIPELINE_STAGE_TRANSFER_BIT = 0x1000;
    constexpr std::uint32_t VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT = 0x400;
    constexpr std::uint32_t VK_ACCESS_TRANSFER_WRITE_BIT = 0x1000;
    constexpr std::uint32_t VK_ACCESS_SHADER_READ_BIT = 0x20;
    constexpr std::uint32_t VK_SHARING_MODE_EXCLUSIVE = 0;
    constexpr std::uint32_t VK_FILTER_LINEAR = 1;
    constexpr std::uint32_t VK_SAMPLER_MIPMAP_MODE_LINEAR = 1;
    constexpr std::uint32_t VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE = 2;
    constexpr std::uint32_t VK_BORDER_COLOR_INT_OPAQUE_BLACK = 3;
    constexpr std::uint32_t VK_COMPARE_OP_ALWAYS = 7;
    constexpr std::uint32_t VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER = 1;
    constexpr std::uint32_t VK_SHADER_STAGE_VERTEX_BIT = 0x1;
    constexpr std::uint32_t VK_SHADER_STAGE_FRAGMENT_BIT = 0x10;
    constexpr std::uint32_t VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST = 3;
    constexpr std::uint32_t VK_POLYGON_MODE_FILL = 0;
    constexpr std::uint32_t VK_CULL_MODE_NONE = 0;
    constexpr std::uint32_t VK_FRONT_FACE_COUNTER_CLOCKWISE = 0;
    constexpr std::uint32_t VK_COLOR_COMPONENT_R_BIT = 0x1;
    constexpr std::uint32_t VK_COLOR_COMPONENT_G_BIT = 0x2;
    constexpr std::uint32_t VK_COLOR_COMPONENT_B_BIT = 0x4;
    constexpr std::uint32_t VK_COLOR_COMPONENT_A_BIT = 0x8;
    constexpr std::uint32_t VK_BLEND_FACTOR_ZERO = 0;
    constexpr std::uint32_t VK_BLEND_FACTOR_ONE = 1;
    constexpr std::uint32_t VK_BLEND_FACTOR_SRC_ALPHA = 6;
    constexpr std::uint32_t VK_BLEND_FACTOR_ONE_MINUS_SRC_ALPHA = 7;
    constexpr std::uint32_t VK_BLEND_OP_ADD = 0;
    constexpr std::uint32_t VK_LOGIC_OP_COPY = 3;
    struct VkExtent2D { std::uint32_t width = 0, height = 0; };
    struct VkExtent3D { std::uint32_t width = 0, height = 0, depth = 1; };
    struct VkOffset2D
    {
        std::int32_t x = 0;
        std::int32_t y = 0;
    };

    struct VkOffset3D
    {
        std::int32_t x = 0;
        std::int32_t y = 0;
        std::int32_t z = 0;
    };

    struct VkRect2D
    {
        VkOffset2D offset;
        VkExtent2D extent;
    };

    struct VkSwapchainCreateInfoKHR
    {
        std::uint32_t sType;
        const void* pNext;
        std::uint32_t flags;
        void* surface;
        std::uint32_t minImageCount;
        std::uint32_t imageFormat;
        std::uint32_t imageColorSpace;
        VkExtent2D imageExtent;
        std::uint32_t imageArrayLayers;
        std::uint32_t imageUsage;
        std::uint32_t imageSharingMode;
        std::uint32_t queueFamilyIndexCount;
        const std::uint32_t* pQueueFamilyIndices;
        std::uint32_t preTransform;
        std::uint32_t compositeAlpha;
        std::uint32_t presentMode;
        std::uint32_t clipped;
        void* oldSwapchain;
    };

    struct VkPresentInfoKHR
    {
        std::uint32_t sType;
        const void* pNext;
        std::uint32_t waitSemaphoreCount;
        const void* const* pWaitSemaphores;
        std::uint32_t swapchainCount;
        const void* const* pSwapchains;
        const std::uint32_t* pImageIndices;
        std::int32_t* pResults;
    };
    static_assert(offsetof(VkPresentInfoKHR, pSwapchains) == 0x28, "Unexpected VkPresentInfoKHR pSwapchains offset");
    static_assert(offsetof(VkPresentInfoKHR, pImageIndices) == 0x30, "Unexpected VkPresentInfoKHR pImageIndices offset");

    struct VkComponentMapping
    {
        std::uint32_t r = VK_COMPONENT_SWIZZLE_IDENTITY;
        std::uint32_t g = VK_COMPONENT_SWIZZLE_IDENTITY;
        std::uint32_t b = VK_COMPONENT_SWIZZLE_IDENTITY;
        std::uint32_t a = VK_COMPONENT_SWIZZLE_IDENTITY;
    };

    struct VkImageSubresourceRange
    {
        std::uint32_t aspectMask = 0;
        std::uint32_t baseMipLevel = 0;
        std::uint32_t levelCount = 0;
        std::uint32_t baseArrayLayer = 0;
        std::uint32_t layerCount = 0;
    };

    struct VkImageSubresourceLayers
    {
        std::uint32_t aspectMask = 0;
        std::uint32_t mipLevel = 0;
        std::uint32_t baseArrayLayer = 0;
        std::uint32_t layerCount = 1;
    };

    struct VkImageViewCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        void* image = nullptr;
        std::uint32_t viewType = VK_IMAGE_VIEW_TYPE_2D;
        std::uint32_t format = 0;
        VkComponentMapping components;
        VkImageSubresourceRange subresourceRange;
    };

    struct VkAttachmentDescription
    {
        std::uint32_t flags = 0;
        std::uint32_t format = 0;
        std::uint32_t samples = VK_SAMPLE_COUNT_1_BIT;
        std::uint32_t loadOp = VK_ATTACHMENT_LOAD_OP_LOAD;
        std::uint32_t storeOp = VK_ATTACHMENT_STORE_OP_STORE;
        std::uint32_t stencilLoadOp = VK_ATTACHMENT_LOAD_OP_DONT_CARE;
        std::uint32_t stencilStoreOp = VK_ATTACHMENT_STORE_OP_DONT_CARE;
        std::uint32_t initialLayout = VK_IMAGE_LAYOUT_PRESENT_SRC_KHR;
        std::uint32_t finalLayout = VK_IMAGE_LAYOUT_PRESENT_SRC_KHR;
    };

    struct VkAttachmentReference
    {
        std::uint32_t attachment = 0;
        std::uint32_t layout = VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL;
    };

    struct VkSubpassDescription
    {
        std::uint32_t flags = 0;
        std::uint32_t pipelineBindPoint = VK_PIPELINE_BIND_POINT_GRAPHICS;
        std::uint32_t inputAttachmentCount = 0;
        const void* pInputAttachments = nullptr;
        std::uint32_t colorAttachmentCount = 0;
        const VkAttachmentReference* pColorAttachments = nullptr;
        const void* pResolveAttachments = nullptr;
        const void* pDepthStencilAttachment = nullptr;
        std::uint32_t preserveAttachmentCount = 0;
        const std::uint32_t* pPreserveAttachments = nullptr;
    };

    struct VkRenderPassCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t attachmentCount = 0;
        const VkAttachmentDescription* pAttachments = nullptr;
        std::uint32_t subpassCount = 0;
        const VkSubpassDescription* pSubpasses = nullptr;
        std::uint32_t dependencyCount = 0;
        const void* pDependencies = nullptr;
    };

    struct VkFramebufferCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        void* renderPass = nullptr;
        std::uint32_t attachmentCount = 0;
        void* const* pAttachments = nullptr;
        std::uint32_t width = 0;
        std::uint32_t height = 0;
        std::uint32_t layers = 1;
    };

    struct VkCommandPoolCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT;
        std::uint32_t queueFamilyIndex = 0;
    };

    struct VkCommandBufferAllocateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO;
        const void* pNext = nullptr;
        void* commandPool = nullptr;
        std::uint32_t level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;
        std::uint32_t commandBufferCount = 0;
    };

    struct VkCommandBufferBeginInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
        const void* pInheritanceInfo = nullptr;
    };

    union VkClearColorValue
    {
        float float32[4];
        std::int32_t int32[4];
        std::uint32_t uint32[4];
    };

    union VkClearValue
    {
        VkClearColorValue color;
    };

    struct VkRenderPassBeginInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO;
        const void* pNext = nullptr;
        void* renderPass = nullptr;
        void* framebuffer = nullptr;
        VkRect2D renderArea;
        std::uint32_t clearValueCount = 0;
        const VkClearValue* pClearValues = nullptr;
    };

    struct VkClearAttachment
    {
        std::uint32_t aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
        std::uint32_t colorAttachment = 0;
        VkClearValue clearValue;
    };

    struct VkClearRect
    {
        VkRect2D rect;
        std::uint32_t baseArrayLayer = 0;
        std::uint32_t layerCount = 1;
    };

    struct VkFenceCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
    };

    struct VkSemaphoreCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
    };

    struct VkMemoryRequirements
    {
        std::uint64_t size = 0;
        std::uint64_t alignment = 0;
        std::uint32_t memoryTypeBits = 0;
    };

    struct VkMemoryAllocateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO;
        const void* pNext = nullptr;
        std::uint64_t allocationSize = 0;
        std::uint32_t memoryTypeIndex = 0;
    };

    struct VkBufferCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint64_t size = 0;
        std::uint32_t usage = 0;
        std::uint32_t sharingMode = VK_SHARING_MODE_EXCLUSIVE;
        std::uint32_t queueFamilyIndexCount = 0;
        const std::uint32_t* pQueueFamilyIndices = nullptr;
    };

    struct VkImageCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t imageType = VK_IMAGE_TYPE_2D;
        std::uint32_t format = VK_FORMAT_R8G8B8A8_UNORM;
        VkExtent3D extent;
        std::uint32_t mipLevels = 1;
        std::uint32_t arrayLayers = 1;
        std::uint32_t samples = VK_SAMPLE_COUNT_1_BIT;
        std::uint32_t tiling = VK_IMAGE_TILING_OPTIMAL;
        std::uint32_t usage = VK_IMAGE_USAGE_TRANSFER_DST_BIT | VK_IMAGE_USAGE_SAMPLED_BIT;
        std::uint32_t sharingMode = VK_SHARING_MODE_EXCLUSIVE;
        std::uint32_t queueFamilyIndexCount = 0;
        const std::uint32_t* pQueueFamilyIndices = nullptr;
        std::uint32_t initialLayout = VK_IMAGE_LAYOUT_UNDEFINED;
    };

    struct VkBufferImageCopy
    {
        std::uint64_t bufferOffset = 0;
        std::uint32_t bufferRowLength = 0;
        std::uint32_t bufferImageHeight = 0;
        VkImageSubresourceLayers imageSubresource;
        VkOffset3D imageOffset;
        VkExtent3D imageExtent;
    };

    struct VkImageMemoryBarrier
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER;
        const void* pNext = nullptr;
        std::uint32_t srcAccessMask = 0;
        std::uint32_t dstAccessMask = 0;
        std::uint32_t oldLayout = VK_IMAGE_LAYOUT_UNDEFINED;
        std::uint32_t newLayout = VK_IMAGE_LAYOUT_UNDEFINED;
        std::uint32_t srcQueueFamilyIndex = 0xFFFFFFFFu;
        std::uint32_t dstQueueFamilyIndex = 0xFFFFFFFFu;
        void* image = nullptr;
        VkImageSubresourceRange subresourceRange;
    };

    struct VkMappedMemoryRange
    {
        std::uint32_t sType = 6;
        const void* pNext = nullptr;
        void* memory = nullptr;
        std::uint64_t offset = 0;
        std::uint64_t size = ~std::uint64_t(0);
    };

    struct VkSamplerCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t magFilter = VK_FILTER_LINEAR;
        std::uint32_t minFilter = VK_FILTER_LINEAR;
        std::uint32_t mipmapMode = VK_SAMPLER_MIPMAP_MODE_LINEAR;
        std::uint32_t addressModeU = VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE;
        std::uint32_t addressModeV = VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE;
        std::uint32_t addressModeW = VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE;
        float mipLodBias = 0.0f;
        std::uint32_t anisotropyEnable = 0;
        float maxAnisotropy = 1.0f;
        std::uint32_t compareEnable = 0;
        std::uint32_t compareOp = VK_COMPARE_OP_ALWAYS;
        float minLod = 0.0f;
        float maxLod = 0.0f;
        std::uint32_t borderColor = VK_BORDER_COLOR_INT_OPAQUE_BLACK;
        std::uint32_t unnormalizedCoordinates = 0;
    };

    struct VkShaderModuleCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::size_t codeSize = 0;
        const std::uint32_t* pCode = nullptr;
    };

    struct VkDescriptorSetLayoutBinding
    {
        std::uint32_t binding = 0;
        std::uint32_t descriptorType = VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER;
        std::uint32_t descriptorCount = 1;
        std::uint32_t stageFlags = VK_SHADER_STAGE_FRAGMENT_BIT;
        const void* pImmutableSamplers = nullptr;
    };

    struct VkDescriptorSetLayoutCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t bindingCount = 0;
        const VkDescriptorSetLayoutBinding* pBindings = nullptr;
    };

    struct VkDescriptorPoolSize
    {
        std::uint32_t type = VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER;
        std::uint32_t descriptorCount = 1;
    };

    struct VkDescriptorPoolCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t maxSets = 1;
        std::uint32_t poolSizeCount = 0;
        const VkDescriptorPoolSize* pPoolSizes = nullptr;
    };

    struct VkDescriptorSetAllocateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO;
        const void* pNext = nullptr;
        void* descriptorPool = nullptr;
        std::uint32_t descriptorSetCount = 0;
        void* const* pSetLayouts = nullptr;
    };

    struct VkDescriptorImageInfo
    {
        void* sampler = nullptr;
        void* imageView = nullptr;
        std::uint32_t imageLayout = VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL;
    };

    struct VkWriteDescriptorSet
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET;
        const void* pNext = nullptr;
        void* dstSet = nullptr;
        std::uint32_t dstBinding = 0;
        std::uint32_t dstArrayElement = 0;
        std::uint32_t descriptorCount = 0;
        std::uint32_t descriptorType = VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER;
        const VkDescriptorImageInfo* pImageInfo = nullptr;
        const void* pBufferInfo = nullptr;
        const void* pTexelBufferView = nullptr;
    };

    struct VkPushConstantRange
    {
        std::uint32_t stageFlags = 0;
        std::uint32_t offset = 0;
        std::uint32_t size = 0;
    };

    struct VkPipelineLayoutCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t setLayoutCount = 0;
        void* const* pSetLayouts = nullptr;
        std::uint32_t pushConstantRangeCount = 0;
        const VkPushConstantRange* pPushConstantRanges = nullptr;
    };

    struct VkPipelineShaderStageCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t stage = 0;
        void* module = nullptr;
        const char* pName = "main";
        const void* pSpecializationInfo = nullptr;
    };

    struct VkPipelineVertexInputStateCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t vertexBindingDescriptionCount = 0;
        const void* pVertexBindingDescriptions = nullptr;
        std::uint32_t vertexAttributeDescriptionCount = 0;
        const void* pVertexAttributeDescriptions = nullptr;
    };

    struct VkPipelineInputAssemblyStateCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t topology = VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST;
        std::uint32_t primitiveRestartEnable = 0;
    };

    struct VkViewport
    {
        float x = 0.0f;
        float y = 0.0f;
        float width = 0.0f;
        float height = 0.0f;
        float minDepth = 0.0f;
        float maxDepth = 1.0f;
    };

    struct VkPipelineViewportStateCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t viewportCount = 0;
        const VkViewport* pViewports = nullptr;
        std::uint32_t scissorCount = 0;
        const VkRect2D* pScissors = nullptr;
    };

    struct VkPipelineRasterizationStateCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t depthClampEnable = 0;
        std::uint32_t rasterizerDiscardEnable = 0;
        std::uint32_t polygonMode = VK_POLYGON_MODE_FILL;
        std::uint32_t cullMode = VK_CULL_MODE_NONE;
        std::uint32_t frontFace = VK_FRONT_FACE_COUNTER_CLOCKWISE;
        std::uint32_t depthBiasEnable = 0;
        float depthBiasConstantFactor = 0.0f;
        float depthBiasClamp = 0.0f;
        float depthBiasSlopeFactor = 0.0f;
        float lineWidth = 1.0f;
    };

    struct VkPipelineMultisampleStateCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t rasterizationSamples = VK_SAMPLE_COUNT_1_BIT;
        std::uint32_t sampleShadingEnable = 0;
        float minSampleShading = 0.0f;
        const void* pSampleMask = nullptr;
        std::uint32_t alphaToCoverageEnable = 0;
        std::uint32_t alphaToOneEnable = 0;
    };

    struct VkPipelineColorBlendAttachmentState
    {
        std::uint32_t blendEnable = 0;
        std::uint32_t srcColorBlendFactor = VK_BLEND_FACTOR_ONE;
        std::uint32_t dstColorBlendFactor = VK_BLEND_FACTOR_ZERO;
        std::uint32_t colorBlendOp = VK_BLEND_OP_ADD;
        std::uint32_t srcAlphaBlendFactor = VK_BLEND_FACTOR_ONE;
        std::uint32_t dstAlphaBlendFactor = VK_BLEND_FACTOR_ZERO;
        std::uint32_t alphaBlendOp = VK_BLEND_OP_ADD;
        std::uint32_t colorWriteMask = VK_COLOR_COMPONENT_R_BIT | VK_COLOR_COMPONENT_G_BIT | VK_COLOR_COMPONENT_B_BIT | VK_COLOR_COMPONENT_A_BIT;
    };

    struct VkPipelineColorBlendStateCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t logicOpEnable = 0;
        std::uint32_t logicOp = VK_LOGIC_OP_COPY;
        std::uint32_t attachmentCount = 0;
        const VkPipelineColorBlendAttachmentState* pAttachments = nullptr;
        float blendConstants[4] = {};
    };

    struct VkGraphicsPipelineCreateInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO;
        const void* pNext = nullptr;
        std::uint32_t flags = 0;
        std::uint32_t stageCount = 0;
        const VkPipelineShaderStageCreateInfo* pStages = nullptr;
        const VkPipelineVertexInputStateCreateInfo* pVertexInputState = nullptr;
        const VkPipelineInputAssemblyStateCreateInfo* pInputAssemblyState = nullptr;
        const void* pTessellationState = nullptr;
        const VkPipelineViewportStateCreateInfo* pViewportState = nullptr;
        const VkPipelineRasterizationStateCreateInfo* pRasterizationState = nullptr;
        const VkPipelineMultisampleStateCreateInfo* pMultisampleState = nullptr;
        const void* pDepthStencilState = nullptr;
        const VkPipelineColorBlendStateCreateInfo* pColorBlendState = nullptr;
        const void* pDynamicState = nullptr;
        void* layout = nullptr;
        void* renderPass = nullptr;
        std::uint32_t subpass = 0;
        void* basePipelineHandle = nullptr;
        std::int32_t basePipelineIndex = -1;
    };

    struct VkSubmitInfo
    {
        std::uint32_t sType = VK_STRUCTURE_TYPE_SUBMIT_INFO;
        const void* pNext = nullptr;
        std::uint32_t waitSemaphoreCount = 0;
        const void* const* pWaitSemaphores = nullptr;
        const std::uint32_t* pWaitDstStageMask = nullptr;
        std::uint32_t commandBufferCount = 0;
        const void* const* pCommandBuffers = nullptr;
        std::uint32_t signalSemaphoreCount = 0;
        const void* const* pSignalSemaphores = nullptr;
    };

    using QueuePresentFn = std::int32_t(__fastcall*)(void* queue, const void* presentInfo);
    using CreateSwapchainFn = std::int32_t(__fastcall*)(void* device, const VkSwapchainCreateInfoKHR* createInfo, const void* allocator, void** swapchain);
    using GetSwapchainImagesFn = std::int32_t(__fastcall*)(void* device, void* swapchain, std::uint32_t* count, void* images);
    using GetDeviceProcAddrFn = void* (__fastcall*)(void* device, const char* name);
    using CreateImageViewFn = std::int32_t(__fastcall*)(void* device, const VkImageViewCreateInfo* createInfo, const void* allocator, void** imageView);
    using DestroyImageViewFn = void(__fastcall*)(void* device, void* imageView, const void* allocator);
    using CreateRenderPassFn = std::int32_t(__fastcall*)(void* device, const VkRenderPassCreateInfo* createInfo, const void* allocator, void** renderPass);
    using DestroyRenderPassFn = void(__fastcall*)(void* device, void* renderPass, const void* allocator);
    using CreateFramebufferFn = std::int32_t(__fastcall*)(void* device, const VkFramebufferCreateInfo* createInfo, const void* allocator, void** framebuffer);
    using DestroyFramebufferFn = void(__fastcall*)(void* device, void* framebuffer, const void* allocator);
    using CreateCommandPoolFn = std::int32_t(__fastcall*)(void* device, const VkCommandPoolCreateInfo* createInfo, const void* allocator, void** commandPool);
    using DestroyCommandPoolFn = void(__fastcall*)(void* device, void* commandPool, const void* allocator);
    using AllocateCommandBuffersFn = std::int32_t(__fastcall*)(void* device, const VkCommandBufferAllocateInfo* allocateInfo, void** commandBuffers);
    using ResetCommandBufferFn = std::int32_t(__fastcall*)(void* commandBuffer, std::uint32_t flags);
    using BeginCommandBufferFn = std::int32_t(__fastcall*)(void* commandBuffer, const VkCommandBufferBeginInfo* beginInfo);
    using EndCommandBufferFn = std::int32_t(__fastcall*)(void* commandBuffer);
    using CmdBeginRenderPassFn = void(__fastcall*)(void* commandBuffer, const VkRenderPassBeginInfo* beginInfo, std::uint32_t contents);
    using CmdEndRenderPassFn = void(__fastcall*)(void* commandBuffer);
    using CmdClearAttachmentsFn = void(__fastcall*)(void* commandBuffer, std::uint32_t attachmentCount, const VkClearAttachment* attachments, std::uint32_t rectCount, const VkClearRect* rects);
    using CreateBufferFn = std::int32_t(__fastcall*)(void* device, const VkBufferCreateInfo* createInfo, const void* allocator, void** buffer);
    using DestroyBufferFn = void(__fastcall*)(void* device, void* buffer, const void* allocator);
    using GetBufferMemoryRequirementsFn = void(__fastcall*)(void* device, void* buffer, VkMemoryRequirements* memoryRequirements);
    using CreateImageFn = std::int32_t(__fastcall*)(void* device, const VkImageCreateInfo* createInfo, const void* allocator, void** image);
    using DestroyImageFn = void(__fastcall*)(void* device, void* image, const void* allocator);
    using GetImageMemoryRequirementsFn = void(__fastcall*)(void* device, void* image, VkMemoryRequirements* memoryRequirements);
    using AllocateMemoryFn = std::int32_t(__fastcall*)(void* device, const VkMemoryAllocateInfo* allocateInfo, const void* allocator, void** memory);
    using FreeMemoryFn = void(__fastcall*)(void* device, void* memory, const void* allocator);
    using BindBufferMemoryFn = std::int32_t(__fastcall*)(void* device, void* buffer, void* memory, std::uint64_t memoryOffset);
    using BindImageMemoryFn = std::int32_t(__fastcall*)(void* device, void* image, void* memory, std::uint64_t memoryOffset);
    using MapMemoryFn = std::int32_t(__fastcall*)(void* device, void* memory, std::uint64_t offset, std::uint64_t size, std::uint32_t flags, void** data);
    using UnmapMemoryFn = void(__fastcall*)(void* device, void* memory);
    using CreateSamplerFn = std::int32_t(__fastcall*)(void* device, const VkSamplerCreateInfo* createInfo, const void* allocator, void** sampler);
    using DestroySamplerFn = void(__fastcall*)(void* device, void* sampler, const void* allocator);
    using CreateShaderModuleFn = std::int32_t(__fastcall*)(void* device, const VkShaderModuleCreateInfo* createInfo, const void* allocator, void** shaderModule);
    using DestroyShaderModuleFn = void(__fastcall*)(void* device, void* shaderModule, const void* allocator);
    using CreateDescriptorSetLayoutFn = std::int32_t(__fastcall*)(void* device, const VkDescriptorSetLayoutCreateInfo* createInfo, const void* allocator, void** setLayout);
    using DestroyDescriptorSetLayoutFn = void(__fastcall*)(void* device, void* setLayout, const void* allocator);
    using CreateDescriptorPoolFn = std::int32_t(__fastcall*)(void* device, const VkDescriptorPoolCreateInfo* createInfo, const void* allocator, void** descriptorPool);
    using DestroyDescriptorPoolFn = void(__fastcall*)(void* device, void* descriptorPool, const void* allocator);
    using AllocateDescriptorSetsFn = std::int32_t(__fastcall*)(void* device, const VkDescriptorSetAllocateInfo* allocateInfo, void** descriptorSets);
    using UpdateDescriptorSetsFn = void(__fastcall*)(void* device, std::uint32_t descriptorWriteCount, const VkWriteDescriptorSet* descriptorWrites, std::uint32_t descriptorCopyCount, const void* descriptorCopies);
    using CreatePipelineLayoutFn = std::int32_t(__fastcall*)(void* device, const VkPipelineLayoutCreateInfo* createInfo, const void* allocator, void** pipelineLayout);
    using DestroyPipelineLayoutFn = void(__fastcall*)(void* device, void* pipelineLayout, const void* allocator);
    using CreateGraphicsPipelinesFn = std::int32_t(__fastcall*)(void* device, void* pipelineCache, std::uint32_t createInfoCount, const VkGraphicsPipelineCreateInfo* createInfos, const void* allocator, void** pipelines);
    using DestroyPipelineFn = void(__fastcall*)(void* device, void* pipeline, const void* allocator);
    using CmdPipelineBarrierFn = void(__fastcall*)(void* commandBuffer, std::uint32_t srcStageMask, std::uint32_t dstStageMask, std::uint32_t dependencyFlags, std::uint32_t memoryBarrierCount, const void* memoryBarriers, std::uint32_t bufferMemoryBarrierCount, const void* bufferMemoryBarriers, std::uint32_t imageMemoryBarrierCount, const VkImageMemoryBarrier* imageMemoryBarriers);
    using CmdCopyBufferToImageFn = void(__fastcall*)(void* commandBuffer, void* srcBuffer, void* dstImage, std::uint32_t dstImageLayout, std::uint32_t regionCount, const VkBufferImageCopy* regions);
    using CmdBindPipelineFn = void(__fastcall*)(void* commandBuffer, std::uint32_t pipelineBindPoint, void* pipeline);
    using CmdBindDescriptorSetsFn = void(__fastcall*)(void* commandBuffer, std::uint32_t pipelineBindPoint, void* layout, std::uint32_t firstSet, std::uint32_t descriptorSetCount, void* const* descriptorSets, std::uint32_t dynamicOffsetCount, const std::uint32_t* dynamicOffsets);
    using CmdPushConstantsFn = void(__fastcall*)(void* commandBuffer, void* layout, std::uint32_t stageFlags, std::uint32_t offset, std::uint32_t size, const void* values);
    using CmdDrawFn = void(__fastcall*)(void* commandBuffer, std::uint32_t vertexCount, std::uint32_t instanceCount, std::uint32_t firstVertex, std::uint32_t firstInstance);
    using FlushMappedMemoryRangesFn = std::int32_t(__fastcall*)(void* device, std::uint32_t rangeCount, const VkMappedMemoryRange* ranges);
    using CreateSemaphoreFn = std::int32_t(__fastcall*)(void* device, const VkSemaphoreCreateInfo* createInfo, const void* allocator, void** semaphore);
    using DestroySemaphoreFn = void(__fastcall*)(void* device, void* semaphore, const void* allocator);
    using CreateFenceFn = std::int32_t(__fastcall*)(void* device, const VkFenceCreateInfo* createInfo, const void* allocator, void** fence);
    using DestroyFenceFn = void(__fastcall*)(void* device, void* fence, const void* allocator);
    using GetFenceStatusFn = std::int32_t(__fastcall*)(void* device, void* fence);
    using ResetFencesFn = std::int32_t(__fastcall*)(void* device, std::uint32_t fenceCount, void* const* fences);
    using QueueSubmitFn = std::int32_t(__fastcall*)(void* queue, std::uint32_t submitCount, const VkSubmitInfo* submits, void* fence);
    using DeviceWaitIdleFn = std::int32_t(__fastcall*)(void* device);

    struct SwapchainRuntimeInfo
    {
        uintptr_t handle = 0;
        uintptr_t device = 0;
        std::uint64_t generation = 0;
        std::uint32_t format = 0;
        std::uint32_t colorSpace = 0;
        std::uint32_t width = 0;
        std::uint32_t height = 0;
        std::uint32_t minImageCount = 0;
        std::uint32_t imageCount = 0;
        std::vector<uintptr_t> images;
    };
    struct VulkanRendererFns
    {
        GetSwapchainImagesFn getSwapchainImages = nullptr;
        CreateImageViewFn createImageView = nullptr;
        DestroyImageViewFn destroyImageView = nullptr;
        CreateRenderPassFn createRenderPass = nullptr;
        DestroyRenderPassFn destroyRenderPass = nullptr;
        CreateFramebufferFn createFramebuffer = nullptr;
        DestroyFramebufferFn destroyFramebuffer = nullptr;
        CreateCommandPoolFn createCommandPool = nullptr;
        DestroyCommandPoolFn destroyCommandPool = nullptr;
        AllocateCommandBuffersFn allocateCommandBuffers = nullptr;
        ResetCommandBufferFn resetCommandBuffer = nullptr;
        BeginCommandBufferFn beginCommandBuffer = nullptr;
        EndCommandBufferFn endCommandBuffer = nullptr;
        CmdBeginRenderPassFn cmdBeginRenderPass = nullptr;
        CmdEndRenderPassFn cmdEndRenderPass = nullptr;
        CmdClearAttachmentsFn cmdClearAttachments = nullptr;
        CreateBufferFn createBuffer = nullptr;
        DestroyBufferFn destroyBuffer = nullptr;
        GetBufferMemoryRequirementsFn getBufferMemoryRequirements = nullptr;
        CreateImageFn createImage = nullptr;
        DestroyImageFn destroyImage = nullptr;
        GetImageMemoryRequirementsFn getImageMemoryRequirements = nullptr;
        AllocateMemoryFn allocateMemory = nullptr;
        FreeMemoryFn freeMemory = nullptr;
        BindBufferMemoryFn bindBufferMemory = nullptr;
        BindImageMemoryFn bindImageMemory = nullptr;
        MapMemoryFn mapMemory = nullptr;
        UnmapMemoryFn unmapMemory = nullptr;
        CreateSamplerFn createSampler = nullptr;
        DestroySamplerFn destroySampler = nullptr;
        CreateShaderModuleFn createShaderModule = nullptr;
        DestroyShaderModuleFn destroyShaderModule = nullptr;
        CreateDescriptorSetLayoutFn createDescriptorSetLayout = nullptr;
        DestroyDescriptorSetLayoutFn destroyDescriptorSetLayout = nullptr;
        CreateDescriptorPoolFn createDescriptorPool = nullptr;
        DestroyDescriptorPoolFn destroyDescriptorPool = nullptr;
        AllocateDescriptorSetsFn allocateDescriptorSets = nullptr;
        UpdateDescriptorSetsFn updateDescriptorSets = nullptr;
        CreatePipelineLayoutFn createPipelineLayout = nullptr;
        DestroyPipelineLayoutFn destroyPipelineLayout = nullptr;
        CreateGraphicsPipelinesFn createGraphicsPipelines = nullptr;
        DestroyPipelineFn destroyPipeline = nullptr;
        CmdPipelineBarrierFn cmdPipelineBarrier = nullptr;
        CmdCopyBufferToImageFn cmdCopyBufferToImage = nullptr;
        CmdBindPipelineFn cmdBindPipeline = nullptr;
        CmdBindDescriptorSetsFn cmdBindDescriptorSets = nullptr;
        CmdPushConstantsFn cmdPushConstants = nullptr;
        CmdDrawFn cmdDraw = nullptr;
        FlushMappedMemoryRangesFn flushMappedMemoryRanges = nullptr;
        CreateSemaphoreFn createSemaphore = nullptr;
        DestroySemaphoreFn destroySemaphore = nullptr;
        CreateFenceFn createFence = nullptr;
        DestroyFenceFn destroyFence = nullptr;
        GetFenceStatusFn getFenceStatus = nullptr;
        ResetFencesFn resetFences = nullptr;
        QueueSubmitFn queueSubmit = nullptr;
        DeviceWaitIdleFn deviceWaitIdle = nullptr;

        bool Ready() const
        {
            return getSwapchainImages != nullptr &&
                createImageView != nullptr &&
                destroyImageView != nullptr &&
                createRenderPass != nullptr &&
                destroyRenderPass != nullptr &&
                createFramebuffer != nullptr &&
                destroyFramebuffer != nullptr &&
                createCommandPool != nullptr &&
                destroyCommandPool != nullptr &&
                allocateCommandBuffers != nullptr &&
                resetCommandBuffer != nullptr &&
                beginCommandBuffer != nullptr &&
                endCommandBuffer != nullptr &&
                cmdBeginRenderPass != nullptr &&
                cmdEndRenderPass != nullptr &&
                cmdClearAttachments != nullptr &&
                createSemaphore != nullptr &&
                destroySemaphore != nullptr &&
                createFence != nullptr &&
                destroyFence != nullptr &&
                getFenceStatus != nullptr &&
                resetFences != nullptr &&
                queueSubmit != nullptr &&
                deviceWaitIdle != nullptr;
        }

        bool TextureReady() const
        {
            return createImageView != nullptr &&
                destroyImageView != nullptr &&
                createBuffer != nullptr &&
                destroyBuffer != nullptr &&
                getBufferMemoryRequirements != nullptr &&
                createImage != nullptr &&
                destroyImage != nullptr &&
                getImageMemoryRequirements != nullptr &&
                allocateMemory != nullptr &&
                freeMemory != nullptr &&
                bindBufferMemory != nullptr &&
                bindImageMemory != nullptr &&
                mapMemory != nullptr &&
                unmapMemory != nullptr &&
                createSampler != nullptr &&
                destroySampler != nullptr &&
                createShaderModule != nullptr &&
                destroyShaderModule != nullptr &&
                createDescriptorSetLayout != nullptr &&
                destroyDescriptorSetLayout != nullptr &&
                createDescriptorPool != nullptr &&
                destroyDescriptorPool != nullptr &&
                allocateDescriptorSets != nullptr &&
                updateDescriptorSets != nullptr &&
                createPipelineLayout != nullptr &&
                destroyPipelineLayout != nullptr &&
                createGraphicsPipelines != nullptr &&
                destroyPipeline != nullptr &&
                cmdPipelineBarrier != nullptr &&
                cmdCopyBufferToImage != nullptr &&
                cmdBindPipeline != nullptr &&
                cmdBindDescriptorSets != nullptr &&
                cmdPushConstants != nullptr &&
                cmdDraw != nullptr;
        }
    };
    struct TexturedResources
    {
        uintptr_t frameTextureImage = 0;
        uintptr_t frameTextureMemory = 0;
        uintptr_t frameTextureImageView = 0;
        uintptr_t frameSampler = 0;
        uintptr_t frameStagingBuffer = 0;
        uintptr_t frameStagingMemory = 0;
        uintptr_t frameDescriptorSetLayout = 0;
        uintptr_t frameDescriptorPool = 0;
        uintptr_t frameDescriptorSet = 0;
        uintptr_t framePipelineLayout = 0;
        uintptr_t framePipeline = 0;
        std::uint32_t frameTextureWidth = 0;
        std::uint32_t frameTextureHeight = 0;
        bool frameTextureUploadPending = false;
        bool frameTextureReady = false;
    };
}
