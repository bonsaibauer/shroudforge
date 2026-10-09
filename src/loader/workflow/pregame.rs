use crate::LoaderError;
use shroudforge_api::ShroudForgeApi;
use shroudforge_compatibility::Compatibility;
use shroudforge_parser::{GameFiles, GameParser, KfcParser};
use std::path::Path;

pub fn run(game_directory: impl AsRef<Path>) -> Result<(), LoaderError> {
    let game_directory = game_directory.as_ref();
    let file_name = target_name(game_directory)?;
    shroudforge_package::config::initialize_loader_config(game_directory)
        .map_err(LoaderError::Pregame)?;
    ensure_game_stopped(file_name)?;
    ensure_original_gamefiles(game_directory, file_name);
    let export_pass_needed = export_pass_needed(game_directory, file_name)?;
    if already_applied(game_directory, file_name)? && !export_pass_needed {
        // The prepared KFC files remain in place. A match means the requested
        // asset mods are already applied, not that they were skipped.
        tracing::debug!(
            "Prepared KFC assets match the current game and mod configuration; keeping the existing applied data"
        );
        return Ok(());
    }
    run_inner(game_directory, file_name, "prepare")
}

/// Applies asset mods from the early in-process bootstrap. The bootstrap calls
/// this before creating the live ECS runtime, matching Shroudtopia's startup
/// activation model. The bootstrap holds the executable entrypoint until this
/// pass completes, so the game cannot read the baseline or half-published assets.
pub(crate) fn run_startup(game_directory: impl AsRef<Path>) -> Result<(), LoaderError> {
    let game_directory = game_directory.as_ref();
    let file_name = process_target_name(game_directory)?;
    shroudforge_package::config::initialize_loader_config(game_directory)
        .map_err(LoaderError::Pregame)?;
    // Capture a first-install original before the early return for installations
    // that do not have asset mods. Uncertain legacy installs remain unclassified.
    ensure_original_gamefiles(game_directory, file_name);
    let lock_path = shroudforge_package::paths::startup_asset_lock(game_directory);
    let _lease = startup_asset_lease(&lock_path)?;
    let export_pass_needed = export_pass_needed(game_directory, file_name)?;
    if already_applied(game_directory, file_name)? && !export_pass_needed {
        // Enshrouded will load the already-prepared KFC data during startup.
        tracing::debug!(
            "Startup KFC assets are already prepared for this game and mod configuration; no rewrite is needed"
        );
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
    let (startup_plan, plan_issues) = environment.plan_report_detailed(
        file_name == "enshrouded_server",
        shroudforge_api::API_VERSION,
    );
    let has_startup_mods = startup_plan
        .iter()
        .any(|item| item.info().requires_pregame());
    let has_previous_apply = shroudforge_package::config::read_document(game_directory, "applied")
        .ok()
        .is_some_and(|value| {
            matches!(
                value["status"].as_str(),
                Some("applied" | "preparing" | "failed")
            )
        });
    if !has_startup_mods && !has_previous_apply {
        for issue in plan_issues {
            tracing::warn!(target: "shroudforge::startup", mod_id = %issue.mod_id,
                code = %issue.code, detail = %issue.detail, "Startup mod skipped by dependency or compatibility planning");
        }
        return Ok(());
    }

    tracing::debug!(target: "shroudforge::startup", "Applying asset mods during early process startup");
    run_inner(game_directory, file_name, "startup")
}

fn ensure_original_gamefiles(game_directory: &Path, file_name: &str) {
    match shroudforge_package::backups::ensure_originals(game_directory, file_name) {
        Ok(status) => tracing::info!(target: "shroudforge::backup", target = file_name,
            state = %status["status"], "Original game-file backup status checked"),
        Err(error) => tracing::warn!(target: "shroudforge::backup", target = file_name,
            %error, "Could not capture original game files"),
    }
}

/// A broken asset mod must not prevent playing with the saved baseline. Return
/// true only after both files are restored; the entrypoint remains parked here.
pub(crate) fn recover_failed_startup(game_directory: &Path, detail: &str) -> bool {
    let result = (|| -> Result<(), LoaderError> {
        let target = process_target_name(game_directory)?;
        let _lease = startup_asset_lease(&shroudforge_package::paths::startup_asset_lock(
            game_directory,
        ))?;
        shroudforge_package::config::write_document(game_directory, "applied", &serde_json::json!({
            "schemaVersion": 1, "status": "failed", "phase": "startup", "target": target,
            "completedAt": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
            "error": detail
        })).map_err(LoaderError::Pregame)?;
        shroudforge_parser::transaction::recover(game_directory, target)
            .map_err(|error| LoaderError::Pregame(error.to_string()))?;
        for extension in ["kfc.bak", "kfc_resources.bak"] {
            if !game_directory
                .join(format!("{target}.{extension}"))
                .is_file()
            {
                return Err(LoaderError::Pregame(format!(
                    "missing {target}.{extension}; cannot restore asset baseline"
                )));
            }
        }
        let game_utf8 = game_directory
            .to_str()
            .ok_or_else(|| LoaderError::Environment("game directory is not valid UTF-8".into()))?;
        if !shroudforge_api::restore(game_utf8, target) {
            return Err(LoaderError::Pregame(
                "could not restore both baseline asset files".into(),
            ));
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            let _ = shroudforge_package::logging::append(
                game_directory,
                'W',
                "startup-assets",
                "Asset mods failed; saved baseline restored. Continuing game startup without asset patches.",
            );
            true
        }
        Err(error) => {
            let _ = shroudforge_package::logging::append(
                game_directory,
                'E',
                "startup-assets",
                &format!("Asset baseline recovery failed: {error}"),
            );
            false
        }
    }
}

fn export_pass_needed(game_directory: &Path, file_name: &str) -> Result<bool, LoaderError> {
    let config =
        shroudforge_package::config::read_loader(game_directory).map_err(LoaderError::Pregame)?;
    if !config
        .pointer("/exports/enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(false);
    }
    let root = game_directory.to_str().ok_or_else(|| {
        LoaderError::Environment(format!(
            "game directory is not valid UTF-8: {}",
            game_directory.display()
        ))
    })?;
    let environment = shroudforge_package::ModEnvironment::load(root)
        .map_err(|report| LoaderError::Environment(format_report(&report)))?;
    Ok(environment
        .plan(
            file_name == "enshrouded_server",
            shroudforge_api::API_VERSION,
        )
        .iter()
        .any(|item| {
            item.info().requires_pregame()
                && item
                    .info()
                    .capabilities
                    .contains(&shroudforge_package::Capability::Export)
        }))
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
        .any(|item| item.info().requires_pregame());
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
    let loader_config =
        shroudforge_package::config::read_loader(game_directory).map_err(LoaderError::Pregame)?;
    let export_enabled = loader_config
        .pointer("/exports/enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let game_utf8 = game_directory.to_str().ok_or_else(|| {
        LoaderError::Environment(format!(
            "game directory is not valid UTF-8: {}",
            game_directory.display()
        ))
    })?;
    let environment = shroudforge_package::ModEnvironment::load(game_utf8)
        .map_err(|report| LoaderError::Environment(format_report(&report)))?;
    let (_, plan_issues) = environment.plan_report_detailed(
        file_name == "enshrouded_server",
        shroudforge_api::API_VERSION,
    );
    for issue in plan_issues {
        tracing::warn!(target: "shroudforge::startup", mod_id = %issue.mod_id,
            code = %issue.code, detail = %issue.detail, "Mod skipped during startup preparation");
    }
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
        let resources = game_directory.join(format!("{file_name}.kfc_resources"));
        let resources = resources.to_str().ok_or_else(|| {
            LoaderError::Environment("companion resource file path is not valid UTF-8".into())
        })?;
        if !shroudforge_api::backup_companion_file(resources) {
            return Err(LoaderError::Pregame(format!(
                "failed to back up the clean {}.kfc_resources baseline",
                file_name
            )));
        }
        if phase == "startup" {
            shroudforge_api::run_with_local_schema(
                &environment,
                shroudforge_api::RunArgs {
                    file_name: file_name.into(),
                    options: shroudforge_api::RunOptions {
                        force_patch: true,
                        patch: true,
                        export: export_enabled,
                        export_dir: None,
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
                        force_patch: true,
                        patch: true,
                        export: export_enabled,
                        export_dir: None,
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
            .filter(|item| item.info().requires_pregame())
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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| LoaderError::Pregame(error.to_string()))?;
    }
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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| LoaderError::Pregame(error.to_string()))?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| LoaderError::Pregame(error.to_string()))
}

#[cfg(windows)]
fn ensure_game_stopped(file_name: &str) -> Result<(), LoaderError> {
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
fn ensure_game_stopped(_file_name: &str) -> Result<(), LoaderError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires SF_BLUEPRINT_GAME_SOURCE; runs only on a private copy of the game assets"]
    fn blueprint_startup_matches_legacy_and_recovers_from_mod_failure() {
        fn recipe_bytes(root: &Path, stem: &str) -> Vec<u8> {
            use kfc::{
                container::{KFCFile, KFCReader},
                reflection::TypeRegistry,
            };
            let registry =
                TypeRegistry::load_from_executable(root.join(format!("{stem}.exe"))).unwrap();
            let file = KFCFile::from_path(root.join(format!("{stem}.kfc")), false).unwrap();
            let ids = shroudforge_parser::kfc_format::resources_by_type(
                &file,
                &registry,
                "keen::RecipeRegistryResource",
            );
            assert_eq!(ids.len(), 1);
            let mut reader = KFCReader::new(root, stem).unwrap().into_cursor().unwrap();
            let mut bytes = Vec::new();
            assert!(reader.read_resource_into(&ids[0], &mut bytes).unwrap());
            bytes
        }
        let source = std::path::PathBuf::from(std::env::var("SF_BLUEPRINT_GAME_SOURCE").unwrap());
        let stem = if source.join("enshrouded_server.exe").is_file() {
            "enshrouded_server"
        } else {
            "enshrouded"
        };
        let root = std::env::temp_dir().join(format!(
            "sf-blueprint-startup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        std::fs::create_dir_all(&root).unwrap();
        let _cleanup = Cleanup(root.clone());
        std::fs::copy(
            source.join(format!("{stem}.exe")),
            root.join(format!("{stem}.exe")),
        )
        .unwrap();
        for extension in ["kfc", "kfc_resources"] {
            let baseline = source.join(format!("{stem}.{extension}.bak"));
            std::fs::copy(&baseline, root.join(format!("{stem}.{extension}"))).unwrap();
            std::fs::copy(&baseline, root.join(format!("{stem}.{extension}.bak"))).unwrap();
        }
        let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../mods/sf-unlock-blueprints");
        let destination = root.join("mods/sf-unlock-blueprints");
        std::fs::create_dir_all(destination.join("src")).unwrap();
        for file in ["mod.json", "src/mod.lua"] {
            std::fs::copy(package.join(file), destination.join(file)).unwrap();
        }
        std::fs::write(
            destination.join("extended.mod.json"),
            r#"{"schemaVersion":1,"enabled":true,"targets":["client","server"]}"#,
        )
        .unwrap();
        let runtime = root.join("mods/runtime-export");
        std::fs::create_dir_all(runtime.join("src")).unwrap();
        std::fs::write(runtime.join("mod.json"),
            r#"{"id":"runtime-export","name":"Runtime export","version":"1.0.0","capabilities":["runtime","export"]}"#).unwrap();
        std::fs::write(
            runtime.join("extended.mod.json"),
            r#"{"schemaVersion":1,"enabled":true,"targets":["client","server"]}"#,
        )
        .unwrap();
        std::fs::write(
            runtime.join("src/mod.lua"),
            "error('runtime export must not run during preparation')",
        )
        .unwrap();
        let baseline = recipe_bytes(&root, stem);
        run_startup(&root).unwrap();
        let patched = recipe_bytes(&root, stem);
        assert!(
            patched != baseline,
            "blueprint preparation did not change recipe data"
        );

        // Compare real serialized output against the pre-2a8f8ac fixed-ID logic.
        let mut legacy = std::fs::read_to_string(destination.join("src/mod.lua")).unwrap();
        let start = legacy
            .find("-- Use the engine's named query/action relation.")
            .unwrap();
        let end = legacy.find("local changed_recipes").unwrap();
        legacy.replace_range(start..end, "local knowledge_id = 1715248921\n\n");
        std::fs::write(destination.join("src/mod.lua"), legacy).unwrap();
        run_startup(&root).unwrap();
        // Container table order can change on serialization; compare the actual
        // recipe resource bytes, independent of container bookkeeping.
        assert!(
            patched == recipe_bytes(&root, stem),
            "current and historical blueprint recipe patches differ"
        );

        std::fs::write(
            destination.join("src/mod.lua"),
            "error('intentional broken asset mod')",
        )
        .unwrap();
        let failure = run_startup(&root).unwrap_err();
        assert!(failure.to_string().contains("intentional broken asset mod"));
        assert!(recover_failed_startup(&root, &failure.to_string()));
        for extension in ["kfc", "kfc_resources"] {
            assert!(
                std::fs::read(root.join(format!("{stem}.{extension}"))).unwrap()
                    == std::fs::read(root.join(format!("{stem}.{extension}.bak"))).unwrap(),
                "baseline not restored: {extension}"
            );
        }
        let status = shroudforge_package::config::read_document(&root, "applied").unwrap();
        assert_eq!(status["status"], "failed");
        assert!(
            status["error"]
                .as_str()
                .unwrap()
                .contains("intentional broken asset mod")
        );
        std::fs::remove_file(root.join(format!("{stem}.kfc_resources.bak"))).unwrap();
        assert!(!recover_failed_startup(&root, "missing companion backup"));
    }

    #[test]
    fn runtime_exports_do_not_restore_prepared_assets_during_startup() {
        let root = std::env::temp_dir().join(format!(
            "sf-startup-export-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        for (id, capabilities) in [
            ("production", serde_json::json!(["patch"])),
            ("editor", serde_json::json!(["runtime", "export"])),
        ] {
            let package = root.join("mods").join(id);
            std::fs::create_dir_all(package.join("src")).unwrap();
            std::fs::write(
                package.join("mod.json"),
                serde_json::to_vec(&serde_json::json!({
                    "id": id, "name": id, "version": "1.0.0", "capabilities": capabilities
                }))
                .unwrap(),
            )
            .unwrap();
            std::fs::write(
                package.join("extended.mod.json"),
                r#"{"schemaVersion":1,"enabled":true,"targets":["client","server"]}"#,
            )
            .unwrap();
            std::fs::write(
                package.join("src/mod.lua"),
                "error('prepared assets must not be reapplied')",
            )
            .unwrap();
        }
        std::fs::write(root.join("enshrouded.exe"), b"test executable identity").unwrap();
        std::fs::write(root.join("enshrouded.kfc"), b"already prepared recipe data").unwrap();
        std::fs::write(root.join("enshrouded.kfc.bak"), b"original recipe data").unwrap();
        shroudforge_package::config::initialize_loader_config(&root).unwrap();
        let environment =
            shroudforge_package::ModEnvironment::load(root.to_str().unwrap()).unwrap();
        assert_eq!(
            environment.plan(false, shroudforge_api::API_VERSION).len(),
            2
        );
        let fingerprint = shroudforge_package::prepared::fingerprint(
            &environment,
            false,
            shroudforge_api::API_VERSION,
        )
        .unwrap();
        shroudforge_package::config::write_document(&root, "applied", &serde_json::json!({
            "schemaVersion": 1, "status": "applied", "target": "enshrouded", "fingerprint": fingerprint,
            "phase": "prepare", "completedAt": 1, "mods": [{"id":"production","version":"1.0.0"}]
        })).unwrap();
        assert!(!export_pass_needed(&root, "enshrouded").unwrap());
        run_startup(&root).unwrap();
        assert_eq!(
            std::fs::read(root.join("enshrouded.kfc")).unwrap(),
            b"already prepared recipe data"
        );
        // A real pregame exporter must still request its export pass.
        std::fs::write(
            root.join("mods/editor/mod.json"),
            r#"{"id":"editor","name":"editor","version":"1.0.0","capabilities":["export"]}"#,
        )
        .unwrap();
        assert!(export_pass_needed(&root, "enshrouded").unwrap());
    }
}
