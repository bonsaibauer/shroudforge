use crate::{
    API_VERSION, RunArgs, ShroudForgeApi,
    cache::{CacheDiff, FileStateCache},
    env::{AppFeatures, AppState},
    logging::debug,
    runner::LuaModRunner,
};
use mod_loader::ModEnvironment;
use std::path::{Path, PathBuf};

pub fn run(env: &ModEnvironment, api: ShroudForgeApi, args: RunArgs) -> anyhow::Result<()> {
    run_with_api(env, Some(api), args)
}

pub fn run_with_local_schema(env: &ModEnvironment, args: RunArgs) -> anyhow::Result<()> {
    run_with_api(env, None, args)
}

/// Run a selected pregame patch plan against a separate asset input root.
///
/// The caller owns the startup lease, input snapshot, and generated output
/// validation. The returned stage is private and cannot be published by the
/// ordinary asset transaction entrypoints.
pub fn run_prepared(
    env: &ModEnvironment,
    api: ShroudForgeApi,
    args: RunArgs,
    asset_input_root: &Path,
    output_root: &Path,
) -> anyhow::Result<PathBuf> {
    run_prepared_with_api(env, Some(api), args, asset_input_root, output_root)
}

/// Local-schema form of [`run_prepared`].
pub fn run_prepared_with_local_schema(
    env: &ModEnvironment,
    args: RunArgs,
    asset_input_root: &Path,
    output_root: &Path,
) -> anyhow::Result<PathBuf> {
    run_prepared_with_api(env, None, args, asset_input_root, output_root)
}

fn run_prepared_with_api(
    env: &ModEnvironment,
    api: Option<ShroudForgeApi>,
    mut args: RunArgs,
    asset_input_root: &Path,
    output_root: &Path,
) -> anyhow::Result<PathBuf> {
    validate_prepared_options(&args)?;
    let stem = args.file_name.clone();
    args.options.skip_cache = true;
    let (asset_input_root, output_root) =
        shroudforge_game_assets::transaction::validate_prepared_roots(
            env.game_dir().as_std_path(),
            asset_input_root,
            output_root,
            &args.file_name,
        )?;
    run_with_api_and_output(env, api, args, Some((&asset_input_root, &output_root)))?;
    Ok(shroudforge_game_assets::transaction::directory(
        &output_root,
        &stem,
    ))
}

fn validate_prepared_options(args: &RunArgs) -> anyhow::Result<()> {
    let selected = args.options.current_asset_mods.as_ref();
    anyhow::ensure!(
        selected.is_some_and(|ids| !ids.is_empty()),
        "prepared generation requires a selected pregame asset mod set"
    );
    anyhow::ensure!(
        args.options.phase == crate::RuntimePhase::Pregame && args.options.patch,
        "prepared generation requires writing pregame patching"
    );
    anyhow::ensure!(
        args.options.defer_current_commit,
        "prepared generation requires deferred asset publication"
    );
    anyhow::ensure!(
        !args.options.export && args.options.export_dir.is_none(),
        "prepared generation requires exports disabled and no export directory"
    );
    Ok(())
}

fn run_with_api(
    env: &ModEnvironment,
    api: Option<ShroudForgeApi>,
    args: RunArgs,
) -> anyhow::Result<()> {
    run_with_api_and_output(env, api, args, None)
}

fn run_with_api_and_output(
    env: &ModEnvironment,
    api: Option<ShroudForgeApi>,
    mut args: RunArgs,
    prepared_roots: Option<(&Path, &Path)>,
) -> anyhow::Result<()> {
    debug!("Running lua with options: {:?}", args);
    if prepared_roots.is_some() {
        validate_prepared_options(&args)?;
        args.options.skip_cache = true;
    }
    let selected = args.options.current_asset_mods.clone();
    let selective_generation = selected.is_some();
    if let Some(ids) = &selected {
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        anyhow::ensure!(
            !ids.is_empty() && unique.len() == ids.len(),
            "empty or duplicate current asset mod selection"
        );
        anyhow::ensure!(
            args.options.phase == crate::RuntimePhase::Pregame && args.options.patch,
            "current asset selection requires a writing pregame session"
        );
        // This mode is invoked only after the caller's exact prepared-state check.
        args.options.force_patch = true;
    }

    // check cache if files have changed

    let cache_diff = if !args.options.skip_cache {
        let current_cache = FileStateCache::read(env.cache_dir());
        let mut new_cache = FileStateCache::default();

        new_cache.track_game_files(env.game_dir());
        new_cache.track_mod_files(env.mods_dir());
        new_cache.track_mod_config(env.game_dir());

        let cache_diff = new_cache.diff(&current_cache);

        if cache_diff.is_none() && args.options.patch && !args.options.force_patch {
            args.options.patch = false;

            debug!("No changes detected, skipping asset writes");
        }

        cache_diff
    } else {
        debug!("Skipping cache check");
        CacheDiff::new_dirty()
    };

    // create a new app state

    let created_app_state = match prepared_roots {
        Some((asset_input_root, output_root)) => AppState::new_prepared(
            env.clone(),
            api,
            args,
            &cache_diff,
            asset_input_root,
            output_root,
        ),
        None => AppState::new(env.clone(), api, args, &cache_diff),
    };
    let app_state = match created_app_state {
        Ok(context) => context,
        Err(_) => anyhow::bail!("Failed to create AppState"),
    };

    // setup and run all mods

    let runner = LuaModRunner::new(app_state)?;

    let is_server = runner.lua.app_data_ref::<AppState>().unwrap().is_server();
    let mut plan = env.plan(is_server, API_VERSION);
    if let Some(ids) = selected {
        anyhow::ensure!(
            ids.iter().all(|id| plan
                .iter()
                .any(|m| &m.info().id == id && m.info().requires_pregame())),
            "current asset selection contains an unavailable pregame mod"
        );
        plan.retain(|m| ids.contains(&m.info().id));
    }
    match runner.setup(plan) {
        Ok(_) => {}
        Err(err) => {
            anyhow::bail!("Failed to setup LuaModRunner: {}", err);
        }
    }

    debug!("Running mods...");

    // TODO: move this somewhere else, maybe to the GameContext?

    runner.run()?;

    let app_state = runner.lua.app_data_ref::<AppState>().unwrap();

    super::commit_asset_changes::commit_asset_changes(&runner.lua, &app_state)?;

    // A partial plan cannot acknowledge inputs belonging to mods it did not run.
    // Its caller owns a separately scoped prepared-state record; leave the
    // ordinary all-mod applied cache byte-for-byte unchanged.
    if selective_generation {
        return Ok(());
    }

    // create a new cache file

    if app_state.has_feature(AppFeatures::PATCH) {
        let mut new_cache = FileStateCache::default();

        new_cache.track_game_files(env.game_dir());
        new_cache.track_mod_files(env.mods_dir());
        new_cache.track_mod_config(env.game_dir());

        new_cache.write(env.cache_dir());
    } else {
        let mut new_cache = FileStateCache::read(env.cache_dir());

        new_cache.track_game_files(env.game_dir());
        new_cache.track_mod_config(env.game_dir());
        new_cache.write(env.cache_dir());
    }

    Ok(())
}
