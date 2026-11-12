// Standard Vulkan creation/resolver broker. Included in the renderer's private
// namespace so dispatch and resource ownership have one lock and one lifetime.
struct InstanceRecord {void* handle{};std::uint32_t api{},families{};};
std::unordered_map<void*,InstanceRecord> boot_instances;
std::unordered_map<void*,void*> boot_physical;
std::unordered_map<void*,std::pair<std::uintptr_t,std::uint32_t>> boot_queues;
GetInstanceProc boot_gipa{};
GetDeviceProcAddrFn boot_gdpa{};
CreateInstanceFn boot_create_instance{};
CreateDeviceFn boot_create_device{};
FARPROC (WINAPI* boot_get_proc)(HMODULE,LPCSTR)=::GetProcAddress;
// Saved entry points and published wrappers survive logical StopGraphics.
// Own the loader and any forwarded export modules continuously for that lifetime.
HMODULE boot_loader{};
std::array<HMODULE,6> boot_export_owners{};
std::size_t boot_export_owner_count{};
BOOL (WINAPI* boot_get_module_reference)(DWORD,LPCWSTR,HMODULE*)=::GetModuleHandleExW;
struct BootReferences {
    std::array<HMODULE,6> pending{};
    std::size_t count{};
    ~BootReferences(){for(std::size_t i=0;i<count;++i)FreeLibrary(pending[i]);}
    bool Keep(const void* address,HMODULE exact=nullptr) {
        HMODULE reference{};
        if(!address || !boot_get_module_reference(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            reinterpret_cast<LPCWSTR>(address),&reference))return false;
        if(exact && reference!=exact){FreeLibrary(reference);return false;}
        for(std::size_t i=0;i<boot_export_owner_count;++i)if(boot_export_owners[i]==reference){FreeLibrary(reference);return true;}
        for(std::size_t i=0;i<count;++i)if(pending[i]==reference){FreeLibrary(reference);return true;}
        if(boot_export_owner_count+count==boot_export_owners.size()){FreeLibrary(reference);return false;}
        pending[count++]=reference;return true;
    }
    void Publish() noexcept {
        for(std::size_t i=0;i<count;++i)boot_export_owners[boot_export_owner_count++]=pending[i];
        count=0; // Published references intentionally remain until process exit.
    }
};
bool BindBootDispatch(HMODULE module) {
    if(boot_loader)return boot_loader==module;
    BootReferences references;
    if(!references.Keep(module,module))return false;
    constexpr const char* names[]{"vkGetInstanceProcAddr","vkGetDeviceProcAddr","vkCreateDevice","vkQueuePresentKHR"};
    std::array<FARPROC,4> exports{};
    for(std::size_t i=0;i<exports.size();++i) {
        exports[i]=boot_get_proc(module,names[i]);
        if(exports[i] && !references.Keep(reinterpret_cast<const void*>(exports[i])))return false;
    }
    if(!exports[0])return false;
    for(std::size_t i=0;i<exports.size();++i)if(boot_get_proc(module,names[i])!=exports[i])return false;
    references.Publish();
    boot_gipa=reinterpret_cast<GetInstanceProc>(exports[0]);
    boot_gdpa=reinterpret_cast<GetDeviceProcAddrFn>(exports[1]);
    boot_create_device=reinterpret_cast<CreateDeviceFn>(exports[2]);
    original_present_fallback=reinterpret_cast<std::uintptr_t>(exports[3]);
    boot_loader=module;
    return true;
}
bool boot_armed{},boot_demand{};
std::uintptr_t* boot_import{};
constexpr const char* maintenance_names[]{"VK_KHR_swapchain_maintenance1","VK_EXT_swapchain_maintenance1"};
constexpr const char* surface_names[]{"VK_KHR_surface_maintenance1","VK_EXT_surface_maintenance1"};
void BootUnavailable(std::uint32_t reason,const char* message) {
    graphics_status=3;graphics_reason=reason;LogRendererThrottled(std::string("[ESM overlay] ")+message);
}
bool HasName(const std::vector<const char*>& names,const char* name) {
    return std::any_of(names.begin(),names.end(),[&](const char* p){return p && std::strcmp(p,name)==0;});
}
bool CopyNames(std::uint32_t count,const char*const* input,std::vector<const char*>& names) {
    if(count>256 || (count && !input))return false;
    for(std::uint32_t i=0;i<count;++i){if(!input[i])return false;names.push_back(input[i]);}return true;
}
template<class Fn,class... Args> bool ExtensionList(Fn fn,std::vector<ExtensionProperties>& list,Args... args) {
    if(!fn)return false;std::uint32_t count{};
    if(fn(args...,nullptr,&count,nullptr)!=0 || count>2048)return false;list.resize(count);
    return fn(args...,nullptr,&count,list.data())==0 && count<=list.size();
}
bool Supported(const std::vector<ExtensionProperties>& list,const char* name) {
    return std::any_of(list.begin(),list.end(),[&](const auto& p){return std::strncmp(p.name,name,sizeof(p.name))==0;});
}
// Bounded inspection only; never clone or write an unknown pNext structure.
bool Chain(const void* next,std::uint32_t wanted,const BaseIn*& found) {
    found=nullptr;for(unsigned i=0;next && i<64;++i) {
        BaseIn p{};if(!read(reinterpret_cast<std::uintptr_t>(next),p))return false;if(p.sType==wanted){if(found)return false;found=static_cast<const BaseIn*>(next);}next=p.pNext;
    }return next==nullptr;
}
void* __fastcall BrokerGipa(void*,const char*);
void* __fastcall BrokerGdpa(void*,const char*);
std::int32_t __fastcall BrokerCreateInstance(const InstanceCreateInfo* info,const void* allocator,void** out) {
    if(!boot_create_instance)return -3;
    std::lock_guard lock(render_mutex);
    std::int32_t result=-3;bool invoked=false;
    try {
    std::vector<const char*> names;std::uint32_t families{};auto adjusted=*info;
    const auto api=info->applicationInfo && info->applicationInfo->apiVersion?info->applicationInfo->apiVersion:0x400000u;
    if(boot_demand && boot_instances.size()<32 && CopyNames(info->extensionCount,info->extensions,names)) {
        const auto enumerate=reinterpret_cast<EnumerateInstanceExtensionsFn>(boot_gipa?boot_gipa(nullptr,"vkEnumerateInstanceExtensionProperties"):nullptr);
        std::vector<ExtensionProperties> available;
        if(ExtensionList(enumerate,available)) {
            const auto require=[&](const char* name){if(HasName(names,name))return true;if(!Supported(available,name))return false;names.push_back(name);return true;};
            // Never change the requested API version. 1.0 uses the enabled KHR query path.
            const bool query=api>=0x401000u || require("VK_KHR_get_physical_device_properties2");
            if(query && require("VK_KHR_surface") && require("VK_KHR_get_surface_capabilities2"))
                for(unsigned i=0;i<2;++i)if(require(surface_names[i]))families|=1u<<i;
        }
        if(families){adjusted.extensionCount=static_cast<std::uint32_t>(names.size());adjusted.extensions=names.data();}
    }
    invoked=true;result=boot_create_instance(families?&adjusted:info,allocator,out);
    if(result!=0 && families) {families=0;result=boot_create_instance(info,allocator,out);}
    if(result==0 && out && *out && boot_instances.size()<32)boot_instances[*out]={*out,api,families};
    return result;
    } catch (...) { return invoked?result:boot_create_instance(info,allocator,out); }
}
std::int32_t __fastcall BrokerEnumerate(void* instance,std::uint32_t* count,void** physical) {
    std::lock_guard lock(render_mutex);const auto fn=reinterpret_cast<EnumeratePhysicalFn>(boot_gipa(instance,"vkEnumeratePhysicalDevices"));
    const auto result=fn(instance,count,physical);
    try { if((result==0||result==5) && physical && count && *count<=256 && boot_instances.contains(instance))
        for(unsigned i=0;i<*count && boot_physical.size()<256;++i)boot_physical[physical[i]]=instance; }catch(...){}
    return result;
}
std::int32_t EnumerateGroup(void* instance,std::uint32_t* count,PhysicalDeviceGroup* groups,const char* name) {
    std::lock_guard lock(render_mutex);const auto fn=reinterpret_cast<EnumerateGroupsFn>(boot_gipa(instance,name));
    const auto result=fn(instance,count,groups);
    try { if((result==0||result==5) && groups && count && *count<=64 && boot_instances.contains(instance))
        for(unsigned i=0;i<*count;++i)if(groups[i].count<=32)
            for(unsigned j=0;j<groups[i].count && boot_physical.size()<256;++j)boot_physical[groups[i].physical[j]]=instance; }catch(...){}
    return result;
}
std::int32_t __fastcall BrokerGroups(void* i,std::uint32_t* n,PhysicalDeviceGroup* p){return EnumerateGroup(i,n,p,"vkEnumeratePhysicalDeviceGroups");}
std::int32_t __fastcall BrokerGroupsKHR(void* i,std::uint32_t* n,PhysicalDeviceGroup* p){return EnumerateGroup(i,n,p,"vkEnumeratePhysicalDeviceGroupsKHR");}
void __fastcall BrokerDestroyInstance(void* instance,const void* allocator) {
    std::lock_guard lock(render_mutex);const auto fn=reinterpret_cast<DestroyDevice>(boot_gipa(instance,"vkDestroyInstance"));
    std::erase_if(boot_physical,[&](const auto& p){return p.second==instance;});boot_instances.erase(instance);fn(instance,allocator);
}
std::int32_t __fastcall BrokerCreateDevice(void* physical,const DeviceCreateInfo* info,const void* allocator,void** out) {
    std::lock_guard lock(render_mutex);
    const auto parent=boot_physical.find(physical);
    const auto instance=parent==boot_physical.end()?nullptr:parent->second;
    const auto fn=instance?reinterpret_cast<CreateDeviceFn>(boot_gipa(instance,"vkCreateDevice")):boot_create_device;
    if(!fn)return -3;
    std::int32_t result=-3;bool invoked=false;
    try {
    std::vector<const char*> names;std::vector<ExtensionProperties> available;MaintenanceFeatures feature;
    auto adjusted=*info;bool enabled{};unsigned family{};const BaseIn* existing{};const BaseIn* group{};
    const bool single_device=Chain(info->pNext,1000070001,group) && (!group || reinterpret_cast<const DeviceGroupCreate*>(group)->count==1);
    std::uint32_t reason=SF_OVERLAY_REASON_BOOT_MISSED;
    if(boot_demand && single_device && instance && devices.size()<64 && boot_instances.contains(instance) && CopyNames(info->extensionCount,info->extensions,names)) {
        const auto& owner=boot_instances.at(instance);
        const auto enumerate=reinterpret_cast<EnumerateDeviceExtensionsFn>(boot_gipa(instance,"vkEnumerateDeviceExtensionProperties"));
        const auto query=reinterpret_cast<Features2Fn>(boot_gipa(instance,owner.api>=0x401000u?"vkGetPhysicalDeviceFeatures2":"vkGetPhysicalDeviceFeatures2KHR"));
        reason=SF_OVERLAY_REASON_MAINTENANCE_UNSUPPORTED;
        if(query && ExtensionList(enumerate,available,physical) && HasName(names,"VK_KHR_swapchain")) {
            PhysicalFeatures2 supported;supported.pNext=&feature;query(physical,&supported);
            for(unsigned i=0;i<2;++i)if((owner.families&(1u<<i)) && Supported(available,maintenance_names[i])){family=i+1;break;}
            const bool chain_valid=Chain(info->pNext,MaintenanceFeatureType,existing);
            if(!chain_valid)reason=SF_OVERLAY_REASON_FEATURE_CONFLICT;
            if(feature.enabled && family && chain_valid) {
                if(existing && !reinterpret_cast<const MaintenanceFeatures*>(existing)->enabled)reason=SF_OVERLAY_REASON_FEATURE_CONFLICT;
                else {
                    if(!HasName(names,maintenance_names[family-1]))names.push_back(maintenance_names[family-1]);
                    adjusted.extensionCount=static_cast<std::uint32_t>(names.size());adjusted.extensions=names.data();
                    if(!existing){feature.pNext=const_cast<void*>(info->pNext);feature.enabled=1;adjusted.pNext=&feature;}
                    enabled=true;
                }
            }
        }
    }
    invoked=true;result=fn(physical,enabled?&adjusted:info,allocator,out);
    if(result!=0 && enabled){enabled=false;reason=SF_OVERLAY_REASON_MAINTENANCE_UNSUPPORTED;result=fn(physical,info,allocator,out);}
    // A successful observed creation, not a function pointer/support query, is authority.
    if(result==0 && out && *out && devices.size()<64 && boot_gdpa) {
        auto d=std::make_shared<Device>();d->handle=reinterpret_cast<std::uintptr_t>(*out);d->generation=++generation_counter;
        d->broker=true;d->maintenance=enabled;d->capability_reason=enabled?0:reason;d->resolver=boot_gdpa;
        d->create=reinterpret_cast<CreateSwapchainFn>(boot_gdpa(*out,"vkCreateSwapchainKHR"));d->images=reinterpret_cast<GetSwapchainImagesFn>(boot_gdpa(*out,"vkGetSwapchainImagesKHR"));
        d->present=reinterpret_cast<QueuePresentFn>(boot_gdpa(*out,"vkQueuePresentKHR"));d->destroy_swapchain=reinterpret_cast<DestroySwapchain>(boot_gdpa(*out,"vkDestroySwapchainKHR"));d->destroy=reinterpret_cast<DestroyDevice>(boot_gdpa(*out,"vkDestroyDevice"));
        d->wait_fences=reinterpret_cast<WaitFencesFn>(boot_gdpa(*out,"vkWaitForFences"));
        d->maintenance=d->maintenance && d->wait_fences && d->create && d->images && d->present && d->destroy && d->destroy_swapchain;
        if(!d->maintenance && !d->capability_reason)d->capability_reason=reason=SF_OVERLAY_REASON_MAINTENANCE_UNSUPPORTED;
        devices[d->handle]=d;
        if(!d->maintenance)BootUnavailable(reason,"present-fence capability unavailable on observed device; supported maintenance feature and boot interception are required");
    }
    return result;
    } catch (...) { return invoked?result:fn(physical,info,allocator,out); }
}
void __fastcall BrokerGetQueue(void* handle,std::uint32_t family,std::uint32_t index,void** out) {
    std::lock_guard lock(render_mutex);const auto fn=reinterpret_cast<GetDeviceQueue>(boot_gdpa(handle,"vkGetDeviceQueue"));fn(handle,family,index,out);
    try { if(out && *out && devices.contains(reinterpret_cast<std::uintptr_t>(handle)) && boot_queues.size()<256)boot_queues[*out]={reinterpret_cast<std::uintptr_t>(handle),family}; }catch(...){}
}
void __fastcall BrokerGetQueue2(void* handle,const DeviceQueueInfo2* info,void** out) {
    std::lock_guard lock(render_mutex);const auto fn=reinterpret_cast<void(__fastcall*)(void*,const DeviceQueueInfo2*,void**)>(boot_gdpa(handle,"vkGetDeviceQueue2"));fn(handle,info,out);
    try { if(out && *out && devices.contains(reinterpret_cast<std::uintptr_t>(handle)) && boot_queues.size()<256)boot_queues[*out]={reinterpret_cast<std::uintptr_t>(handle),info->family}; }catch(...){}
}
void* RouteInstance(void* instance,const char* name,void* raw,HMODULE export_module=nullptr) {
    if(!raw || !name)return raw;
    if(!std::strcmp(name,"vkGetInstanceProcAddr"))return reinterpret_cast<void*>(&BrokerGipa);
    if(!std::strcmp(name,"vkGetDeviceProcAddr"))return boot_gdpa?reinterpret_cast<void*>(&BrokerGdpa):raw;
    if(!std::strcmp(name,"vkCreateInstance")){
        const auto create=reinterpret_cast<CreateInstanceFn>(raw);
        if(boot_create_instance && boot_create_instance!=create)return raw;
        if(!boot_create_instance){
            BootReferences references;
            if(!references.Keep(raw))return raw;
            const auto current=export_module?reinterpret_cast<void*>(boot_get_proc(export_module,name)):boot_gipa(instance,name);
            if(current!=raw)return raw;
            references.Publish();boot_create_instance=create;
        }
        return reinterpret_cast<void*>(&BrokerCreateInstance);
    }
    if(!std::strcmp(name,"vkCreateDevice"))return reinterpret_cast<void*>(&BrokerCreateDevice);
    if(!instance || !boot_instances.contains(instance))return raw;
    if(!std::strcmp(name,"vkEnumeratePhysicalDevices"))return reinterpret_cast<void*>(&BrokerEnumerate);
    if(!std::strcmp(name,"vkEnumeratePhysicalDeviceGroups"))return reinterpret_cast<void*>(&BrokerGroups);
    if(!std::strcmp(name,"vkEnumeratePhysicalDeviceGroupsKHR"))return reinterpret_cast<void*>(&BrokerGroupsKHR);
    if(!std::strcmp(name,"vkDestroyInstance"))return reinterpret_cast<void*>(&BrokerDestroyInstance);
    return raw;
}
void* __fastcall BrokerGipa(void* instance,const char* name) {
    const auto raw=boot_gipa(instance,name);std::lock_guard lock(render_mutex);return RouteInstance(instance,name,raw);
}
void* __fastcall BrokerGdpa(void* handle,const char* name) {
    const auto raw=boot_gdpa(handle,name);std::lock_guard lock(render_mutex);const auto device=FindDevice(handle);
    if(!raw || !name || !device || !device->broker)return raw;
    if(!std::strcmp(name,"vkGetDeviceProcAddr"))return boot_gdpa?reinterpret_cast<void*>(&BrokerGdpa):raw;
    if(!std::strcmp(name,"vkDestroyDevice"))return reinterpret_cast<void*>(&Destroy);
    if(!device->maintenance)return raw;
    if(!std::strcmp(name,"vkGetDeviceQueue"))return reinterpret_cast<void*>(&BrokerGetQueue);
    if(!std::strcmp(name,"vkGetDeviceQueue2"))return reinterpret_cast<void*>(&BrokerGetQueue2);
    if(!std::strcmp(name,"vkCreateSwapchainKHR"))return reinterpret_cast<void*>(&Create);
    if(!std::strcmp(name,"vkGetSwapchainImagesKHR"))return reinterpret_cast<void*>(&Images);
    if(!std::strcmp(name,"vkQueuePresentKHR"))return reinterpret_cast<void*>(&Present);
    if(!std::strcmp(name,"vkDestroySwapchainKHR"))return reinterpret_cast<void*>(&DestroySwap);
    return raw;
}
FARPROC WINAPI BrokerGetProc(HMODULE module,LPCSTR name) {
    const auto raw=boot_get_proc(module,name);
    // Ordinals, other modules, and unavailable exports preserve Win32 behavior.
    if(!raw || reinterpret_cast<std::uintptr_t>(name)<=0xffff || module!=GetModuleHandleW(L"vulkan-1.dll"))return raw;
    std::lock_guard lock(render_mutex);
    if(!BindBootDispatch(module))return raw;
    return reinterpret_cast<FARPROC>(RouteInstance(nullptr,name,reinterpret_cast<void*>(raw),module));
}
