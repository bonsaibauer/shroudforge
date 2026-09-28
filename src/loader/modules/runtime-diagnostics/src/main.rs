use std::{env, path::PathBuf};

fn run_from_args() -> Result<(), String> {
    let args: Vec<_> = env::args().collect();
    let arg = |name: &str| {
        args.windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].as_str())
    };
    let root = PathBuf::from(arg("--root").ok_or("missing --root installation directory")?);

    if args.iter().any(|value| value == "--status") {
        println!("{}", crate::status(&root));
        return Ok(());
    }

    for action in ["start", "stop", "snapshot"] {
        if args.iter().any(|value| value == &format!("--{action}")) {
            crate::request(&root, action)?;
            println!("Diagnostic {action} requested; the loader processes it when running");
            return Ok(());
        }
    }

    Err("choose --start, --snapshot, --stop, or --status".into())
}

pub fn run_module() -> Result<(), String> {
    run_from_args()
}
