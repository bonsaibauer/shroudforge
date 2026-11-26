use super::*;
pub(super) fn lua_export(lua: &mlua::Lua, args: FunctionArgs) -> mlua::Result<()> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();

    if !app_state.has_feature(AppFeatures::EXPORT) {
        return Err(LuaError::generic("exporting is disabled"));
    }

    let path = args.get::<String>(0)?;
    let buffer = args.get::<Either<&[u8], &Buffer>>(1)?;
    note(lua, false, &path);

    let export_dir = &app_state.export_dir();

    assert!(export_dir.is_absolute(), "Export path must be absolute");

    let path = export_dir.join(
        sanitize_path(Path::new(&path))
            .map_err(|e| LuaError::generic(format!("invalid path: {e}")))?,
    );

    if path.exists() && !path.is_file() {
        return Err(LuaError::generic("path exists but is not a file"));
    }

    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return Err(LuaError::generic(format!(
                "failed to create parent directories: {e}"
            )));
        }
    }

    match buffer {
        Either::A(data) => {
            std::fs::write(&path, data)
                .map_err(|e| LuaError::generic(format!("failed to write file: {e}")))?;
        }
        Either::B(buf) => {
            std::fs::write(&path, buf.data()?)
                .map_err(|e| LuaError::generic(format!("failed to write file: {e}")))?;
        }
    }

    Ok(())
}

pub(super) fn lua_export_exists(lua: &mlua::Lua, args: FunctionArgs) -> mlua::Result<bool> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();
    if !app_state.has_feature(AppFeatures::EXPORT) {
        return Err(LuaError::generic("export access is disabled"));
    }

    let argument = args.get::<String>(0)?;
    let relative = sanitize_path(Path::new(&argument))
        .map_err(|error| LuaError::generic(format!("invalid path: {error}")))?;
    note(lua, false, &argument);
    let path = app_state.export_dir().join(relative);
    if !path.exists() {
        return Ok(false);
    }

    let root = std::fs::canonicalize(app_state.export_dir())
        .map_err(|error| LuaError::generic(format!("export directory is unavailable: {error}")))?;
    let path = std::fs::canonicalize(path).map_err(|error| {
        LuaError::generic(format!("export path could not be inspected: {error}"))
    })?;
    if !path.starts_with(&root) {
        return Err(LuaError::generic(
            "export path resolves outside the configured export directory",
        ));
    }

    Ok(true)
}

fn export_root(lua: &mlua::Lua) -> mlua::Result<std::path::PathBuf> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();
    if !app_state.has_feature(AppFeatures::EXPORT) {
        return Err(LuaError::generic("export access is disabled"));
    }
    std::fs::create_dir_all(app_state.export_dir())
        .map_err(|error| LuaError::generic(format!("export directory is unavailable: {error}")))?;
    std::fs::canonicalize(app_state.export_dir())
        .map_err(|error| LuaError::generic(format!("export directory is unavailable: {error}")))
}

fn export_path(root: &std::path::Path, relative: &str) -> mlua::Result<std::path::PathBuf> {
    let relative = sanitize_path(Path::new(relative))
        .map_err(|error| LuaError::generic(format!("invalid export path: {error}")))?;
    Ok(root.join(relative))
}

fn checked_export_file(root: &std::path::Path, relative: &str) -> mlua::Result<std::path::PathBuf> {
    let path = export_path(root, relative)?;
    let canonical = std::fs::canonicalize(&path).map_err(|error| {
        LuaError::generic(format!("export file could not be inspected: {error}"))
    })?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(LuaError::generic(
            "export path must name a file inside the configured export directory",
        ));
    }
    Ok(canonical)
}

fn checked_export_parent(
    root: &std::path::Path,
    relative: &str,
) -> mlua::Result<std::path::PathBuf> {
    let path = export_path(root, relative)?;
    let parent = path
        .parent()
        .ok_or_else(|| LuaError::generic("export path has no parent directory"))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        LuaError::generic(format!("export directory could not be created: {error}"))
    })?;
    let canonical_parent = std::fs::canonicalize(parent).map_err(|error| {
        LuaError::generic(format!("export directory could not be inspected: {error}"))
    })?;
    if !canonical_parent.starts_with(root) {
        return Err(LuaError::generic(
            "export path resolves outside the configured export directory",
        ));
    }
    Ok(path)
}

pub(super) fn lua_export_list(lua: &mlua::Lua, args: FunctionArgs) -> mlua::Result<Vec<String>> {
    let root = export_root(lua)?;
    let argument = args.get::<String>(0)?;
    note(lua, true, &argument);
    let directory = export_path(&root, &argument)?;
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let canonical = std::fs::canonicalize(&directory).map_err(|error| {
        LuaError::generic(format!("export directory could not be inspected: {error}"))
    })?;
    if !canonical.starts_with(&root) || !canonical.is_dir() {
        return Err(LuaError::generic(
            "export path must name a directory inside the configured export directory",
        ));
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(canonical).map_err(|error| {
        LuaError::generic(format!("export directory could not be listed: {error}"))
    })? {
        let entry = entry.map_err(|error| {
            LuaError::generic(format!("export entry could not be read: {error}"))
        })?;
        let path = entry.path();
        let Ok(resolved) = std::fs::canonicalize(&path) else {
            continue;
        };
        if resolved.starts_with(&root) {
            if let Ok(relative) = resolved.strip_prefix(&root) {
                entries.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    entries.sort();
    Ok(entries)
}

pub(super) fn lua_export_rename(lua: &mlua::Lua, args: FunctionArgs) -> mlua::Result<()> {
    let root = export_root(lua)?;
    let source_name = args.get::<String>(0)?;
    let destination_name = args.get::<String>(1)?;
    note(lua, false, &source_name);
    note(lua, false, &destination_name);
    let source = checked_export_file(&root, &source_name)?;
    let destination = checked_export_parent(&root, &destination_name)?;
    if destination.exists() {
        return Err(LuaError::generic("destination export file already exists"));
    }
    std::fs::rename(source, destination)
        .map_err(|error| LuaError::generic(format!("export file could not be renamed: {error}")))
}

pub(super) fn lua_export_copy(lua: &mlua::Lua, args: FunctionArgs) -> mlua::Result<()> {
    let root = export_root(lua)?;
    let source_name = args.get::<String>(0)?;
    let destination_name = args.get::<String>(1)?;
    note(lua, false, &source_name);
    note(lua, false, &destination_name);
    let source = checked_export_file(&root, &source_name)?;
    let destination = checked_export_parent(&root, &destination_name)?;
    if destination.exists() {
        return Err(LuaError::generic("destination export file already exists"));
    }
    std::fs::copy(source, destination)
        .map(|_| ())
        .map_err(|error| LuaError::generic(format!("export file could not be copied: {error}")))
}

pub(super) fn lua_export_delete(lua: &mlua::Lua, args: FunctionArgs) -> mlua::Result<()> {
    let root = export_root(lua)?;
    let argument = args.get::<String>(0)?;
    note(lua, false, &argument);
    let path = checked_export_file(&root, &argument)?;
    std::fs::remove_file(path)
        .map_err(|error| LuaError::generic(format!("export file could not be deleted: {error}")))
}

pub(super) fn lua_read_export_to_string(
    lua: &mlua::Lua,
    args: FunctionArgs,
) -> mlua::Result<mlua::String> {
    let app_state = lua.app_data_ref::<AppState>().unwrap();
    if !app_state.has_feature(AppFeatures::EXPORT) {
        return Err(LuaError::generic("export access is disabled"));
    }
    let argument = args.get::<String>(0)?;
    let relative = sanitize_path(Path::new(&argument))
        .map_err(|error| LuaError::generic(format!("invalid export path: {error}")))?;
    note(lua, false, &argument);
    let root = std::fs::canonicalize(app_state.export_dir())
        .map_err(|error| LuaError::generic(format!("export directory is unavailable: {error}")))?;
    let path = std::fs::canonicalize(root.join(relative))
        .map_err(|error| LuaError::generic(format!("export file could not be opened: {error}")))?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err(LuaError::generic(
            "export path must name a file inside the configured export directory",
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| LuaError::generic(format!("export file could not be read: {error}")))?;
    lua.create_string(&bytes)
}

/// Notes a path a pregame pass's mod uses (`AppState::export_touches`), so that the
/// loader's record of the pass holds what the pass used and nothing else of the export folder.
fn note(lua: &mlua::Lua, listing: bool, relative: &str) {
    let app_state = lua.app_data_ref::<AppState>().unwrap();
    let Some(touches) = app_state.export_touches() else {
        return;
    };
    let Ok(relative) = sanitize_path(Path::new(relative)) else {
        return;
    };
    let relative = relative.as_str().replace('\\', "/");
    if listing {
        touches.listing(&relative);
    } else {
        touches.path(&relative);
    }
}

fn sanitize_path(path: &Path) -> std::io::Result<PathBuf> {
    let mut result = PathBuf::new();

    for component in path.components() {
        match component {
            camino::Utf8Component::Prefix(_) => {
                return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
            }
            camino::Utf8Component::RootDir => {
                return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
            }
            camino::Utf8Component::CurDir => {}
            camino::Utf8Component::ParentDir => {
                if !result.pop() {
                    return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
                }
            }
            camino::Utf8Component::Normal(part) => result.push(part),
        }
    }

    Ok(result)
}
