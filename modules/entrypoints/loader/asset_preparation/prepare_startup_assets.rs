use super::{
    already_applied, ensure_original_gamefiles, export_pass_needed, format_report,
    process_target_name, run_inner, startup_asset_lease,
};
use crate::LoaderError;
use std::path::Path;

pub(crate) fn prepare_vanilla(game_directory: &Path) -> Result<(), LoaderError> {
    let target = process_target_name(game_directory)?;
    super::require_game_stopped::ensure_no_other_game_process(target, game_directory)?;
    let other_target = if target == "enshrouded" {
        "enshrouded_server"
    } else {
        "enshrouded"
    };
    super::require_game_stopped::ensure_no_other_game_process(other_target, game_directory)?;
    #[cfg(windows)]
    let _update_lease =
        shroudforge_loader_config::update_queue_lock::acquire_queue_worker_mutex(game_directory)
            .map_err(LoaderError::Pregame)?
            .ok_or_else(|| {
                LoaderError::Pregame("An update or game-file restoration is still running".into())
            })?;
    let _lease = startup_asset_lease(&shroudforge_loader_config::paths::startup_asset_lock(
        game_directory,
    ))?;
    shroudforge_game_assets::transaction::recover(game_directory, target)
        .map_err(|error| LoaderError::Pregame(error.to_string()))?;
    let original = shroudforge_game_assets::originals::ensure_originals(game_directory, target)
        .map_err(LoaderError::Pregame)?;
    if original["status"] != "ready" {
        return Err(LoaderError::Pregame(
            "Verified original game files are unavailable for this build".into(),
        ));
    }
    // A previous fingerprint must never describe the restored original files.
    shroudforge_loader_config::config::write_document(
        game_directory,
        shroudforge_game_assets::documents::APPLIED,
        &serde_json::json!({"schemaVersion": 1, "status": "preparing", "target": target}),
    )
    .map_err(LoaderError::Pregame)?;
    shroudforge_game_assets::originals::restore_originals_for_startup(game_directory, target)
        .map_err(LoaderError::Pregame)
}

pub(crate) fn run_startup(game_directory: impl AsRef<Path>) -> Result<(), LoaderError> {
    let game_directory = game_directory.as_ref();
    let file_name = process_target_name(game_directory)?;
    shroudforge_loader_config::config::initialize_loader_config(game_directory)
        .map_err(LoaderError::Pregame)?;
    // Capture a first-install original before the early return for installations
    // that do not have asset mods. Uncertain legacy installs remain unclassified.
    ensure_original_gamefiles(game_directory, file_name);
    let lock_path = shroudforge_loader_config::paths::startup_asset_lock(game_directory);
    let _lease = startup_asset_lease(&lock_path)?;
    let export_pass_needed = export_pass_needed(game_directory, file_name)?;
    if already_applied(game_directory, file_name, export_pass_needed)? {
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
    let has_previous_apply = shroudforge_loader_config::config::read_document(
        game_directory,
        shroudforge_game_assets::documents::APPLIED,
    )
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
