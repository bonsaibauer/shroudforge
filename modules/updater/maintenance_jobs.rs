use serde::Deserialize;
use std::{collections::HashSet, fs, path::Path};

const MAX_JOB_FILES: usize = 64;
const MAX_JOB_FILE_BYTES: u64 = 16 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JobDefinition {
    schema_version: u32,
    id: String,
    revision: u32,
    name: String,
    description: String,
    trigger: Trigger,
    repeatable: bool,
    action: Action,
    #[serde(default)]
    settings: Vec<SettingUpdate>,
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct SettingUpdate {
    path: Vec<String>,
    value: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Trigger {
    BeforeUpdate,
    AfterInstall,
}

impl Trigger {
    pub(crate) fn from_queue(value: &serde_json::Value) -> Self {
        match value.as_str() {
            Some("after-install") => Self::AfterInstall,
            _ => Self::BeforeUpdate,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::BeforeUpdate => "before-update",
            Self::AfterInstall => "after-install",
        }
    }
}

// Settings-only migrations use one declarative loader-config action. Keep changed
// setting paths and replacement values in each job JSON instead of adding a Rust
// action and handler per setting. Dedicated handlers are for migrations that need
// validation, conversion, or other domain-specific work.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Action {
    MigrateLegacyBackups,
    MigrateLegacyUpdateFiles,
    MigrateLegacyMods,
    MigrateLegacyEventLevels,
    MigrateLegacyToggleKeys,
    UpdateLoaderSettings,
    PurgeExpiredRecycleBin,
}

impl Action {
    fn is_migration(self) -> bool {
        matches!(
            self,
            Self::MigrateLegacyBackups
                | Self::MigrateLegacyUpdateFiles
                | Self::MigrateLegacyMods
                | Self::MigrateLegacyEventLevels
                | Self::MigrateLegacyToggleKeys
                | Self::UpdateLoaderSettings
        )
    }
}

// Key required jobs by ID so multiple JSON jobs can share a generic action.
const REQUIRED_MIGRATIONS: [(Trigger, &str, Action, u32); 6] = [
    (
        Trigger::BeforeUpdate,
        "migration.legacy-update-backups",
        Action::MigrateLegacyBackups,
        2,
    ),
    (
        Trigger::BeforeUpdate,
        "migration.legacy-update-files",
        Action::MigrateLegacyUpdateFiles,
        3,
    ),
    (
        Trigger::BeforeUpdate,
        "migration.legacy-shroudforge-mods",
        Action::MigrateLegacyMods,
        1,
    ),
    (
        Trigger::AfterInstall,
        "migration.notification-event-levels",
        Action::MigrateLegacyEventLevels,
        2,
    ),
    (
        Trigger::AfterInstall,
        "migration.debug-console-log-settings",
        Action::UpdateLoaderSettings,
        1,
    ),
    (
        Trigger::AfterInstall,
        "migration.legacy-toggle-key-settings",
        Action::MigrateLegacyToggleKeys,
        1,
    ),
];

const CURRENT_MOD_IDS: &[&str] = &[
    "sf-crafting-essentials",
    "sf-survival",
    "sf-unlimited-flight",
    "sf-unlock-blueprints",
    "world-editor",
];

const LEGACY_SHROUDFORGE_MODS: &[(&str, &str)] = &[
    ("flight", "mod.flight"),
    ("unlock-blueprints", "mod.unlock-blueprints"),
    ("sf-world-editor", "sf-world-editor"),
    ("no-stamina-loss", "mod.no-stamina-loss"),
    ("sf-no-stamina-loss", "sf-no-stamina-loss"),
    ("sf-auto-stamina-refill", "sf-auto-stamina-refill"),
    ("infinite-item-split", "mod.infinite-item-split"),
    ("sf-infinite-item-split", "sf-infinite-item-split"),
    ("infinite-item-use", "mod.infinite-item-use"),
    ("sf-infinite-item-use", "sf-infinite-item-use"),
    ("no-fall-damage", "mod.no-fall-damage"),
    ("sf-no-fall-damage", "sf-no-fall-damage"),
    ("no-resource-cost", "mod.no-resource-cost"),
    ("sf-no-resource-cost", "sf-no-resource-cost"),
    ("sf-production-time", "sf-production-time"),
];

const DEFAULT_JOBS: &[(&str, &str)] = &[
    (
        "migrate-legacy-update-files.json",
        include_str!("migrate-legacy-update-files.json"),
    ),
    (
        "migrate-legacy-backups.json",
        include_str!("migrate-legacy-backups.json"),
    ),
    (
        "migrate-legacy-mods.json",
        include_str!("migrate-legacy-mods.json"),
    ),
    (
        "migrate-legacy-event-levels.json",
        include_str!("migrate-legacy-event-levels.json"),
    ),
    (
        "migrate-debug-console-log-settings.json",
        include_str!("migrate-debug-console-log-settings.json"),
    ),
    (
        "migrate-legacy-toggle-key-settings.json",
        include_str!("migrate-legacy-toggle-key-settings.json"),
    ),
    (
        "purge-expired-recycle-bin.json",
        include_str!("purge-expired-recycle-bin.json"),
    ),
];

struct JobCatalog {
    jobs: Vec<JobDefinition>,
    errors: Vec<String>,
}

pub(crate) fn enqueue_before_update(root: &Path) -> Result<(), String> {
    enqueue_for_trigger(root, Trigger::BeforeUpdate)
}

fn enqueue_for_trigger(root: &Path, trigger: Trigger) -> Result<(), String> {
    let catalog = discover(root);
    for error in catalog.errors {
        let _ = shroudforge_logging::append(root, 'W', "updater", &error);
    }
    if REQUIRED_MIGRATIONS.iter().any(
        |(required_trigger, required_id, required_action, minimum_revision)| {
            *required_trigger == trigger
                && !catalog.jobs.iter().any(|job| {
                    job.trigger == trigger
                        && job.id == *required_id
                        && job.action == *required_action
                        && job.revision >= *minimum_revision
                })
        },
    ) {
        return Err(format!(
            "required updater migration jobs for {} are missing or invalid",
            trigger.as_str()
        ));
    }
    crate::update_queue_storage::update_update_queue(root, |queue| {
        if queue["run"]["state"] == "running" {
            return Err("the update queue is already running".into());
        }
        let items = queue["items"]
            .as_array_mut()
            .ok_or("update queue has an invalid items list")?;
        for job in &catalog.jobs {
            supersede_obsolete_revisions(items, job);
        }
        let mut triggered_jobs = catalog
            .jobs
            .into_iter()
            .filter(|job| job.trigger == trigger)
            .collect::<Vec<_>>();
        // Convert raw integer hotkeys before any settings migration validates the loader document.
        triggered_jobs.sort_by_key(|job| match job.action {
            Action::MigrateLegacyToggleKeys => 0,
            _ => 1,
        });
        for job in triggered_jobs {
            let id = format!("maintenance:{}:{}", job.id, job.revision);
            let existing_index = items.iter().position(|item| item["id"] == id);
            let Some(index) = existing_index else {
                items.push(queue_item(&id, job));
                continue;
            };
            if (items[index]["state"] == "complete" && job.repeatable)
                || matches!(items[index]["state"].as_str(), Some("error" | "cancelled"))
            {
                items[index] = queue_item(&id, job);
            }
        }
        Ok(())
    })
}

#[cfg(windows)]
pub(crate) fn run_after_install(root: &Path) -> Result<(), String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let Some(receipt) = read_install_receipt(root)? else {
        return Ok(());
    };
    let queue = crate::update_queue_storage::read_update_queue_value(root);
    if queue["lastAfterInstall"] == receipt {
        return Ok(());
    }
    let Some(_worker_mutex) = crate::acquire_queue_worker_mutex(root)? else {
        return Err(
            "post-install maintenance was deferred while the update queue worker is active".into(),
        );
    };
    let queue = crate::update_queue_storage::read_update_queue_value(root);
    if queue["lastAfterInstall"] == receipt {
        return Ok(());
    }
    if queue["run"]["state"] == "running" {
        if Trigger::from_queue(&queue["run"]["trigger"]) == Trigger::BeforeUpdate {
            return Err(
                "post-install maintenance was deferred until the previous update queue finishes"
                    .into(),
            );
        }
        crate::run_update_queue(root)?;
    }
    enqueue_for_trigger(root, Trigger::AfterInstall)?;
    let mut ids = crate::update_queue_storage::read_update_queue_value(root)["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| {
            item["kind"] == "maintenance"
                && Trigger::from_queue(&item["trigger"]) == Trigger::AfterInstall
                && item["state"] == "queued"
        })
        .filter_map(|item| item["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    // A queued settings update can validate loader.json, so migrate integer hotkeys first.
    ids.sort_by_key(|id| {
        if id.starts_with("maintenance:migration.legacy-toggle-key-settings:") {
            0
        } else {
            1
        }
    });

    if !ids.is_empty() {
        crate::update_queue_storage::update_update_queue(root, |queue| {
            if queue["run"]["state"] == "running" {
                return Err("the update queue is already running".into());
            }
            queue["run"] = serde_json::json!({
                "state": "running",
                "runId": crate::update_queue_storage::new_update_queue_run_id(),
                "trigger": Trigger::AfterInstall.as_str(),
                "waitForGame": false,
                "waitPid": 0,
                "showWindow": false,
                "itemIds": ids,
                "activeItemId": null,
                "installing": false,
                "startedAt": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            });
            Ok(())
        })?;
        crate::run_update_queue(root)?;

        let queue = crate::update_queue_storage::read_update_queue_value(root);
        let run_ids = queue["run"]["itemIds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>();
        let failures = queue["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| {
                run_ids.iter().any(|id| item["id"].as_str() == Some(id))
                    && item["state"] != "complete"
            })
            .map(|item| {
                format!(
                    "{}: {}",
                    item["title"].as_str().unwrap_or("maintenance job"),
                    item["message"]
                        .as_str()
                        .unwrap_or(item["state"].as_str().unwrap_or("failed"))
                )
            })
            .collect::<Vec<_>>();
        if !failures.is_empty() {
            return Err(failures.join(", "));
        }
    }

    crate::update_queue_storage::update_update_queue(root, |queue| {
        queue["lastAfterInstall"] = receipt;
        Ok(())
    })
}

#[cfg(windows)]
fn read_install_receipt(root: &Path) -> Result<Option<serde_json::Value>, String> {
    let Some(updates) = shroudforge_loader_config::config::read_state_section(root, "updates")?
    else {
        return Ok(None);
    };
    if updates["status"] != "installed" {
        return Ok(None);
    }
    let Some(version) = updates["version"]
        .as_str()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let build = updates["build"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| updates["build"].as_u64().map(|value| value.to_string()));
    let Some(build) = build else {
        return Ok(None);
    };
    let Some(installed_at) = updates["installedAt"].as_u64() else {
        return Ok(None);
    };
    Ok(Some(serde_json::json!({
        "version": version,
        "build": build,
        "installedAt": installed_at
    })))
}

fn supersede_obsolete_revisions(items: &mut [serde_json::Value], job: &JobDefinition) {
    let prefix = format!("maintenance:{}:", job.id);
    let current_id = format!("maintenance:{}:{}", job.id, job.revision);
    for item in items.iter_mut().filter(|item| {
        item["kind"] == "maintenance"
            && item["id"]
                .as_str()
                .is_some_and(|id| id.starts_with(&prefix) && id != current_id)
    }) {
        if item["state"] == "queued" || item["state"] == "error" {
            item["state"] = serde_json::Value::String("cancelled".into());
            item["selected"] = serde_json::Value::Bool(false);
            item["message"] = serde_json::Value::String(format!(
                "Superseded by maintenance job revision {}",
                job.revision
            ));
        }
    }
}

pub(crate) fn execute(root: &Path, item: &serde_json::Value) -> Result<(), String> {
    let action = action_from_item(item)?;
    crate::check_update_cancelled(root)?;
    match action {
        Action::MigrateLegacyBackups => {
            crate::recycle_bin::migrate_legacy_backups(root)?;
        }
        Action::MigrateLegacyUpdateFiles => {
            #[cfg(windows)]
            migrate_legacy_update_files(root)?;
        }
        Action::MigrateLegacyMods => migrate_legacy_mods(root)?,
        Action::MigrateLegacyEventLevels => migrate_legacy_event_levels(root)?,
        Action::MigrateLegacyToggleKeys => migrate_legacy_toggle_keys(root)?,
        Action::UpdateLoaderSettings => update_loader_settings(root, item)?,
        Action::PurgeExpiredRecycleBin => {
            if crate::recycle_bin::automatic_cleanup_enabled(root) {
                crate::recycle_bin::purge_expired(root)?;
            }
        }
    }
    crate::check_update_cancelled(root)
}

pub(crate) fn is_required_migration(item: &serde_json::Value) -> bool {
    action_from_item(item).is_ok_and(Action::is_migration)
}

fn action_from_item(item: &serde_json::Value) -> Result<Action, String> {
    let id = item["id"]
        .as_str()
        .ok_or("maintenance queue item is missing its ID")?;
    match item["operation"].as_str() {
        Some("MigrateLegacyBackups") => Ok(Action::MigrateLegacyBackups),
        Some("MigrateLegacyUpdateFiles") => Ok(Action::MigrateLegacyUpdateFiles),
        Some("MigrateLegacyMods") => Ok(Action::MigrateLegacyMods),
        Some("MigrateLegacyEventLevels") => Ok(Action::MigrateLegacyEventLevels),
        Some("MigrateLegacyToggleKeys") => Ok(Action::MigrateLegacyToggleKeys),
        Some("UpdateLoaderSettings") => Ok(Action::UpdateLoaderSettings),
        Some("PurgeExpiredRecycleBin") => Ok(Action::PurgeExpiredRecycleBin),
        _ => Err(format!("unsupported maintenance job in queue item {id}")),
    }
}

fn migrate_legacy_event_levels(root: &Path) -> Result<(), String> {
    shroudforge_news::migrate_legacy_event_levels(root).map(|_| ())
}

fn migrate_legacy_toggle_keys(root: &Path) -> Result<(), String> {
    shroudforge_loader_config::config::migrate_legacy_toggle_key_settings(root).map(|_| ())
}

fn update_loader_settings(root: &Path, item: &serde_json::Value) -> Result<(), String> {
    let settings: Vec<SettingUpdate> = serde_json::from_value(item["settings"].clone())
        .map_err(|error| format!("invalid loader settings in maintenance job: {error}"))?;
    validate_settings(Action::UpdateLoaderSettings, &settings)?;
    shroudforge_loader_config::config::update_loader(root, |config| {
        for setting in &settings {
            set_loader_setting(config, &setting.path, &setting.value)?;
        }
        Ok(())
    })
}

fn set_loader_setting(
    config: &mut serde_json::Value,
    path: &[String],
    value: &serde_json::Value,
) -> Result<(), String> {
    let Some((key, remaining)) = path.split_first() else {
        return Err("loader setting path cannot be empty".into());
    };
    let object = config
        .as_object_mut()
        .ok_or_else(|| format!("loader setting path is not an object: {}", path.join(".")))?;
    let setting = object
        .get_mut(key)
        .ok_or_else(|| format!("loader setting does not exist: {}", path.join(".")))?;
    if remaining.is_empty() {
        *setting = value.clone();
        return Ok(());
    }
    set_loader_setting(setting, remaining, value)
}

fn migrate_legacy_mods(root: &Path) -> Result<(), String> {
    let mods = shroudforge_loader_config::paths::mods_dir(root);
    let mut moved = 0;
    for (folder, expected_id) in LEGACY_SHROUDFORGE_MODS {
        crate::check_update_cancelled(root)?;
        if CURRENT_MOD_IDS.iter().any(|current| {
            current.eq_ignore_ascii_case(folder) || current.eq_ignore_ascii_case(expected_id)
        }) {
            return Err(format!(
                "legacy mod migration list contains a current mod ID: {folder} ({expected_id})"
            ));
        }
        let source = mods.join(folder);
        let metadata = match fs::symlink_metadata(&source) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "cannot inspect legacy mod folder {}: {error}",
                    source.display()
                ));
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "legacy mod path is not a regular folder: {}",
                source.display()
            ));
        }
        let manifest_path = source.join("mod.json");
        let manifest_metadata = fs::symlink_metadata(&manifest_path).map_err(|error| {
            format!(
                "cannot inspect legacy mod manifest {}: {error}",
                manifest_path.display()
            )
        })?;
        if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
            return Err(format!(
                "legacy mod manifest is not a regular file: {}",
                manifest_path.display()
            ));
        }
        let manifest = read_json(&manifest_path)?;
        let actual_id = manifest["id"]
            .as_str()
            .ok_or_else(|| format!("legacy mod manifest has no ID: {}", manifest_path.display()))?;
        if CURRENT_MOD_IDS
            .iter()
            .any(|current| current.eq_ignore_ascii_case(actual_id))
        {
            return Err(format!(
                "legacy mod folder contains current mod ID {actual_id}: {}",
                source.display()
            ));
        }
        if actual_id != *expected_id {
            return Err(format!(
                "legacy mod ID mismatch in {}: expected {}, found {actual_id}",
                source.display(),
                expected_id
            ));
        }
        crate::recycle_bin::recycle_mod_directory(
            root,
            &source,
            expected_id,
            "Removed legacy ShroudForge mod",
        )?;
        crate::check_update_cancelled(root)?;
        shroudforge_logging::write_activity(
            root,
            "Updates",
            "Legacy mod migration",
            "Moved to recycle bin",
            Some(&format!("{folder} ({expected_id})")),
            "info",
        );
        moved += 1;
    }
    if moved == 0 {
        shroudforge_logging::write_activity(
            root,
            "Updates",
            "Legacy mod migration",
            "No legacy mods found",
            None,
            "info",
        );
    }
    Ok(())
}

#[cfg(windows)]
fn migrate_legacy_update_files(root: &Path) -> Result<usize, String> {
    use std::fs;

    let updates = shroudforge_loader_config::paths::updates_dir(root);
    let task_name = legacy_updater_task_name(root)?;
    let task_removed = remove_legacy_updater_task(&task_name)?;
    let cache_removed = remove_legacy_updater_cache(&task_name)?;
    let mut migrated = usize::from(task_removed) + usize::from(cache_removed);
    if task_removed || cache_removed {
        let result = match (task_removed, cache_removed) {
            (true, true) => "Removed the legacy updater task and its AppData cache",
            (true, false) => "Removed the legacy updater task",
            (false, true) => "Removed the per-installation legacy updater AppData cache",
            (false, false) => unreachable!(),
        };
        shroudforge_logging::write_activity(
            root,
            "Updates",
            "Legacy updater cleanup",
            result,
            Some(&task_name),
            "info",
        );
    }
    let stage_request = updates.join("system-stage-request.json");
    let mut install_staged_update = false;
    if stage_request.is_file() {
        let request = normalize_legacy_stage_request(read_json(&stage_request)?)?;
        crate::system_stage::stage_system_update(root, &request)?;
        // Older stage requests created a worker-queue request after a successful
        // download. Complete that same install here because the new stager does
        // not create the legacy worker file.
        install_staged_update = true;
    }

    let mod_request = updates.join("mod-install-queue.json");
    if mod_request.is_file() {
        let request = read_json(&mod_request)?;
        let queue_item_id = request["queueItemId"]
            .as_str()
            .unwrap_or("migration.legacy-mod-install");
        let project_id = request["projectId"]
            .as_str()
            .ok_or("legacy mod install request has no projectId")?;
        let provider = serde_json::from_value(request["provider"].clone())
            .map_err(|error| format!("invalid legacy mod provider settings: {error}"))?;
        let update_mod_id = request["modId"].as_str();
        crate::mod_installation::install::install_mod_from_catalog(
            root,
            &provider,
            project_id,
            update_mod_id,
            queue_item_id,
        )?;
        fs::remove_file(&mod_request).map_err(|error| {
            format!("legacy mod request completed but could not be removed: {error}")
        })?;
        migrated += 1;
    }

    let worker_request = updates.join("worker-queue.json");
    if worker_request.is_file() {
        let request = read_json(&worker_request)?;
        if request["operation"] != "installPending" {
            return Err("unsupported legacy worker queue operation".into());
        }
        let pending = updates.join("pending");
        if pending.is_dir() {
            install_staged_update = true;
        }
    }

    if install_staged_update {
        let pending = updates.join("pending");
        if !pending.is_dir() {
            return Err("legacy system update request produced no staged package".into());
        }
        crate::system_install::install_staged(root, &pending)?;
    }

    if stage_request.is_file() {
        fs::remove_file(&stage_request).map_err(|error| {
            format!("legacy system stage request completed but could not be removed: {error}")
        })?;
        migrated += 1;
    }
    if worker_request.is_file() {
        fs::remove_file(&worker_request).map_err(|error| {
            format!("legacy worker request completed but could not be removed: {error}")
        })?;
        migrated += 1;
    }
    Ok(migrated)
}

fn normalize_legacy_stage_request(
    mut request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let version = request["version"]
        .as_str()
        .ok_or("legacy system stage request has no version")?;
    let (base_version, build) = crate::parse_release_tag(version)?;
    request["baseVersion"] = serde_json::Value::String(base_version);
    request["build"] = serde_json::json!(build);
    Ok(request)
}

#[cfg(windows)]
fn legacy_updater_task_name(root: &Path) -> Result<String, String> {
    let canonical = root.canonicalize().map_err(|error| error.to_string())?;
    let mut hash = 0xcbf29ce484222325u64;
    for byte in canonical.to_string_lossy().to_ascii_lowercase().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("ShroudForgeUpdater_{hash:016x}"))
}

#[cfg(windows)]
fn remove_legacy_updater_task(name: &str) -> Result<bool, String> {
    use std::process::Command;

    let query = Command::new("schtasks.exe")
        .args(["/Query", "/TN", name])
        .output()
        .map_err(|error| format!("could not inspect legacy updater task: {error}"))?;
    if !query.status.success() {
        return Ok(false);
    }
    let deletion = Command::new("schtasks.exe")
        .args(["/Delete", "/TN", name, "/F"])
        .output()
        .map_err(|error| format!("could not remove legacy updater task: {error}"))?;
    if !deletion.status.success() {
        return Err(format!(
            "could not remove legacy updater task {}: {}",
            name,
            String::from_utf8_lossy(&deletion.stderr).trim()
        ));
    }
    Ok(true)
}

#[cfg(windows)]
fn remove_legacy_updater_cache(task_name: &str) -> Result<bool, String> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .ok_or("LOCALAPPDATA is unavailable while cleaning the legacy updater cache")?;
    remove_legacy_updater_cache_at(Path::new(&local_app_data), task_name)
}

fn remove_legacy_updater_cache_at(local_app_data: &Path, task_name: &str) -> Result<bool, String> {
    let Some(hash) = task_name.strip_prefix("ShroudForgeUpdater_") else {
        return Err("legacy updater cache has an invalid installation identifier".into());
    };
    if hash.len() != 16 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("legacy updater cache has an invalid installation identifier".into());
    }

    let local_app_data = local_app_data.canonicalize().map_err(|error| {
        format!(
            "cannot resolve LOCALAPPDATA {} while cleaning the legacy updater cache: {error}",
            local_app_data.display()
        )
    })?;
    let shroudforge = local_app_data.join("ShroudForge");
    let updater = shroudforge.join("Updater");
    let installation_cache = updater.join(task_name);
    for (path, label) in [
        (&shroudforge, "ShroudForge AppData directory"),
        (&updater, "legacy updater cache directory"),
        (&installation_cache, "installation-specific updater cache"),
    ] {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(format!(
                    "cannot inspect {label} {}: {error}",
                    path.display()
                ));
            }
        };
        if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
            return Err(format!(
                "refusing to remove legacy updater cache through a non-directory or link at {}",
                path.display()
            ));
        }
    }

    fs::remove_dir_all(&installation_cache).map_err(|error| {
        format!(
            "cannot remove legacy updater cache {}: {error}",
            installation_cache.display()
        )
    })?;
    remove_empty_legacy_updater_directory(&updater)?;
    remove_empty_legacy_updater_directory(&shroudforge)?;
    Ok(true)
}

fn remove_empty_legacy_updater_directory(path: &Path) -> Result<(), String> {
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                || error.kind() == std::io::ErrorKind::DirectoryNotEmpty =>
        {
            Ok(())
        }
        Err(error) => Err(format!(
            "cannot remove empty legacy updater directory {}: {error}",
            path.display()
        )),
    }
}

fn is_symlink_or_reparse_point(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        return metadata.file_attributes() & 0x400 != 0;
    }
    #[cfg(not(windows))]
    false
}

fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("invalid {}: {error}", path.display()))
}

fn queue_item(id: &str, job: JobDefinition) -> serde_json::Value {
    let mut item = serde_json::json!({
        "id": id,
        "kind": "maintenance",
        "operation": format!("{:?}", job.action),
        "title": job.name,
        "version": job.revision,
        "trigger": job.trigger.as_str(),
        "state": "queued",
        "selected": job.trigger == Trigger::BeforeUpdate,
        "required": job.action.is_migration(),
        "message": job.description
    });
    if !job.settings.is_empty() {
        item["settings"] = serde_json::json!(job.settings);
    }
    item
}

fn discover(root: &Path) -> JobCatalog {
    let directory = shroudforge_loader_config::paths::updater_jobs_dir(root);
    let mut errors = Vec::new();
    if let Err(error) = fs::create_dir_all(&directory) {
        errors.push(format!(
            "Cannot read maintenance jobs folder {}: {error}",
            directory.display()
        ));
        return JobCatalog {
            jobs: Vec::new(),
            errors,
        };
    }
    for (name, contents) in DEFAULT_JOBS {
        let path = directory.join(name);
        if path.exists() {
            match *name {
                "migrate-legacy-event-levels.json" => {
                    if let Err(error) = upgrade_legacy_event_job_phase(&path) {
                        errors.push(error);
                    }
                }
                "migrate-legacy-update-files.json" => {
                    if let Err(error) = upgrade_legacy_update_files_job_revision(&path) {
                        errors.push(error);
                    }
                }
                _ => {}
            }
            continue;
        }
        if let Err(error) = fs::write(&path, contents) {
            errors.push(format!(
                "Cannot install default maintenance job {}: {error}",
                path.display()
            ));
        }
    }
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(format!(
                "Cannot read maintenance jobs folder {}: {error}",
                directory.display()
            ));
            return JobCatalog {
                jobs: Vec::new(),
                errors,
            };
        }
    };
    let mut files = entries
        .filter_map(|entry| match entry {
            Ok(entry)
                if entry.file_type().is_ok_and(|kind| kind.is_file())
                    && entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("json")) =>
            {
                Some(entry.path())
            }
            Ok(_) => None,
            Err(error) => {
                errors.push(format!("Cannot read maintenance job entry: {error}"));
                None
            }
        })
        .collect::<Vec<_>>();
    files.sort();
    if files.len() > MAX_JOB_FILES {
        errors.push(format!(
            "Maintenance jobs folder contains more than {MAX_JOB_FILES} JSON files"
        ));
        files.truncate(MAX_JOB_FILES);
    }
    let mut ids = HashSet::new();
    let mut jobs = Vec::new();
    for path in files {
        match fs::metadata(&path) {
            Ok(metadata) if metadata.len() <= MAX_JOB_FILE_BYTES => {}
            Ok(_) => {
                errors.push(format!(
                    "Maintenance job exceeds 16 KiB: {}",
                    path.display()
                ));
                continue;
            }
            Err(error) => {
                errors.push(format!(
                    "Cannot inspect maintenance job {}: {error}",
                    path.display()
                ));
                continue;
            }
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                errors.push(format!(
                    "Cannot read maintenance job {}: {error}",
                    path.display()
                ));
                continue;
            }
        };
        let job: JobDefinition = match serde_json::from_slice(&bytes) {
            Ok(job) => job,
            Err(error) => {
                errors.push(format!(
                    "Invalid maintenance job {}: {error}",
                    path.display()
                ));
                continue;
            }
        };
        if let Err(error) = validate(&job) {
            errors.push(format!(
                "Invalid maintenance job {}: {error}",
                path.display()
            ));
            continue;
        }
        if !ids.insert(job.id.clone()) {
            errors.push(format!("Duplicate maintenance job ID: {}", job.id));
            continue;
        }
        jobs.push(job);
    }
    JobCatalog { jobs, errors }
}

fn upgrade_legacy_event_job_phase(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect maintenance job {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "cannot upgrade non-regular maintenance job {}",
            path.display()
        ));
    }
    let mut job = read_json(path)?;
    if job["id"] != "migration.notification-event-levels"
        || job["action"] != "migrate-legacy-event-levels"
        || job["revision"] != 1
        || job["trigger"] != "before-update"
    {
        return Ok(());
    }
    job["revision"] = serde_json::json!(2);
    job["trigger"] = serde_json::json!("after-install");
    let contents = serde_json::to_vec_pretty(&job)
        .map_err(|error| format!("cannot encode maintenance job {}: {error}", path.display()))?;
    fs::write(path, contents)
        .map_err(|error| format!("cannot update maintenance job {}: {error}", path.display()))
}

fn upgrade_legacy_update_files_job_revision(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect maintenance job {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "cannot upgrade non-regular maintenance job {}",
            path.display()
        ));
    }
    let mut job = read_json(path)?;
    if job["id"] != "migration.legacy-update-files"
        || job["action"] != "migrate-legacy-update-files"
        || job["revision"] != 2
        || job["trigger"] != "before-update"
    {
        return Ok(());
    }
    job["revision"] = serde_json::json!(3);
    job["name"] = serde_json::json!("Migrate legacy updater files");
    job["description"] = serde_json::json!(
        "Removes the previous scheduled task and its per-installation AppData cache, then finishes pending requests from older updater versions."
    );
    let contents = serde_json::to_vec_pretty(&job)
        .map_err(|error| format!("cannot encode maintenance job {}: {error}", path.display()))?;
    fs::write(path, contents)
        .map_err(|error| format!("cannot update maintenance job {}: {error}", path.display()))
}

fn validate(job: &JobDefinition) -> Result<(), String> {
    if job.schema_version != 1 {
        return Err("schemaVersion must be 1".into());
    }
    if !(job.id.starts_with("maintenance.") || job.id.starts_with("migration."))
        || job.id.len() > 100
        || !job.id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
    {
        return Err("id must use the maintenance. or migration. namespace".into());
    }
    if job.revision == 0 {
        return Err("revision must be greater than zero".into());
    }
    if job.name.trim().is_empty() || job.name.chars().count() > 100 {
        return Err("name must contain 1 to 100 characters".into());
    }
    if job.description.chars().count() > 500 {
        return Err("description must contain at most 500 characters".into());
    }
    validate_settings(job.action, &job.settings)?;
    Ok(())
}

fn validate_settings(action: Action, settings: &[SettingUpdate]) -> Result<(), String> {
    if action != Action::UpdateLoaderSettings {
        return if settings.is_empty() {
            Ok(())
        } else {
            Err("settings are only supported by the update-loader-settings action".into())
        };
    }
    if settings.is_empty() {
        return Err("update-loader-settings requires at least one setting".into());
    }
    for setting in settings {
        if setting.path.len() < 3
            || setting.path.first().map(String::as_str) != Some("modules")
            || setting.path.iter().any(|part| part.trim().is_empty())
        {
            return Err(
                "setting paths must target an existing module setting under modules".into(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legacy_updater_cache_cleanup_is_scoped_to_one_installation() {
        let local_app_data = tempfile::tempdir().unwrap();
        let task_name = "ShroudForgeUpdater_0123456789abcdef";
        let cache = local_app_data
            .path()
            .join("ShroudForge/Updater")
            .join(task_name);
        let cached_updater = cache.join("123456789/shroudforge-updater.exe");
        let other_cache_file = local_app_data
            .path()
            .join("ShroudForge/Updater/ShroudForgeUpdater_fedcba9876543210/keep.exe");
        fs::create_dir_all(cached_updater.parent().unwrap()).unwrap();
        fs::create_dir_all(other_cache_file.parent().unwrap()).unwrap();
        fs::write(&cached_updater, b"legacy updater").unwrap();
        fs::write(&other_cache_file, b"another installation").unwrap();

        assert!(remove_legacy_updater_cache_at(local_app_data.path(), task_name).unwrap());

        assert!(!cache.exists());
        assert_eq!(fs::read(other_cache_file).unwrap(), b"another installation");
        assert!(!remove_legacy_updater_cache_at(local_app_data.path(), task_name).unwrap());
    }

    #[test]
    fn legacy_updater_cache_cleanup_rejects_invalid_installation_identifiers() {
        let local_app_data = tempfile::tempdir().unwrap();
        let error =
            remove_legacy_updater_cache_at(local_app_data.path(), "..\\outside").unwrap_err();
        assert!(error.contains("invalid installation identifier"));
    }

    #[test]
    fn legacy_update_job_revision_upgrade_queues_appdata_cleanup() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("migrate-legacy-update-files.json");
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({
                "schemaVersion": 1,
                "id": "migration.legacy-update-files",
                "revision": 2,
                "name": "Migrate legacy updater requests",
                "description": "Removes the previous scheduled task and finishes pending requests from older updater versions.",
                "trigger": "before-update",
                "repeatable": false,
                "action": "migrate-legacy-update-files"
            }))
            .unwrap(),
        )
        .unwrap();

        upgrade_legacy_update_files_job_revision(&path).unwrap();

        let upgraded: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(upgraded["revision"], 3);
        assert_eq!(upgraded["name"], "Migrate legacy updater files");
        assert!(
            upgraded["description"]
                .as_str()
                .unwrap()
                .contains("AppData cache")
        );
    }

    #[test]
    fn legacy_event_migration_preserves_state_and_loader_settings() {
        let root = tempfile::tempdir().unwrap();
        let config_path = shroudforge_loader_config::paths::loader_config(root.path());
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let config_bytes = br#"{"schemaVersion":1,"modules":{"modloaderUi":{"enabled":false}},"paths":{"state":"custom-state"}}"#;
        fs::write(&config_path, config_bytes).unwrap();

        let state_path = shroudforge_loader_config::paths::state_file(root.path());
        assert_eq!(state_path, root.path().join("custom-state/state.json"));
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        let before = json!({
            "schemaVersion": 1,
            "events": {
                "success.json": {"id":"install-success","title":"Installed","message":"Done","level":"success","values":{"keep":true}},
                "update.json": {"id":"install-update","title":"Updated","message":"Done","level":"update"},
                "warning.json": {"id":"install-warning","title":"Warning","message":"Details","level":"warning"},
                "modern.json": {"id":"install-info","title":"Info","message":"Details","level":"info"}
            },
            "windows": {"modloader": {"width": 900, "height": 700}},
            "updates": {"lastResult": {"version": "1.2.3", "status": "complete"}}
        });
        fs::write(&state_path, serde_json::to_vec_pretty(&before).unwrap()).unwrap();

        execute(
            root.path(),
            &json!({"id":"maintenance:migration.notification-event-levels:2","operation":"MigrateLegacyEventLevels"}),
        )
        .unwrap();

        let migrated: serde_json::Value =
            serde_json::from_slice(&fs::read(state_path).unwrap()).unwrap();
        let mut expected = before;
        expected["events"]["success.json"]["level"] = json!("info");
        expected["events"]["update.json"]["level"] = json!("info");
        expected["events"]["warning.json"]["level"] = json!("warn");
        assert_eq!(migrated, expected);
        assert_eq!(fs::read(config_path).unwrap(), config_bytes);
    }

    #[test]
    fn legacy_event_migration_is_installed_as_a_required_after_install_job() {
        let root = tempfile::tempdir().unwrap();
        enqueue_for_trigger(root.path(), Trigger::AfterInstall).unwrap();

        let queue_path = crate::update_queue_storage::update_queue_path(root.path());
        let queue: serde_json::Value =
            serde_json::from_slice(&fs::read(queue_path).unwrap()).unwrap();
        let item = queue["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "maintenance:migration.notification-event-levels:2")
            .unwrap();
        assert_eq!(item["operation"], "MigrateLegacyEventLevels");
        assert_eq!(item["required"], true);
        assert_eq!(item["state"], "queued");
        assert_eq!(item["trigger"], "after-install");
        assert_eq!(item["selected"], false);
    }

    #[cfg(windows)]
    #[test]
    fn after_install_migration_runs_for_a_string_build_receipt() {
        let root = tempfile::tempdir().unwrap();
        let state_path = shroudforge_loader_config::paths::state_file(root.path());
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        fs::write(
            &state_path,
            serde_json::to_vec_pretty(&json!({
                "schemaVersion": 1,
                "updates": {
                    "status": "installed",
                    "version": "1.7.3",
                    "build": "39293575337",
                    "installedAt": 12345
                },
                "events": {
                    "event.json": {"id":"old-event","title":"Old event","message":"Done","level":"success"}
                }
            }))
            .unwrap(),
        )
        .unwrap();

        run_after_install(root.path()).unwrap();

        let migrated: serde_json::Value =
            serde_json::from_slice(&fs::read(state_path).unwrap()).unwrap();
        assert_eq!(migrated["events"]["event.json"]["level"], "info");
        let queue = crate::update_queue_storage::read_update_queue_value(root.path());
        assert_eq!(queue["lastAfterInstall"]["build"], "39293575337");
        assert_eq!(
            queue["items"][0]["state"], "complete",
            "post-install migration must complete"
        );
    }

    #[cfg(windows)]
    #[test]
    fn updater_request_stops_when_required_post_install_job_is_invalid() {
        let root = tempfile::tempdir().unwrap();
        let state_path = shroudforge_loader_config::paths::state_file(root.path());
        fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        fs::write(
            &state_path,
            serde_json::to_vec_pretty(&json!({
                "schemaVersion": 1,
                "updates": {
                    "status": "installed",
                    "version": "1.7.0",
                    "build": 1,
                    "installedAt": 12345
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let jobs = shroudforge_loader_config::paths::updater_jobs_dir(root.path());
        fs::create_dir_all(&jobs).unwrap();
        fs::write(jobs.join("migrate-legacy-event-levels.json"), "{").unwrap();

        let error =
            crate::run_request(root.path(), &json!({"command":"check-system-update"})).unwrap_err();

        assert!(error.contains("required updater migration jobs for after-install"));
    }
}
