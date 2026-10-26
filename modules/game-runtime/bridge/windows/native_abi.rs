pub use crate::runtime_records::{Component, GridSpecResult, PropRecipe, PropRecord};

use std::ffi::{c_char, c_void};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RuntimeResourceId {
    pub(super) guid: [u8; 16],
    pub(super) type_hash: u32,
    pub(super) part_index: u32,
}

impl From<&kfc::guid::ResourceId> for RuntimeResourceId {
    fn from(resource: &kfc::guid::ResourceId) -> Self {
        Self {
            guid: *resource.guid().data(),
            type_hash: resource.type_hash(),
            part_index: resource.part_index(),
        }
    }
}

pub(super) type Ready = unsafe extern "C" fn() -> bool;
pub(super) type Configure = unsafe extern "C" fn(*const *const c_char, *const u32, usize) -> bool;
pub(super) type Describe = unsafe extern "C" fn(*const c_char, *mut u32) -> bool;
pub(super) type Registry = unsafe extern "C" fn(*mut c_char, usize) -> usize;
pub(super) type Registration = unsafe extern "C" fn(*const c_char, *mut c_char, usize) -> usize;
pub(super) type FunctionCode = unsafe extern "C" fn(u32, *mut u8, usize) -> usize;
pub(super) type ResourcesByType =
    unsafe extern "C" fn(u32, *mut RuntimeResourceId, usize, *mut usize) -> bool;
pub(super) type ResourceRead =
    unsafe extern "C" fn(*const RuntimeResourceId, u32, *mut u8, usize, *mut usize) -> bool;
#[repr(C)]
pub(super) struct ResourceScalarRequest {
    pub(super) resource: RuntimeResourceId,
    pub(super) static_hash: u32,
}
pub(super) type ResourceReadScalarBatch = unsafe extern "C" fn(
    *const ResourceScalarRequest,
    usize,
    u32,
    usize,
    usize,
    u64,
    *mut u32,
    usize,
    *mut usize,
) -> u32;
pub(super) type ResourceResolveItem =
    unsafe extern "C" fn(u32, u32, usize, usize, usize, u64, *mut RuntimeResourceId) -> u32;
pub(super) type EntityTemplateUuid = unsafe extern "C" fn(u32, *mut u8) -> bool;
pub(super) type ServiceRead =
    unsafe extern "C" fn(*const c_char, u32, u32, u64, *mut u8, usize) -> bool;
pub(super) type ImpactCompareEntityIds = unsafe extern "C" fn(u64, u32, u32, *mut u8) -> u32;
pub(super) type ServiceCompareExchange = unsafe extern "C" fn(
    *const c_char,
    u32,
    u32,
    u64,
    *const c_void,
    *const c_void,
    *const c_void,
) -> u32;
pub(super) type ResourceCompareExchangeScalarForSession = unsafe extern "C" fn(
    *const RuntimeResourceId,
    u32,
    usize,
    usize,
    *const c_void,
    *const c_void,
    usize,
    u64,
) -> u32;
pub(super) type ResolveType = unsafe extern "C" fn(*const c_char, *mut c_char, usize) -> bool;
pub(super) type Query = unsafe extern "C" fn(*const *const c_char, usize, *mut u32, usize) -> usize;
pub(super) type ReadMany = unsafe extern "C" fn(
    u32,
    *const *const c_char,
    *const u32,
    usize,
    *mut u8,
    usize,
    *mut u8,
) -> bool;
pub(super) type QueryBounds =
    unsafe extern "C" fn(*const *const c_char, usize, *const f64, f64, *mut u32, usize) -> usize;
pub(super) type ResolveEntity = unsafe extern "C" fn(u32) -> u32;
pub(super) type Read = unsafe extern "C" fn(u32, *const c_char, *mut c_void, usize) -> bool;
pub(super) type CompareExchange = unsafe extern "C" fn(
    u32,
    *const c_char,
    *const c_void,
    *const c_void,
    *const c_void,
    usize,
) -> u32;
pub(super) type WorldOperationAvailable = unsafe extern "C" fn(*const c_char) -> bool;
pub(super) type WorldContextActive = unsafe extern "C" fn() -> bool;
pub(super) type WorldContextKind = unsafe extern "C" fn() -> u32;
pub(super) type MainMenuState = unsafe extern "C" fn() -> u32;
pub(super) type WorldSessionId = unsafe extern "C" fn() -> u64;
pub(super) type WorldEntityContextReady = unsafe extern "C" fn() -> bool;
pub(super) type WorldEntityQueryProps =
    unsafe extern "C" fn(*const f64, f64, *mut PropRecord, usize) -> usize;
pub(super) type WorldEntityQueryPropsInBounds =
    unsafe extern "C" fn(*const f64, *mut PropRecord, usize) -> usize;
pub(super) type WorldEntityRegisterPropRecipes =
    unsafe extern "C" fn(*const PropRecipe, usize) -> bool;
pub(super) type WorldEntityGetTransform = unsafe extern "C" fn(u32, *mut PropRecord) -> bool;
pub(super) type WorldEntityReadTransformForSession =
    unsafe extern "C" fn(u32, u64, *mut PropRecord) -> u32;
pub(super) type WorldEntityIdentity = unsafe extern "C" fn(u32, *mut u32) -> bool;
pub(super) type WorldEntitySetScale = unsafe extern "C" fn(u32, *const f64) -> bool;
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GridSpec {
    pub(super) id: [c_char; 16],
    pub(super) origin: [f64; 3],
    pub(super) cell_size: [f64; 3],
    pub(super) maximum: [u64; 3],
}
pub(super) type WorldGridGetSpecForSession =
    unsafe extern "C" fn(u64, *const c_char, *mut GridSpec) -> u32;
pub(super) type WorldCursorRead = unsafe extern "C" fn(*mut u8, usize, *mut u64) -> bool;
pub(super) type SetLocalCameraLayout = unsafe extern "C" fn(usize, *const u32, usize) -> u32;
pub(super) type WorldCursorReadInput =
    unsafe extern "C" fn(usize, u64, *mut u8, usize, *mut u64) -> u32;
pub(super) type WorldCursorInputDetail = unsafe extern "C" fn(*mut u8, usize) -> usize;
pub(super) type WorldVoxelReadForSession =
    unsafe extern "C" fn(u64, *const i32, *const u32, *mut u16, usize, *mut usize) -> u32;
pub(super) type WorldVoxelWrite =
    unsafe extern "C" fn(*const i32, *const u32, *const u16, usize, *mut u32) -> bool;
pub(super) type WorldEntitySpawn =
    unsafe extern "C" fn(*const u64, *const f64, *const f64, u32, u32, *mut u32, *mut u32) -> bool;
pub(super) type WorldEntityPlacement =
    unsafe extern "C" fn(*const f64, *const f64, *const f32, u32, u32, *mut u32) -> bool;
pub(super) type WorldEntityDestroyHandle = unsafe extern "C" fn(u32, *mut u32) -> bool;
pub(super) type WorldEntityFinish = unsafe extern "C" fn(bool, *mut u32) -> bool;
pub(super) type RuntimePatchAvailable = unsafe extern "C" fn(*const c_char) -> bool;
pub(super) type RuntimePatchSetEnabled =
    unsafe extern "C" fn(*const c_char, bool, *mut u32) -> bool;
pub(super) struct Provider {
    pub(super) configure: Configure,
    pub(super) ready: Ready,
    pub(super) prop_query_ready: Option<Ready>,
    pub(super) can_write: Ready,
    pub(super) can_write_component: Option<RuntimePatchAvailable>,
    pub(super) describe: Describe,
    pub(super) registry: Option<Registry>,
    pub(super) registration: Option<Registration>,
    pub(super) functions: Option<Registry>,
    pub(super) function_code: Option<FunctionCode>,
    pub(super) service_read: Option<ServiceRead>,
    pub(super) impact_compare_entity_ids: Option<ImpactCompareEntityIds>,
    pub(super) service_compare_exchange: Option<ServiceCompareExchange>,
    pub(super) resources_by_type: Option<ResourcesByType>,
    pub(super) resource_read: Option<ResourceRead>,
    pub(super) resource_read_scalar_batch: Option<ResourceReadScalarBatch>,
    pub(super) resource_resolve_item: Option<ResourceResolveItem>,
    pub(super) resource_can_write: Option<Ready>,
    pub(super) resource_compare_exchange_scalar_for_session:
        Option<ResourceCompareExchangeScalarForSession>,
    pub(super) resource_status: Option<Registry>,
    pub(super) entity_template_uuid: Option<EntityTemplateUuid>,
    pub(super) resolve_type: Option<ResolveType>,
    pub(super) query: Query,
    pub(super) query_bounds: Option<QueryBounds>,
    pub(super) resolve_entity: ResolveEntity,
    pub(super) read: Read,
    pub(super) read_many: Option<ReadMany>,
    pub(super) compare_exchange: Option<CompareExchange>,
    pub(super) world_operation_available: WorldOperationAvailable,
    pub(super) world_context_active: WorldContextActive,
    pub(super) world_context_kind: Option<WorldContextKind>,
    pub(super) world_session_id: Option<WorldSessionId>,
    pub(super) world_entity_context_ready: WorldEntityContextReady,
    pub(super) world_entity_query_props: WorldEntityQueryProps,
    pub(super) world_entity_query_props_in_bounds: WorldEntityQueryPropsInBounds,
    pub(super) world_entity_register_prop_recipes: WorldEntityRegisterPropRecipes,
    pub(super) world_entity_get_transform: WorldEntityGetTransform,
    pub(super) world_entity_read_transform_for_session: Option<WorldEntityReadTransformForSession>,
    pub(super) world_entity_identity: WorldEntityIdentity,
    pub(super) world_entity_set_scale: WorldEntitySetScale,
    pub(super) world_cursor_read: Option<WorldCursorRead>,
    pub(super) set_local_camera_layout: Option<SetLocalCameraLayout>,
    pub(super) world_cursor_read_input: Option<WorldCursorReadInput>,
    pub(super) world_cursor_input_detail: Option<WorldCursorInputDetail>,
    pub(super) world_voxel_read_for_session: Option<WorldVoxelReadForSession>,
    pub(super) world_voxel_write: WorldVoxelWrite,
    pub(super) world_grid_get_spec_for_session: Option<WorldGridGetSpecForSession>,
    pub(super) world_entity_spawn: WorldEntitySpawn,
    pub(super) world_entity_place: WorldEntityPlacement,
    pub(super) world_entity_destroy: WorldEntityPlacement,
    pub(super) world_entity_destroy_handle: WorldEntityDestroyHandle,
    pub(super) world_entity_finish: WorldEntityFinish,
    pub(super) runtime_patch_available: RuntimePatchAvailable,
    pub(super) runtime_patch_set_enabled: RuntimePatchSetEnabled,
    pub(super) abi: u32,
    pub(super) status: unsafe extern "C" fn(*mut c_char, usize),
    pub(super) selected_character_name: Option<Registry>,
    pub(super) main_menu_state: Option<MainMenuState>,
}
