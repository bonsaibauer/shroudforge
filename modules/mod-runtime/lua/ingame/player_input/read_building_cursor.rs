use super::*;

pub(crate) fn lua_world_cursor_get(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    let state = lua.app_data_ref::<AppState>().unwrap();
    if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.cursor.get") {
        return Ok((LuaValue::Nil, Some(reason)));
    }
    if args.len() != 0 {
        return Err(LuaError::generic(
            "runtime.world.cursor.get does not accept arguments",
        ));
    }
    let Some((bytes, sequence)) = runtime_provider::world_cursor_read() else {
        return match reflected_local_cursor(lua, &state) {
            Ok(snapshot) => Ok((LuaValue::Table(snapshot), None)),
            Err(reason) => Ok((LuaValue::Nil, Some(reason))),
        };
    };
    if bytes.len() != 0xa0 || bytes[0x98] > 1 {
        return Ok((
            LuaValue::Nil,
            Some("native cursor snapshot failed layout validation".into()),
        ));
    }
    let transform = |base: usize| -> mlua::Result<mlua::Table> {
        let output = lua.create_table()?;
        let position = lua.create_table()?;
        for (axis, name) in ["x", "y", "z"].iter().enumerate() {
            let start = base + axis * 8;
            position.raw_set(
                *name,
                i64::from_le_bytes(bytes[start..start + 8].try_into().unwrap()),
            )?;
        }
        let rotation = lua.create_table()?;
        for index in 0..4 {
            let start = base + 0x18 + index * 4;
            rotation.raw_set(
                index + 1,
                f32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()),
            )?;
        }
        let scale = lua.create_table()?;
        for index in 0..3 {
            let start = base + 0x28 + index * 4;
            scale.raw_set(
                index + 1,
                f32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()),
            )?;
        }
        output.raw_set("position", position)?;
        output.raw_set("rotation", rotation)?;
        output.raw_set("scale", scale)?;
        Ok(output)
    };
    let value = lua.create_table()?;
    value.raw_set("primaryTransform", transform(0)?)?;
    value.raw_set("secondaryTransform", transform(0x38)?)?;
    value.raw_set("primaryFlags", bytes[0x70])?;
    value.raw_set("secondaryFlags", bytes[0x71])?;
    value.raw_set("material", bytes[0x72])?;
    value.raw_set(
        "selectionVersion",
        format!(
            "{}",
            u64::from_le_bytes(bytes[0x78..0x80].try_into().unwrap())
        ),
    )?;
    let selected = lua.create_table()?;
    selected.raw_set(
        "value",
        format!(
            "{:016x}",
            u64::from_le_bytes(bytes[0x80..0x88].try_into().unwrap())
        ),
    )?;
    selected.raw_set(
        "type",
        u32::from_le_bytes(bytes[0x88..0x8c].try_into().unwrap()),
    )?;
    value.raw_set("selectedObject", selected)?;
    value.raw_set("attachedToProp", bytes[0x98] != 0)?;
    let result = lua.create_table()?;
    result.raw_set("sequence", sequence)?;
    result.raw_set("value", value)?;
    Ok((LuaValue::Table(result), None))
}

// The 1.0 ClientCursorInput has different flag widths and field offsets. Read
// its original reflected fields from one owned local ClientPlayerInput snapshot
// rather than reinterpreting the legacy hook's 160-byte layout.
fn reflected_local_cursor(lua: &mlua::Lua, state: &AppState) -> Result<mlua::Table, String> {
    let metadata = state
        .type_registry()
        .get_by_name(kfc::reflection::LookupKey::Qualified(
            "keen::ecs::ClientPlayerInput",
        ))
        .ok_or("local ClientPlayerInput reflection is unavailable")?;
    let (bytes, sequence) = runtime_provider::world_cursor_read_input(metadata.size as usize)?;
    let field = |path: &str| -> Result<LuaValue, String> {
        state
            .inline_field(metadata.index, &format!("data.cursorInput.{path}"))?
            .read(lua, &bytes)
    };
    let integer = |path: &str| -> Result<i64, String> {
        match field(path)? {
            LuaValue::Integer(value) => Ok(value),
            _ => Err(format!("local cursor field is not an integer: {path}")),
        }
    };
    let unsigned = |path: &str| -> Result<u64, String> {
        reflected_cursor_unsigned(state, metadata.index, &bytes, path)
    };
    let number = |path: &str| -> Result<f64, String> {
        match field(path)? {
            LuaValue::Number(value) if value.is_finite() => Ok(value),
            _ => Err(format!("local cursor field is not a finite float: {path}")),
        }
    };
    let transform = |name: &str| -> Result<mlua::Table, String> {
        let output = lua.create_table().map_err(|error| error.to_string())?;
        let position = lua.create_table().map_err(|error| error.to_string())?;
        let rotation = lua.create_table().map_err(|error| error.to_string())?;
        let scale = lua.create_table().map_err(|error| error.to_string())?;
        for (index, axis) in ["x", "y", "z"].iter().enumerate() {
            position
                .raw_set(*axis, integer(&format!("{name}.position.{axis}"))?)
                .map_err(|error| error.to_string())?;
            scale
                .raw_set(index + 1, number(&format!("{name}.scale.{axis}"))?)
                .map_err(|error| error.to_string())?;
        }
        for (index, axis) in ["x", "y", "z", "w"].iter().enumerate() {
            rotation
                .raw_set(index + 1, number(&format!("{name}.orientation.{axis}"))?)
                .map_err(|error| error.to_string())?;
        }
        output
            .raw_set("position", position)
            .map_err(|error| error.to_string())?;
        output
            .raw_set("rotation", rotation)
            .map_err(|error| error.to_string())?;
        output
            .raw_set("scale", scale)
            .map_err(|error| error.to_string())?;
        Ok(output)
    };
    let value = lua.create_table().map_err(|error| error.to_string())?;
    value
        .raw_set("primaryTransform", transform("primaryTransform")?)
        .map_err(|error| error.to_string())?;
    value
        .raw_set("secondaryTransform", transform("secondaryTransform")?)
        .map_err(|error| error.to_string())?;
    for (public, reflected) in [
        ("primaryFlags", "primaryFlags"),
        ("secondaryFlags", "secondaryFlags"),
        ("material", "hoveredVoxelMaterialId"),
    ] {
        value
            .raw_set(public, unsigned(reflected)?)
            .map_err(|error| error.to_string())?;
    }
    value
        .raw_set(
            "selectionVersion",
            unsigned("selectObjectAction.versionData.version")?.to_string(),
        )
        .map_err(|error| error.to_string())?;
    let selected = lua.create_table().map_err(|error| error.to_string())?;
    selected
        .raw_set(
            "value",
            format!(
                "{:016x}",
                unsigned("selectObjectAction.selectedObjectId.value")?
            ),
        )
        .map_err(|error| error.to_string())?;
    selected
        .raw_set(
            "type",
            unsigned("selectObjectAction.selectedObjectId.type")?,
        )
        .map_err(|error| error.to_string())?;
    value
        .raw_set("selectedObject", selected)
        .map_err(|error| error.to_string())?;
    let LuaValue::Boolean(attached) = field("isCursorAttachedToProp")? else {
        return Err("local cursor attachment field is not boolean".into());
    };
    value
        .raw_set("attachedToProp", attached)
        .map_err(|error| error.to_string())?;
    // The local player's camera, from the same input snapshot. Left out when
    // this game's layout has no such fields: the cursor stays readable.
    if let Ok(camera) = reflected_local_camera(lua, state, metadata.index, metadata.size as usize, &bytes) {
        value
            .raw_set("camera", camera)
            .map_err(|error| error.to_string())?;
    }
    let result = lua.create_table().map_err(|error| error.to_string())?;
    result
        .raw_set("sequence", sequence)
        .map_err(|error| error.to_string())?;
    result
        .raw_set("value", value)
        .map_err(|error| error.to_string())?;
    Ok(result)
}

// The camera the local player's client sends with its input
// (data.camera, a keen::ecs::ClientCamera): position in Q32.32 integers like
// the cursor, orientation as {x, y, z, w}, vertical field of view, aspect
// ratio, near and far planes and orbit distance as reflected.
fn reflected_local_camera(
    lua: &mlua::Lua,
    state: &AppState,
    root: kfc::reflection::TypeIndex,
    component_size: usize,
    bytes: &[u8],
) -> Result<mlua::Table, String> {
    share_local_camera_layout(state, root, component_size);
    let field = |path: &str| -> Result<LuaValue, String> {
        state
            .inline_field(root, &format!("data.camera.{path}"))?
            .read(lua, bytes)
    };
    let number = |path: &str| -> Result<f64, String> {
        match field(path)? {
            LuaValue::Number(value) if value.is_finite() => Ok(value),
            LuaValue::Integer(value) => Ok(value as f64),
            _ => Err(format!("local camera field is not a finite number: {path}")),
        }
    };
    let camera = lua.create_table().map_err(|error| error.to_string())?;
    let position = lua.create_table().map_err(|error| error.to_string())?;
    for axis in ["x", "y", "z"] {
        let LuaValue::Integer(value) = field(&format!("position.{axis}"))? else {
            return Err(format!("local camera position is not an integer: {axis}"));
        };
        position
            .raw_set(axis, value)
            .map_err(|error| error.to_string())?;
    }
    let orientation = lua.create_table().map_err(|error| error.to_string())?;
    for (index, axis) in ["x", "y", "z", "w"].iter().enumerate() {
        orientation
            .raw_set(index + 1, number(&format!("orientation.{axis}"))?)
            .map_err(|error| error.to_string())?;
    }
    camera
        .raw_set("position", position)
        .map_err(|error| error.to_string())?;
    camera
        .raw_set("orientation", orientation)
        .map_err(|error| error.to_string())?;
    for name in ["fovY", "aspectRatio", "nearPlane", "farPlane", "distance"] {
        camera
            .raw_set(name, number(name)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(camera)
}

// The native runtime reads the camera for the game overlay at every frame,
// at the byte offsets the reflection gives: sent once per component size.
fn share_local_camera_layout(state: &AppState, root: kfc::reflection::TypeIndex, component_size: usize) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SENT: AtomicUsize = AtomicUsize::new(0);
    if SENT.load(Ordering::Relaxed) == component_size {
        return;
    }
    // The camera, then the building cursor's transform and flags: the overlay
    // also draws the aimed face at every frame.
    let paths = [
        "camera.position.x", "camera.position.y", "camera.position.z", "camera.orientation.x",
        "camera.orientation.y", "camera.orientation.z", "camera.orientation.w", "camera.fovY",
        "camera.aspectRatio", "cursorInput.primaryTransform.position.x", "cursorInput.primaryTransform.position.y",
        "cursorInput.primaryTransform.position.z", "cursorInput.primaryTransform.orientation.x",
        "cursorInput.primaryTransform.orientation.y", "cursorInput.primaryTransform.orientation.z",
        "cursorInput.primaryTransform.orientation.w", "cursorInput.primaryFlags",
    ];
    let mut offsets = Vec::with_capacity(paths.len() + 1);
    for path in paths {
        match state.inline_field(root, &format!("data.{path}")) {
            Ok(field) => offsets.push(field.byte_range().start as u32),
            Err(_) => return,
        }
    }
    // The flags' width (8, 16 or 32 bits as reflected).
    match state.inline_field(root, "data.cursorInput.primaryFlags") {
        Ok(field) => offsets.push(field.byte_range().len() as u32),
        Err(_) => return,
    }
    if runtime_provider::set_local_camera_layout(component_size, &offsets) {
        SENT.store(component_size, Ordering::Relaxed);
    }
}

// Cursor snapshots expose numeric flags/enums for compatibility. The general
// value codec instead exposes named bitmask values. Follow the reflected
// storage type and width here, preserving all 32 flag bits in the 1.0 layout.
fn reflected_cursor_unsigned(
    state: &AppState,
    root: kfc::reflection::TypeIndex,
    bytes: &[u8],
    path: &str,
) -> Result<u64, String> {
    let field = state.inline_field(root, &format!("data.cursorInput.{path}"))?;
    let mut ty = state
        .type_registry()
        .get(field.type_index())
        .ok_or("local cursor scalar type is unresolved")?;
    for _ in 0..64 {
        if !matches!(
            ty.primitive_type,
            PrimitiveType::Typedef | PrimitiveType::Enum
        ) {
            break;
        }
        ty = ty
            .inner_type
            .and_then(|index| state.type_registry().get(index))
            .ok_or("local cursor scalar storage is unresolved")?;
    }
    let width = match ty.primitive_type {
        PrimitiveType::UInt8 | PrimitiveType::Bitmask8 => 1,
        PrimitiveType::UInt16 | PrimitiveType::Bitmask16 => 2,
        PrimitiveType::UInt32 | PrimitiveType::Bitmask32 => 4,
        PrimitiveType::UInt64 | PrimitiveType::Bitmask64 => 8,
        _ => {
            return Err(format!(
                "local cursor field is not an unsigned scalar: {path}"
            ));
        }
    };
    let source = bytes
        .get(field.byte_range())
        .filter(|source| source.len() == width)
        .ok_or("local cursor scalar exceeds its reflected snapshot")?;
    let mut output = [0u8; 8];
    output[..width].copy_from_slice(source);
    Ok(u64::from_le_bytes(output))
}
