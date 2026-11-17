use std::path::PathBuf;

pub fn create_mod(
    game: &PathBuf,
    id: &str,
    name: &str,
    capabilities: &[&str],
    export_lua_definitions: impl FnOnce(&str, &str, bool) -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !crate::valid_id(id) {
        return Err(format!("invalid mod id: {id}").into());
    }
    if name.trim().is_empty() {
        return Err("mod name cannot be empty".into());
    }
    if !game.join("enshrouded.exe").is_file()
        && !game.join("enshrouded_server.exe").is_file()
        && !game.join("enshrouded.kfc").is_file()
        && !game.join("enshrouded_server.kfc").is_file()
    {
        return Err(format!("not an Enshrouded game directory: {}", game.display()).into());
    }
    let capabilities = if capabilities.is_empty() {
        vec![]
    } else {
        let mut values = Vec::new();
        for capability in capabilities {
            if !matches!(*capability, "patch" | "export" | "runtime") {
                return Err(format!("unsupported EML capability: {capability}").into());
            }
            if !values.contains(capability) {
                values.push(*capability);
            }
        }
        values
    };
    let mods = shroudforge_loader_config::paths::mods_dir(game);
    let destination = mods.join(id);
    let manifest = serde_json::json!({
        "$schema": "https://shroudforge.net/schemas/1.8.0/manifest.schema.json",
        "id": id,
        "name": name,
        "version": "0.1.0",
        "authors": [],
        "dependencies": [],
        "capabilities": capabilities
    });
    let extension = serde_json::json!({
        "$schema": "https://shroudforge.net/schemas/1.8.0/extended.mod.schema.json",
        "schemaVersion": 1,
        "enabled": true,
        "targets": ["client"],
        "launcher": "SF",
        "settings": {}
    });
    let lua_library = shroudforge_loader_config::paths::cache_dir(game)
        .join("lua")
        .to_string_lossy()
        .into_owned();
    let lua_ls = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
        "runtime": { "version": "Lua 5.4" },
        "workspace": { "library": [lua_library] }
    });
    shroudforge_loader_config::config::validate_document(game, crate::documents::MOD, &manifest)?;
    shroudforge_loader_config::config::validate_document(
        game,
        crate::documents::EXTENDED_MOD,
        &extension,
    )?;
    std::fs::create_dir_all(&mods)?;
    std::fs::create_dir(&destination)
        .map_err(|error| format!("could not create {}: {error}", destination.display()))?;
    std::fs::create_dir(destination.join("src"))?;
    let files = [
        ("mod.json", serde_json::to_vec_pretty(&manifest)?),
        ("extended.mod.json", serde_json::to_vec_pretty(&extension)?),
        (
            "README.md",
            format!("# {name}\n\nNew ShroudForge mod.\n").into_bytes(),
        ),
        (".luarc.json", serde_json::to_vec_pretty(&lua_ls)?),
        (
            "src/mod.lua",
            b"print(\"Hello from ShroudForge\")\n".to_vec(),
        ),
    ];
    for (relative, contents) in files {
        std::fs::write(destination.join(relative), contents)?;
    }
    let file_name = if game.join("enshrouded.exe").is_file() {
        "enshrouded"
    } else {
        "enshrouded_server"
    };
    let game_utf8 = game.to_str().ok_or("game directory must be valid UTF-8")?;
    if !export_lua_definitions(game_utf8, file_name, true) {
        eprintln!("mod created, but Lua definitions could not be generated from the game files");
    }
    println!("created mod at {}", destination.display());
    Ok(())
}
