#pragma once
// ABI subset verified against Khronos Vulkan-Headers v1.4.321, vulkan_core.h.
// Original declarations: Copyright 2015-2025 The Khronos Group Inc., Apache-2.0.
#include "graphics/vulkan_api.h"
namespace OverlayRuntime::Vulkan {
constexpr std::uint32_t MaintenanceFeatureType=1000275000, PresentFenceType=1000275001;
struct BaseIn {std::uint32_t sType;const void* pNext;};
struct ApplicationInfo {std::uint32_t sType;const void* pNext;const char* applicationName;std::uint32_t applicationVersion;const char* engineName;std::uint32_t engineVersion,apiVersion;};
struct InstanceCreateInfo {std::uint32_t sType;const void* pNext;std::uint32_t flags;const ApplicationInfo* applicationInfo;std::uint32_t layerCount;const char*const* layers;std::uint32_t extensionCount;const char*const* extensions;};
struct DeviceCreateInfo {std::uint32_t sType;const void* pNext;std::uint32_t flags,queueCount;const void* queues;std::uint32_t layerCount;const char*const* layers;std::uint32_t extensionCount;const char*const* extensions;const void* features;};
struct MaintenanceFeatures {std::uint32_t sType=MaintenanceFeatureType;void* pNext{};std::uint32_t enabled{};};
struct PhysicalFeatures2 {std::uint32_t sType=1000059000;void* pNext{};std::uint32_t features[55]{};};
constexpr std::uint32_t QueueGraphicsBit=0x1;
struct QueueFamilyProperties {std::uint32_t flags,count,timestampValidBits;VkExtent3D minImageTransferGranularity;};
using QueueFamilyPropertiesFn=void(__fastcall*)(void*,std::uint32_t*,QueueFamilyProperties*);
static_assert(sizeof(QueueFamilyProperties)==24);
struct ExtensionProperties {char name[256];std::uint32_t version;};
struct PhysicalDeviceGroup {std::uint32_t sType;void* pNext;std::uint32_t count;void* physical[32];std::uint32_t subset;};
struct DeviceGroupCreate {std::uint32_t sType;const void* pNext;std::uint32_t count;void*const* physical;};
struct DeviceQueueInfo2 {std::uint32_t sType;const void* pNext;std::uint32_t flags,family,index;};
struct PresentFences {std::uint32_t sType=PresentFenceType;const void* pNext{};std::uint32_t count{};void*const* fences{};};
using GetInstanceProc=void*(__fastcall*)(void*,const char*);
using CreateInstanceFn=std::int32_t(__fastcall*)(const InstanceCreateInfo*,const void*,void**);
using CreateDeviceFn=std::int32_t(__fastcall*)(void*,const DeviceCreateInfo*,const void*,void**);
using EnumerateInstanceExtensionsFn=std::int32_t(__fastcall*)(const char*,std::uint32_t*,ExtensionProperties*);
using EnumerateDeviceExtensionsFn=std::int32_t(__fastcall*)(void*,const char*,std::uint32_t*,ExtensionProperties*);
using EnumeratePhysicalFn=std::int32_t(__fastcall*)(void*,std::uint32_t*,void**);
using EnumerateGroupsFn=std::int32_t(__fastcall*)(void*,std::uint32_t*,PhysicalDeviceGroup*);
using Features2Fn=void(__fastcall*)(void*,PhysicalFeatures2*);
using WaitFencesFn=std::int32_t(__fastcall*)(void*,std::uint32_t,void*const*,std::uint32_t,std::uint64_t);
static_assert(sizeof(InstanceCreateInfo)==64 && sizeof(DeviceCreateInfo)==72 && sizeof(PhysicalFeatures2)==240);
static_assert(sizeof(MaintenanceFeatures)==24 && sizeof(PresentFences)==32);
}
