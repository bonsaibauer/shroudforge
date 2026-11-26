#[cfg(windows)]
use crate::headless_check_system_updates;

#[derive(Clone, Copy)]
enum UpdateKind {
    System,
    Mods,
    Schematics,
    Modpacks,
}

fn configured_update_check_interval(settings: Option<&serde_json::Value>) -> std::time::Duration {
    std::time::Duration::from_secs(crate::update_check::system_update_check_interval_seconds(
        settings,
    ))
}

fn configured_retry_delay(root: &std::path::Path) -> std::time::Duration {
    let settings = shroudforge_loader_config::config::read_loader(root).ok();
    configured_update_check_interval(settings.as_ref())
}

fn update_check_delay(root: &std::path::Path, kind: UpdateKind) -> std::time::Duration {
    let settings = shroudforge_loader_config::config::read_loader(root).ok();
    let module_enabled = settings
        .as_ref()
        .and_then(|value| value.pointer("/modules/updates/enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let enabled = module_enabled
        && settings
            .as_ref()
            .and_then(|value| value.pointer("/modules/updates/automatic/enabled"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
    if !enabled {
        return std::time::Duration::from_secs(86_400);
    }
    let interval_seconds = configured_update_check_interval(settings.as_ref()).as_secs();
    let last_check_at = match kind {
        UpdateKind::System => crate::update_check::last_system_update_check_at(settings.as_ref()),
        UpdateKind::Mods => settings
            .as_ref()
            .and_then(|value| value.pointer("/modules/updates/mods/lastCheckAt"))
            .and_then(serde_json::Value::as_u64),
        UpdateKind::Schematics => settings
            .as_ref()
            .and_then(|value| value.pointer("/modules/updates/schematics/lastCheckAt"))
            .and_then(serde_json::Value::as_u64),
        UpdateKind::Modpacks => settings
            .as_ref()
            .and_then(|value| value.pointer("/modules/updates/modpacks/lastCheckAt"))
            .and_then(serde_json::Value::as_u64),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    std::time::Duration::from_secs(
        last_check_at
            .map(|checked| interval_seconds.saturating_sub(now.saturating_sub(checked)))
            .unwrap_or(0),
    )
}

pub(crate) fn system_update_check_delay(root: &std::path::Path) -> std::time::Duration {
    update_check_delay(root, UpdateKind::System)
}

pub(crate) fn mod_update_check_delay(root: &std::path::Path) -> std::time::Duration {
    [
        UpdateKind::Mods,
        UpdateKind::Schematics,
        UpdateKind::Modpacks,
    ]
    .into_iter()
    .map(|kind| update_check_delay(root, kind))
    .min()
    .unwrap_or_default()
}

pub(crate) fn check_due_mod_updates(root: &std::path::Path) -> std::time::Instant {
    for (kind, name) in [
        (UpdateKind::Mods, "mod"),
        (UpdateKind::Schematics, "schematic"),
        (UpdateKind::Modpacks, "modpack"),
    ] {
        if !update_check_delay(root, kind).is_zero() {
            continue;
        }
        let result = if matches!(kind, UpdateKind::Mods) {
            crate::mod_installation::catalog::check_catalog_mod_updates(root, false)
        } else {
            crate::mod_installation::check_catalog_content_updates::check_catalog_content_updates(
                root, name, false,
            )
        };
        match result {
            Ok(_) => {}
            Err(error) => {
                let _ = shroudforge_logging::append(
                    root,
                    'W',
                    "updates",
                    &format!("Automatic {name} update check failed: {error}"),
                );
            }
        }
    }
    let delay = mod_update_check_delay(root);
    let next = if delay.is_zero() {
        let delay = mod_update_check_delay(root);
        if delay.is_zero() {
            configured_retry_delay(root)
        } else {
            delay
        }
    } else {
        delay
    };
    std::time::Instant::now() + next
}

#[cfg(windows)]
pub(crate) fn check_due_system_update(root: &std::path::Path) -> std::time::Instant {
    if !crate::update_check::updates_enabled(root) {
        return std::time::Instant::now() + std::time::Duration::from_secs(86_400);
    }
    let delay = system_update_check_delay(root);
    if delay.is_zero() {
        match headless_check_system_updates(
            root,
            crate::update_check::SystemUpdateCheckMode::Automatic,
        ) {
            Ok(message) => {
                let _ = shroudforge_logging::append(
                    root,
                    if message == "ShroudForge is up to date." {
                        'D'
                    } else {
                        'I'
                    },
                    "updates",
                    &format!("Headless update check: {message}"),
                );
            }
            Err(error) => {
                let _ = shroudforge_logging::append(
                    root,
                    'W',
                    "updates",
                    &format!("Headless update check failed: {error}"),
                );
            }
        }
    }
    let next_delay = if delay.is_zero() {
        let next = system_update_check_delay(root);
        if next.is_zero() {
            configured_retry_delay(root)
        } else {
            next
        }
    } else {
        delay
    };
    std::time::Instant::now() + next_delay
}
