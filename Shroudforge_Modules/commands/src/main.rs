use std::{env, fs::OpenOptions, io::Write, path::PathBuf};

use serde::Serialize;

#[derive(Debug, Serialize)]
struct ModuleStatus<'a> {
    id: &'a str,
    name: &'a str,
    version: &'a str,
    status: &'a str,
}

fn main() {
    let status = ModuleStatus {
        id: "shroudforge.commands",
        name: "ShroudForge Commands",
        version: env!("CARGO_PKG_VERSION"),
        status: "available",
    };

    if env::args().any(|arg| arg == "--status") {
        println!(
            "{}",
            serde_json::to_string_pretty(&status).expect("status is serializable")
        );
        return;
    }

    if let Some(root) = module_root() {
        let log_path = root.join("shroudforge.log");
        if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(log_path) {
            let line = shroudforge_package::logging::format_line(
                'I',
                "commands",
                "Command gateway module available",
            );
            let _ = writeln!(log, "{line}");
        }
    }
}

fn module_root() -> Option<PathBuf> {
    env::args()
        .skip_while(|arg| arg != "--root")
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
}
