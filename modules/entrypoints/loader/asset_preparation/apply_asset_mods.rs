use super::format_report;
use crate::LoaderError;
use std::path::Path;

pub(super) fn run_inner(
    game_directory: &Path,
    file_name: &str,
    phase: &str,
) -> Result<(), LoaderError> {
    let loader_config = shroudforge_loader_config::config::read_loader(game_directory)
        .map_err(LoaderError::Pregame)?;
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
    // The record of the last pass goes before the files it describes change.
    shroudforge_game_assets::prepared_stamp::forget(game_directory, file_name)
        .map_err(LoaderError::Pregame)?;
    shroudforge_loader_config::config::write_document(
        game_directory,
        shroudforge_game_assets::documents::APPLIED,
        &serde_json::json!({
            "schemaVersion": 1, "status": "preparing", "target": file_name
        }),
    )
    .map_err(LoaderError::Pregame)?;
    // What the pass's mods use in the export folder, for the record of the pass.
    let export_touches = shroudforge_game_assets::prepared_stamp::ExportTouches::default();
    let apply_result = (|| -> Result<Vec<serde_json::Value>, LoaderError> {
        shroudforge_game_assets::transaction::recover(game_directory, file_name)
            .map_err(|error| LoaderError::Pregame(error.to_string()))?;
        let resources = game_directory.join(format!("{file_name}.kfc_resources"));
        if shroudforge_api::working_baseline_is_incompatible(game_utf8, file_name) {
            crate::recycle_incompatible_game_asset_backups(game_directory, file_name)
                .map_err(LoaderError::Pregame)?;
            shroudforge_api::create_working_kfc_backup(game_utf8, file_name).map_err(|()| {
                LoaderError::Pregame(format!(
                    "could not create a new {file_name}.kfc working baseline"
                ))
            })?;
            let resources_utf8 = resources.to_str().ok_or_else(|| {
                LoaderError::Environment("companion resource file path is not valid UTF-8".into())
            })?;
            if !shroudforge_api::backup_companion_file(resources_utf8) {
                return Err(LoaderError::Pregame(format!(
                    "could not create a new {file_name}.kfc_resources working baseline"
                )));
            }
            tracing::warn!(target: "shroudforge::startup", target = file_name,
                "Incompatible working asset backups were moved to the recycle bin and recaptured from the current game files");
        } else if backup.is_file() && !shroudforge_api::restore(game_utf8, file_name) {
            return Err(LoaderError::Pregame(format!(
                "failed to restore the clean {file_name}.kfc baseline"
            )));
        }
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
                        export_touches: Some(export_touches.clone()),
                        phase: shroudforge_api::RuntimePhase::Pregame,
                        ..Default::default()
                    },
                },
            )
            .map_err(|error| LoaderError::Pregame(error.to_string()))?;
        } else {
            let api = super::parse_game_assets::parse_game_assets(game_directory, file_name)?;
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
                        export_touches: Some(export_touches.clone()),
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
            let _ = shroudforge_loader_config::config::write_document(
                game_directory,
                shroudforge_game_assets::documents::APPLIED,
                &serde_json::json!({
                    "schemaVersion": 1, "status": "failed", "phase": phase, "target": file_name,
                    "completedAt": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                    "error": error.to_string()
                }),
            );
            return Err(error);
        }
    };
    // What this pass left, so that the next start can skip an unchanged pass. Without the record
    // the next start applies again. While exports are on, every pregame package can use the export
    // API (it checks the switch, not the package's `export` capability), so what the pass used
    // there is recorded with or without a pregame exporter.
    let exports = shroudforge_loader_config::paths::export_dir(game_directory);
    if let Err(error) = shroudforge_game_assets::prepared_stamp::record(
        game_directory,
        file_name,
        &shroudforge_game_assets::prepared_stamp::PassInputs {
            fingerprint: &fingerprint,
            exports_enabled: export_enabled,
            exporters: export_enabled.then_some(exports.as_path()),
        },
        &export_touches,
    ) {
        tracing::warn!(target: "shroudforge::startup", target = file_name, %error,
            "Could not record the asset pass; the next start applies the asset mods again");
    }
    shroudforge_loader_config::config::write_document(game_directory, shroudforge_game_assets::documents::APPLIED, &serde_json::json!({
        "schemaVersion": 1, "status": "applied", "phase": phase, "target": file_name,
        "fingerprint": fingerprint,
        "completedAt": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
        "mods": mods
    })).map_err(LoaderError::Pregame)?;
    Ok(())
}
