use std::{env, io::{BufRead, BufReader, Read}, path::{Path, PathBuf}, process::{Command, Stdio}, thread, time::{Duration, Instant}};

const TOOLS: &[&str] = &["dump-live-components", "inspect-component-metadata", "live-entity-manager-sample", "live-entity-managers", "live-type-references"];

fn capture(stream: impl Read + Send + 'static, tool: String) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for line in BufReader::new(stream.take(2 * 1024 * 1024)).lines() {
            match line {
                Ok(line) => tracing::info!(target: "shroudforge::diagnostics", tool = %tool, "{}", line),
                Err(error) => { tracing::warn!(target: "shroudforge::diagnostics", "output read failed: {error}"); break; }
            }
        }
    })
}

fn run_tool(root: &Path, tool: &str, pid: u32, address: Option<&str>, timeout: Duration) -> Result<(), String> {
    let directory = env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("missing executable directory")?.to_path_buf();
    let mut command = Command::new(directory.join("tools").join(format!("{tool}.exe")));
    command.arg(pid.to_string()).current_dir(root).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(address) = address { command.arg(address); }
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|e| format!("{tool}: {e}"))?;
    let output = capture(child.stdout.take().ok_or("missing stdout")?, tool.into());
    let errors = capture(child.stderr.take().ok_or("missing stderr")?, tool.into());
    let start = Instant::now();
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => break if status.success() { Ok(()) } else { Err(format!("{tool} exited with {status}")) },
            Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(50)),
            other => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(match other { Err(e) => e.to_string(), _ => format!("{tool} exceeded {} seconds", timeout.as_secs()) });
            }
        }
    };
    let _ = output.join();
    let _ = errors.join();
    result
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().collect();
    if args.iter().any(|arg| arg == "--list") { println!("{}", TOOLS.join("\n")); return Ok(()); }
    let arg = |name: &str| args.windows(2).find(|pair| pair[0] == name).map(|pair| pair[1].as_str());
    let root = PathBuf::from(arg("--root").ok_or("missing --root installation directory")?);
    if args.iter().any(|value|value=="--status") {println!("{}",shroudforge_runtime_diagnostics::status(&root));return Ok(());}
    for action in ["start","stop","snapshot"] {
        if args.iter().any(|value|value==&format!("--{action}")) {
            shroudforge_runtime_diagnostics::request(&root,action)?;
            println!("Diagnostic {action} requested; the loader processes it when running");
            return Ok(());
        }
    }
    let tool = arg("--tool").filter(|tool| TOOLS.contains(tool)).ok_or("choose a supported --tool; use --list")?;
    let pid: u32 = arg("--pid").ok_or("missing --pid")?.parse().map_err(|_| "invalid pid")?;
    if pid == 0 { return Err("invalid pid".into()); }
    let address = if tool == "inspect-component-metadata" {
        let address = arg("--address").ok_or("metadata inspection requires --address in hexadecimal")?;
        u64::from_str_radix(address, 16).map_err(|_| "invalid address")?;
        Some(address)
    } else { None };
    let seconds: u64 = arg("--timeout-seconds").unwrap_or("10").parse().map_err(|_| "invalid timeout")?;
    if !(1..=60).contains(&seconds) { return Err("timeout must be between 1 and 60 seconds".into()); }
    shroudforge_package::logging::initialize(&root, false).map_err(|e| e.to_string())?;
    let session_started=Instant::now();
    loop {
        let config = shroudforge_package::config::read_loader(&root)?;
        let settings = &config["modules"]["runtimeDiagnostics"];
        let continuous = args.iter().any(|arg| arg == "--continuous");
        if continuous && (settings["enabled"] != true || settings["continuous"] != true) {
            return Err("continuous diagnostics require enabled and continuous in loader configuration".into());
        }
        tracing::info!(target: "shroudforge::diagnostics", tool, pid, "Starting explicitly requested diagnostic");
        let limit=Duration::from_secs(settings["maximumDurationSeconds"].as_u64().unwrap_or(120).clamp(1,3600));
        let Some(remaining)=limit.checked_sub(session_started.elapsed()) else {return Ok(());};
        run_tool(&root, tool, pid, address, Duration::from_secs(seconds).min(remaining))?;
        if !continuous { return Ok(()); }
        let next=Instant::now()+Duration::from_millis(settings["intervalMilliseconds"].as_u64().unwrap_or(2000).clamp(500,60000));
        while Instant::now()<next {
            if session_started.elapsed()>=limit {return Ok(());}
            let config=shroudforge_package::config::read_loader(&root)?;
            if config["modules"]["runtimeDiagnostics"]["enabled"]!=true {return Ok(());}
            thread::sleep(Duration::from_millis(250));
        }
    }
}

fn main() {
    if let Err(error) = run() {
        tracing::error!(target: "shroudforge::diagnostics", "{error}");
        eprintln!("{error}");
        std::process::exit(1);
    }
}
