pub mod schematic_library;
pub mod validate_schematic_v8;

#[cfg(feature = "lua-api")]
pub mod lua_api;

pub fn enabled(root: &std::path::Path) -> Result<bool, String> {
    let config = shroudforge_loader_config::config::read_loader(root)?;
    Ok(config["modules"]["schematics"]["enabled"]
        .as_bool()
        .unwrap_or(true))
}

pub fn require_enabled(root: &std::path::Path) -> Result<(), String> {
    if enabled(root)? {
        Ok(())
    } else {
        Err("Schematics is disabled in Settings > Modules".into())
    }
}
