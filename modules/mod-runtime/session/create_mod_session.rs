use super::*;
impl AppState {
    pub fn new(
        env: ModEnvironment,
        api: Option<ShroudForgeApi>,
        args: RunArgs,
        cache_diff: &CacheDiff,
    ) -> Result<Self, ()> {
        let game_dir = env.game_dir();
        let cache_dir = env.cache_dir();
        let file_name = &args.file_name;

        // prepare configuration

        let options = args.options;
        let game_root = env.game_dir().as_std_path();
        let default_export_dir =
            PathBuf::from_path_buf(shroudforge_loader_config::paths::export_dir(game_root))
                .expect("a UTF-8 game path joined with fixed loader paths remains UTF-8");
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
            let directory = game_root.join(directory);
            if let Err(error) = std::fs::create_dir_all(&directory) {
                warn!(error = %error, path = %directory.display(), "Failed to create export directory");
                return None;
            }
            let canonical_root = match std::fs::canonicalize(game_root) {
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
            && crate::native_dlls::manager_enabled(game_root).unwrap_or(false)
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

        let (type_registry, is_dirty) = crate::reflection::load_type_registry::load_type_registry(
            game_dir, cache_dir, file_name,
        )?;
        if phase == RuntimePhase::Pregame {
            crate::lua::definitions::export_lua_definitions::export_to_cache(
                cache_dir,
                &type_registry,
                options.skip_cache || is_dirty || cache_diff.build_id_changed(),
            );
        }
        let type_registry = Rc::new(type_registry);

        let kfc_path = game_dir.join(file_name).with_extension("kfc");
        let (ref_file, reader) = if phase == RuntimePhase::Ingame {
            (
                crate::game_files::load_kfc_file(&kfc_path)?,
                crate::game_files::create_reader_with_extension(game_dir, file_name, "kfc")?,
            )
        } else {
            let bak_path = crate::startup::manage_asset_baseline::create_backup(&kfc_path)?;
            (
                crate::game_files::load_kfc_file(&bak_path)?,
                crate::game_files::create_reader(game_dir, file_name)?,
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
            let stage =
                shroudforge_game_assets::transaction::begin(game_dir.as_std_path(), file_name)
                    .map_err(|error| {
                        error!(error = %error, "Failed to begin typed asset transaction");
                    })?;
            let staged_writer = (|| {
                for suffix in ["kfc", "kfc_resources"] {
                    let name = format!("{file_name}.{suffix}");
                    std::fs::copy(game_dir.join(&name), stage.join(&name)).map_err(|error| {
                        error!(error = %error, file = %name, "Failed to stage game asset container");
                    })?;
                }
                let stage = PathBuf::from_path_buf(stage.clone()).map_err(|_| {
                    error!("Asset staging path is not valid UTF-8");
                })?;
                crate::game_files::create_writer(&stage, file_name, &type_registry, &ref_file)
            })();
            match staged_writer {
                Ok(writer) => Some(writer),
                Err(()) => {
                    if let Err(error) = shroudforge_game_assets::transaction::abort(
                        game_dir.as_std_path(),
                        file_name,
                    ) {
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
            runtime_effects: RefCell::new(HashMap::new()),
            runtime_write_fault: RefCell::new(None),
            runtime_ownership: Rc::new(RefCell::new(
                crate::lua::ingame::overrides::claim_runtime_storage::OwnershipStore::default(),
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
            asset_stem: file_name.clone(),

            new_contents: RefCell::new(HashMap::new()),
            resources: RefCell::new(HashMap::new()),
            loaded_resource_indexes: RefCell::new(
                super::loaded_resource_indexes::Indexes::default(),
            ),
            types: RefCell::new(HashMap::new()),
        })
    }
}
