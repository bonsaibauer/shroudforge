use super::read_runtime_profiles::runtime_profile_ids;
use super::validate_message_name::normalize_message_name;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn save_setting(
    root: &Path,
    scope: &str,
    module: Option<&str>,
    key: &str,
    setting: &serde_json::Value,
) -> Result<(), String> {
    if scope == "general" {
        if !matches!(
            key,
            "compactMode"
                | "reducedMotion"
                | "showNativeModMenu"
                | "zoomPercent"
                | "usernameMode"
                | "messageNameFallback"
        ) {
            return Err("unsupported general setting".into());
        }
        if key == "showNativeModMenu" && !setting.is_boolean() {
            return Err("native mod menu setting must be boolean".into());
        }
        let setting = if key == "messageNameFallback" {
            normalize_message_name(setting.as_str().ok_or("message name must be text")?)?.into()
        } else if key == "usernameMode" {
            match setting.as_str() {
                Some("auto") => "auto".into(),
                Some("manual") => "manual".into(),
                _ => return Err("username mode must be auto or manual".into()),
            }
        } else if key == "zoomPercent" {
            let value = setting
                .as_u64()
                .filter(|value| (70..=150).contains(value))
                .ok_or("zoom percent must be an integer from 70 to 150")?;
            serde_json::Value::from(value)
        } else {
            setting.clone()
        };
        return shroudforge_loader_config::config::update_loader(root, |value| {
            value["general"][key] = setting;
            Ok(())
        });
    }
    if scope == "logging" {
        if key != "minimumLevel" {
            return Err("unsupported logging setting".into());
        }
        return shroudforge_loader_config::config::update_loader(root, |value| {
            value["logging"][key] = setting.clone();
            Ok(())
        });
    }
    if scope == "exports" {
        if key != "enabled" {
            return Err("unsupported export setting".into());
        }
        return shroudforge_loader_config::config::update_loader(root, |value| {
            value["exports"][key] = setting.clone();
            Ok(())
        });
    }
    if scope == "paths" {
        if !matches!(
            key,
            "mods" | "state" | "logs" | "cache" | "exports" | "updates" | "ui" | "runtime"
        ) {
            return Err("unsupported storage path".into());
        }
        let normalized = if setting.is_null() {
            serde_json::Value::Null
        } else {
            let text = setting
                .as_str()
                .ok_or("storage path must be a folder path or null")?;
            let selected = PathBuf::from(text);
            if !selected.is_absolute() || !selected.is_dir() {
                return Err("select an existing folder".into());
            }
            let selected = selected.canonicalize().map_err(|error| error.to_string())?;
            if selected == root.canonicalize().unwrap_or_else(|_| root.to_path_buf()) {
                return Err("a storage folder cannot be the game installation root".into());
            }
            serde_json::Value::String(selected.to_string_lossy().into_owned())
        };
        return shroudforge_loader_config::config::update_loader(root, |value| {
            value["paths"][key] = normalized;
            Ok(())
        });
    }
    if scope == "runtime" {
        if key != "profileId" {
            return Err("unsupported runtime setting".into());
        }
        if let Some(id) = setting.as_str() {
            if !runtime_profile_ids(root)
                .iter()
                .any(|profile| profile == id)
            {
                return Err(
                    "runtime profile was not found under shroudforge/runtime/profiles".into(),
                );
            }
        } else if !setting.is_null() {
            return Err(
                "runtime profile selection must be a profile id or null for automatic".into(),
            );
        }
        return shroudforge_loader_config::config::update_loader(root, |value| {
            value["runtime"]["profileId"] = setting.clone();
            Ok(())
        });
    }
    if scope != "module" {
        return Err("unsupported settings scope".into());
    }
    let module = module.ok_or("missing module setting target")?;
    let valid = match module {
        "modDetection" => matches!(key, "enabled" | "tolerateUnknownManifestFields"),
        "dllManager" => matches!(key, "enabled" | "profiles" | "excludedDllNames"),
        "debugConsole" => {
            matches!(
                key,
                "enabled"
                    | "popupMode"
                    | "popupCooldownSeconds"
                    | "defaultSource"
                    | "levelFilter"
                    | "autoScroll"
                    | "toggleKey"
                    | "refreshMilliseconds"
                    | "tailBytes"
            ) || key == "window.position"
        }
        "modloaderUi" => {
            matches!(
                key,
                "enabled" | "startPage" | "toggleKey" | "refreshMilliseconds"
            ) || key == "window.position"
        }
        "worldEditor" => matches!(key, "enabled" | "toggleKey" | "refreshMilliseconds"),
        "network" => matches!(key, "enabled" | "serverSteamId" | "allowedClientSteamIds"),
        "runtimeDiagnostics" => matches!(
            key,
            "enabled"
                | "jobId"
                | "intervalMilliseconds"
                | "maximumDurationSeconds"
                | "slowCallbackMilliseconds"
                | "continuous"
                | "onlyChanges"
        ),
        "updates" => key == "enabled",
        "updates.automatic" => matches!(key, "enabled" | "checkMinutes" | "queueUpdates"),
        "updates.system" => key == "includePrereleases",
        "updates.recycleBin" => matches!(key, "automaticCleanup" | "retentionDays"),
        _ => false,
    };
    if !valid {
        return Err("unsupported module setting".into());
    }
    if module == "updates"
        && key == "enabled"
        && setting == &serde_json::Value::Bool(true)
        && !shroudforge_loader_config::paths::updater_available(root)
    {
        return Err(super::report_command_result::missing_updater_message(root));
    }
    if module == "runtimeDiagnostics" && key == "jobId" {
        let job_id = setting.as_str().ok_or("diagnostic job id must be text")?;
        let config = shroudforge_loader_config::config::read_loader(root)?;
        let catalog = shroudforge_runtime_diagnostics::diagnostic_job_catalog(
            root,
            &config["modules"]["runtimeDiagnostics"],
        );
        if !catalog.jobs.iter().any(|job| job.id == job_id) {
            return Err(format!("diagnostic job was not found: {job_id}"));
        }
        return shroudforge_loader_config::config::update_loader(root, |config| {
            config["modules"]["runtimeDiagnostics"]["jobId"] = setting.clone();
            Ok(())
        });
    }
    if module == "runtimeDiagnostics"
        && key == "enabled"
        && setting == &serde_json::Value::Bool(false)
    {
        return shroudforge_loader_config::config::update_loader(root, |config| {
            config["modules"]["runtimeDiagnostics"]["enabled"] = setting.clone();
            config["modules"]["runtimeDiagnostics"]["sessionActive"] = false.into();
            Ok(())
        });
    }
    let normalized_setting = match (module, key) {
        (_, "toggleKey") if setting.is_null() => {
            let defaults = shroudforge_loader_config::config::read_loader_defaults()?;
            Some(
                defaults
                    .get("modules")
                    .and_then(|modules| modules.get(module))
                    .and_then(|module| module.get(key))
                    .cloned()
                    .ok_or("module hotkey has no default")?,
            )
        }
        ("dllManager", "profiles") => Some(shroudforge_package::dll_manager::validate_profiles(
            root, setting,
        )?),
        ("dllManager", "excludedDllNames") => {
            Some(shroudforge_package::dll_manager::normalize_excluded_dll_names(setting)?)
        }
        _ => None,
    };
    if module == "network" && key != "enabled" {
        let text = setting.as_str().ok_or("Network settings must be text")?;
        if key == "serverSteamId"
            && !text.is_empty()
            && (!(16..=20).contains(&text.len())
                || !text.bytes().all(|byte| byte.is_ascii_digit())
                || text.parse::<u64>().map_or(true, |id| id == 0))
        {
            return Err("server SteamID64 must be empty or contain 16 to 20 decimal digits".into());
        }
        if key == "allowedClientSteamIds" {
            if text.len() > 4096
                || text.split([',', ';']).map(str::trim).any(|peer| {
                    !peer.is_empty()
                        && (!(16..=20).contains(&peer.len())
                            || !peer.bytes().all(|byte| byte.is_ascii_digit())
                            || peer.parse::<u64>().map_or(true, |id| id == 0))
                })
            {
                return Err("authorized client IDs must be decimal SteamID64 values separated by commas or semicolons".into());
            }
        }
    }
    shroudforge_loader_config::config::update_loader(root, |config| {
        if key == "window.position" {
            config["modules"][module]["window"]["position"] = setting.clone();
        } else if module == "updates.automatic" {
            config["modules"]["updates"]["automatic"][key] = setting.clone();
        } else if module == "updates.system" {
            config["modules"]["updates"]["system"][key] = setting.clone();
        } else if module == "updates.recycleBin" {
            config["modules"]["updates"]["recycleBin"][key] = setting.clone();
        } else {
            config["modules"][module][key] = normalized_setting
                .clone()
                .unwrap_or_else(|| setting.clone());
        }
        if key == "toggleKey" || (key == "enabled" && setting == &serde_json::Value::Bool(true)) {
            shroudforge_package::validate_hotkey_configuration(root, config, None)?;
        }
        Ok(())
    })
}
