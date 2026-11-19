use std::{env, ffi::OsString, path::PathBuf};

pub(crate) fn run(args: &[OsString]) -> Option<Result<(), Box<dyn std::error::Error>>> {
    match args.get(1).and_then(|value| value.to_str()) {
        Some("--keyboard-input") => {
            let parent = args
                .windows(2)
                .find(|pair| pair[0] == "--parent-pid")
                .and_then(|pair| pair[1].to_str())
                .and_then(|value| value.parse::<u32>().ok());
            return Some(
                parent
                    .ok_or_else(|| std::io::Error::other("missing --parent-pid"))
                    .and_then(|parent| {
                        shroudforge_hotkeys::run_input_process(parent)
                            .map_err(std::io::Error::other)
                    })
                    .map_err(Into::into),
            );
        }
        Some("--module-ui") => {
            if !args.iter().any(|arg| arg == "--vanilla-session") && let Some(root) = args
                .windows(2)
                .find(|pair| pair[0].to_str() == Some("--root"))
                .map(|pair| PathBuf::from(&pair[1]))
                && let Err(error) = shroudforge_modloader::run_post_install_migrations(&root)
            {
                let message = format!(
                    "Post-install migration did not finish before the Modloader opened: {error}"
                );
                let _ = shroudforge_logging::append(&root, 'E', "updater", &message);
                #[cfg(windows)]
                show_post_install_migration_error(&message);
            }
            return Some(shroudforge_modloader_ui::run_module());
        }
        Some("--debug-console") => {
            return Some(match shroudforge_debug_console::run_module() {
                Ok(()) => Ok(()),
                Err(error) => {
                    let detail = format!("Debug Console failed to start: {error}");
                    let root = args
                        .windows(2)
                        .find(|pair| pair[0] == "--root")
                        .map(|pair| PathBuf::from(&pair[1]));
                    if let Some(root) = root {
                        let _ = shroudforge_logging::append(&root, 'E', "debug-console", &detail);
                        #[cfg(windows)]
                        {
                            use std::os::windows::ffi::OsStrExt;
                            use windows_sys::Win32::UI::WindowsAndMessaging::{
                                MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW,
                            };
                            let message = format!(
                                "{detail}\n\nShroudForge log:\n{}",
                                shroudforge_loader_config::paths::current_log(&root).display()
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
            });
        }
        Some("--world-editor-ui") => {
            return Some(match shroudforge_world_editor_ui::run_module() {
                Ok(()) => Ok(()),
                Err(error) => {
                    let detail = format!("World Editor window failed to start: {error}");
                    if let Some(root) = args
                        .windows(2)
                        .find(|pair| pair[0] == "--root")
                        .map(|pair| PathBuf::from(&pair[1]))
                    {
                        let _ = shroudforge_logging::append(&root, 'E', "world-editor-ui", &detail);
                    }
                    Err(std::io::Error::other(detail).into())
                }
            });
        }
        Some("--commands") => {
            shroudforge_commands::run_module();
            return Some(Ok(()));
        }
        Some("--runtime-diagnostics") => {
            return Some(
                shroudforge_runtime_diagnostics::run_module()
                    .map_err(|error| std::io::Error::other(error).into()),
            );
        }
        _ => {}
    }
    None
}

#[cfg(windows)]
fn show_post_install_migration_error(message: &str) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW,
    };

    let message: Vec<u16> = std::ffi::OsStr::new(message)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let title: Vec<u16> = "ShroudForge Updater"
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

pub(crate) fn launch_desktop() -> Result<(), Box<dyn std::error::Error>> {
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
