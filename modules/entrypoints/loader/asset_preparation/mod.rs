mod apply_asset_mods;
mod capture_original_game_files;
mod check_asset_preparation;
mod lock_startup_assets;
mod parse_game_assets;
mod prepare_game_assets;
mod prepare_startup_assets;
mod recover_failed_startup_assets;
mod require_game_stopped;
mod select_game_target;

use apply_asset_mods::run_inner;
use capture_original_game_files::ensure_original_gamefiles;
use check_asset_preparation::{already_applied, export_pass_needed};
use lock_startup_assets::startup_asset_lease;
pub use prepare_game_assets::run;
pub(crate) use prepare_startup_assets::{prepare_vanilla, run_startup};
pub(crate) use recover_failed_startup_assets::recover_failed_startup;
use require_game_stopped::ensure_game_stopped;
use select_game_target::{process_target_name, target_name};

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
mod tests;
