use crate::LoaderError;
use shroudforge_api::ShroudForgeApi;
use shroudforge_compatibility::Compatibility;
use shroudforge_parser::{GameFiles, GameParser, KfcParser};
use std::path::Path;

pub fn run(game_directory: impl AsRef<Path>) -> Result<(), LoaderError> {
    let game_directory = game_directory.as_ref();
    ensure_game_stopped()?;
    let file_name = target_name(game_directory)?;
    if already_applied(game_directory, file_name)? {
        // The prepared KFC files remain in place. A match means the requested
        // asset mods are already applied, not that they were skipped.
        tracing::info!("Prepared KFC assets match the current game and mod configuration; keeping the existing applied data");
        return Ok(());
    }
    run_inner(game_directory, file_name, "prepare")
}

/// Applies asset mods from the early in-process bootstrap. The bootstrap calls
/// this before creating the live ECS runtime, matching Shroudtopia's startup
/// activation model. The transaction publishes atomically; a direct launch
/// cannot pause the game while this bootstrap worker runs.
pub(crate) fn run_startup(game_directory: impl AsRef<Path>) -> Result<(), LoaderError> {
    let game_directory = game_directory.as_ref();
    let lock_path = game_directory.join(".shroudforge-startup-assets.lock");
    let _lease = startup_asset_lease(&lock_path)?;
    for error in shroudforge_package::migration::migrate_installation(game_directory)
        .map_err(LoaderError::Pregame)?
    {
        tracing::warn!(%error, "Installation migration failed during bootstrap startup");
    }

    let file_name = process_target_name(game_directory)?;
    if already_applied(game_directory, file_name)? {
        // Enshrouded will load the already-prepared KFC data during startup.
        tracing::info!("Startup KFC assets are already prepared for this game and mod configuration; no rewrite is needed");
        return Ok(());
    }

    let game_utf8 = game_directory.to_str().ok_or_else(|| {
        LoaderError::Environment(format!(
            "game directory is not valid UTF-8: {}",
            game_directory.display()
        ))
    })?;
    let environment = shroudforge_package::ModEnvironment::load(game_utf8)
        .map_err(|report| LoaderError::Environment(format_report(&report)))?;
    let has_startup_mods = environment
        .plan(
            file_name == "enshrouded_server",
            shroudforge_api::API_VERSION,
        )
        .iter()
        .any(|item| {
            item.info().capabilities.iter().any(|capability| {
                matches!(
                    capability,
                    shroudforge_package::Capability::AssetsWrite
                        | shroudforge_package::Capability::Export
                )
            })
        });
    let has_previous_apply = shroudforge_package::config::read_document(game_directory, "applied")
        .ok()
        .is_some_and(|value| {
            matches!(
                value["status"].as_str(),
                Some("applied" | "preparing" | "failed")
            )
        });
    if !has_startup_mods && !has_previous_apply {
        return Ok(());
    }

    tracing::info!(target: "shroudforge::startup", "Applying asset mods during early process startup");
    run_inner(game_directory, file_name, "startup")
}

fn already_applied(game_directory: &Path, file_name: &str) -> Result<bool, LoaderError> {
    let game_utf8 = game_directory.to_str().ok_or_else(|| {
        LoaderError::Environment(format!(
            "game directory is not valid UTF-8: {}",
            game_directory.display()
        ))
    })?;
    let environment = shroudforge_package::ModEnvironment::load(game_utf8)
        .map_err(|report| LoaderError::Environment(format_report(&report)))?;
    let server = file_name == "enshrouded_server";
    let has_startup_mods = environment
        .plan(server, shroudforge_api::API_VERSION)
        .iter()
        .any(|item| {
            item.info().capabilities.iter().any(|capability| {
                matches!(
                    capability,
                    shroudforge_package::Capability::AssetsWrite
                        | shroudforge_package::Capability::Export
                )
            })
        });
    let has_previous_apply = shroudforge_package::config::read_document(game_directory, "applied")
        .ok()
        .is_some_and(|value| {
            matches!(
                value["status"].as_str(),
                Some("applied" | "preparing" | "failed")
            )
        });
    if !has_startup_mods && !has_previous_apply {
        return Ok(true);
    }
    let fingerprint = shroudforge_package::prepared::fingerprint(
        &environment,
        server,
        shroudforge_api::API_VERSION,
    )
    .map_err(LoaderError::Pregame)?;
    Ok(shroudforge_package::prepared::matches(
        game_directory,
        &fingerprint,
    ))
}

fn run_inner(game_directory: &Path, file_name: &str, phase: &str) -> Result<(), LoaderError> {
    for error in shroudforge_package::migration::migrate_installation(game_directory)
        .map_err(LoaderError::Pregame)?
    {
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
    let backup = game_directory.join(format!("{file_name}.kfc.bak"));
    let fingerprint = shroudforge_package::prepared::fingerprint(
        &environment,
        file_name == "enshrouded_server",
        shroudforge_api::API_VERSION,
    )
    .map_err(LoaderError::Pregame)?;
    shroudforge_package::config::write_document(
        game_directory,
        "applied",
        &serde_json::json!({
            "schemaVersion": 1, "status": "preparing", "target": file_name
        }),
    )
    .map_err(LoaderError::Pregame)?;
    let apply_result = (|| -> Result<Vec<serde_json::Value>, LoaderError> {
        shroudforge_parser::transaction::recover(game_directory, file_name)
            .map_err(|error| LoaderError::Pregame(error.to_string()))?;
        if backup.is_file() && !shroudforge_api::restore(game_utf8, file_name) {
            return Err(LoaderError::Pregame(format!(
                "failed to restore the clean {file_name}.kfc baseline"
            )));
        }
        if phase == "startup" {
            shroudforge_api::run_with_local_schema(
                &environment,
                shroudforge_api::RunArgs {
                    file_name: file_name.into(),
                    options: shroudforge_api::RunOptions {
                        force_assets: true,
                        assets_write: true,
                        export: true,
                        phase: shroudforge_api::RuntimePhase::Pregame,
                        ..Default::default()
                    },
                },
            )
            .map_err(|error| LoaderError::Pregame(error.to_string()))?;
        } else {
            let files = if file_name == "enshrouded" {
                GameFiles::client(game_directory)
            } else {
                GameFiles::server(game_directory)
            };
            let game_file = game_directory.join(if file_name == "enshrouded" {
                "enshrouded.exe"
            } else {
                "enshrouded_server.exe"
            });
            let metadata = std::fs::metadata(&game_file)
                .map_err(|error| LoaderError::Pregame(error.to_string()))?;
            let modified = metadata
                .modified()
                .map_err(|error| LoaderError::Pregame(error.to_string()))?
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let completed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let schema = match KfcParser.parse(&files) {
                Ok(schema) => {
                    shroudforge_package::config::write_document(game_directory,"parser-status",&serde_json::json!({
                    "status":"parsed","target":file_name,"completedAt":completed,"gameFileSize":metadata.len(),"gameFileModified":modified
                })).map_err(LoaderError::Pregame)?;
                    schema
                }
                Err(error) => {
                    let detail = error.to_string();
                    let _ = shroudforge_package::config::write_document(
                        game_directory,
                        "parser-status",
                        &serde_json::json!({
                            "status":"failed","target":file_name,"completedAt":completed,"gameFileSize":metadata.len(),"gameFileModified":modified,"error":detail
                        }),
                    );
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
                        skip_cache: true,
                        force_assets: true,
                        assets_write: true,
                        export: true,
                        ..Default::default()
                    },
                },
            )
            .map_err(|error| LoaderError::Pregame(error.to_string()))?;
        }
        let mods = environment
            .plan(
                file_name == "enshrouded_server",
                shroudforge_api::API_VERSION,
            )
            .into_iter()
            .filter(|item| {
                item.info().capabilities.iter().any(|capability| {
                    matches!(
                        capability,
                        shroudforge_package::Capability::AssetsWrite
                            | shroudforge_package::Capability::Export
                    )
                })
            })
            .map(|item| {
                Ok(serde_json::json!({"id": item.info().id, "version": item.info().version}))
            })
            .collect::<Result<Vec<_>, String>>()
            .map_err(LoaderError::Pregame)?;
        Ok(mods)
    })();
    let mods = match apply_result {
        Ok(mods) => mods,
        Err(error) => {
            let _ = shroudforge_package::config::write_document(
                game_directory,
                "applied",
                &serde_json::json!({
                    "schemaVersion": 1, "status": "failed", "phase": phase, "target": file_name,
                    "completedAt": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                    "error": error.to_string()
                }),
            );
            return Err(error);
        }
    };
    shroudforge_package::config::write_document(game_directory, "applied", &serde_json::json!({
        "schemaVersion": 1, "status": "applied", "phase": phase, "target": file_name,
        "fingerprint": fingerprint,
        "completedAt": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
        "mods": mods
    })).map_err(LoaderError::Pregame)?;
    Ok(())
}

fn target_name(game_directory: &Path) -> Result<&'static str, LoaderError> {
    if game_directory.join("enshrouded.exe").is_file() {
        Ok("enshrouded")
    } else if game_directory.join("enshrouded_server.exe").is_file() {
        Ok("enshrouded_server")
    } else {
        Err(LoaderError::Environment(format!(
            "no Enshrouded executable in {}",
            game_directory.display()
        )))
    }
}

fn process_target_name(game_directory: &Path) -> Result<&'static str, LoaderError> {
    if let Ok(executable) = std::env::current_exe() {
        if executable
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("enshrouded_server.exe"))
        {
            return Ok("enshrouded_server");
        }
    }
    target_name(game_directory)
}

#[cfg(windows)]
fn startup_asset_lease(path: &Path) -> Result<std::fs::File, LoaderError> {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .share_mode(0)
        .open(path)
        .map_err(|error| {
            LoaderError::Pregame(format!(
                "asset startup is already active or locked: {error}"
            ))
        })
}

#[cfg(not(windows))]
fn startup_asset_lease(path: &Path) -> Result<std::fs::File, LoaderError> {
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| LoaderError::Pregame(error.to_string()))
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
