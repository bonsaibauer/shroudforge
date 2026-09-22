use shroudforge_api::ShroudForgeApi;
use shroudforge_compatibility::Compatibility;
use shroudforge_modloader::{ModLoader, prepare};
use shroudforge_parser::{GameFiles, GameParser, KfcParser, export_api_snapshot};
use std::{env, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
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
            let api = ShroudForgeApi::new(
                Compatibility::new([
                    "runtime.lifecycle",
                    "runtime.ecs.query",
                    "runtime.ecs.resolve",
                    "runtime.ecs.read",
                    "runtime.ecs.write",
                ])
                .resolve(schema),
            );
            let mut loader = ModLoader::new(api, is_client);
            let report = loader.load_directory(game.join("mods"))?;
            println!("Lua runtime mods: {}", report.lua.join(", "));
        }
        _ => {
            eprintln!("usage: shroudforge types <game-directory> <output-directory>");
            eprintln!("       shroudforge prepare <game-directory>");
            eprintln!("       shroudforge launch <game-directory> [-- <game-arguments>]");
            eprintln!("       shroudforge inspect <game-directory>");
            std::process::exit(2);
        }
    }
    Ok(())
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
