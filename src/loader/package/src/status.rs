use serde_json::{Value, json};
use std::{fs, path::Path};

pub fn configuration(root: &Path, server: bool, api: &str) -> Value {
    let mut errors = Vec::new();
    let mut checks = Vec::new();
    let mut mod_states = serde_json::Map::new();
    let runtime = crate::config::read_document(root, "mod-status").ok();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let heartbeat_pid = fs::read(crate::paths::runtime_dir(root).join("heartbeat.json"))
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
            && value["pid"].as_u64() == heartbeat_pid
            && value["updatedAt"]
                .as_u64()
                .is_some_and(|timestamp| timestamp <= now && now - timestamp <= 5)
    });
    let awaiting_world = runtime_fresh
        && runtime.as_ref().is_some_and(|value| {
            value["runtimeProvider"]["available"] == true
                && value["runtimeProvider"]["awaitingWorld"] == true
        });
    if let Ok(events) = crate::config::read_document(root, "events-state") {
        if let Some(events) = events.as_object() {
            for (id, value) in events {
                if let Err(error) = crate::news::validate_event(root, value) {
                    errors.push(format!("event {id}: {error}"));
                }
            }
        }
    }
    for result in [
        crate::config::read_loader(root).map(|_| ()),
        crate::news::read(root).map(|_| ()),
        crate::news::read_ids(root).map(|_| ()),
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
            format!("Runtime reports API {version}; expected {api}."),
        ),
        None => (
            "neutral",
            "No current API initialization report from the game process.".into(),
        ),
    };
    checks.push(json!({"id":"api","group":"api","state":api_state,"detail":api_detail}));
    let mut assets = json!({"state":"unknown","detail":"Preparation status has not been checked."});
    if let Ok(entries) = fs::read_dir(crate::paths::mods_dir(root)) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() && path.extension().is_none_or(|extension| extension != "zip") {
                continue;
            }
            if let Err(error) = crate::config::read_manifest_path(root, &path) {
                errors.push(format!("{}: {error}", path.display()));
            }
        }
    }
    match root
        .to_str()
        .ok_or("installation path is not UTF-8")
        .and_then(|path| crate::ModEnvironment::load(path).map_err(|_| "mod discovery failed"))
    {
        Ok(env) => {
            let (plan, failures) = env.plan_report_detailed(server, api);
            let plan_issues: std::collections::HashMap<_, _> = failures
                .iter()
                .map(|issue| (issue.mod_id.as_str(), issue))
                .collect();
            let applied_assets = crate::prepared::fingerprint(&env, server, api)
                .ok()
                .is_some_and(|fingerprint| crate::prepared::matches(root, &fingerprint));
            let preparation = crate::config::read_document(root, "applied").ok();
            let preparation_error = preparation.as_ref().filter(|value| {
                value["status"] == "failed"
                    && value["target"] == if server { "enshrouded_server" } else { "enshrouded" }
            }).map(|value| value["error"].as_str().unwrap_or("Asset preparation failed; inspect the startup log."));
            for item in env.mod_registry().values() {
                let manifest = item.info();
                let fingerprint =
                    crate::config::revision(&serde_json::to_vec(manifest).unwrap_or_default());
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
                let state = if runtime_mod
                    && runtime_error.is_some_and(|reason| {
                        reason.starts_with("plan-blocked:")
                            || reason.starts_with("dependency-blocked:")
                    }) {
                    let reason = runtime_error.unwrap();
                    json!({"state":"blocked","code":"runtime-plan","detail":reason.split_once(": ").map(|(_, detail)| detail).unwrap_or(reason)})
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
                        json!({"state":"not-running","detail":"Activation is saved; the mod is not active in the running process."})
                    } else {
                        json!({"state":"disabled","detail":"The mod is disabled."})
                    }
                } else if pregame_mod && !applied_assets {
                    json!({"state":"restart-required","detail":"This mod's asset changes are not included in startup preparation. Restart the game to apply them."})
                } else if pregame_mod && !runtime_mod && applied_assets {
                    match native_dll_effect.and_then(|value| value["state"].as_str()) {
                        Some("error") => {
                            json!({"state":"failed","detail":native_dll_effect.and_then(|value|value["detail"].as_str()).unwrap_or("The native DLL failed to load.")})
                        }
                        Some("loading") => {
                            json!({"state":"applied","detail":"Asset changes are applied; the native DLL is still loading."})
                        }
                        Some("loaded") => {
                            json!({"state":"applied","detail":native_dll_effect.and_then(|value|value["detail"].as_str()).unwrap_or("Asset changes and native DLL are active for this session.")})
                        }
                        _ => {
                            json!({"state":"applied","detail":"This mod was included in the startup preparation pass for the current game and configuration."})
                        }
                    }
                } else if manifest.enabled {
                    json!({"state":"unconfirmed","detail":"Activation is saved; no current runtime report is available."})
                } else {
                    json!({"state":"disabled","detail":"The mod is disabled."})
                };
                mod_states.insert(manifest.id.clone(), state);
            }
            checks.push(json!({"id":"mods","group":"mods","state":if failures.is_empty(){"ok"}else{"warning"},"detail":if failures.is_empty(){format!("{} mods satisfy activation, inferred process scope, and dependency requirements. Execution status is reported separately.",plan.len())}else{failures.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")}}));
            let needs_prepare = plan.iter().any(|item| item.info().requires_pregame());
            let blocked_pregame = failures
                .iter()
                .filter(|issue| {
                    env.mod_registry()
                        .get(&issue.mod_id)
                        .is_some_and(|item| item.info().requires_pregame())
                })
                .count();
            let existing = crate::config::read_document(root, "applied").is_ok();
            if needs_prepare || existing {
                assets = if let Some(error) = preparation_error {
                    json!({"state":"failed","code":"asset-preparation-failed","detail":error})
                } else { match crate::prepared::fingerprint(&env, server, api) {
                    Ok(fingerprint) => {
                        if crate::prepared::matches(root, &fingerprint) {
                            if blocked_pregame > 0 {
                                json!({"state":"partial","detail":format!("Preparation matches the runnable mods; {blocked_pregame} startup mod(s) were skipped because of compatibility or dependency issues.")})
                            } else {
                                json!({"state":"applied","detail":"Preparation record matches the game and current mod configuration."})
                            }
                        } else {
                            json!({"state":"prepare-required","detail":"The startup preparation pass will apply this configuration on the next game start. Use `shroudforge launch` if the game has already passed its early loading window."})
                        }
                    }
                    Err(error) => json!({"state":"unknown","detail":error}),
                } };
            } else if blocked_pregame > 0 {
                assets = json!({"state":"partial","detail":format!("{blocked_pregame} startup mod(s) were skipped because of compatibility or dependency issues.")});
            } else {
                assets = json!({"state":"not-required","detail":"No enabled pregame mods and no previous preparation record."});
            }
        }
        Err(error) => errors.push(error.into()),
    }
    let executable = root.join(if server {
        "enshrouded_server.exe"
    } else {
        "enshrouded.exe"
    });
    checks.push(json!({"id":"game","group":"game","state":if executable.is_file(){"ok"}else{"warning"},"detail":if executable.is_file(){"Game executable found; build and hook checks run in the target process."}else{"Game executable is missing from this installation."}}));
    let parser = crate::config::read_document(root, "parser-status").ok();
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
            "Parser result belongs to an older game binary; prepare/launch will refresh it.".into(),
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
    let runtime_check = runtime
        .as_ref()
        .filter(|_| runtime_fresh)
        .map(|value| &value["runtimeProvider"]);
    let (runtime_state, runtime_detail) = match runtime_check {
        Some(value) if value["available"] == true && value["awaitingWorld"] == true => (
            "neutral",
            "KFC Runtime is waiting for the player to enter a game world.".into(),
        ),
        Some(value) if value["available"] == true => (
            if value["ready"] == true {
                "ok"
            } else {
                "warning"
            },
            format!(
                "KFC Runtime ABI {}; initialized={}; ready={}; writable={}; {}",
                value["abi"],
                value["initialized"],
                value["ready"],
                value["writable"],
                value["detail"]
                    .as_str()
                    .unwrap_or("provider status unavailable")
            ),
        ),
        Some(value) => (
            "warning",
            format!(
                "KFC Runtime provider unavailable (reported ABI {}): {}",
                value["abi"],
                value["reason"]
                    .as_str()
                    .unwrap_or("provider or ABI unavailable")
            ),
        ),
        None => (
            "neutral",
            "Waiting for a fresh status from the game process.".into(),
        ),
    };
    checks.push(json!({"id":"runtime","group":"game","state":runtime_state,"detail":runtime_detail,"phase":if awaiting_world {"waiting-world"} else {"normal"}}));
    let updates = match crate::config::read_document(root, "state") {
        Ok(value) => value,
        Err(error) if error.contains("has no updates state") => Value::Null,
        Err(error) => {
            errors.push(error);
            Value::Null
        }
    };
    if let Err(error) = crate::config::read_document(root, "applied") {
        if !error.contains("has no assets state") {
            errors.push(error);
        }
    }
    json!({"errors":errors,"checks":checks,"assets":assets,"lastUpdate":updates,"modStates":mod_states})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_asset_startup_is_not_reported_as_another_restart_request() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("mods/asset-test");
        fs::create_dir_all(package.join("src")).unwrap();
        fs::write(package.join("mod.json"), serde_json::to_vec(&json!({
            "id":"asset-test","name":"Asset test","version":"1.0.0","capabilities":["patch"]
        })).unwrap()).unwrap();
        fs::write(package.join("extended.mod.json"), r#"{"schemaVersion":1,"enabled":true}"#).unwrap();
        fs::write(package.join("src/mod.lua"), "return {}").unwrap();
        crate::config::write_document(root.path(), "applied", &json!({
            "schemaVersion":1,"status":"failed","target":"enshrouded","phase":"startup",
            "completedAt":1,"error":"runtime.lifecycle unavailable in world-editor"
        })).unwrap();
        let status = configuration(root.path(), false, "1.0.0");
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
            crate::config::validate_document(Path::new("."), "mod-status", &document)
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
        crate::config::validate_document(Path::new("."), "mod-status", &document)
            .expect("current runtime effect fields must pass mod-status validation");
    }

    #[test]
    fn installed_update_state_uses_the_updates_section_schema() {
        let root = tempfile::tempdir().expect("temporary installation root");
        crate::config::write_document(
            root.path(),
            "window-state",
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
        crate::config::write_document(root.path(), "state", &update)
            .expect("installed update state");
        assert_eq!(
            crate::config::read_document(root.path(), "state").expect("stored update state"),
            update
        );
        assert!(crate::config::window_state(root.path()).get("debugConsole").is_some());
    }
}
