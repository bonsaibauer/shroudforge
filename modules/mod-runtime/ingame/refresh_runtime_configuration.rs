use super::IngameRuntime;
use crate::env::AppState;

pub(super) fn settings_only_manifest_change(
    desired: &serde_json::Value,
    applied: &serde_json::Value,
) -> bool {
    let mut metadata = desired.clone();
    metadata["settingValues"] = applied["settingValues"].clone();
    metadata["enabled"] = applied["enabled"].clone();

    let Some(desired_settings) = metadata
        .get_mut("settings")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return false;
    };
    let Some(applied_settings) = applied
        .get("settings")
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };

    for desired_setting in desired_settings {
        let Some(key) = desired_setting.get("key") else {
            return false;
        };
        let Some(applied_setting) = applied_settings
            .iter()
            .find(|setting| setting.get("key") == Some(key))
        else {
            continue;
        };
        if let Some(default) = applied_setting.get("default") {
            desired_setting["default"] = default.clone();
        }
    }

    metadata == *applied
}

pub(super) fn settings_to_apply_live(
    manifest: &mod_loader::ModManifest,
    applied_values: &serde_json::Value,
) -> Option<serde_json::Value> {
    if !manifest
        .capabilities
        .iter()
        .any(|capability| capability.requires_runtime())
    {
        return None;
    }

    let mut values = manifest.setting_values.clone();
    if manifest
        .capabilities
        .contains(&mod_loader::Capability::Patch)
    {
        let next_values = values.as_object_mut()?;
        for setting in &manifest.settings {
            let desired = manifest.setting_values.get(&setting.key);
            let applied = applied_values.get(&setting.key);
            let is_live = setting.apply_at == "live" && !setting.restart_required;
            if desired != applied && !is_live {
                if let Some(applied) = applied {
                    next_values.insert(setting.key.clone(), applied.clone());
                } else {
                    next_values.remove(&setting.key);
                }
            }
        }
    }
    Some(values)
}

impl IngameRuntime {
    pub(super) fn refresh_configuration(&mut self) {
        let world_editor_enabled = shroudforge_loader_config::config::read_loader_in_background(&self.root)
            .ok()
            .and_then(|config| {
                config
                    .pointer("/modules/worldEditor/enabled")
                    .and_then(serde_json::Value::as_bool)
            })
            .unwrap_or(false);
        let configured_enabled: std::collections::HashMap<String, bool> = self
            .configurations
            .iter()
            .map(|(id, _, configuration)| {
                (
                    id.clone(),
                    configuration["enabled"].as_bool().unwrap_or(false),
                )
            })
            .collect();
        let active_ids: std::collections::HashSet<String> = self
            .lifecycle
            .iter()
            .filter(|item| item.active)
            .map(|item| item.id.clone())
            .collect();
        let mut active_dependents = std::collections::HashMap::<String, Vec<String>>::new();
        for (dependent_id, _, manifest) in &self.configurations {
            if !active_ids.contains(dependent_id) {
                continue;
            }
            for dependency in manifest["dependencies"].as_array().into_iter().flatten() {
                if dependency["optional"].as_bool().unwrap_or(false) {
                    continue;
                }
                if let Some(dependency_id) = dependency["id"].as_str() {
                    active_dependents
                        .entry(dependency_id.to_owned())
                        .or_default()
                        .push(dependent_id.clone());
                }
            }
        }
        for (id, package, applied) in &mut self.configurations {
            // Read off the mods' loop; unknown until first read.
            let package_signature = match self.package_revisions.revision(package) {
                None => continue,
                Some(Ok(signature)) => signature,
                Some(Err(error)) => {
                    let signature = format!("read-error:{error}");
                    if self.observed.get(package) != Some(&signature) {
                        tracing::error!(target:"shroudforge::runtime", mod_id=%id,"Configuration read failed. Keeping applied values: {error}");
                        self.observed.insert(package.clone(), signature);
                    }
                    continue;
                }
            };
            let signature = if id == "world-editor" {
                format!("{package_signature}:worldEditor.enabled={world_editor_enabled}")
            } else {
                package_signature
            };
            if self.observed.get(package) == Some(&signature) {
                continue;
            }
            self.observed.insert(package.clone(), signature);
            let manifest = match mod_loader::config::read_manifest_path(&self.root, package) {
                Ok(manifest) if manifest.id == *id => manifest,
                Ok(_) => {
                    tracing::error!(target:"shroudforge::runtime", mod_id=%id,"Mod identity changed. Restart required, retaining applied configuration");
                    continue;
                }
                Err(error) => {
                    tracing::error!(target:"shroudforge::runtime", mod_id=%id,"Configuration rejected. Keeping applied values: {error}");
                    continue;
                }
            };
            let mut desired = serde_json::to_value(&manifest).expect("manifest is serializable");
            if id == "world-editor" && !world_editor_enabled {
                desired["enabled"] = false.into();
            }
            if desired == *applied {
                if self
                    .lifecycle
                    .iter()
                    .any(|item| item.id == *id && item.active)
                    && self
                        .errors
                        .get(id)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|reason| reason.starts_with("dependency-blocked:"))
                {
                    self.errors.remove(id);
                }
                continue;
            }
            let metadata_unchanged = settings_only_manifest_change(&desired, applied);
            let runtime_mod = manifest
                .capabilities
                .iter()
                .any(|capability| capability.requires_runtime());
            let live_mod = runtime_mod
                && !manifest
                    .capabilities
                    .contains(&mod_loader::Capability::Patch);
            let wanted_enabled = desired["enabled"].as_bool().unwrap_or(false);
            let was_enabled = applied["enabled"].as_bool().unwrap_or(false);
            if metadata_unchanged && live_mod && wanted_enabled != was_enabled {
                let transition_block = if wanted_enabled {
                    manifest
                        .dependencies
                        .iter()
                        .find(|dependency| {
                            if dependency.optional.unwrap_or(false) {
                                return false;
                            }
                            let config_disabled = configured_enabled
                                .get(&dependency.id)
                                .is_some_and(|enabled| !enabled);
                            let runtime_disabled = self.lifecycle.iter().any(|candidate| {
                                candidate.id == dependency.id && !candidate.active
                            });
                            config_disabled || runtime_disabled
                        })
                        .map(|dependency| {
                            format!("required dependency is not active: {}", dependency.id)
                        })
                } else {
                    active_dependents.get(id).map(|dependents| {
                        format!(
                            "cannot disable this dependency while required by active mod(s): {}",
                            dependents.join(", ")
                        )
                    })
                };
                if let Some(error) = transition_block {
                    self.errors
                        .insert(id.clone(), format!("dependency-blocked: {error}").into());
                    tracing::warn!(target:"shroudforge::runtime", mod_id=%id, "Live activation transition deferred: {error}");
                } else if let Some(item) = self.lifecycle.iter_mut().find(|item| item.id == *id) {
                    let transition = if wanted_enabled && item.cleanup_pending {
                        Err("Previous runtime mod cleanup is still pending".to_owned())
                    } else if wanted_enabled {
                        item.activate_runtime_mod(&self.runner.lua, None)
                            .and_then(|result| result)
                            .map_err(|error| error.to_string())
                    } else if item.active {
                        self.diagnostics.release_mod_host(id);
                        item.try_unload(&self.runner.lua)
                            .map_err(|error| error.to_string())
                    } else {
                        Ok(())
                    };
                    match transition {
                        Ok(()) => {
                            item.active = wanted_enabled;
                            if !wanted_enabled {
                                self.runner
                                    .lua
                                    .app_data_ref::<AppState>()
                                    .unwrap()
                                    .set_runtime_mod_active(id, false);
                            }
                            applied["enabled"] = serde_json::json!(wanted_enabled);
                            self.errors.remove(id);
                            tracing::info!(target:"shroudforge::runtime", mod_id=%id, enabled=wanted_enabled, "Runtime mod activation changed live");
                        }
                        Err(error) => {
                            if !wanted_enabled {
                                // Keep the requested disable pending until on_unload
                                // has released the mod's temporary engine state.
                                self.observed.remove(package);
                            }
                            if wanted_enabled {
                                if let Err(rollback) =
                                    item.rollback_failed_activation(&self.runner.lua)
                                {
                                    tracing::error!(target:"shroudforge::runtime", mod_id=%id,
                                        "Live activation rollback is pending and will be retried: {rollback}");
                                }
                            }
                            self.errors.insert(
                                id.clone(),
                                format!("live activation failed: {error}").into(),
                            );
                            tracing::error!(target:"shroudforge::runtime", mod_id=%id, "Live activation transition failed: {error}");
                        }
                    }
                } else {
                    self.errors.insert(
                        id.clone(),
                        "restart-required: Runtime module was not present in the startup plan. Restart the game to activate it."
                            .into(),
                    );
                }
            }
            let desired_settings = manifest.setting_values.clone();
            let settings_changed = desired_settings != applied["settingValues"];
            if metadata_unchanged && settings_changed {
                if let Some(live_settings) =
                    settings_to_apply_live(&manifest, &applied["settingValues"])
                {
                    if live_settings != applied["settingValues"] {
                        match crate::env::shroudforge::set_settings(
                            &self.runner.lua,
                            id,
                            &live_settings,
                        ) {
                            Ok(()) => {
                                let restart_settings_pending = live_settings != desired_settings;
                                applied["settingValues"] = live_settings;
                                applied["settings"] = desired["settings"].clone();
                                if restart_settings_pending {
                                    tracing::info!(target:"shroudforge::runtime", mod_id=%id,
                                        "Live runtime settings applied; restart-required settings remain pending");
                                } else {
                                    tracing::info!(target:"shroudforge::runtime", mod_id=%id,
                                        "Runtime settings applied live");
                                }
                            }
                            Err(error) => {
                                self.errors.insert(
                                    id.clone(),
                                    format!("live settings update failed: {error}").into(),
                                );
                                tracing::error!(target:"shroudforge::runtime", mod_id=%id,
                                    "Live settings update failed: {error}");
                            }
                        }
                    }
                }
            }
            let now_applied = desired.clone();
            let changed_but_deferred = now_applied != *applied;
            let deferred_is_only_enable_or_settings =
                settings_only_manifest_change(&now_applied, applied);
            if changed_but_deferred && !deferred_is_only_enable_or_settings {
                tracing::info!(target:"shroudforge::runtime", mod_id=%id, "Mod changes require the next game start");
            } else if changed_but_deferred && !runtime_mod {
                tracing::info!(target:"shroudforge::runtime", mod_id=%id, "Mod changes require the next game start");
            }
            if metadata_unchanged
                && applied["enabled"] == desired["enabled"]
                && applied["settingValues"] == desired["settingValues"]
            {
                let mut active_manifest = manifest.clone();
                active_manifest.enabled = applied["enabled"].as_bool().unwrap_or(false);
                active_manifest.setting_values = applied["settingValues"].clone();
                self.loaded.insert(
                    id.clone(),
                    shroudforge_loader_config::config::revision(
                        &serde_json::to_vec(&active_manifest).expect("manifest is serializable"),
                    )
                    .into(),
                );
            }
        }
    }
}
