use std::fs;
use std::path::Path;

pub(crate) fn runtime_profile_ids(root: &Path) -> Vec<String> {
    let directory = shroudforge_loader_config::paths::runtime_profiles_dir(root);
    let mut profiles = Vec::new();
    let mut pending = vec![directory];
    while let Some(current) = pending.pop() {
        if current == shroudforge_loader_config::paths::mod_profiles_dir(root) {
            continue;
        }
        let Ok(entries) = fs::read_dir(current) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                pending.push(entry.path());
            } else if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                if let Ok(bytes) = fs::read(entry.path()) {
                    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        if let Some(id) = value.get("id").and_then(serde_json::Value::as_str) {
                            profiles.push(id.to_owned());
                        }
                    }
                }
            }
        }
    }
    profiles.sort();
    profiles.dedup();
    profiles
}

pub(crate) fn active_runtime_profile(root: &Path) -> Option<String> {
    let status = shroudforge_loader_config::config::read_document(
        root,
        shroudforge_runtime_diagnostics::documents::MOD_STATUS,
    )
    .ok()?;
    profile_from_status(&status)
}

fn profile_from_status(status: &serde_json::Value) -> Option<String> {
    if let Some(profile) = status
        .pointer("/runtimeProvider/resourceService/profileId")
        .and_then(serde_json::Value::as_str)
        .filter(|profile| !profile.is_empty())
    {
        return Some(profile.to_owned());
    }
    let detail = status.pointer("/runtimeProvider/detail")?.as_str()?;
    let profile = detail.split("profile=").nth(1)?.split_whitespace().next()?;
    Some(profile.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn game_profile_discovery_display_prefers_structured_resource_profile() {
        let status = json!({"runtimeProvider": {
            "detail": "profile=missing-or-ambiguous-profile game_thread=unavailable",
            "resourceService": {"profileId": "beta_enshrouded-client-26404966"}
        }});
        assert_eq!(
            profile_from_status(&status).as_deref(),
            Some("beta_enshrouded-client-26404966")
        );
    }

    #[test]
    fn game_profile_discovery_display_preserves_legacy_profile_status() {
        let status = json!({"runtimeProvider": {
            "detail": "types=541/1541 profile=enshrouded-client-1076226 game_thread=ready"
        }});
        assert_eq!(
            profile_from_status(&status).as_deref(),
            Some("enshrouded-client-1076226")
        );
        assert_eq!(profile_from_status(&json!({})), None);
    }
}
