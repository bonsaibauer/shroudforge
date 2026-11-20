use crate::LoaderError;

#[cfg(windows)]
pub(super) fn ensure_game_stopped(file_name: &str) -> Result<(), LoaderError> {
    ensure_processes_stopped(file_name, None)
}

#[cfg(windows)]
pub(super) fn ensure_no_other_game_process(
    file_name: &str,
    root: &std::path::Path,
) -> Result<(), LoaderError> {
    ensure_processes_stopped(file_name, Some(root))
}

#[cfg(windows)]
fn ensure_processes_stopped(
    file_name: &str,
    root: Option<&std::path::Path>,
) -> Result<(), LoaderError> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
    };
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(LoaderError::Pregame(
                "cannot verify that Enshrouded is stopped".into(),
            ));
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = Process32FirstW(snapshot, &mut entry) != 0;
        let mut running = false;
        while found {
            let len = entry
                .szExeFile
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            if name.eq_ignore_ascii_case(&format!("{file_name}.exe")) {
                if let Some(root) = root {
                    if entry.th32ProcessID == std::process::id() {
                        found = Process32NextW(snapshot, &mut entry) != 0;
                        continue;
                    }
                    use windows_sys::Win32::System::Threading::{
                        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
                    };
                    let process =
                        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                    if process.is_null() {
                        CloseHandle(snapshot);
                        return Err(LoaderError::Pregame(
                            "cannot verify another Enshrouded process".into(),
                        ));
                    }
                    let mut path = vec![0u16; 32768];
                    let mut length = path.len() as u32;
                    let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length);
                    CloseHandle(process);
                    if ok == 0 {
                        CloseHandle(snapshot);
                        return Err(LoaderError::Pregame(
                            "cannot verify another Enshrouded process path".into(),
                        ));
                    }
                    let executable = std::path::PathBuf::from(String::from_utf16_lossy(
                        &path[..length as usize],
                    ));
                    let other_root = executable
                        .parent()
                        .and_then(|path| path.canonicalize().ok());
                    let selected_root = root.canonicalize().ok();
                    if other_root.is_none() || selected_root.is_none() {
                        CloseHandle(snapshot);
                        return Err(LoaderError::Pregame(
                            "cannot resolve Enshrouded installation paths".into(),
                        ));
                    }
                    if other_root != selected_root {
                        found = Process32NextW(snapshot, &mut entry) != 0;
                        continue;
                    }
                }
                running = true;
                break;
            }
            found = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        if running {
            return Err(LoaderError::Pregame("close Enshrouded before preparing asset mods; live runtime mods do not need prepare".into()));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn ensure_no_other_game_process(
    _file_name: &str,
    _root: &std::path::Path,
) -> Result<(), LoaderError> {
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn ensure_game_stopped(_file_name: &str) -> Result<(), LoaderError> {
    Ok(())
}
