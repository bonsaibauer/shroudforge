use super::{
    already_applied, ensure_game_stopped, ensure_original_gamefiles, export_pass_needed, run_inner,
    target_name,
};
use crate::LoaderError;
use std::path::Path;

pub fn run(game_directory: impl AsRef<Path>) -> Result<(), LoaderError> {
    let game_directory = game_directory.as_ref();
    let file_name = target_name(game_directory)?;
    shroudforge_loader_config::config::initialize_loader_config(game_directory)
        .map_err(LoaderError::Pregame)?;
    ensure_game_stopped(file_name)?;
    ensure_original_gamefiles(game_directory, file_name);
    let export_pass_needed = export_pass_needed(game_directory, file_name)?;
    if already_applied(game_directory, file_name, export_pass_needed)? {
        // The prepared KFC files remain in place. A match means the requested
        // asset mods are already applied, not that they were skipped.
        tracing::debug!(
            "Prepared KFC assets match the current game and mod configuration; keeping the existing applied data"
        );
        return Ok(());
    }
    run_inner(game_directory, file_name, "prepare")
}
