use kfc::reflection::{PrimitiveType, TypeMetadata, TypeRegistry};
use mod_loader::{Capability, Mod};
use shroudforge_compatibility::Availability;
use std::rc::Rc;

use crate::{
    RuntimePhase,
    alias::MappedValue,
    env::{
        AppFeatures, AppState, Type,
        game::value::{
            convert_lua_to_value, convert_value_to_lua, mapped_source_bytes,
            validate_and_clone_lua_value,
        },
        util::{add_function, add_function_with_mod},
    },
    lua::{Either, FunctionArgs, LuaError, LuaValue},
};

pub(crate) fn runtime_provider_report() -> serde_json::Value {
    runtime_provider::report()
}

/// Creates the canonical `runtime.*` namespace used in every execution phase.
pub fn create(lua: &mlua::Lua, r#mod: Mod) -> mlua::Result<mlua::Table> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();
    if app_state.phase() == RuntimePhase::Ingame && !app_state.runtime_configured.replace(true) {
        let contract: Vec<_> = app_state
            .type_registry()
            .iter()
            .filter(|metadata| {
                metadata.size > 0 && metadata.qualified_name.starts_with("keen::ecs::")
            })
            .map(|metadata| (metadata.qualified_name.clone(), metadata.size))
            .collect();
        if !runtime_provider::configure(&contract) {
            tracing::warn!("KFC Runtime rejected the component contract");
        }
    }
    let table = lua.create_table()?;
    table.raw_set("phase", app_state.phase().as_str())?;
    table.raw_set("is_client", app_state.is_client())?;
    table.raw_set("is_server", app_state.is_server())?;
    add_function(lua, &table, "has_mod", lua_has_mod)?;
    add_function_with_mod(lua, &table, "has", &r#mod, lua_has)?;
    add_function_with_mod(lua, &table, "require", &r#mod, lua_require)?;
    add_function_with_mod(lua, &table, "status", &r#mod, lua_status)?;

    table.raw_set("ecs", create_ecs(lua, &r#mod)?)?;

    Ok(table)
}

fn has_capability(r#mod: &Mod, capability: Capability) -> bool {
    r#mod.info().capabilities.contains(&capability)
}

/// EML's original Lua surface is an adapter, not a renamed runtime API.
pub fn create_eml(lua: &mlua::Lua, r#mod: &Mod) -> mlua::Result<mlua::Table> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let table = lua.create_table()?;
    table.raw_set("is_client", state.is_client())?;
    table.raw_set("is_server", state.is_server())?;
    add_function(lua, &table, "has_mod", lua_has_mod)?;
    let features = lua.create_table()?;
    features.raw_set("patch", available(&state, r#mod, "game.assets.write"))?;
    features.raw_set("export", available(&state, r#mod, "export"))?;
    table.raw_set("features", features)?;
    Ok(table)
}

fn available(state: &AppState, r#mod: &Mod, feature: &str) -> bool {
    match feature {
        "game.assets.write" => {
            state.has_feature(AppFeatures::ASSETS_WRITE)
                && has_capability(r#mod, Capability::AssetsWrite)
        }
        "export" => {
            state.has_feature(AppFeatures::EXPORT) && has_capability(r#mod, Capability::Export)
        }
        "runtime.lifecycle" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime("runtime.lifecycle")
                && has_capability(r#mod, Capability::Runtime)
        }
        "runtime.ecs.query" | "runtime.ecs.resolve" | "runtime.ecs.read" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::ready()
        }
        "runtime.ecs.write" => {
            state.phase() == RuntimePhase::Ingame
                && state.api().has_runtime(feature)
                && has_capability(r#mod, Capability::Runtime)
                && runtime_provider::can_write()
        }
        _ => false,
    }
}

fn lua_has_mod(lua: &mlua::Lua, args: FunctionArgs) -> mlua::Result<bool> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    Ok(state
        .env()
        .mod_registry()
        .contains_key(&args.get::<String>(0)?))
}

fn lua_has(lua: &mlua::Lua, args: FunctionArgs, r#mod: &Mod) -> mlua::Result<bool> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    Ok(available(&state, r#mod, &args.get::<String>(0)?))
}

fn lua_status(lua: &mlua::Lua, args: FunctionArgs, r#mod: &Mod) -> mlua::Result<mlua::Table> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let feature = args.get::<String>(0)?;
    if !has_capability_for_feature(r#mod, &feature) {
        return availability_to_lua(
            lua,
            Availability::Unavailable {
                reason: format!("mod '{}' lacks the required capability", r#mod.info().id),
            },
        );
    }
    let availability = state.api().runtime(&feature);
    if feature.starts_with("runtime.ecs.")
        && matches!(availability, Availability::Available)
        && !runtime_provider::ready()
    {
        return availability_to_lua(
            lua,
            Availability::Unavailable {
                reason: "KFC Runtime is waiting for the live Keen ECS world".into(),
            },
        );
    }
    if feature == "runtime.ecs.write"
        && matches!(availability, Availability::Available)
        && !runtime_provider::can_write()
    {
        return availability_to_lua(
            lua,
            Availability::Unavailable {
                reason: "KFC Runtime has no writable live component context".into(),
            },
        );
    }
    availability_to_lua(lua, availability)
}

fn lua_require(lua: &mlua::Lua, args: FunctionArgs, r#mod: &Mod) -> mlua::Result<()> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let feature = args.get::<String>(0)?;
    if available(&state, r#mod, &feature) {
        Ok(())
    } else {
        Err(LuaError::generic(format!(
            "ShroudForge feature is unavailable: {feature}"
        )))
    }
}

fn create_ecs(lua: &mlua::Lua, r#mod: &Mod) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;

    add_function_with_mod(lua, &table, "get_components", r#mod, lua_ecs_get_components)?;
    add_function_with_mod(lua, &table, "query", r#mod, lua_ecs_query)?;
    add_function_with_mod(lua, &table, "resolve", r#mod, lua_ecs_resolve)?;
    add_function_with_mod(lua, &table, "read", r#mod, lua_ecs_read)?;
    add_function_with_mod(lua, &table, "write", r#mod, lua_ecs_write)?;

    Ok(table)
}

fn lua_ecs_resolve(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.ecs.resolve") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    let entity_id = args.get::<u32>(0)?;
    match runtime_provider::resolve_entity(entity_id) {
        Some(handle) => Ok((LuaValue::Integer(i64::from(handle)), None)),
        None => Ok((
            LuaValue::Nil,
            Some(format!("live ECS entity unavailable: {entity_id}")),
        )),
    }
}

fn lua_ecs_get_components(
    lua: &mlua::Lua,
    _args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.ecs.query") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if !runtime_provider::ready() {
        return Ok((LuaValue::Nil, Some("KFC Runtime is not ready".into())));
    }
    let result = lua.create_table()?;
    for metadata in state.type_registry().iter() {
        if !metadata.qualified_name.starts_with("keen::ecs::") {
            continue;
        }
        let Some(component) = runtime_provider::resolve(&metadata.qualified_name) else {
            continue;
        };
        if component.size != metadata.size {
            continue;
        }
        if let Some(value) = state.get_type(lua, metadata.index)? {
            result.push(value)?;
        }
    }
    Ok((LuaValue::Table(result), None))
}

fn lua_ecs_query(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.ecs.query") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if args.len() == 0 {
        return Err(LuaError::generic(
            "runtime.ecs.query requires at least one keen::ecs type",
        ));
    }

    if !runtime_provider::ready() {
        return Ok((LuaValue::Nil, Some("KFC Runtime is not ready".into())));
    }

    let mut components = Vec::with_capacity(args.len());
    for index in 0..args.len() {
        let name = runtime_type_name(lua, &args, index)?;
        if runtime_provider::resolve(&name).is_none() {
            return Ok((
                LuaValue::Nil,
                Some(format!("live ECS component unavailable: {name}")),
            ));
        }
        components.push(name);
    }
    let Some(entities) = runtime_provider::query(&components) else {
        return Ok((
            LuaValue::Nil,
            Some("live ECS query failed or timed out".into()),
        ));
    };
    let result = lua.create_table_with_capacity(entities.len(), 0)?;
    for (index, entity) in entities.into_iter().enumerate() {
        result.raw_set(index + 1, entity)?;
    }
    Ok((LuaValue::Table(result), None))
}

fn lua_ecs_read(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.ecs.read") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if !matches!(
        state.api().runtime("runtime.ecs.read"),
        Availability::Available
    ) {
        return unavailable_runtime_result(lua, &state, "runtime.ecs.read");
    }
    let entity_id = args.get::<u32>(0)?;
    let component_name = runtime_type_name(lua, &args, 1)?;
    let Some(component) = runtime_provider::resolve(&component_name) else {
        return Ok((
            LuaValue::Nil,
            Some(format!("live ECS component unavailable: {component_name}")),
        ));
    };
    let Some(metadata) = state
        .type_registry()
        .get_by_name(kfc::reflection::LookupKey::Qualified(&component_name))
    else {
        return Ok((
            LuaValue::Nil,
            Some(format!("type metadata unavailable: {component_name}")),
        ));
    };
    if metadata.size != component.size {
        return Ok((
            LuaValue::Nil,
            Some(format!(
                "component layout mismatch for {component_name}: parser={}, runtime={}",
                metadata.size, component.size
            )),
        ));
    }
    let Some(bytes) = runtime_provider::read(entity_id, &component_name, component.size) else {
        return Ok((
            LuaValue::Nil,
            Some(format!("component read failed: {component_name}")),
        ));
    };
    let bytes: Rc<[u8]> = bytes.into();
    let mapped = MappedValue::from_bytes(state.type_registry(), metadata, &bytes)
        .map_err(LuaError::external)?;
    Ok((convert_value_to_lua(&mapped, lua)?, None))
}

fn lua_ecs_write(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(bool, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.ecs.write") {
        return Ok((false, Some(reason)));
    }
    match state.api().runtime("runtime.ecs.write") {
        Availability::Unavailable { reason } => return Ok((false, Some(reason))),
        Availability::Available => {}
    }
    if !runtime_provider::ready() {
        return Ok((false, Some("KFC Runtime is not ready".into())));
    }
    let entity_id = args.get::<u32>(0)?;
    let component_name = runtime_type_name(lua, &args, 1)?;
    let lua_value = args.get::<LuaValue>(2)?;
    let Some(component) = runtime_provider::resolve(&component_name) else {
        return Ok((
            false,
            Some(format!("live ECS component unavailable: {component_name}")),
        ));
    };
    let registry = state.type_registry();
    let Some(metadata) =
        registry.get_by_name(kfc::reflection::LookupKey::Qualified(&component_name))
    else {
        return Ok((
            false,
            Some(format!("type metadata unavailable: {component_name}")),
        ));
    };
    if metadata.size != component.size {
        return Ok((
            false,
            Some(format!(
                "component layout mismatch for {component_name}: parser={}, runtime={}",
                metadata.size, component.size
            )),
        ));
    }
    let Some(source) = mapped_source_bytes(&lua_value)? else {
        return Ok((
            false,
            Some(format!(
                "runtime.ecs.write requires the value returned by runtime.ecs.read for {component_name}"
            )),
        ));
    };
    if source.len() != component.size as usize {
        return Ok((
            false,
            Some(format!("runtime source size mismatch: {component_name}")),
        ));
    }
    let handle = crate::alias::TypeHandle::new(registry.clone(), metadata.index);
    let checked =
        validate_and_clone_lua_value(&lua_value, &handle, lua).map_err(LuaError::external)?;
    let value = convert_lua_to_value(&checked, &handle).map_err(LuaError::external)?;
    let bytes = value
        .to_bytes(registry, metadata)
        .map_err(LuaError::external)?;
    if bytes.len() != component.size as usize {
        return Ok((
            false,
            Some(format!(
                "serialized component size mismatch: {component_name}"
            )),
        ));
    }
    let Some(fresh) = runtime_provider::read(entity_id, &component_name, component.size) else {
        return Ok((
            false,
            Some(format!(
                "component changed or disappeared: {component_name}"
            )),
        ));
    };
    let mut ranges = Vec::new();
    collect_changed_ranges(registry, metadata, 0, &source, &bytes, &mut ranges)
        .map_err(LuaError::generic)?;
    if ranges.is_empty() {
        return Ok((true, None));
    }
    let mut merged = fresh.clone();
    for (start, end) in ranges {
        merged[start..end].copy_from_slice(&bytes[start..end]);
    }
    if runtime_provider::write(entity_id, &component_name, &fresh, &merged) {
        Ok((true, None))
    } else {
        Ok((
            false,
            Some(format!("component write failed: {component_name}")),
        ))
    }
}

fn collect_changed_ranges(
    registry: &TypeRegistry,
    metadata: &TypeMetadata,
    base: usize,
    source: &[u8],
    candidate: &[u8],
    ranges: &mut Vec<(usize, usize)>,
) -> Result<(), String> {
    let end = base
        .checked_add(metadata.size as usize)
        .ok_or_else(|| format!("runtime layout overflow: {}", metadata.qualified_name))?;
    if end > source.len() || end > candidate.len() {
        return Err(format!(
            "runtime layout outside component: {}",
            metadata.qualified_name
        ));
    }
    match metadata.primitive_type {
        PrimitiveType::Typedef => {
            let inner = registry
                .get_inner_type(metadata)
                .ok_or_else(|| format!("missing inner type: {}", metadata.qualified_name))?;
            collect_changed_ranges(registry, inner, base, source, candidate, ranges)
        }
        PrimitiveType::Struct => {
            if let Some(parent) = registry.get_inner_type(metadata) {
                collect_changed_ranges(registry, parent, base, source, candidate, ranges)?;
            }
            for field in metadata.struct_fields.values() {
                let field_type = registry.get(field.r#type).ok_or_else(|| {
                    format!(
                        "missing field type: {}.{}",
                        metadata.qualified_name, field.name
                    )
                })?;
                let offset = usize::try_from(field.data_offset).map_err(|_| {
                    format!(
                        "invalid field offset: {}.{}",
                        metadata.qualified_name, field.name
                    )
                })?;
                collect_changed_ranges(
                    registry,
                    field_type,
                    base + offset,
                    source,
                    candidate,
                    ranges,
                )?;
            }
            Ok(())
        }
        PrimitiveType::StaticArray => {
            let inner = registry.get_inner_type(metadata).ok_or_else(|| {
                format!("missing array element type: {}", metadata.qualified_name)
            })?;
            for index in 0..metadata.field_count as usize {
                collect_changed_ranges(
                    registry,
                    inner,
                    base + index * inner.size as usize,
                    source,
                    candidate,
                    ranges,
                )?;
            }
            Ok(())
        }
        PrimitiveType::DsArray
        | PrimitiveType::DsString
        | PrimitiveType::DsOptional
        | PrimitiveType::DsVariant
        | PrimitiveType::BlobArray
        | PrimitiveType::BlobString
        | PrimitiveType::BlobOptional
        | PrimitiveType::BlobVariant => {
            if source[base..end] != candidate[base..end] {
                return Err(format!(
                    "runtime write for dynamic field type is not supported: {}",
                    metadata.qualified_name
                ));
            }
            Ok(())
        }
        _ => {
            if source[base..end] != candidate[base..end] {
                if let Some((_, previous_end)) =
                    ranges.last_mut().filter(|(_, value)| *value == base)
                {
                    *previous_end = end;
                } else {
                    ranges.push((base, end));
                }
            }
            Ok(())
        }
    }
}

fn runtime_type_name(lua: &mlua::Lua, args: &FunctionArgs, index: usize) -> mlua::Result<String> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    let value = args.get::<Either<String, &Type>>(index)?;
    let name = match value {
        Either::A(name) => name,
        Either::B(r#type) => r#type.qualified_name.clone(),
    };

    let Some(r#type) = state
        .type_registry()
        .get_by_name(kfc::reflection::LookupKey::Qualified(&name))
    else {
        return Err(LuaError::generic(format!("runtime type not found: {name}")));
    };

    if !r#type.qualified_name.starts_with("keen::ecs::") {
        return Err(LuaError::generic(format!(
            "runtime ECS operation requires a keen::ecs::* type, got {}",
            r#type.qualified_name
        )));
    }

    Ok(r#type.qualified_name.clone())
}

fn unavailable_runtime_result(
    lua: &mlua::Lua,
    state: &AppState,
    operation: &str,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let availability = state.api().runtime(operation);
    match availability {
        Availability::Available => Ok((LuaValue::Table(lua.create_table()?), None)),
        Availability::Unavailable { reason } => Ok((LuaValue::Nil, Some(reason))),
    }
}

fn availability_to_lua(lua: &mlua::Lua, availability: Availability) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;
    match availability {
        Availability::Available => {
            table.raw_set("available", true)?;
        }
        Availability::Unavailable { reason } => {
            table.raw_set("available", false)?;
            table.raw_set("reason", reason)?;
        }
    }

    Ok(table)
}

fn has_capability_for_feature(r#mod: &Mod, feature: &str) -> bool {
    match feature {
        "game.assets.write" => has_capability(r#mod, Capability::AssetsWrite),
        "export" => has_capability(r#mod, Capability::Export),
        value if value.starts_with("runtime.") => has_capability(r#mod, Capability::Runtime),
        _ => true,
    }
}

fn runtime_denial_reason(state: &AppState, r#mod: &Mod, feature: &str) -> Option<String> {
    if state.phase() != RuntimePhase::Ingame {
        return Some("runtime ECS is available only during the ingame phase".into());
    }
    if !has_capability(r#mod, Capability::Runtime) {
        return Some(format!(
            "mod '{}' requires capabilities: [\"runtime\"]",
            r#mod.info().id
        ));
    }
    match state.api().runtime(feature) {
        Availability::Available => None,
        Availability::Unavailable { reason } => Some(reason),
    }
}

#[cfg(windows)]
mod runtime_provider {
    use std::{
        ffi::{CString, c_char, c_void},
        sync::OnceLock,
    };

    #[derive(Clone, Copy)]
    pub struct Component {
        pub size: u32,
    }

    type Ready = unsafe extern "C" fn() -> bool;
    type Configure = unsafe extern "C" fn(*const *const c_char, *const u32, usize) -> bool;
    type Describe = unsafe extern "C" fn(*const c_char, *mut u32) -> bool;
    type Query = unsafe extern "C" fn(*const *const c_char, usize, *mut u32, usize) -> usize;
    type ResolveEntity = unsafe extern "C" fn(u32) -> u32;
    type Read = unsafe extern "C" fn(u32, *const c_char, *mut c_void, usize) -> bool;
    type Write =
        unsafe extern "C" fn(u32, *const c_char, *const c_void, *const c_void, usize) -> bool;
    struct Provider {
        configure: Configure,
        ready: Ready,
        can_write: Ready,
        describe: Describe,
        query: Query,
        resolve_entity: ResolveEntity,
        read: Read,
        write: Write,
        abi: u32,
        status: unsafe extern "C" fn(*mut c_char, usize),
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *const c_void;
    }

    static PROVIDER: OnceLock<Option<Provider>> = OnceLock::new();

    fn provider() -> Option<&'static Provider> {
        PROVIDER
            .get_or_init(|| unsafe {
                let module_name: Vec<u16> = "kfc-runtime.dll\0".encode_utf16().collect();
                let module = GetModuleHandleW(module_name.as_ptr());
                if module.is_null() {
                    return None;
                }
                macro_rules! symbol {
                    ($name:literal, $kind:ty) => {{
                        let pointer = GetProcAddress(module, concat!($name, "\0").as_ptr().cast());
                        if pointer.is_null() {
                            return None;
                        }
                        std::mem::transmute::<*const c_void, $kind>(pointer)
                    }};
                }
                let abi = symbol!("KfcRuntimeAbi", unsafe extern "C" fn() -> u32);
                if abi() != 1 {
                    return None;
                }
                Some(Provider {
                    configure: symbol!("ShroudforgeEcsConfigure", Configure),
                    ready: symbol!("ShroudforgeEcsReady", Ready),
                    can_write: symbol!("ShroudforgeEcsCanWrite", Ready),
                    describe: symbol!("ShroudforgeEcsDescribe", Describe),
                    query: symbol!("ShroudforgeEcsQuery", Query),
                    resolve_entity: symbol!("ShroudforgeEcsResolve", ResolveEntity),
                    read: symbol!("ShroudforgeEcsRead", Read),
                    write: symbol!("ShroudforgeEcsWrite", Write),
                    abi: abi(),
                    status: symbol!("KfcRuntimeStatus", unsafe extern "C" fn(*mut c_char, usize)),
                })
            })
            .as_ref()
    }

    fn observed_abi() -> Option<u32> {
        unsafe {
            let module_name: Vec<u16> = "kfc-runtime.dll\0".encode_utf16().collect();
            let module=GetModuleHandleW(module_name.as_ptr());
            if module.is_null(){return None;}
            let pointer=GetProcAddress(module,c"KfcRuntimeAbi".as_ptr());
            if pointer.is_null(){return None;}
            let abi:unsafe extern "C" fn()->u32=std::mem::transmute(pointer);
            Some(abi())
        }
    }

    pub fn configure(types: &[(String, u32)]) -> bool {
        let Some(provider) = provider() else {
            return false;
        };
        let Ok(names) = types
            .iter()
            .map(|(name, _)| CString::new(name.as_str()))
            .collect::<Result<Vec<_>, _>>()
        else {
            return false;
        };
        let pointers: Vec<_> = names.iter().map(|name| name.as_ptr()).collect();
        let sizes: Vec<_> = types.iter().map(|(_, size)| *size).collect();
        unsafe { (provider.configure)(pointers.as_ptr(), sizes.as_ptr(), pointers.len()) }
    }
    pub fn ready() -> bool {
        provider().is_some_and(|value| unsafe { (value.ready)() })
    }
    pub fn can_write() -> bool {
        provider().is_some_and(|value| unsafe { (value.can_write)() })
    }
    pub fn report() -> serde_json::Value {
        let Some(provider) = provider() else { return serde_json::json!({"available":false,"abi":observed_abi(),"reason":"provider-or-ABI-unavailable"}); };
        let mut buffer = [0i8; 4096];
        unsafe { (provider.status)(buffer.as_mut_ptr(), buffer.len()); }
        let bytes: Vec<u8> = buffer.iter().take_while(|byte| **byte != 0).map(|byte| *byte as u8).collect();
        serde_json::json!({"available":true,"abi":provider.abi,"initialized":true,"ready":unsafe{(provider.ready)()},"writable":unsafe{(provider.can_write)()},"detail":String::from_utf8_lossy(&bytes)})
    }
    pub fn resolve(name: &str) -> Option<Component> {
        let provider = provider()?;
        let name = CString::new(name).ok()?;
        let mut size = 0;
        unsafe { (provider.describe)(name.as_ptr(), &mut size) }.then_some(Component { size })
    }
    pub fn query(components: &[String]) -> Option<Vec<u32>> {
        let Some(provider) = provider() else {
            return None;
        };
        let Ok(names) = components
            .iter()
            .map(|name| CString::new(name.as_str()))
            .collect::<Result<Vec<_>, _>>()
        else {
            return None;
        };
        let pointers: Vec<_> = names.iter().map(|name| name.as_ptr()).collect();
        // Most player queries fit in one call. Do not count then rescan the
        // whole world on another engine tick for every query.
        let mut entities = vec![0; 256];
        for _ in 0..3 {
            let actual = unsafe {
                (provider.query)(
                    pointers.as_ptr(),
                    pointers.len(),
                    entities.as_mut_ptr(),
                    entities.len(),
                )
            };
            if actual <= entities.len() {
                entities.truncate(actual);
                return Some(entities);
            }
            if actual > 1 << 20 {
                return None;
            }
            entities.resize(actual, 0);
        }
        None
    }
    pub fn read(entity: u32, name: &str, size: u32) -> Option<Vec<u8>> {
        let provider = provider()?;
        let name = CString::new(name).ok()?;
        let mut bytes = vec![0; size as usize];
        unsafe {
            (provider.read)(
                entity,
                name.as_ptr(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
            )
        }
        .then_some(bytes)
    }
    pub fn resolve_entity(entity_id: u32) -> Option<u32> {
        let provider = provider()?;
        let handle = unsafe { (provider.resolve_entity)(entity_id) };
        (handle != 0).then_some(handle)
    }
    pub fn write(entity: u32, name: &str, expected: &[u8], bytes: &[u8]) -> bool {
        let Some(provider) = provider() else {
            return false;
        };
        let Ok(name) = CString::new(name) else {
            return false;
        };
        if expected.len() != bytes.len() {
            return false;
        }
        unsafe {
            (provider.write)(
                entity,
                name.as_ptr(),
                expected.as_ptr().cast(),
                bytes.as_ptr().cast(),
                bytes.len(),
            )
        }
    }
}

#[cfg(not(windows))]
mod runtime_provider {
    #[derive(Clone, Copy)]
    pub struct Component {
        pub size: u32,
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
    pub fn report() -> serde_json::Value { serde_json::json!({"available":false,"reason":"windows-runtime-only"}) }
    pub fn resolve(_: &str) -> Option<Component> {
        None
    }
    pub fn query(_: &[String]) -> Option<Vec<u32>> {
        None
    }
    pub fn read(_: u32, _: &str, _: u32) -> Option<Vec<u8>> {
        None
    }
    pub fn resolve_entity(_: u32) -> Option<u32> {
        None
    }
    pub fn write(_: u32, _: &str, _: &[u8], _: &[u8]) -> bool {
        false
    }
}
