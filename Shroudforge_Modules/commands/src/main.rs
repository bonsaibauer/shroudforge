use std::{env, path::PathBuf};

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
        let _ = shroudforge_package::logging::append(&root, 'I', "commands", "Command gateway module available");
    }
}

fn module_root() -> Option<PathBuf> {
    env::args()
        .skip_while(|arg| arg != "--root")
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
}
