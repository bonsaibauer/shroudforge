use super::read_activity_log::read_activity;
use super::read_game_version::read_game_version;
use super::read_host_arguments::Arguments;
use super::read_missing_updater_announcement;
use super::read_release_status::read_release_status;
use super::read_runtime_profiles::active_runtime_profile;
use super::read_runtime_profiles::runtime_profile_ids;
use super::read_storage_paths::storage_paths;
use super::read_ui_configuration::load_user_config;
use super::save_language::is_valid_locale;
use super::ui_snapshot_types::ModInfo;
use super::ui_snapshot_types::Snapshot;
use super::ui_snapshot_types::SnapshotSettings;
use super::validate_message_name::default_message_name_fallback;
use super::validate_message_name::normalize_message_name;
use shroudforge_loader_config::runtime_status::runtime_is_active;
use shroudforge_package::config_revision;
use std::sync::Arc;
use std::sync::Mutex;

pub(crate) fn read_ui_snapshot(
    arguments: &Arguments,
    provider: &serde_json::Value,
    release: &Arc<Mutex<serde_json::Value>>,
    release_announcement: &Arc<Mutex<serde_json::Value>>,
    catalog: &Arc<Mutex<serde_json::Value>>,
    central: serde_json::Value,
    mods: Vec<ModInfo>,
    player_name_detection: &shroudforge_player_name::DetectionReport,
) -> (Snapshot, shroudforge_player_name::ResolvedName) {
    let logging_level = central
        .pointer("/logging/minimumLevel")
        .and_then(|v| v.as_str())
        .unwrap_or("INFO")
        .to_owned();
    let export_enabled = central
        .pointer("/exports/enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let release_snapshot = read_release_status(&arguments.root, &central, release);
    let version = release_snapshot["currentVersion"]
        .as_str()
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .to_owned();
    let username_mode = central
        .pointer("/general/usernameMode")
        .and_then(|value| value.as_str())
        .filter(|value| matches!(*value, "auto" | "manual"))
        .unwrap_or("auto")
        .to_owned();
    let fallback_username = central
        .pointer("/general/messageNameFallback")
        .and_then(|value| value.as_str())
        .and_then(|value| normalize_message_name(value).ok())
        .unwrap_or_else(default_message_name_fallback);
    let resolved_username =
        player_name_detection.resolve_name(username_mode == "manual", &fallback_username);
    let detected_username = player_name_detection.name.clone();
    let diagnostic_jobs = shroudforge_runtime_diagnostics::diagnostic_job_catalog(
        &arguments.root,
        &central["modules"]["runtimeDiagnostics"],
    );
    let notification_mods: Vec<_> = mods
        .iter()
        .map(|item| shroudforge_news::ModNotification {
            id: &item.id,
            name: &item.name,
            version: &item.version,
            ecosystem: item.ecosystem,
            changelog: &item.changelog,
        })
        .collect();
    let snapshot = Snapshot {
        diagnostics: shroudforge_runtime_diagnostics::status(&arguments.root),
        dll_candidates: shroudforge_package::dll_manager::discover_dll_candidates(&arguments.root),
        dll_chain_entries: shroudforge_package::dll_manager::chain_load_entries(&arguments.root),
        windows: shroudforge_loader_config::config::window_state(&arguments.root),
        configuration: super::read_configuration_status::configuration(
            &arguments.root,
            arguments.server,
            env!("CARGO_PKG_VERSION"),
        ),
        connected: !arguments.desktop && runtime_is_active(&arguments.root),
        mode: if arguments.desktop {
            "DESKTOP"
        } else if !arguments.server {
            "CLIENT"
        } else {
            "SERVER"
        },
        game_version: read_game_version(&arguments.root),
        version,
        notices: shroudforge_news::read_notifications(&arguments.root, &notification_mods),
        mods,
        activity: read_activity(&arguments.root).unwrap_or_default(),
        news_templates: shroudforge_news::read(&arguments.root)
            .ok()
            .and_then(|value| value.get("templates").cloned())
            .unwrap_or_else(|| serde_json::json!({})),
        read_notice_ids: shroudforge_news::read_news_state(&arguments.root),
        release: release_snapshot,
        release_announcement: release_announcement
            .lock()
            .map(|value| value.clone())
            .unwrap_or_else(|_| read_missing_updater_announcement::read_cached(&arguments.root)),
        update_queue: read_update_queue(&arguments.root),
        backup_management: shroudforge_game_assets::originals::management_status(
            &arguments.root,
            if arguments.server { "server" } else { "client" },
        ),
        settings: SnapshotSettings {
            config_revision: config_revision(&central),
            module_preferences: central["modules"].clone(),
            compact_mode: central
                .pointer("/general/compactMode")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            reduced_motion: central
                .pointer("/general/reducedMotion")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            show_native_mod_menu: central
                .pointer("/general/showNativeModMenu")
                .and_then(|value| value.as_bool())
                .unwrap_or(true),
            zoom_percent: central
                .pointer("/general/zoomPercent")
                .and_then(|value| value.as_u64())
                .filter(|value| (70..=150).contains(value))
                .unwrap_or(100) as u16,
            username_mode,
            detected_username,
            effective_username: resolved_username.name.clone(),
            username_source: resolved_username.source.key().to_owned(),
            message_name_fallback: fallback_username,
            log_level: logging_level,
            export_enabled,
            runtime_profile_id: central
                .pointer("/runtime/profileId")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            active_runtime_profile: active_runtime_profile(&arguments.root),
            runtime_profiles: runtime_profile_ids(&arguments.root),
            runtime_profile_directory: shroudforge_loader_config::paths::runtime_profiles_dir(
                &arguments.root,
            )
            .display()
            .to_string(),
            diagnostic_jobs: diagnostic_jobs.jobs,
            diagnostic_job_errors: diagnostic_jobs.errors,
            diagnostic_job_directory: diagnostic_jobs.directory,
            diagnostic_selected_job_id: diagnostic_jobs.selected_job_id,
            base_url: provider["baseUrl"].as_str().unwrap_or_default().to_owned(),
            project_id: provider["projectId"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            paths: storage_paths(&arguments.root),
        },
        catalog: catalog
            .lock()
            .map(|value| value.clone())
            .unwrap_or_else(|_| default_catalog_state()),
        locale: load_user_config(&arguments.root)
            .locale
            .filter(|locale| is_valid_locale(locale))
            .unwrap_or_else(|| "en-GB".into()),
    };
    (snapshot, resolved_username)
}

fn read_update_queue(root: &std::path::Path) -> serde_json::Value {
    std::fs::read(shroudforge_loader_config::paths::updates_dir(root).join("update-queue.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .filter(|value: &serde_json::Value| value["items"].is_array())
        .unwrap_or_else(|| serde_json::json!({"schemaVersion":1,"items":[],"run":null}))
}

pub(crate) fn default_catalog_state() -> serde_json::Value {
    serde_json::json!({"state":"idle","query":"","message":null,"items":[]})
}
