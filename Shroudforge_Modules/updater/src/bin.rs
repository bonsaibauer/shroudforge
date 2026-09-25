#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let result = if arguments.iter().any(|arg| arg == "--queue-install") {
        let value = |name: &str| arguments.windows(2).find(|pair| pair[0] == name).map(|pair| pair[1].clone());
        match (value("--root"), value("--wait-pid").and_then(|pid| pid.parse::<u32>().ok())) {
            (Some(root), Some(pid)) => shroudforge_updater::request_install_after_game(std::path::Path::new(&root), pid),
            _ => Err("missing or invalid --root/--wait-pid".into()),
        }
    } else {
        shroudforge_updater::run_module()
    };
    if let Err(error) = result {
        eprintln!("ShroudForge updater: {error}");
        std::process::exit(1);
    }
}
