use shroudforge_modloader::prepare;
use shroudforge_parser::{GameFiles, GameParser, KfcParser, export_api_snapshot};
use std::{env, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() == 1 {
        return launch_desktop();
    }
    match args.get(1).and_then(|value| value.to_str()) {
        Some("--module-ui") => return shroudforge_modloader_ui::run_module(),
        Some("--catalog-install-worker") => {
            return shroudforge_modloader_ui::run_catalog_install_worker()
                .map_err(|error| std::io::Error::other(error).into());
        }
        Some("--debug-console") => {
            return match shroudforge_debug_console::run_module() {
                Ok(()) => Ok(()),
                Err(error) => {
                    let detail = format!("Debug Console failed to start: {error}");
                    let root = args
                        .windows(2)
                        .find(|pair| pair[0] == "--root")
                        .map(|pair| PathBuf::from(&pair[1]));
                    if let Some(root) = root {
                        let _ = shroudforge_package::logging::append(
                            &root,
                            'E',
                            "debug-console",
                            &detail,
                        );
                        #[cfg(windows)]
                        {
                            use std::os::windows::ffi::OsStrExt;
                            use windows_sys::Win32::UI::WindowsAndMessaging::{
                                MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW,
                            };
                            let message = format!(
                                "{detail}\n\nShroudForge log:\n{}",
                                shroudforge_package::paths::current_log(&root).display()
                            );
                            let message: Vec<u16> = std::ffi::OsStr::new(&message)
                                .encode_wide()
                                .chain(std::iter::once(0))
                                .collect();
                            let title: Vec<u16> = "ShroudForge Debug Console"
                                .encode_utf16()
                                .chain(std::iter::once(0))
                                .collect();
                            unsafe {
                                MessageBoxW(
                                    std::ptr::null_mut(),
                                    message.as_ptr(),
                                    title.as_ptr(),
                                    MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
                                );
                            }
                        }
                    }
                    Err(error)
                }
            };
        }
        Some("--world-editor-ui") => {
            return match shroudforge_world_editor_ui::run_module() {
                Ok(()) => Ok(()),
                Err(error) => {
                    let detail = format!("World Editor window failed to start: {error}");
                    if let Some(root) = args
                        .windows(2)
                        .find(|pair| pair[0] == "--root")
                        .map(|pair| PathBuf::from(&pair[1]))
                    {
                        let _ = shroudforge_package::logging::append(
                            &root,
                            'E',
                            "world-editor-ui",
                            &detail,
                        );
                    }
                    Err(std::io::Error::other(detail).into())
                }
            };
        }
        Some("--commands") => {
            shroudforge_commands::run_module();
            return Ok(());
        }
        Some("--runtime-diagnostics") => {
            return shroudforge_runtime_diagnostics::run_module()
                .map_err(|error| std::io::Error::other(error).into());
        }
        Some("--update-worker") => {
            return shroudforge_updater::run_module()
                .map_err(|error| std::io::Error::other(error).into());
        }
        _ => {}
    }
    if let Some(game) = args.get(2) {
        let _ = shroudforge_package::logging::initialize(PathBuf::from(game), true);
    }
    match args.get(1).and_then(|value| value.to_str()) {
        Some("types") if args.len() == 4 => {
            let game = PathBuf::from(&args[2]);
            let schema = KfcParser.parse(&detect_files(&game))?;
            let result = export_api_snapshot(&schema, PathBuf::from(&args[3]))?;
            println!(
                "exported {} native types and {} fields to {}",
                result.type_count,
                result.field_count,
                result.directory.display()
            );
        }
        Some("prepare") if args.len() == 3 => {
            prepare(PathBuf::from(&args[2]))?;
            println!("pregame phase complete");
        }
        Some("create") if (args.len() == 5 || args.len() == 6) => {
            let game = PathBuf::from(&args[2]);
            let id = args[3].to_str().ok_or("mod id must be valid UTF-8")?;
            let name = args[4].to_str().ok_or("mod name must be valid UTF-8")?;
            let capabilities = args
                .get(5)
                .and_then(|value| value.to_str())
                .unwrap_or("patch")
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            create_mod(&game, id, name, &capabilities)?;
        }
        Some("restore") if args.len() == 3 => {
            let game = PathBuf::from(&args[2]);
            let file_name = if game.join("enshrouded.exe").is_file() {
                "enshrouded"
            } else {
                "enshrouded_server"
            };
            let game_utf8 = game.to_str().ok_or("game directory must be valid UTF-8")?;
            if !shroudforge_api::restore(game_utf8, file_name) {
                return Err(format!("could not restore the original {file_name}.kfc file").into());
            }
            println!("original game data restored");
        }
        Some("launch") if args.len() >= 3 => {
            let game = PathBuf::from(&args[2]);
            prepare(&game)?;
            let executable = game_executable(&game)?;
            let forwarded = args
                .iter()
                .skip(3)
                .skip_while(|argument| argument.to_str() == Some("--"));
            let status = Command::new(&executable)
                .current_dir(&game)
                .args(forwarded)
                .status()?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Some("inspect") if args.len() == 3 => {
            let game = PathBuf::from(&args[2]);
            let is_client = game.join("enshrouded.exe").is_file();
            let schema = KfcParser.parse(&detect_files(&game))?;
            let root = game
                .to_str()
                .ok_or("game directory path must be valid UTF-8")?;
            let environment = shroudforge_package::ModEnvironment::load(root)
                .map_err(|report| format!("mod discovery failed: {report:?}"))?;
            let is_server = !is_client;
            let (plan, errors) = environment.plan_report(is_server, shroudforge_api::API_VERSION);
            let ids = plan
                .iter()
                .map(|item| item.info().id.as_str())
                .collect::<Vec<_>>();
            println!(
                "{} target: {}",
                if is_server { "Server" } else { "Client" },
                game.display()
            );
            println!("Planned mods: {}", ids.join(", "));
            for error in errors {
                eprintln!("Skipped mod: {error}");
            }
            let _ = schema;
        }
        _ => {
            eprintln!("usage: shroudforge types <game-directory> <output-directory>");
            eprintln!(
                "       shroudforge create <game-directory> <mod-id> <mod-name> [patch,export,runtime]"
            );
            eprintln!("       shroudforge prepare <game-directory>");
            eprintln!("       shroudforge restore <game-directory>");
            eprintln!("       shroudforge launch <game-directory> [-- <game-arguments>]");
            eprintln!("       shroudforge inspect <game-directory>");
            std::process::exit(2);
        }
    }
    Ok(())
}

fn create_mod(
    game: &PathBuf,
    id: &str,
    name: &str,
    capabilities: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    if !shroudforge_package::valid_id(id) {
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
    let mods = shroudforge_package::paths::mods_dir(game);
    let destination = mods.join(id);
    let manifest = serde_json::json!({
        "$schema": "https://bonsaibauer.github.io/shroudforge/schemas/manifest.schema.json",
        "id": id,
        "name": name,
        "version": "0.1.0",
        "authors": [],
        "dependencies": [],
        "capabilities": capabilities
    });
    let extension = serde_json::json!({
        "$schema": "https://bonsaibauer.github.io/shroudforge/schemas/extended.mod.schema.json",
        "schemaVersion": 1,
        "enabled": true,
        "targets": ["client"],
        "launcher": "SF",
        "settings": {}
    });
    let lua_library = shroudforge_package::paths::cache_dir(game)
        .join("lua")
        .to_string_lossy()
        .into_owned();
    let lua_ls = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
        "runtime": { "version": "Lua 5.4" },
        "workspace": { "library": [lua_library] }
    });
    shroudforge_package::config::validate_document(game, "mod", &manifest)?;
    shroudforge_package::config::validate_document(game, "extended-mod", &extension)?;
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
    if !shroudforge_api::export_lua_definitions(game_utf8, file_name, true) {
        eprintln!("mod created, but Lua definitions could not be generated from the game files");
    }
    println!("created mod at {}", destination.display());
    Ok(())
}

fn launch_desktop() -> Result<(), Box<dyn std::error::Error>> {
    let executable = env::current_exe()?;
    let executable_directory = executable
        .parent()
        .ok_or("ShroudForge executable has no installation directory")?;
    let root = executable_directory
        .parent()
        .unwrap_or(executable_directory);
    let target = if root.join("enshrouded.exe").is_file() {
        "client"
    } else if root.join("enshrouded_server.exe").is_file() {
        "server"
    } else {
        // Open the desktop UI so it can report the missing game path clearly.
        "client"
    };

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new(&executable)
            .args(["--module-ui", "--desktop", "--root"])
            .arg(root)
            .args(["--target", target])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        let _ = (executable, target);
        Err("Explorer desktop launch is available on Windows only".into())
    }
}

fn game_executable(game: &PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    for name in ["enshrouded.exe", "enshrouded_server.exe"] {
        let executable = game.join(name);
        if executable.is_file() {
            return Ok(executable);
        }
    }
    Err(format!("no Enshrouded executable in {}", game.display()).into())
}

fn detect_files(game: &PathBuf) -> GameFiles {
    if game.join("enshrouded.exe").is_file() {
        GameFiles::client(game)
    } else {
        GameFiles::server(game)
    }
}
