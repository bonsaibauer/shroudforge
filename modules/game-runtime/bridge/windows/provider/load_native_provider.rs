use crate::KFC_RUNTIME_ABI_VERSION;
use crate::windows::native_abi::*;
use std::{
    ffi::{c_char, c_void},
    sync::OnceLock,
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *const c_void;
}

static PROVIDER: OnceLock<Provider> = OnceLock::new();
pub(in crate::windows) static PROVIDER_ERROR: OnceLock<String> = OnceLock::new();

pub(in crate::windows) fn provider() -> Option<&'static Provider> {
    if let Some(provider) = PROVIDER.get() {
        return Some(provider);
    }
    let loaded = unsafe {
        (|| -> Result<Provider, String> {
            let module_name: Vec<u16> = "kfc-runtime.dll\0".encode_utf16().collect();
            let module = GetModuleHandleW(module_name.as_ptr());
            if module.is_null() {
                Err("provider-module-not-loaded".to_string())
            } else {
                macro_rules! symbol {
                    ($name:literal, $kind:ty) => {{
                        let pointer = GetProcAddress(module, concat!($name, "\0").as_ptr().cast());
                        if pointer.is_null() {
                            return Err(concat!("missing-export:", $name).to_string());
                        }
                        std::mem::transmute::<*const c_void, $kind>(pointer)
                    }};
                }
                macro_rules! optional_symbol {
                    ($name:literal, $kind:ty) => {{
                        let pointer = GetProcAddress(module, concat!($name, "\0").as_ptr().cast());
                        (!pointer.is_null())
                            .then(|| std::mem::transmute::<*const c_void, $kind>(pointer))
                    }};
                }
                let abi = symbol!("KfcRuntimeAbi", unsafe extern "C" fn() -> u32);
                let actual_abi = abi();
                if actual_abi != KFC_RUNTIME_ABI_VERSION {
                    Err(format!(
                        "provider-abi-mismatch:expected={KFC_RUNTIME_ABI_VERSION},actual={actual_abi}"
                    ))
                } else {
                    Ok(Provider {
                        configure: symbol!("KfcRuntimeEcsConfigure", Configure),
                        ready: symbol!("KfcRuntimeEcsReady", Ready),
                        prop_query_ready: optional_symbol!("KfcRuntimeEcsPropQueryReady", Ready),
                        can_write: symbol!("KfcRuntimeEcsCanWrite", Ready),
                        can_write_component: optional_symbol!(
                            "KfcRuntimeEcsCanWriteComponent",
                            RuntimePatchAvailable
                        ),
                        describe: symbol!("KfcRuntimeEcsDescribe", Describe),
                        registry: optional_symbol!("KfcRuntimeEcsRegistry", Registry),
                        registration: optional_symbol!("KfcRuntimeEcsRegistration", Registration),
                        functions: optional_symbol!("KfcRuntimeFunctions", Registry),
                        function_code: optional_symbol!("KfcRuntimeFunctionCode", FunctionCode),
                        service_read: optional_symbol!("KfcRuntimeServiceRead", ServiceRead),
                        impact_compare_entity_ids: optional_symbol!(
                            "KfcRuntimeImpactCompareEntityIds",
                            ImpactCompareEntityIds
                        ),
                        service_compare_exchange: optional_symbol!(
                            "KfcRuntimeServiceCompareExchange",
                            ServiceCompareExchange
                        ),
                        resources_by_type: optional_symbol!(
                            "KfcRuntimeResourcesByType",
                            ResourcesByType
                        ),
                        resource_read: optional_symbol!("KfcRuntimeResourceRead", ResourceRead),
                        resource_read_scalar_batch: optional_symbol!(
                            "KfcRuntimeResourceReadScalarBatch",
                            ResourceReadScalarBatch
                        ),
                        resource_resolve_item: optional_symbol!(
                            "KfcRuntimeResourceResolveItem",
                            ResourceResolveItem
                        ),
                        resource_status: optional_symbol!("KfcRuntimeResourceStatus", Registry),
                        resource_can_write: optional_symbol!("KfcRuntimeResourceCanWrite", Ready),
                        resource_compare_exchange_scalar_for_session: optional_symbol!(
                            "KfcRuntimeResourceCompareExchangeScalarForSession",
                            ResourceCompareExchangeScalarForSession
                        ),
                        resolve_type: optional_symbol!("KfcRuntimeEcsResolveType", ResolveType),
                        entity_template_uuid: optional_symbol!(
                            "KfcRuntimeEcsTemplateUuid",
                            EntityTemplateUuid
                        ),
                        query: symbol!("KfcRuntimeEcsQuery", Query),
                        query_bounds: optional_symbol!("KfcRuntimeEcsQueryBounds", QueryBounds),
                        resolve_entity: symbol!("KfcRuntimeEcsResolve", ResolveEntity),
                        read: symbol!("KfcRuntimeEcsRead", Read),
                        read_many: optional_symbol!("KfcRuntimeEcsReadMany", ReadMany),
                        compare_exchange: optional_symbol!(
                            "KfcRuntimeEcsCompareExchange",
                            CompareExchange
                        ),
                        world_operation_available: symbol!(
                            "KfcRuntimeWorldOperationAvailable",
                            WorldOperationAvailable
                        ),
                        world_session_id: optional_symbol!(
                            "KfcRuntimeWorldSessionId",
                            WorldSessionId
                        ),
                        world_context_active: symbol!(
                            "KfcRuntimeWorldContextActive",
                            WorldContextActive
                        ),
                        world_context_kind: optional_symbol!(
                            "KfcRuntimeWorldContextKind",
                            WorldContextKind
                        ),
                        world_entity_context_ready: symbol!(
                            "KfcRuntimeWorldEntityContextReady",
                            WorldEntityContextReady
                        ),
                        world_entity_query_props: symbol!(
                            "KfcRuntimeWorldEntityQueryProps",
                            WorldEntityQueryProps
                        ),
                        world_entity_query_props_in_bounds: symbol!(
                            "KfcRuntimeWorldEntityQueryPropsInBounds",
                            WorldEntityQueryPropsInBounds
                        ),
                        world_entity_register_prop_recipes: symbol!(
                            "KfcRuntimeWorldEntityRegisterPropRecipes",
                            WorldEntityRegisterPropRecipes
                        ),
                        world_entity_get_transform: symbol!(
                            "KfcRuntimeWorldEntityGetTransform",
                            WorldEntityGetTransform
                        ),
                        world_entity_read_transform_for_session: optional_symbol!(
                            "KfcRuntimeWorldEntityReadTransformForSession",
                            WorldEntityReadTransformForSession
                        ),
                        world_entity_identity: symbol!(
                            "KfcRuntimeEcsEntityIdentity",
                            WorldEntityIdentity
                        ),
                        world_entity_set_scale: symbol!(
                            "KfcRuntimeWorldEntitySetScale",
                            WorldEntitySetScale
                        ),
                        world_cursor_read: optional_symbol!(
                            "KfcRuntimeWorldCursorRead",
                            WorldCursorRead
                        ),
                        world_cursor_read_input: optional_symbol!(
                            "KfcRuntimeWorldCursorReadInput",
                            WorldCursorReadInput
                        ),
                        world_cursor_input_detail: optional_symbol!(
                            "KfcRuntimeWorldCursorInputDetail",
                            WorldCursorInputDetail
                        ),
                        world_voxel_read_for_session: optional_symbol!(
                            "KfcRuntimeWorldVoxelReadForSession",
                            WorldVoxelReadForSession
                        ),
                        world_voxel_write: symbol!("KfcRuntimeWorldVoxelWrite", WorldVoxelWrite),
                        world_grid_get_spec_for_session: optional_symbol!(
                            "KfcRuntimeWorldGridGetSpecForSession",
                            WorldGridGetSpecForSession
                        ),
                        world_entity_spawn: symbol!("KfcRuntimeWorldEntitySpawn", WorldEntitySpawn),
                        world_entity_place: symbol!(
                            "KfcRuntimeWorldEntityPlace",
                            WorldEntityPlacement
                        ),
                        world_entity_destroy: symbol!(
                            "KfcRuntimeWorldEntityDestroy",
                            WorldEntityPlacement
                        ),
                        world_entity_destroy_handle: symbol!(
                            "KfcRuntimeWorldEntityDestroyHandle",
                            WorldEntityDestroyHandle
                        ),
                        world_entity_finish: symbol!(
                            "KfcRuntimeWorldEntityFinishBuilding",
                            WorldEntityFinish
                        ),
                        runtime_patch_available: symbol!(
                            "KfcRuntimePatchAvailable",
                            RuntimePatchAvailable
                        ),
                        runtime_patch_set_enabled: symbol!(
                            "KfcRuntimePatchSetEnabled",
                            RuntimePatchSetEnabled
                        ),
                        abi: actual_abi,
                        status: symbol!(
                            "KfcRuntimeStatus",
                            unsafe extern "C" fn(*mut c_char, usize)
                        ),
                    })
                }
            }
        })()
    };
    match loaded {
        Ok(provider) => {
            let _ = PROVIDER.set(provider);
            PROVIDER.get()
        }
        Err(error) => {
            // The runtime DLL may be loaded just after Lua environments are
            // created. Treat that as a transient startup condition and try
            // again on the next API call instead of poisoning this process.
            if error != "provider-module-not-loaded" {
                let _ = PROVIDER_ERROR.set(error);
            }
            None
        }
    }
}

pub(in crate::windows) fn observed_abi() -> Option<u32> {
    unsafe {
        let module_name: Vec<u16> = "kfc-runtime.dll\0".encode_utf16().collect();
        let module = GetModuleHandleW(module_name.as_ptr());
        if module.is_null() {
            return None;
        }
        let pointer = GetProcAddress(module, c"KfcRuntimeAbi".as_ptr());
        if pointer.is_null() {
            return None;
        }
        let abi: unsafe extern "C" fn() -> u32 = std::mem::transmute(pointer);
        Some(abi())
    }
}
