use crate::LoaderError;
use shroudforge_api::ShroudForgeApi;
use shroudforge_compatibility::Compatibility;
use shroudforge_parser::{GameFiles, GameParser, KfcParser};
use std::path::Path;

pub fn run(game_directory: impl AsRef<Path>) -> Result<(), LoaderError> {
    let game_directory = game_directory.as_ref();
    ensure_game_stopped()?;
    for error in shroudforge_package::migration::migrate_installation(game_directory).map_err(LoaderError::Pregame)? {
        tracing::warn!(%error,"Installation migration failed");
    }
    let game_utf8 = game_directory.to_str().ok_or_else(|| {
        LoaderError::Environment(format!(
            "game directory is not valid UTF-8: {}",
            game_directory.display()
        ))
    })?;
    let environment = shroudforge_package::ModEnvironment::load(game_utf8)
        .map_err(|report| LoaderError::Environment(format_report(&report)))?;
    let file_name = if game_directory.join("enshrouded.exe").is_file() {
        "enshrouded"
    } else if game_directory.join("enshrouded_server.exe").is_file() {
        "enshrouded_server"
    } else {
        return Err(LoaderError::Environment(format!(
            "no Enshrouded executable in {}",
            game_directory.display()
        )));
    };
    let backup = game_directory.join(format!("{file_name}.kfc.bak"));
    let fingerprint = shroudforge_package::prepared::fingerprint(&environment, file_name == "enshrouded_server", shroudforge_api::API_VERSION).map_err(LoaderError::Pregame)?;
    shroudforge_package::config::write_document(
        game_directory,
        "applied",
        &serde_json::json!({
            "schemaVersion": 1, "status": "preparing", "target": file_name
        }),
    )
    .map_err(LoaderError::Pregame)?;
    shroudforge_parser::transaction::recover(game_directory, file_name)
        .map_err(|error| LoaderError::Pregame(error.to_string()))?;
    if backup.is_file() && !shroudforge_api::restore(game_utf8, file_name) {
        return Err(LoaderError::Pregame(format!(
            "failed to restore the clean {file_name}.kfc baseline"
        )));
    }
    let files = if file_name == "enshrouded" {
        GameFiles::client(game_directory)
    } else {
        GameFiles::server(game_directory)
    };
    let game_file=game_directory.join(if file_name=="enshrouded"{"enshrouded.exe"}else{"enshrouded_server.exe"});
    let metadata=std::fs::metadata(&game_file).map_err(|error|LoaderError::Pregame(error.to_string()))?;
    let modified=metadata.modified().map_err(|error|LoaderError::Pregame(error.to_string()))?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let completed=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let schema = match KfcParser.parse(&files) {
        Ok(schema)=>{
            shroudforge_package::config::write_document(game_directory,"parser-status",&serde_json::json!({
                "status":"parsed","target":file_name,"completedAt":completed,"gameFileSize":metadata.len(),"gameFileModified":modified
            })).map_err(LoaderError::Pregame)?;
            schema
        }
        Err(error)=>{
            let detail=error.to_string();
            let _=shroudforge_package::config::write_document(game_directory,"parser-status",&serde_json::json!({
                "status":"failed","target":file_name,"completedAt":completed,"gameFileSize":metadata.len(),"gameFileModified":modified,"error":detail
            }));
            return Err(LoaderError::Pregame(detail));
        }
    };
    let api = ShroudForgeApi::new(Compatibility::default().resolve(schema));
    shroudforge_api::run(
        &environment,
        api,
        shroudforge_api::RunArgs {
            file_name: file_name.into(),
            options: shroudforge_api::RunOptions {
                // Restoring the baseline invalidates the applied-asset cache.
                skip_cache: true,
                assets_write: true,
                export: true,
                ..Default::default()
            },
        },
    )
    .map_err(|error| LoaderError::Pregame(error.to_string()))?;
    let mods = environment.plan(file_name == "enshrouded_server", shroudforge_api::API_VERSION)
        .into_iter().filter(|item| item.info().capabilities.iter().any(|capability|
            matches!(capability, shroudforge_package::Capability::AssetsWrite | shroudforge_package::Capability::Export)
        )).map(|item| {
            Ok(serde_json::json!({"id": item.info().id, "version": item.info().version}))
        }).collect::<Result<Vec<_>, String>>().map_err(LoaderError::Pregame)?;
    shroudforge_package::config::write_document(game_directory, "applied", &serde_json::json!({
        "schemaVersion": 1, "status": "applied", "phase": "prepare", "target": file_name,
        "fingerprint": fingerprint,
        "completedAt": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
        "mods": mods
    })).map_err(LoaderError::Pregame)?;
    Ok(())
}

#[cfg(windows)]
fn ensure_game_stopped() -> Result<(), LoaderError> {
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
            if name.eq_ignore_ascii_case("enshrouded.exe")
                || name.eq_ignore_ascii_case("enshrouded_server.exe")
            {
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
fn ensure_game_stopped() -> Result<(), LoaderError> {
    Ok(())
}

fn format_report(report: &shroudforge_package::ModEnvironmentErrorReport) -> String {
    if let Some(error) = &report.error {
        return error.to_string();
    }
    report
        .mods
        .iter()
        .map(|value| format!("{}: {}", value.path, value.error))
        .collect::<Vec<_>>()
        .join("; ")
}
