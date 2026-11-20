use crate::ModTarget;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

const MAX_REPORT_MODS: usize = 512;
const MAX_REPORT_PROBLEMS: usize = 128;
const MAX_VERSION_BYTES: usize = 128;
const MAX_PROBLEM_DETAIL_CHARS: usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientModVersion {
    pub id: String,
    pub version: String,
    pub enabled: bool,
    pub target: ModTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientCompatibilityProblem {
    pub code: String,
    pub state: String,
    pub mod_id: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientCompatibilityReport {
    pub schema_version: u8,
    pub loader_version: String,
    pub player_name: Option<String>,
    pub mods: Vec<ClientModVersion>,
    pub problems: Vec<ClientCompatibilityProblem>,
}

impl ClientCompatibilityReport {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != 1 {
            return Err("unsupported compatibility report schema");
        }
        if self.loader_version.trim().is_empty() || self.loader_version.len() > MAX_VERSION_BYTES {
            return Err("invalid client Modloader version");
        }
        if self.player_name.as_deref().is_some_and(|name| {
            name.trim().is_empty()
                || name.chars().count() > 80
                || name.chars().any(char::is_control)
        }) {
            return Err("invalid detected player name");
        }
        if self.mods.len() > MAX_REPORT_MODS {
            return Err("too many mods in compatibility report");
        }
        let mut ids = HashSet::with_capacity(self.mods.len());
        for item in &self.mods {
            if !crate::package_lookup::valid_identifier(&item.id)
                || !ids.insert(item.id.as_str())
                || item.version.trim().is_empty()
                || item.version.len() > MAX_VERSION_BYTES
            {
                return Err("invalid or duplicate mod in compatibility report");
            }
        }
        if self.problems.len() > MAX_REPORT_PROBLEMS {
            return Err("too many client problems in compatibility report");
        }
        for problem in &self.problems {
            if problem.code.trim().is_empty()
                || problem.code.len() > 64
                || problem.state.trim().is_empty()
                || problem.state.len() > 32
                || problem.code.chars().any(char::is_control)
                || problem.state.chars().any(char::is_control)
                || problem.detail.chars().count() > MAX_PROBLEM_DETAIL_CHARS
                || problem.detail.chars().any(char::is_control)
                || problem
                    .mod_id
                    .as_deref()
                    .is_some_and(|id| !crate::package_lookup::valid_identifier(id))
            {
                return Err("invalid problem in compatibility report");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ClientReportState {
    Pending,
    Missing,
    Stale,
    Reported(ClientCompatibilityReport),
}

#[derive(Debug, Clone)]
pub struct ConnectedClient {
    pub steam_id: String,
    pub state: ClientReportState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientVersionIssue {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mod_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedClientStatus {
    pub steam_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_name: Option<String>,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    pub issues: Vec<ClientVersionIssue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiplayerVersionCheck {
    pub id: &'static str,
    pub group: &'static str,
    pub state: &'static str,
    pub detail: String,
    pub clients: Vec<ConnectedClientStatus>,
}

pub fn compatibility_check(
    server_version: &str,
    server_mods: &[ClientModVersion],
    clients: Vec<ConnectedClient>,
    peer_list_available: bool,
) -> MultiplayerVersionCheck {
    let mut server_by_id = BTreeMap::new();
    for item in server_mods {
        server_by_id.entry(item.id.as_str()).or_insert(item);
    }

    let mut statuses = clients
        .into_iter()
        .map(|client| {
            let mut issues = Vec::new();
            let (player_name, loader_version, state) = match client.state {
                ClientReportState::Pending => {
                    issues.push(issue("report-pending", None, None, None, None));
                    (None, None, "neutral")
                }
                ClientReportState::Missing => {
                    issues.push(issue("report-missing", None, None, None, None));
                    (None, None, "warning")
                }
                ClientReportState::Stale => {
                    issues.push(issue("report-stale", None, None, None, None));
                    (None, None, "warning")
                }
                ClientReportState::Reported(report) => {
                    compare_report(server_version, &server_by_id, &report, &mut issues);
                    let client_state = if issues.is_empty() { "ok" } else { "warning" };
                    (
                        report.player_name,
                        Some(report.loader_version),
                        client_state,
                    )
                }
            };
            ConnectedClientStatus {
                steam_id: client.steam_id,
                player_name,
                state: state.into(),
                loader_version,
                issues,
            }
        })
        .collect::<Vec<_>>();
    statuses.sort_by(|left, right| {
        left.player_name
            .as_deref()
            .unwrap_or(&left.steam_id)
            .to_lowercase()
            .cmp(
                &right
                    .player_name
                    .as_deref()
                    .unwrap_or(&right.steam_id)
                    .to_lowercase(),
            )
    });

    let issue_count = statuses
        .iter()
        .filter(|client| client.state == "warning")
        .map(|client| client.issues.len())
        .sum::<usize>();
    let state = if issue_count > 0 {
        "warning"
    } else if !peer_list_available {
        "neutral"
    } else if statuses.is_empty() {
        "neutral"
    } else if statuses.iter().any(|client| client.state == "neutral") {
        "neutral"
    } else {
        "ok"
    };
    let detail = if !peer_list_available {
        "Connected clients could not be enumerated.".into()
    } else if statuses.is_empty() {
        "No connected clients were found.".into()
    } else {
        format!(
            "{} connected client(s), {} compatibility issue(s).",
            statuses.len(),
            issue_count
        )
    };
    MultiplayerVersionCheck {
        id: "networkVersions",
        group: "network",
        state,
        detail,
        clients: statuses,
    }
}

fn compare_report(
    server_version: &str,
    server_mods: &BTreeMap<&str, &ClientModVersion>,
    report: &ClientCompatibilityReport,
    issues: &mut Vec<ClientVersionIssue>,
) {
    if report.loader_version != server_version {
        issues.push(issue(
            "loader-version",
            None,
            Some(&report.loader_version),
            Some(server_version),
            None,
        ));
    }

    let mut client_by_id = BTreeMap::new();
    for item in &report.mods {
        client_by_id.insert(item.id.as_str(), item);
    }

    for (id, server_mod) in server_mods {
        let client_mod = client_by_id.get(id).copied();
        let server_active = server_mod.enabled && server_mod.target.supports_process(true);
        let client_active =
            client_mod.is_some_and(|item| item.enabled && item.target.supports_process(false));
        match client_mod {
            Some(client_mod) if server_active && client_active => {
                if client_mod.version != server_mod.version {
                    issues.push(issue(
                        "mod-version",
                        Some(id),
                        Some(&client_mod.version),
                        Some(&server_mod.version),
                        None,
                    ));
                }
            }
            _ if server_active
                && server_mod.target.supports_process(false)
                && client_mod.is_none_or(|item| item.target.supports_process(false)) =>
            {
                issues.push(issue(
                    "mod-missing-client",
                    Some(id),
                    client_mod.map(|item| item.version.as_str()),
                    Some(&server_mod.version),
                    None,
                ));
            }
            _ => {}
        }
    }

    for (id, client_mod) in &client_by_id {
        let server_mod = server_mods.get(id).copied();
        let client_active = client_mod.enabled && client_mod.target.supports_process(false);
        let server_active =
            server_mod.is_some_and(|item| item.enabled && item.target.supports_process(true));
        if client_active && client_mod.target.supports_process(true) && !server_active {
            issues.push(issue(
                "mod-missing-server",
                Some(id),
                Some(&client_mod.version),
                server_mod.map(|item| item.version.as_str()),
                None,
            ));
        }
    }

    for problem in &report.problems {
        let detail = match &problem.mod_id {
            Some(id) => format!("{}: {}: {}", id, problem.code, problem.detail),
            None => format!("{}: {}", problem.code, problem.detail),
        };
        issues.push(issue(
            "client-problem",
            problem.mod_id.as_deref(),
            None,
            None,
            Some(&detail),
        ));
    }
}

fn issue(
    code: &str,
    mod_id: Option<&str>,
    client_version: Option<&str>,
    server_version: Option<&str>,
    detail: Option<&str>,
) -> ClientVersionIssue {
    ClientVersionIssue {
        code: code.into(),
        mod_id: mod_id.map(str::to_owned),
        client_version: client_version.map(str::to_owned),
        server_version: server_version.map(str::to_owned),
        detail: detail.map(str::to_owned),
    }
}

pub fn client_problems(configuration: &serde_json::Value) -> Vec<ClientCompatibilityProblem> {
    let mut problems = Vec::new();
    if let Some(checks) = configuration
        .get("checks")
        .and_then(serde_json::Value::as_array)
    {
        for check in checks {
            let state = check["state"].as_str().unwrap_or_default();
            let code = check["id"].as_str().unwrap_or_default();
            if !matches!(state, "warning" | "error") || matches!(code, "mods" | "dependencies") {
                continue;
            }
            let detail = clean_detail(check["detail"].as_str().unwrap_or_default());
            problems.push(ClientCompatibilityProblem {
                code: code.into(),
                state: state.into(),
                mod_id: None,
                detail,
            });
        }
    }
    if let Some(mod_states) = configuration
        .get("modStates")
        .and_then(serde_json::Value::as_object)
    {
        for (id, status) in mod_states {
            let state = status["state"].as_str().unwrap_or_default();
            let code = status["code"].as_str().unwrap_or_default();
            if !matches!(
                state,
                "blocked" | "failed" | "restart-required" | "not-running" | "no-target"
            ) || matches!(
                code,
                "missing-dependency"
                    | "dependency-disabled"
                    | "dependency-version"
                    | "dependency-cycle"
            ) {
                continue;
            }
            problems.push(ClientCompatibilityProblem {
                code: if code.is_empty() { state } else { code }.into(),
                state: state.into(),
                mod_id: Some(id.clone()),
                detail: clean_detail(status["detail"].as_str().unwrap_or_default()),
            });
        }
    }
    if let Some(errors) = configuration
        .get("errors")
        .and_then(serde_json::Value::as_array)
    {
        for error in errors.iter().filter_map(serde_json::Value::as_str) {
            problems.push(ClientCompatibilityProblem {
                code: "configuration-error".into(),
                state: "error".into(),
                mod_id: None,
                detail: clean_detail(error),
            });
        }
    }
    problems.truncate(MAX_REPORT_PROBLEMS);
    problems
}

fn clean_detail(detail: &str) -> String {
    detail
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_PROBLEM_DETAIL_CHARS)
        .collect()
}
