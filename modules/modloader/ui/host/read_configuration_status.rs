use serde_json::{Value, json};
use std::{fs, io::Read, path::Path};

fn profile_discovery_notice(
    root: &Path,
    pid: Option<u32>,
    provider: Option<&Value>,
    now: u64,
) -> Option<Value> {
    let pid = pid?;
    let path = shroudforge_loader_config::paths::cache_dir(root)
        .join("game-profile-discovery")
        .join(format!("status-{pid}.json"));
    let progress = (|| {
        let mut bytes = Vec::new();
        fs::File::open(path)
            .ok()?
            .take(16 * 1024 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > 16 * 1024 {
            return None;
        }
        serde_json::from_slice::<Value>(&bytes).ok()
    })();
    if let Some(progress) = progress.filter(|value| {
        value["schemaVersion"] == 1
            && value["enabled"] == true
            && value["state"] == "searching"
            && value["pid"].as_u64() == Some(u64::from(pid))
            && value["updatedAt"]
                .as_u64()
                .is_some_and(|time| time <= now && now - time <= 120)
    }) {
        return Some(json!({"state":"searching", "image":progress["image"]}));
    }
    let provider = provider.filter(|value| value["available"] == true)?;
    let profile = provider.pointer("/resourceService/profileId")?.as_str()?;
    profile
        .starts_with("beta_")
        .then(|| json!({"state":"beta-active", "profileId":profile}))
}

fn runtime_problem_code(provider: &Value) -> Option<&'static str> {
    if provider["available"] != true {
        return Some("provider-unavailable");
    }

    let resource_profile = provider
        .pointer("/resourceService/profile")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let read_profile_format = provider
        .pointer("/resourceService/profileSelection/kfcFormat")
        .and_then(Value::as_u64);
    let read_profile_expected = read_profile_format == Some(4);
    if read_profile_expected
        && matches!(
            resource_profile,
            "no-exact-resource-read-profile" | "ambiguous-exact-resource-read-profile"
        )
    {
        return Some("profile-unavailable");
    }
    if read_profile_format != Some(3)
        && (resource_profile.starts_with("resource-profile-error:")
            || resource_profile.starts_with("engine-owner-profile-error:"))
    {
        return Some("profile-error");
    }

    // This status is published after exact profile selection, including before
    // a world exists. Released providers do not yet expose profileId or
    // profileSelection. Resource availability is a separate live-world check.
    let resource_profile_selected = resource_profile
        .strip_suffix(":exact-image-resource-service")
        .is_some_and(|id| !id.is_empty());
    if !resource_profile_selected {
        let detail = provider["detail"].as_str().unwrap_or_default();
        let profile = detail
            .split_whitespace()
            .find_map(|part| part.strip_prefix("execution_profile="))
            .or_else(|| {
                detail
                    .split_whitespace()
                    .find_map(|part| part.strip_prefix("profile="))
            })
            .unwrap_or_default();
        if matches!(
            profile,
            "missing-or-ambiguous-profile" | "ambiguous-exact-profile"
        ) {
            return Some("profile-unavailable");
        }
        if profile.starts_with("manual-profile-not-found:")
            || profile.starts_with("manual-profile-id-ambiguous:")
        {
            return Some("profile-selection-invalid");
        }
        if profile.starts_with("profile-error:") {
            return Some("profile-error");
        }
    }
    None
}

fn runtime_compatibility_check(
    provider: Option<&Value>,
    desktop: bool,
    server: bool,
    profile_searching: bool,
) -> Value {
    let phase = if profile_searching {
        "searching-profile"
    } else if desktop {
        "desktop"
    } else if !server && provider.is_some_and(|value| value["mainMenu"] == true) {
        "main-menu"
    } else if provider.is_some_and(|value| value["awaitingWorld"] == true) {
        "waiting-world"
    } else {
        "normal"
    };
    let problem = if desktop {
        None
    } else {
        provider
            .and_then(runtime_problem_code)
            .filter(|code| !profile_searching || *code == "provider-unavailable")
            .or_else(|| {
                (phase == "normal" && provider.is_some_and(|value| value["ready"] != true))
                    .then_some("runtime-not-ready")
            })
    };
    let provider_detail = match provider {
        Some(value) if value["available"] == true => format!(
            "KFC Runtime ABI {}. Initialized={}. Ready={}. Writable={}. {}",
            value["abi"],
            value["initialized"],
            value["ready"],
            value["writable"],
            value["detail"]
                .as_str()
                .unwrap_or("provider status unavailable")
        ),
        Some(value) => format!(
            "KFC Runtime provider unavailable (reported ABI {}): {}",
            value["abi"],
            value["reason"]
                .as_str()
                .unwrap_or("provider or ABI unavailable")
        ),
        None => "Waiting for a fresh status from the game process.".into(),
    };
    let (state, detail) = if problem.is_some() {
        ("warning", provider_detail)
    } else {
        match phase {
            "searching-profile" => (
                "neutral",
                "The native game profile module is comparing the exact baseline and current executable images.".into(),
            ),
            "desktop" => (
                "ok",
                "Desktop mode. World access is not expected until a game world is entered.".into(),
            ),
            "main-menu" => (
                "ok",
                "The original game state confirms the main menu. World access is not expected here.".into(),
            ),
            "waiting-world" => (
                "neutral",
                "KFC Runtime is waiting for the player to enter a game world.".into(),
            ),
            _ => (if provider.is_some() { "ok" } else { "neutral" }, provider_detail),
        }
    };
    json!({"id":"runtime","group":"game","state":state,"code":problem,"detail":detail,"phase":phase})
}

pub fn configuration(root: &Path, server: bool, api: &str, game_pid: Option<u32>) -> Value {
    let desktop = game_pid.is_none();
    let mut errors = Vec::new();
    let mut checks = Vec::new();
    let mut mod_states = serde_json::Map::new();
    let runtime = shroudforge_loader_config::config::read_document(
        root,
        shroudforge_runtime_diagnostics::documents::MOD_STATUS,
    )
    .ok();
    let loader_config = shroudforge_loader_config::config::read_loader(root).ok();
    let world_editor_module_enabled = loader_config.as_ref().and_then(|config| {
        config
            .pointer("/modules/worldEditor/enabled")
            .and_then(Value::as_bool)
    });
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let heartbeat_pid =
        fs::read(shroudforge_loader_config::paths::runtime_dir(root).join("heartbeat.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .filter(|value| {
                value["updatedAt"]
                    .as_u64()
                    .is_some_and(|timestamp| timestamp <= now && now - timestamp <= 8)
            })
            .and_then(|value| value["pid"].as_u64());
    let runtime_fresh = runtime.as_ref().is_some_and(|value| {
        value["running"] == true
            && game_pid.is_some_and(|pid| value["pid"].as_u64() == Some(u64::from(pid)))
            && value["pid"].as_u64() == heartbeat_pid
            && value["updatedAt"]
                .as_u64()
                .is_some_and(|timestamp| timestamp <= now && now - timestamp <= 5)
    });
    let runtime_check = runtime
        .as_ref()
        .filter(|_| runtime_fresh)
        .map(|value| &value["runtimeProvider"]);
    let discovery_notice = profile_discovery_notice(root, game_pid, runtime_check, now);
    let profile_searching = discovery_notice
        .as_ref()
        .is_some_and(|value| value["state"] == "searching");
    let runtime_status =
        runtime_compatibility_check(runtime_check, desktop, server, profile_searching);
    let main_menu = runtime_status["phase"] == "main-menu" && runtime_status["code"].is_null();
    let awaiting_world =
        runtime_status["phase"] == "waiting-world" && runtime_status["code"].is_null();
    match shroudforge_loader_config::config::read_document(
        root,
        shroudforge_news::documents::EVENTS_STATE,
    ) {
        Ok(events) => {
            if let Some(events) = events.as_object() {
                for (id, value) in events {
                    let normalized = shroudforge_news::normalize_legacy_event_level(value.clone());
                    if let Err(error) = shroudforge_news::validate_event(root, &normalized) {
                        errors.push(format!("event {id}: {error}"));
                    }
                }
            }
        }
        Err(error) if error.contains("has no events state") => {}
        Err(error) => errors.push(format!("Could not read news events: {error}")),
    }
    for result in [
        shroudforge_loader_config::config::read_loader(root).map(|_| ()),
        shroudforge_news::read(root).map(|_| ()),
        shroudforge_news::read_ids(root).map(|_| ()),
    ] {
        if let Err(error) = result {
            errors.push(error);
        }
    }
    let (api_state, api_detail) = match runtime
        .as_ref()
        .filter(|_| runtime_fresh)
        .and_then(|value| value["apiVersion"].as_str())
    {
        Some(version) if version == api => (
            "ok",
            format!("The running ShroudForge runtime process reports Lua API {version}."),
        ),
        Some(version) => (
            "warning",
            format!("Runtime reports API {version}. Expected {api}."),
        ),
        None => (
            "neutral",
            "No current API initialization report from the game process.".into(),
        ),
    };
    checks.push(json!({"id":"api","group":"api","state":api_state,"detail":api_detail}));
    let mut assets = json!({"state":"unknown","detail":"Preparation status has not been checked."});
    if let Ok(entries) = fs::read_dir(shroudforge_loader_config::paths::mods_dir(root)) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() && path.extension().is_none_or(|extension| extension != "zip") {
                continue;
            }
            // Native-only directories are inventoried by the DLL Manager.
            if path.is_dir() && !path.join("mod.json").is_file() {
                continue;
            }
            if let Err(error) = shroudforge_package::config::read_manifest_path(root, &path) {
                errors.push(format!("{}: {error}", path.display()));
            }
        }
    }
    match root
        .to_str()
        .ok_or("installation path is not UTF-8")
        .and_then(|path| {
            shroudforge_package::ModEnvironment::load(path).map_err(|_| "mod discovery failed")
        }) {
        Ok(env) => {
            let (plan, failures) = env.plan_report_detailed(server, api);
            let plan_issues: std::collections::HashMap<_, _> = failures
                .iter()
                .map(|issue| (issue.mod_id.as_str(), issue))
                .collect();
            let applied_assets = shroudforge_package::prepared::fingerprint(&env, server, api)
                .ok()
                .is_some_and(|fingerprint| {
                    shroudforge_package::prepared::matches(root, &fingerprint)
                });
            let preparation = shroudforge_loader_config::config::read_document(
                root,
                shroudforge_game_assets::documents::APPLIED,
            )
            .ok();
            let preparation_error = preparation
                .as_ref()
                .filter(|value| {
                    value["status"] == "failed"
                        && value["target"]
                            == if server {
                                "enshrouded_server"
                            } else {
                                "enshrouded"
                            }
                })
                .map(|value| {
                    value["error"]
                        .as_str()
                        .unwrap_or("Asset preparation failed. Inspect the startup log.")
                });
            for item in env.mod_registry().values() {
                let manifest = item.info();
                let fingerprint = shroudforge_loader_config::config::revision(
                    &serde_json::to_vec(manifest).unwrap_or_default(),
                );
                let pregame_mod = manifest.requires_pregame();
                let runtime_mod = manifest
                    .capabilities
                    .iter()
                    .any(|capability| capability.requires_runtime());
                let native_dll_effect = runtime
                    .as_ref()
                    .filter(|_| runtime_fresh)
                    .and_then(|value| value["effects"].get(&manifest.id));
                let runtime_error = runtime
                    .as_ref()
                    .filter(|_| runtime_fresh)
                    .and_then(|value| value["errors"].get(&manifest.id))
                    .and_then(Value::as_str);
                let dll_still_loaded = native_dll_effect
                    .and_then(|value| value["state"].as_str())
                    .is_some_and(|state| matches!(state, "loading" | "loaded"));
                let state = if manifest.id == "world-editor"
                    && world_editor_module_enabled == Some(false)
                {
                    json!({"state":"disabled","code":"module-disabled","detail":"World Editor module is disabled in Settings. The World Editor mod is unavailable until it is enabled."})
                } else if runtime_mod
                    && runtime_error.is_some_and(|reason| {
                        reason.starts_with("plan-blocked:")
                            || reason.starts_with("dependency-blocked:")
                    })
                {
                    let reason = runtime_error.unwrap();
                    json!({"state":"blocked","code":"runtime-plan","detail":reason.split_once(": ").map(|(_, detail)| detail).unwrap_or(reason)})
                } else if runtime_mod
                    && runtime_error.is_some_and(|reason| reason.starts_with("restart-required:"))
                {
                    let reason = runtime_error.unwrap();
                    json!({"state":"restart-required","code":"runtime-restart-required","detail":reason.split_once(": ").map(|(_, detail)| detail).unwrap_or(reason)})
                } else if runtime_mod && runtime_error.is_some() {
                    json!({"state":"failed","detail":runtime_error.unwrap()})
                } else if manifest.enabled
                    && let Some(issue) = plan_issues.get(manifest.id.as_str())
                {
                    json!({"state":"blocked","code":issue.code,"detail":issue.detail})
                } else if !manifest.enabled && dll_still_loaded {
                    json!({"state":"restart-required","code":"native-dll-disable-pending","detail":"The mod is disabled in settings, but its native DLL is still loaded in this game session. It will not load after the next game restart."})
                } else if !manifest.enabled {
                    json!({"state":"disabled","detail":"The mod is disabled."})
                } else if pregame_mod && preparation_error.is_some() {
                    json!({"state":"failed","code":"asset-preparation-failed","detail":preparation_error.unwrap()})
                } else if let Some(effect) = native_dll_effect.filter(|effect| {
                    matches!(effect["state"].as_str(), Some("error" | "write-failed"))
                }) {
                    json!({"state":"failed","detail":effect["detail"].as_str().unwrap_or("The mod reported a runtime error.")})
                } else if let Some(effect) =
                    native_dll_effect.filter(|effect| effect["state"] == "restart-required")
                {
                    json!({"state":"restart-required","detail":effect["detail"].as_str().unwrap_or("Restart the game to apply this mod configuration.")})
                } else if desktop {
                    json!({"state":"ready","detail":"The mod is enabled and ready for the next game start. World access is not expected in desktop mode."})
                } else if runtime_fresh && runtime_mod {
                    let runtime = runtime.as_ref().unwrap();
                    if let Some(reason) = runtime["errors"].get(&manifest.id) {
                        json!({"state":"failed","detail":reason})
                    } else if pregame_mod && !applied_assets {
                        json!({"state":"restart-required","detail":"This mod's asset changes are not included in the current startup preparation. Restart the game to apply them."})
                    } else if runtime["loaded"]
                        .get(&manifest.id)
                        .is_some_and(|loaded| loaded != &fingerprint)
                        || (plan
                            .iter()
                            .any(|candidate| candidate.info().id == manifest.id)
                            && runtime["loaded"].get(&manifest.id).is_none())
                    {
                        json!({"state":"restart-required","detail":"The current game session has not loaded this mod configuration. Restart the game to apply it."})
                    } else if main_menu
                        && native_dll_effect.is_none_or(|effect| effect["state"] != "loading")
                        && runtime["active"]
                            .as_array()
                            .is_some_and(|active| active.iter().any(|id| id == &manifest.id))
                    {
                        json!({"state":"ready","detail":"The mod is loaded and ready. World access is not expected in the main menu."})
                    } else if awaiting_world {
                        json!({"state":"waiting-world","detail":"The mod is loaded for this session and is waiting for the game world."})
                    } else if runtime["active"]
                        .as_array()
                        .is_some_and(|active| active.iter().any(|id| id == &manifest.id))
                    {
                        let effect = runtime["effects"].get(&manifest.id);
                        match effect.and_then(|value| value["state"].as_str()) {
                            Some("restart-required") => {
                                json!({"state":"restart-required","detail":effect.and_then(|value|value["detail"].as_str()).unwrap_or("Restart the game to load this mod's native DLL.")})
                            }
                            Some("error") => {
                                json!({"state":"failed","detail":effect.and_then(|value|value["detail"].as_str()).unwrap_or("The native DLL failed to load.")})
                            }
                            Some("loaded") => {
                                json!({"state":"active","detail":effect.and_then(|value|value["detail"].as_str()).unwrap_or("The native DLL loaded successfully.")})
                            }
                            Some("loading") => {
                                json!({"state":"waiting","detail":effect.and_then(|value|value["detail"].as_str()).unwrap_or("The native DLL is loading.")})
                            }
                            Some("write-confirmed") => {
                                json!({"state":"write-confirmed","detail":effect.and_then(|value|value["detail"].as_str()).unwrap_or("A typed ECS write was confirmed in memory. The gameplay effect still requires in-game confirmation.")} )
                            }
                            Some("write-failed") => {
                                json!({"state":"failed","detail":effect.and_then(|value|value["detail"].as_str()).unwrap_or("The mod could not write its target component.")} )
                            }
                            Some("waiting") => {
                                json!({"state":"waiting","detail":effect.and_then(|value|value["detail"].as_str()).unwrap_or("The mod is waiting for its ECS operation.")} )
                            }
                            Some("no-target") => {
                                json!({"state":"no-target","detail":effect.and_then(|value|value["detail"].as_str()).unwrap_or("The mod query found no target entity.")} )
                            }
                            Some("no-change") => {
                                json!({"state":"no-change","detail":effect.and_then(|value|value["detail"].as_str()).unwrap_or("No component value required a change in this update.")} )
                            }
                            _ => {
                                json!({"state":"active","detail":"The mod loaded into the game runtime. Its gameplay effect has not been confirmed."})
                            }
                        }
                    } else if manifest.enabled {
                        json!({"state":"not-running","detail":"Activation is saved. The mod is not active in the running process."})
                    } else {
                        json!({"state":"disabled","detail":"The mod is disabled."})
                    }
                } else if pregame_mod && !applied_assets {
                    json!({"state":"restart-required","detail":"This mod's asset changes are not included in startup preparation. Restart the game to apply them."})
                } else if main_menu {
                    json!({"state":"ready","detail":"The mod is enabled and ready. World access is not expected in the main menu."})
                } else if pregame_mod && !runtime_mod && applied_assets {
                    match native_dll_effect.and_then(|value| value["state"].as_str()) {
                        Some("error") => {
                            json!({"state":"failed","detail":native_dll_effect.and_then(|value|value["detail"].as_str()).unwrap_or("The native DLL failed to load.")})
                        }
                        Some("loading") => {
                            json!({"state":"applied","detail":"Asset changes are applied. The native DLL is still loading."})
                        }
                        Some("loaded") => {
                            json!({"state":"applied","detail":native_dll_effect.and_then(|value|value["detail"].as_str()).unwrap_or("Asset changes and native DLL are active for this session.")})
                        }
                        _ => {
                            json!({"state":"applied","detail":"This mod was included in the startup preparation pass for the current game and configuration."})
                        }
                    }
                } else if manifest.enabled {
                    json!({"state":"unconfirmed","detail":"Activation is saved. No current runtime report is available."})
                } else {
                    json!({"state":"disabled","detail":"The mod is disabled."})
                };
                mod_states.insert(manifest.id.clone(), state);
            }
            checks.push(json!({"id":"mods","group":"mods","state":if failures.is_empty(){"ok"}else{"warning"},"detail":if failures.is_empty(){format!("{} mods satisfy activation, inferred process scope, and dependency requirements. Execution status is reported separately.",plan.len())}else{failures.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")}}));
            let needs_prepare = plan.iter().any(|item| item.info().requires_pregame());
            let blocked_pregame = failures
                .iter()
                .filter(|issue| {
                    env.mod_registry()
                        .get(&issue.mod_id)
                        .is_some_and(|item| item.info().requires_pregame())
                })
                .count();
            let existing = shroudforge_loader_config::config::read_document(
                root,
                shroudforge_game_assets::documents::APPLIED,
            )
            .is_ok();
            if needs_prepare || existing {
                assets = if let Some(error) = preparation_error {
                    json!({"state":"failed","code":"asset-preparation-failed","detail":error})
                } else {
                    match shroudforge_package::prepared::fingerprint(&env, server, api) {
                        Ok(fingerprint) => {
                            if shroudforge_package::prepared::matches(root, &fingerprint) {
                                if blocked_pregame > 0 {
                                    json!({"state":"partial","detail":format!("Preparation matches the runnable mods. {blocked_pregame} startup mod(s) were skipped because of compatibility or dependency issues.")})
                                } else {
                                    json!({"state":"applied","detail":"Preparation record matches the game and current mod configuration."})
                                }
                            } else {
                                json!({"state":"prepare-required","detail":"The startup preparation pass will apply this configuration on the next game start. Use `shroudforge launch` if the game has already passed its early loading window."})
                            }
                        }
                        Err(error) => json!({"state":"unknown","detail":error}),
                    }
                };
            } else if blocked_pregame > 0 {
                assets = json!({"state":"partial","detail":format!("{blocked_pregame} startup mod(s) were skipped because of compatibility or dependency issues.")});
            } else {
                assets = json!({"state":"not-required","detail":"No enabled pregame mods and no previous preparation record."});
            }
        }
        Err(error) => errors.push(error.into()),
    }
    // The UI reports the same resolved plan that the runtime executes.
    let dll_plan = shroudforge_package::dll_manager::resolve_load_plan(root, server);
    let dll_plan_changed = runtime_fresh
        && runtime.as_ref().is_some_and(|runtime| {
            runtime["dllPlan"] != shroudforge_package::dll_manager::load_plan_identity(&dll_plan)
        });
    let dll_enabled = loader_config
        .as_ref()
        .and_then(|config| config.pointer("/modules/dllManager/enabled"))
        .and_then(Value::as_bool)
        != Some(false);
    let planned_dlls: std::collections::HashSet<_> = dll_plan
        .entries
        .iter()
        .map(|entry| entry.status_id.clone())
        .collect();
    let planned_paths: std::collections::HashSet<_> = dll_plan
        .entries
        .iter()
        .map(|entry| entry.path.to_string_lossy().to_ascii_lowercase())
        .collect();
    for entry in dll_plan.entries {
        let effect = runtime
            .as_ref()
            .filter(|_| runtime_fresh)
            .and_then(|value| value["effects"].get(&entry.status_id));
        let state = match effect.and_then(|value| value["state"].as_str()) {
            Some("loaded") if !dll_enabled => {
                json!({"state":"restart-required","detail":"DLL Manager is off. This DLL remains loaded until the game restarts."})
            }
            Some("loaded") if dll_plan_changed => {
                json!({"state":"restart-required","detail":"The DLL selection, initialization mode or load order changed. Restart the game to apply it."})
            }
            Some("loaded") if main_menu => {
                json!({"state":"ready","detail":effect.unwrap()["detail"]})
            }
            Some("loaded") => json!({"state":"dll-loaded","detail":effect.unwrap()["detail"]}),
            Some("loading") => json!({"state":"waiting","detail":effect.unwrap()["detail"]}),
            Some("error") => json!({"state":"failed","detail":effect.unwrap()["detail"]}),
            _ if !dll_enabled => {
                json!({"state":"disabled","detail":"DLL Manager is off. This DLL will not load at the next game start."})
            }
            _ if desktop => {
                json!({"state":"ready","detail":"Scheduled for the next game start. A runtime load report is not expected in desktop mode."})
            }
            _ if runtime_fresh => {
                json!({"state":"restart-required","detail":"Restart the game to apply this DLL selection."})
            }
            _ => {
                json!({"state":"unconfirmed","detail":"Scheduled for the next game start. No current load report is available."})
            }
        };
        mod_states.insert(entry.status_id, state);
    }
    if let Some(effects) = runtime
        .as_ref()
        .filter(|_| runtime_fresh)
        .and_then(|value| value["effects"].as_object())
    {
        for (id, effect) in effects {
            if id.starts_with("dll:")
                && !planned_dlls.contains(id)
                && matches!(
                    effect["state"].as_str(),
                    Some("loaded" | "loading" | "error")
                )
            {
                let mut parts = id.splitn(3, ':');
                let _ = parts.next();
                let directory = parts.next().unwrap_or_default();
                let relative = parts.next().unwrap_or_default();
                let selected_by_plan = shroudforge_loader_config::paths::mods_dir(root)
                    .join(directory)
                    .join(relative)
                    .canonicalize()
                    .is_ok_and(|path| {
                        planned_paths.contains(&path.to_string_lossy().to_ascii_lowercase())
                    });
                let lua_active = effect["nativeSource"] == "Lua register_dll"
                    && dll_enabled
                    && !shroudforge_package::dll_manager::is_excluded_dll_name(root, relative)
                    && id.split(':').nth(1).is_some_and(|directory| {
                        let enabled = shroudforge_package::dll_manager::read_selections(root)
                            .into_iter()
                            .find(|selection| {
                                selection.package_directory.eq_ignore_ascii_case(directory)
                            })
                            .is_none_or(|selection| {
                                selection.enabled
                                    && selection.approved
                                    && selection.target.applies_to(server)
                            });
                        enabled
                            && shroudforge_package::read_manifest_path(
                                root,
                                &shroudforge_loader_config::paths::mods_dir(root).join(directory),
                            )
                            .is_ok_and(|manifest| {
                                manifest.enabled && manifest.target.supports_process(server)
                            })
                    });
                let state = if effect["state"] == "error" {
                    json!({"state":"failed","detail":effect["detail"]})
                } else if (lua_active || selected_by_plan) && !dll_plan_changed {
                    json!({"state":if effect["state"] == "loaded" {if main_menu {"ready"} else {"dll-loaded"}} else {"waiting"},"detail":effect["detail"]})
                } else {
                    json!({"state":"restart-required","detail":"This DLL is no longer selected. It remains loaded until the game restarts."})
                };
                mod_states.insert(id.clone(), state);
            }
        }
    }
    for issue in dll_plan.issues {
        mod_states.insert(
            issue.status_id,
            json!({"state":"failed","detail":issue.detail}),
        );
    }
    for entry in shroudforge_package::dll_manager::chain_load_entries(root) {
        mod_states.entry(entry.status_id).or_insert_with(|| {
            json!({
                "state":"disabled", "detail":if dll_enabled {
                    "This chain entry is inactive for the current mod activation or process target."
                } else { "DLL Manager is disabled." }
            })
        });
    }
    let executable = root.join(if server {
        "enshrouded_server.exe"
    } else {
        "enshrouded.exe"
    });
    checks.push(json!({"id":"game","group":"game","state":if executable.is_file(){"ok"}else{"warning"},"detail":if executable.is_file(){"Game executable found. Build and hook checks run in the target process."}else{"Game executable is missing from this installation."}}));
    let parser = shroudforge_loader_config::config::read_document(
        root,
        shroudforge_game_assets::documents::PARSER_STATUS,
    )
    .ok();
    let parser_current = parser
        .as_ref()
        .zip(fs::metadata(&executable).ok())
        .is_some_and(|(status, metadata)| {
            let modified = metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|value| value.as_secs());
            status["target"]
                == if server {
                    "enshrouded_server"
                } else {
                    "enshrouded"
                }
                && status["gameFileSize"].as_u64() == Some(metadata.len())
                && status["gameFileModified"].as_u64() == modified
        });
    let (parser_state, parser_detail) = match parser {
        Some(_) if !parser_current => (
            "neutral",
            "Parser result belongs to an older game binary. Prepare or launch will refresh it."
                .into(),
        ),
        Some(value) if value["status"] == "parsed" => (
            "ok",
            format!(
                "KFC Parser successfully processed build {}.",
                value["target"]
            ),
        ),
        Some(value) => (
            "warning",
            format!(
                "Parser reported an error: {}",
                value["error"].as_str().unwrap_or("unknown error")
            ),
        ),
        None => (
            "neutral",
            "No parser result has been saved by prepare/launch.".into(),
        ),
    };
    checks
        .push(json!({"id":"parser","group":"parser","state":parser_state,"detail":parser_detail}));
    checks.push(runtime_status);
    let state_path = shroudforge_loader_config::paths::state_file(root);
    let updates = match fs::read(&state_path) {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(state) => state.get("updates").cloned().unwrap_or(Value::Null),
            Err(error) => {
                errors.push(format!("{}: {error}", state_path.display()));
                Value::Null
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Value::Null,
        Err(error) => {
            errors.push(format!("{}: {error}", state_path.display()));
            Value::Null
        }
    };
    if let Err(error) = shroudforge_loader_config::config::read_document(
        root,
        shroudforge_game_assets::documents::APPLIED,
    ) {
        if !error.contains("has no assets state") {
            errors.push(error);
        }
    }
    json!({"errors":errors,"checks":checks,"assets":assets,"lastUpdate":updates,"modStates":mod_states,"gameProfileDiscovery":discovery_notice})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_profile_discovery_banner_requires_live_beta_or_current_process_progress() {
        let root = tempfile::tempdir().unwrap();
        let path = shroudforge_loader_config::paths::cache_dir(root.path())
            .join("game-profile-discovery/status-42.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let progress =
            json!({"schemaVersion":1,"enabled":true,"state":"searching","pid":42,"updatedAt":100});
        fs::write(&path, progress.to_string()).unwrap();
        assert_eq!(
            profile_discovery_notice(root.path(), Some(42), None, 101).unwrap()["state"],
            "searching"
        );
        assert!(profile_discovery_notice(root.path(), Some(43), None, 101).is_none());
        assert!(profile_discovery_notice(root.path(), Some(42), None, 221).is_none());
        assert!(profile_discovery_notice(root.path(), Some(42), None, 99).is_none());
        assert!(profile_discovery_notice(root.path(), None, None, 101).is_none());
        let mut finished = progress;
        finished["state"] = "finished".into();
        fs::write(&path, finished.to_string()).unwrap();
        assert!(profile_discovery_notice(root.path(), Some(42), None, 101).is_none());
        let beta =
            json!({"available":true,"resourceService":{"profileId":"beta_enshrouded-client-42"}});
        let notice = profile_discovery_notice(root.path(), Some(42), Some(&beta), 101).unwrap();
        assert_eq!(notice["state"], "beta-active");
        assert_eq!(notice["profileId"], "beta_enshrouded-client-42");
        let normal =
            json!({"available":true,"resourceService":{"profileId":"enshrouded-client-42"}});
        assert!(profile_discovery_notice(root.path(), Some(42), Some(&normal), 101).is_none());
        let unavailable =
            json!({"available":false,"resourceService":{"profileId":"beta_enshrouded-client-42"}});
        assert!(profile_discovery_notice(root.path(), Some(42), Some(&unavailable), 101).is_none());
    }

    #[test]
    fn game_profile_discovery_does_not_hide_missing_profiles_in_the_main_menu() {
        let unavailable = json!({"available":true,"ready":false,"mainMenu":true,
            "resourceService":{"profile":"no-exact-resource-read-profile","profileSelection":{"kfcFormat":4}}});
        assert_eq!(
            runtime_problem_code(&unavailable),
            Some("profile-unavailable")
        );
        let beta_waiting = json!({"available":true,"ready":false,"mainMenu":true,
            "resourceService":{"profile":"beta_enshrouded-client-42:exact-image-resource-service"}});
        assert_eq!(runtime_problem_code(&beta_waiting), None);
        let broken = json!({"available":true,"ready":true,
            "resourceService":{"profile":"engine-owner-profile-error:contract-mismatch","profileSelection":{"kfcFormat":4}}});
        assert_eq!(runtime_problem_code(&broken), Some("profile-error"));
        let legacy = json!({"available":true,"ready":true,
            "resourceService":{"profile":"no-exact-resource-read-profile","profileSelection":{"kfcFormat":3}},
            "detail":"profile=enshrouded-client-1076226 game_thread=ready"});
        assert_eq!(runtime_problem_code(&legacy), None);
    }

    #[test]
    fn runtime_banner_accepts_selected_resource_profiles_before_world_readiness() {
        for id in [
            "enshrouded-client-26404966",
            "beta_enshrouded-client-26404966",
        ] {
            // The released provider has neither profileId nor profileSelection.
            let mut provider = json!({
                "available":true,"initialized":true,"ready":false,
                "mainMenu":true,"awaitingWorld":false,
                "detail":"profile=missing-or-ambiguous-profile game_thread=unavailable",
                "resourceService":{"available":false,"profile":format!("{id}:exact-image-resource-service")}
            });
            let menu = runtime_compatibility_check(Some(&provider), false, false, false);
            assert_eq!(menu["state"], "ok", "{menu}");
            assert!(menu["code"].is_null(), "{menu}");
            assert_eq!(menu["phase"], "main-menu");

            provider["mainMenu"] = json!(false);
            provider["awaitingWorld"] = json!(true);
            let waiting = runtime_compatibility_check(Some(&provider), false, false, false);
            assert_eq!(waiting["state"], "neutral", "{waiting}");
            assert!(waiting["code"].is_null(), "{waiting}");
            assert_eq!(waiting["phase"], "waiting-world");

            provider["awaitingWorld"] = json!(false);
            provider["ready"] = json!(true);
            provider["resourceService"]["available"] = json!(true);
            let world = runtime_compatibility_check(Some(&provider), false, false, false);
            assert_eq!(world["state"], "ok", "{world}");
            assert!(world["code"].is_null(), "{world}");

            provider["ready"] = json!(false);
            let lost_readiness = runtime_compatibility_check(Some(&provider), false, false, false);
            assert_eq!(lost_readiness["state"], "warning");
            assert_eq!(lost_readiness["code"], "runtime-not-ready");
        }
    }

    #[test]
    fn runtime_banner_keeps_real_profile_errors_during_menu_wait_and_ready_world() {
        for (profile, code) in [
            ("no-exact-resource-read-profile", "profile-unavailable"),
            (
                "ambiguous-exact-resource-read-profile",
                "profile-unavailable",
            ),
            ("resource-profile-error:invalid-layout", "profile-error"),
            (
                "engine-owner-profile-error:contract-mismatch",
                "profile-error",
            ),
        ] {
            for (main_menu, awaiting_world, ready) in [
                (true, false, false),
                (false, true, false),
                (false, false, true),
            ] {
                let provider = json!({
                    "available":true,"ready":ready,"mainMenu":main_menu,"awaitingWorld":awaiting_world,
                    "detail":"profile=enshrouded-client-1076226:exact-image",
                    "resourceService":{"profile":profile,"profileSelection":{"kfcFormat":4}}
                });
                let check = runtime_compatibility_check(Some(&provider), false, false, false);
                assert_eq!(check["state"], "warning", "{check}");
                assert_eq!(check["code"], code, "{check}");
            }
        }
        let unavailable = json!({"available":false,"ready":true,"mainMenu":true,
            "reason":"ABI mismatch","resourceService":{"profile":"enshrouded-client-26404966:exact-image-resource-service"}});
        let check = runtime_compatibility_check(Some(&unavailable), false, false, false);
        assert_eq!(check["state"], "warning");
        assert_eq!(check["code"], "provider-unavailable");

        let released_error = json!({"available":true,"ready":true,
            "resourceService":{"profile":"resource-profile-error:invalid-layout"}});
        assert_eq!(runtime_problem_code(&released_error), Some("profile-error"));
    }

    #[test]
    fn runtime_banner_preserves_legacy_profile_checks_and_requires_selected_resource_profile() {
        let mut legacy = json!({"available":true,"ready":false,"mainMenu":true,
            "resourceService":{"profile":"no-exact-resource-read-profile","profileSelection":{"kfcFormat":3}},
            "detail":"profile=enshrouded-client-1076226:exact-image game_thread=installed-awaiting-world"});
        let check = runtime_compatibility_check(Some(&legacy), false, false, false);
        assert_eq!(check["state"], "ok", "{check}");
        assert!(check["code"].is_null());
        for (detail, code) in [
            (
                "profile=missing-or-ambiguous-profile",
                "profile-unavailable",
            ),
            (
                "execution_profile=ambiguous-exact-profile",
                "profile-unavailable",
            ),
            (
                "profile=manual-profile-not-found:missing",
                "profile-selection-invalid",
            ),
            (
                "profile=manual-profile-id-ambiguous:duplicate",
                "profile-selection-invalid",
            ),
            ("profile=profile-error:invalid-layout", "profile-error"),
        ] {
            legacy["detail"] = json!(detail);
            let check = runtime_compatibility_check(Some(&legacy), false, false, false);
            assert_eq!(check["state"], "warning", "{check}");
            assert_eq!(check["code"], code, "{check}");
        }
        for profile in [
            "",
            ":exact-image-resource-service",
            "enshrouded-client-26404966",
        ] {
            let provider = json!({"available":true,"ready":true,
                "detail":"profile=missing-or-ambiguous-profile",
                "resourceService":{"profile":profile,"profileId":"enshrouded-client-26404966"}});
            assert_eq!(runtime_problem_code(&provider), Some("profile-unavailable"));
        }
    }

    #[test]
    fn runtime_banner_uses_the_same_decision_in_fresh_process_configuration() {
        let root = tempfile::tempdir().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let runtime_directory = shroudforge_loader_config::paths::runtime_dir(root.path());
        fs::create_dir_all(&runtime_directory).unwrap();
        fs::write(
            runtime_directory.join("heartbeat.json"),
            json!({"pid":42,"updatedAt":now}).to_string(),
        )
        .unwrap();
        let mut runtime = json!({"schemaVersion":1,"pid":42,"updatedAt":now,"running":true,
        "active":[],"loaded":{},"errors":{},"runtimeProvider":{
            "available":true,"initialized":true,"ready":false,"mainMenu":true,"awaitingWorld":false,
            "detail":"profile=missing-or-ambiguous-profile game_thread=unavailable",
            "resourceService":{"available":false,"profile":"enshrouded-client-26404966:exact-image-resource-service"}
        }});
        for (main_menu, ready) in [(true, false), (false, true)] {
            runtime["runtimeProvider"]["mainMenu"] = json!(main_menu);
            runtime["runtimeProvider"]["ready"] = json!(ready);
            shroudforge_loader_config::config::write_document(
                root.path(),
                shroudforge_runtime_diagnostics::documents::MOD_STATUS,
                &runtime,
            )
            .unwrap();
            let configuration = configuration(root.path(), false, "1.0.0", Some(42));
            let check = configuration["checks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|check| check["id"] == "runtime")
                .unwrap();
            assert_eq!(check["state"], "ok", "{check}");
            assert!(check["code"].is_null(), "{check}");
            assert_eq!(
                check["phase"],
                if main_menu { "main-menu" } else { "normal" }
            );
        }
        runtime["updatedAt"] = json!(now - 20);
        shroudforge_loader_config::config::write_document(
            root.path(),
            shroudforge_runtime_diagnostics::documents::MOD_STATUS,
            &runtime,
        )
        .unwrap();
        let configuration = configuration(root.path(), false, "1.0.0", Some(42));
        let check = configuration["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["id"] == "runtime")
            .unwrap();
        assert_eq!(check["state"], "neutral", "{check}");
        assert!(check["code"].is_null());
    }

    #[test]
    fn runtime_banner_waits_for_status_and_profile_search_but_keeps_provider_failures() {
        let no_status = runtime_compatibility_check(None, false, false, false);
        assert_eq!(no_status["state"], "neutral");
        assert!(no_status["code"].is_null());
        let unresolved = json!({"available":true,"ready":false,
            "detail":"profile=missing-or-ambiguous-profile"});
        let searching = runtime_compatibility_check(Some(&unresolved), false, false, true);
        assert_eq!(searching["state"], "neutral");
        assert!(searching["code"].is_null());
        assert_eq!(searching["phase"], "searching-profile");
        let unavailable = json!({"available":false,"reason":"ABI mismatch"});
        let failed = runtime_compatibility_check(Some(&unavailable), false, false, true);
        assert_eq!(failed["state"], "warning");
        assert_eq!(failed["code"], "provider-unavailable");
        let desktop = runtime_compatibility_check(Some(&unavailable), true, false, false);
        assert_eq!(desktop["state"], "ok");
        assert!(desktop["code"].is_null());
    }

    #[test]
    fn legacy_success_event_does_not_make_configuration_invalid() {
        let root = tempfile::tempdir().expect("temporary installation root");
        shroudforge_loader_config::config::write_document(
            root.path(),
            shroudforge_news::documents::EVENTS_STATE,
            &json!({
                "event-mod-install-sf-survival-1.0.0.json": {
                    "id": "mod-install-sf-survival-1.0.0",
                    "title": "",
                    "message": "",
                    "level": "success"
                }
            }),
        )
        .expect("legacy event state");

        let status = configuration(root.path(), false, "1.0.0", None);
        let errors = status["errors"].as_array().expect("configuration errors");
        assert!(!errors.iter().any(|error| {
            error.as_str().is_some_and(|error| {
                error.starts_with("event event-mod-install-sf-survival-1.0.0.json:")
            })
        }));
    }

    #[test]
    fn failed_asset_startup_is_not_reported_as_another_restart_request() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("mods/asset-test");
        fs::create_dir_all(package.join("src")).unwrap();
        fs::write(
            package.join("mod.json"),
            serde_json::to_vec(&json!({
                "id":"asset-test","name":"Asset test","version":"1.0.0","capabilities":["patch"]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            package.join("extended.mod.json"),
            r#"{"schemaVersion":1,"enabled":true}"#,
        )
        .unwrap();
        fs::write(package.join("src/mod.lua"), "return {}").unwrap();
        shroudforge_loader_config::config::write_document(
            root.path(),
            shroudforge_game_assets::documents::APPLIED,
            &json!({
                "schemaVersion":1,"status":"failed","target":"enshrouded","phase":"startup",
                "completedAt":1,"error":"runtime.lifecycle unavailable in world-editor"
            }),
        )
        .unwrap();
        let status = configuration(root.path(), false, "1.0.0", None);
        let encoded = serde_json::to_string(&status).unwrap();
        assert!(encoded.contains("asset-preparation-failed"), "{encoded}");
        assert!(!encoded.contains("restart-required"), "{encoded}");
    }

    #[test]
    fn native_plugin_lifecycle_effects_pass_runtime_status_validation() {
        for state in ["loading", "loaded", "error"] {
            let document = json!({
                "schemaVersion": 1,
                "pid": 1,
                "updatedAt": 1,
                "running": true,
                "active": [],
                "loaded": {},
                "errors": {},
                "effects": {
                    "community-mod": {
                        "state": state,
                        "processTarget": "client",
                        "scope": "this-process",
                        "detail": "sidecar lifecycle test",
                        "updatedAt": 1
                    }
                }
            });
            shroudforge_loader_config::config::validate_document(
                Path::new("."),
                shroudforge_runtime_diagnostics::documents::MOD_STATUS,
                &document,
            )
            .unwrap_or_else(|error| panic!("native DLL state '{state}' was rejected: {error}"));
        }
    }

    #[test]
    fn restart_required_runtime_effect_passes_runtime_status_validation() {
        let document = json!({
            "schemaVersion": 1,
            "pid": 1,
            "updatedAt": 1,
            "running": true,
            "active": [],
            "loaded": {},
            "errors": {},
            "effects": {
                "community-mod": {
                    "state": "restart-required",
                    "processTarget": "server",
                    "scope": "this-process",
                    "detail": "native DLL remains loaded for this game session",
                    "updatedAt": 1
                }
            }
        });
        shroudforge_loader_config::config::validate_document(
            Path::new("."),
            shroudforge_runtime_diagnostics::documents::MOD_STATUS,
            &document,
        )
        .expect("current runtime effect fields must pass mod-status validation");
    }

    #[test]
    fn installed_update_state_preserves_the_window_state_section() {
        let root = tempfile::tempdir().expect("temporary installation root");
        shroudforge_loader_config::config::write_document(
            root.path(),
            shroudforge_loader_config::documents::WINDOW_STATE,
            &json!({"debugConsole":{"visible":false,"requestedVisible":false,"requestId":0}}),
        )
        .expect("initial window state");
        let update = json!({
            "schemaVersion": 1,
            "status": "installed",
            "version": "1.6.4",
            "build": "test",
            "installedAt": 1,
            "backup": "backup/1"
        });
        shroudforge_loader_config::config::update_state_section(root.path(), "updates", |_| {
            Ok(update.clone())
        })
        .expect("installed update state section");
        assert_eq!(
            serde_json::from_slice::<Value>(
                &fs::read(shroudforge_loader_config::paths::state_file(root.path()))
                    .expect("stored state")
            )
            .expect("parse stored state")["updates"],
            update
        );
        assert!(
            shroudforge_loader_config::config::window_state(root.path())
                .get("debugConsole")
                .is_some()
        );
    }
}
