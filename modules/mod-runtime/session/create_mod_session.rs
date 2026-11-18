use super::*;
impl AppState {
    pub fn new(
        env: ModEnvironment,
        api: Option<ShroudForgeApi>,
        args: RunArgs,
        cache_diff: &CacheDiff,
    ) -> Result<Self, ()> {
        Self::new_with_prepared_roots(env, api, args, cache_diff, None)
    }

    pub(crate) fn new_prepared(
        env: ModEnvironment,
        api: Option<ShroudForgeApi>,
        args: RunArgs,
        cache_diff: &CacheDiff,
        asset_input_root: &std::path::Path,
        output_root: &std::path::Path,
    ) -> Result<Self, ()> {
        Self::new_with_prepared_roots(
            env,
            api,
            args,
            cache_diff,
            Some((asset_input_root, output_root)),
        )
    }

    fn new_with_prepared_roots(
        env: ModEnvironment,
        api: Option<ShroudForgeApi>,
        args: RunArgs,
        cache_diff: &CacheDiff,
        prepared_roots: Option<(&std::path::Path, &std::path::Path)>,
    ) -> Result<Self, ()> {
        let game_dir = env.game_dir();
        let cache_dir = env.cache_dir();
        let file_name = &args.file_name;

        // prepare configuration

        let options = args.options;
        let generated_assets = options.current_asset_mods.is_some();
        let prepared_asset_output = prepared_roots.is_some();
        if options.defer_current_commit && !generated_assets {
            error!("Deferred publication requires current-composition mode");
            return Err(());
        }
        if generated_assets && (options.phase != RuntimePhase::Pregame || !options.patch) {
            error!("Current-composition asset generation requires a writing pregame session");
            return Err(());
        }
        if prepared_asset_output
            && (options
                .current_asset_mods
                .as_ref()
                .is_none_or(Vec::is_empty)
                || !options.defer_current_commit
                || !options.skip_cache
                || options.export
                || options.export_dir.is_some())
        {
            error!(
                "Prepared asset generation requires a selected deferred patch plan without exports"
            );
            return Err(());
        }
        let environment_root = game_dir.as_std_path();
        let (asset_game_dir, asset_input_root, asset_output_root) = match prepared_roots {
            Some((source_root, output_root)) => {
                let (source_root, output_root) =
                    match shroudforge_game_assets::transaction::validate_prepared_roots(
                        environment_root,
                        source_root,
                        output_root,
                        file_name,
                    ) {
                        Ok(paths) => paths,
                        Err(error) => {
                            error!(%error, "Invalid prepared asset roots");
                            return Err(());
                        }
                    };
                let asset_game_dir = PathBuf::from_path_buf(source_root.clone()).map_err(|_| {
                    error!("Prepared asset input path is not valid UTF-8");
                })?;
                (asset_game_dir, source_root, output_root)
            }
            None => (
                game_dir.to_owned(),
                environment_root.to_path_buf(),
                environment_root.to_path_buf(),
            ),
        };
        let default_export_dir = PathBuf::from_path_buf(
            shroudforge_loader_config::paths::export_dir(environment_root),
        )
        .expect("fixed loader paths remain UTF-8");
        let configured_export_dir = options.export_dir.and_then(|directory| {
            use std::path::Component;
            if directory.is_absolute()
                || directory.components().any(|component| {
                    !matches!(component, Component::Normal(_) | Component::CurDir)
                })
            {
                warn!(
                    path = %directory.display(),
                    "Export directory must be a relative path inside the game directory"
                );
                return None;
            }
            let directory = environment_root.join(directory);
            if let Err(error) = std::fs::create_dir_all(&directory) {
                warn!(error = %error, path = %directory.display(), "Failed to create export directory");
                return None;
            }
            let canonical_root = match std::fs::canonicalize(environment_root) {
                Ok(path) => path,
                Err(error) => {
                    warn!(error = %error, "Failed to resolve game directory for export path validation");
                    return None;
                }
            };
            let canonical_directory = match std::fs::canonicalize(&directory) {
                Ok(path) => path,
                Err(error) => {
                    warn!(error = %error, "Failed to resolve export directory");
                    return None;
                }
            };
            if !canonical_directory.starts_with(&canonical_root) {
                warn!(path = %directory.display(), "Export directory resolves outside the game directory");
                return None;
            }
            PathBuf::from_path_buf(canonical_directory).ok()
        });
        let export_dir = configured_export_dir.unwrap_or(default_export_dir);

        let phase = options.phase;
        let is_server = options
            .is_server
            .unwrap_or_else(|| file_name.to_lowercase().contains("server"));

        let mut feature_flags = AppFeatures::empty();

        if options.patch {
            feature_flags |= AppFeatures::PATCH;
        }

        if options.export {
            feature_flags |= AppFeatures::EXPORT;
        }

        #[cfg(windows)]
        if phase == RuntimePhase::Ingame
            && crate::native_dlls::manager_enabled(environment_root).unwrap_or(false)
        {
            feature_flags |= AppFeatures::RUNTIME_DLL;
        }

        let config = AppConfig {
            export_dir,

            is_server,
            phase,

            feature_flags,
        };

        // load and attach to resources

        // The prepared caller supplies cache_dir from its disposable mod
        // environment, while asset_game_dir remains the separate read-only input.
        let (type_registry, is_dirty) = crate::reflection::load_type_registry::load_type_registry(
            &asset_game_dir,
            cache_dir,
            file_name,
        )?;
        let type_registry = Rc::new(type_registry);
        if phase == RuntimePhase::Pregame && !generated_assets {
            crate::lua::definitions::export_lua_definitions::export_to_cache(
                cache_dir,
                &type_registry,
                options.skip_cache || is_dirty || cache_diff.build_id_changed(),
            );
        }

        let kfc_path = asset_game_dir.join(file_name).with_extension("kfc");
        if generated_assets
            && shroudforge_game_assets::transaction::directory(&asset_input_root, file_name)
                .exists()
        {
            error!(
                "Resolve the existing asset transaction under the startup lease before reading the current composition"
            );
            return Err(());
        }
        let (ref_file, reader) = if phase == RuntimePhase::Ingame || generated_assets {
            (
                crate::game_files::load_kfc_file(&kfc_path)?,
                crate::game_files::create_reader_with_extension(&asset_game_dir, file_name, "kfc")?,
            )
        } else {
            let bak_path = crate::startup::manage_asset_baseline::create_backup(&kfc_path)?;
            (
                crate::game_files::load_kfc_file(&bak_path)?,
                crate::game_files::create_reader(&asset_game_dir, file_name)?,
            )
        };
        let api = match api {
            Some(api) => api,
            None => {
                let operations =
                    crate::runtime_operations(game_dir.as_std_path()).map_err(|error| {
                        error!(%error, "Invalid installed API contract");
                    })?;
                let schema = kfc::schema_from_registry(&type_registry, &ref_file);
                ShroudForgeApi::new(
                    shroudforge_compatibility::Compatibility::new(operations).resolve(schema),
                )
            }
        };
        let writer = if options.patch {
            let stage = if prepared_asset_output {
                shroudforge_game_assets::transaction::begin_generated_prepared(
                    &asset_input_root,
                    &asset_output_root,
                    file_name,
                )
            } else if generated_assets {
                shroudforge_game_assets::transaction::begin_generated(environment_root, file_name)
            } else {
                shroudforge_game_assets::transaction::begin(environment_root, file_name)
            };
            let stage = stage.map_err(|error| {
                error!(error = %error, "Failed to begin typed asset transaction");
            })?;
            let staged_writer = (|| {
                if !generated_assets {
                    for suffix in ["kfc", "kfc_resources"] {
                        let name = format!("{file_name}.{suffix}");
                        std::fs::copy(asset_game_dir.join(&name), stage.join(&name)).map_err(|error| {
                            error!(error = %error, file = %name, "Failed to stage game asset container");
                        })?;
                    }
                }
                let stage = PathBuf::from_path_buf(stage.clone()).map_err(|_| {
                    error!("Asset staging path is not valid UTF-8");
                })?;
                // Both generated routes start with an empty output stage.
                // Read the existing composition from the input root; the
                // ordinary incremental writer requires pre-existing outputs.
                if generated_assets {
                    KFCWriter::new_generated_incremental(
                        &asset_input_root,
                        &stage,
                        file_name,
                        ref_file.clone(),
                        type_registry.clone(),
                    )
                    .map_err(|error| {
                        error!(error = %error, "Failed to create generated KFC writer");
                    })
                } else {
                    crate::game_files::create_writer_with_policy(
                        &stage,
                        file_name,
                        &type_registry,
                        &ref_file,
                        generated_assets,
                    )
                }
            })();
            match staged_writer {
                Ok(writer) => Some(writer),
                Err(()) => {
                    if prepared_asset_output {
                        warn!("Prepared asset stage retained after writer initialization failure");
                    } else if let Err(error) =
                        shroudforge_game_assets::transaction::abort(environment_root, file_name)
                    {
                        warn!(error = %error, "Failed to roll back asset initialization");
                    }
                    return Err(());
                }
            }
        } else {
            None
        };

        Ok(Self {
            runtime_configured: Cell::new(false),
            runtime_active_mods: RefCell::new(HashSet::new()),
            map_lua_owners: RefCell::new(HashMap::new()),
            runtime_effects: RefCell::new(HashMap::new()),
            runtime_write_fault: RefCell::new(None),
            runtime_ownership: Rc::new(RefCell::new(
                crate::lua::ingame::overrides::claim_runtime_storage::OwnershipStore::default(),
            )),
            native_dlls: RefCell::new(Vec::new()),
            pending_native_dlls: RefCell::new(Vec::new()),
            native_plugin_services: RefCell::new(std::sync::Arc::new(
                super::native_plugin_abi::NoNativePluginServices,
            )),
            env,
            api,
            config,

            type_registry,
            attribute_catalog: OnceCell::new(),
            inline_fields: RefCell::new(HashMap::new()),
            inline_field_bytes: Cell::new(0),

            ref_file,
            reader: RefCell::new(Some(reader)),
            writer: RefCell::new(writer),
            asset_transaction: Cell::new(options.patch),
            generated_asset_transaction: generated_assets,
            defer_current_commit: options.defer_current_commit,
            prepared_asset_output,
            asset_input_root,
            asset_output_root,
            asset_stem: file_name.clone(),

            new_contents: RefCell::new(HashMap::new()),
            content_reads_in_flight: Arc::new(AtomicUsize::new(0)),
            resources: RefCell::new(HashMap::new()),
            loaded_resource_indexes: RefCell::new(
                super::loaded_resource_indexes::Indexes::default(),
            ),
            types: RefCell::new(HashMap::new()),
        })
    }
}
