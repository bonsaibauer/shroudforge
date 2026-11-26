use crate::{
    API_VERSION, RunArgs, ShroudForgeApi,
    cache::{CacheDiff, FileStateCache},
    env::{
        AppFeatures, AppState,
        encode_in_strips::{EncodeThreads, encode_threads},
    },
    logging::debug,
    runner::LuaModRunner,
};
use mod_loader::ModEnvironment;

pub fn run(env: &ModEnvironment, api: ShroudForgeApi, args: RunArgs) -> anyhow::Result<()> {
    run_with_api(env, Some(api), args)
}

pub fn run_with_local_schema(env: &ModEnvironment, args: RunArgs) -> anyhow::Result<()> {
    run_with_api(env, None, args)
}

fn run_with_api(
    env: &ModEnvironment,
    api: Option<ShroudForgeApi>,
    mut args: RunArgs,
) -> anyhow::Result<()> {
    debug!("Running lua with options: {:?}", args);

    // A pregame pass encodes block-compressed textures on several threads (the same bytes), and
    // puts the count back when it ends. The strip encode never waits for a helper thread to start,
    // so a caller under the Windows loader lock only encodes serially.
    let _encode_threads = (args.options.phase == crate::RuntimePhase::Pregame).then(|| {
        let threads = EncodeThreads::for_pregame_pass();
        tracing::info!(target: "shroudforge::startup", threads = encode_threads(),
            "Block-compressed textures are encoded on {} thread(s) ({} overrides)",
            encode_threads(), crate::ENCODE_THREADS_VARIABLE);
        threads
    });

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

    let app_state = match AppState::new(env.clone(), api, args, &cache_diff) {
        Ok(context) => context,
        Err(_) => anyhow::bail!("Failed to create AppState"),
    };

    // setup and run all mods

    let runner = LuaModRunner::new(app_state)?;

    let is_server = runner.lua.app_data_ref::<AppState>().unwrap().is_server();
    match runner.setup(env.plan(is_server, API_VERSION)) {
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
