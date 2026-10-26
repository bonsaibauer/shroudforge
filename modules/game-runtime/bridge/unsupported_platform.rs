pub use crate::runtime_records::{Component, GridSpecResult, PropRecipe, PropRecord};

pub fn impact_compare_entity_ids(_: u64, _: u32, _: u32) -> Result<bool, String> {
    Err("original registered Impact operations are available on Windows only".into())
}

pub fn service_read(_: &str, _: u32, _: u32, _: u64) -> Result<Vec<u8>, String> {
    Err("live engine service access is available on Windows only".into())
}
pub fn service_compare_exchange(
    _: &str,
    _: u32,
    _: u32,
    _: u64,
    _: &[u8],
    _: &[u8],
    _: &[u8],
) -> Result<(), String> {
    Err("live engine service access is available on Windows only".into())
}
pub fn entity_template_uuid(_: u32) -> Option<kfc::guid::Guid> {
    None
}
pub fn resource_can_write() -> bool {
    false
}
pub fn resource_compare_exchange_scalar_for_session(
    _: &kfc::guid::ResourceId,
    _: usize,
    _: usize,
    _: &[u8],
    _: &[u8],
    _: u64,
) -> Result<(), String> {
    Err("live ResourceService access is available on Windows only".into())
}
pub fn resources_by_type(_: u32) -> Result<Vec<kfc::guid::ResourceId>, String> {
    Err("live ResourceService access is available on Windows only".into())
}
pub fn resource_exists(_: &kfc::guid::ResourceId) -> Result<bool, String> {
    Err("live ResourceService access is available on Windows only".into())
}
pub fn resource_scalar_batch(
    _: &[kfc::guid::ResourceId],
    _: u32,
    _: usize,
    _: usize,
    _: u64,
) -> Result<Vec<[u8; 4]>, String> {
    Err("live ResourceService access is available on Windows only".into())
}
pub fn resource_resolve_item(
    _: u32,
    _: u32,
    _: usize,
    _: usize,
    _: usize,
    _: u64,
) -> Result<Option<kfc::guid::ResourceId>, String> {
    Err("original ItemRegistry is available on Windows only".into())
}
pub fn resource_read(_: &kfc::guid::ResourceId) -> Result<Vec<u8>, String> {
    Err("live ResourceService access is available on Windows only".into())
}
pub fn configure(_: &[(String, u32)]) -> bool {
    false
}
pub fn ready() -> bool {
    false
}
pub fn can_write() -> bool {
    false
}
pub fn can_write_component(_: &str) -> bool {
    false
}
pub fn world_operation_available(_: &str) -> bool {
    false
}
pub fn world_session_id() -> u64 {
    0
}
pub fn world_context_active() -> bool {
    false
}
pub fn world_context_kind() -> u32 {
    0
}
pub fn world_entity_context_ready() -> bool {
    false
}
pub fn world_cursor_read() -> Option<(Vec<u8>, u64)> {
    None
}
pub fn set_local_camera_layout(_: usize, _: &[u32]) -> bool {
    false
}
pub fn world_cursor_read_input(_: usize) -> Result<(Vec<u8>, u64), String> {
    Err("native local cursor access is available on Windows only".into())
}
pub fn world_voxel_read(_: [i32; 3], _: [u32; 3], _: usize) -> Result<Vec<u16>, String> {
    Err("native voxel access is available on Windows only".into())
}
pub fn world_voxel_write(_: [i32; 3], _: [u32; 3], _: &[u16]) -> Result<(), String> {
    Err("native world runtime is available on Windows only".into())
}
pub fn world_entity_spawn(
    _: [u64; 2],
    _: [f64; 3],
    _: [f64; 4],
    _: u32,
    _: u32,
) -> Result<u32, String> {
    Err("native world runtime is available on Windows only".into())
}
pub fn world_entity_placement(
    _: bool,
    _: [f64; 3],
    _: [f64; 4],
    _: [f32; 6],
    _: u32,
    _: u32,
) -> Result<(), String> {
    Err("native world runtime is available on Windows only".into())
}
pub fn world_entity_query_props(_: [f64; 6], _: f64) -> Result<Vec<PropRecord>, &'static str> {
    Err("native world runtime is available on Windows only")
}
pub fn world_entity_register_prop_recipes(_: &[PropRecipe]) -> Result<(), String> {
    Err("native world runtime is available on Windows only".into())
}
pub fn world_entity_query_props_in_bounds(_: [f64; 6]) -> Result<Vec<PropRecord>, &'static str> {
    Err("native world runtime is available on Windows only")
}
pub fn world_entity_get_transform(_: u32) -> Option<PropRecord> {
    None
}
pub fn world_entity_read_transform(_: u32) -> Result<Option<PropRecord>, String> {
    Err("native prop inspection is available on Windows only".into())
}
pub fn world_entity_identity(_: u32) -> Option<u32> {
    None
}
pub fn world_entity_set_scale(_: u32, _: [f64; 3]) -> Result<(), String> {
    Err("native world runtime is available on Windows only".into())
}
pub fn world_grid_get_spec(_: &str) -> Result<GridSpecResult, String> {
    Err("native voxel grid access is available on Windows only".into())
}
pub fn world_entity_destroy_handle(_: u32) -> Result<(), String> {
    Err("native world runtime is available on Windows only".into())
}
pub fn world_entity_finish_building(_: bool) -> Result<(), String> {
    Err("native world runtime is available on Windows only".into())
}
pub fn runtime_patch_available(_: &str) -> bool {
    false
}
pub fn runtime_patch_set_enabled(_: &str, _: bool) -> Result<(), String> {
    Err("native runtime patches are available on Windows only".into())
}
pub fn report() -> serde_json::Value {
    serde_json::json!({"available":false,"reason":"windows-runtime-only","mainMenu":null})
}
pub fn selected_character_name() -> Option<String> {
    None
}
pub fn is_main_menu() -> Option<bool> {
    None
}
pub fn registry() -> Result<serde_json::Value, String> {
    Err("native component registry is available on Windows only".into())
}
pub fn registration(_: &str) -> Result<serde_json::Value, String> {
    registry()
}
pub fn functions() -> Result<&'static serde_json::Value, String> {
    Err("native function inventory is available on Windows only".into())
}
pub fn function_code(_: u32) -> Option<Vec<u8>> {
    None
}
pub fn resolve_type(_: &str) -> Option<String> {
    None
}
pub fn resolve(_: &str) -> Option<Component> {
    None
}
pub fn query(_: &[String]) -> Result<Vec<u32>, &'static str> {
    Err("native ECS runtime is available on Windows only")
}
pub fn query_bounds(_: &[String], _: [f64; 6], _: f64) -> Result<Vec<u32>, &'static str> {
    Err("native ECS runtime is available on Windows only")
}
pub fn read(_: u32, _: &str, _: u32) -> Option<Vec<u8>> {
    None
}
pub fn read_many(
    _: u32,
    _: &[(String, u32)],
) -> Result<std::collections::HashMap<String, Vec<u8>>, String> {
    Err("native ECS component batches are available on Windows only".into())
}
pub fn resolve_entity(_: u32) -> Option<u32> {
    None
}
pub fn compare_exchange(_: u32, _: &str, _: &[u8], _: &[u8], _: &[u8]) -> Result<(), String> {
    Err("native ECS runtime is available on Windows only".into())
}
