//! Resolve the Modloader name from the existing runtime status publication.
//! The native main-menu owner maintains the character cache. This module never
//! reads game memory, starts a worker, or persists a second character cache.
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectionStatus {
    Detected,
    RuntimeUnavailable,
    SelectionNotFound,
}

impl DetectionStatus {
    pub fn description(self) -> &'static str {
        match self {
            Self::Detected => {
                "The selected character name is available in the native runtime cache"
            }
            Self::RuntimeUnavailable => {
                "No current status from the connected game runtime is available"
            }
            Self::SelectionNotFound => "The native runtime has no selected character name",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameSource {
    Live,
    Fallback,
}

impl NameSource {
    pub fn key(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Fallback => "fallback",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Live => "Selected character name from the native runtime cache",
            Self::Fallback => "Configured fallback used",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedName {
    pub name: String,
    pub source: NameSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectionReport {
    pub name: Option<String>,
    pub status: DetectionStatus,
    pub process_id: Option<u32>,
    pub detail: Option<String>,
}

impl DetectionReport {
    pub fn resolve_name(&self, manual_mode: bool, fallback: &str) -> ResolvedName {
        if !manual_mode && let Some(name) = &self.name {
            return ResolvedName {
                name: name.clone(),
                source: NameSource::Live,
            };
        }
        ResolvedName {
            name: fallback.to_owned(),
            source: NameSource::Fallback,
        }
    }
}

pub fn detect_active_character(root: &Path) -> DetectionReport {
    detect_active_character_for_process(root, None)
}

/// Read the same published native cache used by Lua. Repeated UI refreshes do
/// not query the game's character model or initiate character-name discovery.
pub fn detect_active_character_for_process(
    root: &Path,
    game_process_id: Option<u32>,
) -> DetectionReport {
    let mut report = DetectionReport {
        name: None,
        status: DetectionStatus::RuntimeUnavailable,
        process_id: game_process_id,
        detail: None,
    };
    let Some(pid) = shroudforge_loader_config::runtime_status::runtime_pid(root)
        .filter(|pid| *pid != 0 && game_process_id.is_none_or(|expected| expected == *pid))
    else {
        return report;
    };
    report.process_id = Some(pid);
    let status = match shroudforge_loader_config::config::read_document(
        root,
        shroudforge_runtime_diagnostics::documents::MOD_STATUS,
    ) {
        Ok(status) => status,
        Err(error) => {
            report.detail = Some(error);
            return report;
        }
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if status["running"] != true
        || status["pid"].as_u64() != Some(u64::from(pid))
        || !status["updatedAt"]
            .as_u64()
            .is_some_and(|at| at <= now && now - at <= 5)
    {
        return report;
    }
    report.name = status
        .pointer("/runtimeProvider/selectedCharacterName")
        .and_then(serde_json::Value::as_str)
        .filter(|name| {
            !name.trim().is_empty()
                && name.chars().count() <= 80
                && !name.chars().any(char::is_control)
        })
        .map(str::to_owned);
    report.status = if report.name.is_some() {
        DetectionStatus::Detected
    } else {
        DetectionStatus::SelectionNotFound
    };
    report
}

#[cfg(test)]
mod tests {
    use super::{DetectionReport, DetectionStatus, NameSource};

    fn report(name: Option<&str>) -> DetectionReport {
        DetectionReport {
            name: name.map(str::to_owned),
            status: if name.is_some() {
                DetectionStatus::Detected
            } else {
                DetectionStatus::SelectionNotFound
            },
            process_id: None,
            detail: None,
        }
    }

    #[test]
    fn live_name_has_precedence_over_fallback() {
        let resolved = report(Some("BonsaiBauer")).resolve_name(false, "Flameborn");
        assert_eq!(resolved.name, "BonsaiBauer");
        assert_eq!(resolved.source, NameSource::Live);
    }

    #[test]
    fn fallback_is_used_until_a_name_has_been_detected() {
        let resolved = report(None).resolve_name(false, "Flameborn");
        assert_eq!(resolved.name, "Flameborn");
        assert_eq!(resolved.source, NameSource::Fallback);
    }

    #[test]
    fn manual_mode_always_uses_fallback() {
        let resolved = report(Some("BonsaiBauer")).resolve_name(true, "Flameborn");
        assert_eq!(resolved.name, "Flameborn");
        assert_eq!(resolved.source, NameSource::Fallback);
    }
}
