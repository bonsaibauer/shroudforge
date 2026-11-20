use std::path::PathBuf;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;

pub(crate) const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;

pub(crate) struct Arguments {
    pub(crate) root: PathBuf,
    pub(crate) game_pid: u32,
    pub(crate) game_process: HANDLE,
    pub(crate) stop_name: Option<String>,
    pub(crate) desktop: bool,
    pub(crate) server: bool,
    pub(crate) vanilla_session: bool,
}

pub(crate) fn arguments() -> Result<Arguments, Box<dyn std::error::Error>> {
    let values: Vec<String> = std::env::args().collect();
    let value = |name: &str| {
        values
            .windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].clone())
    };
    let root = PathBuf::from(value("--root").ok_or("missing --root")?);
    let desktop = values.iter().any(|value| value == "--desktop");
    let game_pid = if desktop {
        unsafe { GetCurrentProcessId() }
    } else {
        value("--game-pid").ok_or("missing --game-pid")?.parse()?
    };
    let mut game_process: HANDLE = std::ptr::null_mut();
    let stop_name = if desktop {
        None
    } else {
        Some(value("--stop-event").ok_or("missing --stop-event")?)
    };
    let server =
        if desktop {
            match value("--target").as_deref() {
                Some("server") => true,
                Some("client") => false,
                Some(_) => return Err("--target must be client or server".into()),
                None => {
                    match (
                        root.join("enshrouded.exe").is_file(),
                        root.join("enshrouded_server.exe").is_file(),
                    ) {
                        (true, false) => false,
                        (false, true) => true,
                        _ => return Err(
                            "Select an installation target with --target client or --target server"
                                .into(),
                        ),
                    }
                }
            }
        } else {
            use windows_sys::Win32::System::Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            };
            let process = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE_ACCESS,
                    0,
                    game_pid,
                )
            };
            if process.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let mut buffer = vec![0u16; 32768];
            let mut length = buffer.len() as u32;
            let success =
                unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) };
            let error = std::io::Error::last_os_error();
            if success == 0 {
                unsafe {
                    CloseHandle(process);
                }
                return Err(error.into());
            }
            let executable = PathBuf::from(String::from_utf16(&buffer[..length as usize])?);
            if executable
                .parent()
                .ok_or("target has no installation directory")?
                .canonicalize()?
                != root.canonicalize()?
            {
                unsafe {
                    CloseHandle(process);
                }
                return Err("Target process does not belong to the selected installation".into());
            }
            let server = match executable
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("enshrouded_server.exe") => true,
                Some("enshrouded.exe") => false,
                _ => {
                    unsafe {
                        CloseHandle(process);
                    }
                    return Err(
                        "Target process is not an Enshrouded client or dedicated server".into(),
                    );
                }
            };
            game_process = process;
            server
        };
    Ok(Arguments {
        root,
        game_pid,
        game_process,
        stop_name,
        desktop,
        server,
        vanilla_session: values.iter().any(|value| value == "--vanilla-session"),
    })
}
