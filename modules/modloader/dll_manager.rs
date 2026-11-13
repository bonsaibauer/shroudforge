//! Detection and one native load plan. This module never executes or rewrites mods.
use crate::mod_profiles::{self, relative_file, single_folder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DllProfile {
    pub id: String,
    pub package_directory: String,
    pub name: String,
    pub target: DllProfileTarget,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub approved: bool,
    #[serde(default)]
    pub compatibility_profile: Option<String>,
    #[serde(default)]
    pub files: Vec<DllProfileFile>,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DllProfileTarget {
    Client,
    Server,
    #[default]
    Both,
}
impl DllProfileTarget {
    pub fn applies_to(self, server: bool) -> bool {
        matches!(
            (self, server),
            (Self::Client, false) | (Self::Server, true) | (Self::Both, _)
        )
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DllLoadMode {
    #[default]
    Library,
    Dinput8,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DllProfileFile {
    pub path: String,
    #[serde(default)]
    pub mode: DllLoadMode,
}

pub fn enabled(root: &Path) -> bool {
    shroudforge_loader_config::config::read_loader(root)
        .ok()
        .and_then(|config| {
            config
                .pointer("/modules/dllManager/enabled")
                .and_then(Value::as_bool)
        })
        .unwrap_or(true)
}
pub fn read_selections(root: &Path) -> Vec<DllProfile> {
    shroudforge_loader_config::config::read_loader(root)
        .ok()
        .and_then(|config| config.pointer("/modules/dllManager/profiles").cloned())
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}
pub fn normalize_excluded_dll_names(value: &Value) -> Result<Value, String> {
    let entries = value
        .as_array()
        .ok_or("excludedDllNames must be an array")?;
    if entries.len() > 64 {
        return Err("At most 64 DLL exclusions are supported".into());
    }
    let mut names = Vec::new();
    for entry in entries {
        let name = entry
            .as_str()
            .ok_or("DLL exclusions must be text")?
            .trim()
            .to_ascii_lowercase();
        if name.len() > 255
            || !single_folder(&name)
            || name.contains(['*', '?', '"', '<', '>', '|'])
            || !name.ends_with(".dll")
        {
            return Err(format!("Invalid excluded DLL name: {name}"));
        }
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names.sort();
    Ok(serde_json::json!(names))
}
pub fn excluded_dll_names(root: &Path) -> HashSet<String> {
    shroudforge_loader_config::config::read_loader(root)
        .ok()
        .and_then(|config| {
            config
                .pointer("/modules/dllManager/excludedDllNames")
                .cloned()
        })
        .and_then(|value| normalize_excluded_dll_names(&value).ok())
        .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
        .unwrap_or_default()
        .into_iter()
        .collect()
}
pub fn is_excluded_dll_name(root: &Path, path: &str) -> bool {
    excluded_dll_names(root).contains(&file_name(path).to_ascii_lowercase())
}
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}
fn normalize_dll_path(path: &str) -> String {
    path.trim_start_matches("./")
        .replace('\\', "/")
        .to_ascii_lowercase()
}
fn is_v2_declaration(declaration: &PluginIniDeclaration) -> bool {
    declaration
        .lifecycle
        .as_deref()
        .is_some_and(|lifecycle| lifecycle.eq_ignore_ascii_case("shroudforge-v2"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DllCandidate {
    pub package_directory: String,
    pub name: String,
    pub shipped_profile: Option<String>,
    pub shipped_profile_files: Vec<String>,
    pub filtered_dlls: Vec<String>,
    pub manifest_state: &'static str,
    pub has_lua: bool,
    pub capabilities: Vec<String>,
    pub target: &'static str,
    pub manifest_enabled: Option<bool>,
    pub ini_state: &'static str,
    pub declared_dll: Option<String>,
    pub dlls: Vec<DllCandidateFile>,
    pub available_profiles: Vec<ProfileChoice>,
    pub selected_profile: Option<String>,
    pub source: String,
    pub load_entries: Vec<CandidateLoadEntry>,
    pub issues: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DllCandidateFile {
    pub path: String,
    pub size: u64,
}
#[derive(Serialize)]
pub struct ProfileChoice {
    pub id: String,
    pub name: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateLoadEntry {
    pub status_id: String,
    pub path: String,
    pub mode: DllLoadMode,
    pub source: String,
}
#[derive(Clone, Debug)]
pub struct DllLoadEntry {
    pub status_id: String,
    pub mod_id: Option<String>,
    pub package_directory: String,
    pub path: PathBuf,
    pub mode: DllLoadMode,
    pub source: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DllPlanIssue {
    pub status_id: String,
    pub detail: String,
}
#[derive(Default)]
pub struct DllLoadPlan {
    pub entries: Vec<DllLoadEntry>,
    pub issues: Vec<DllPlanIssue>,
}
pub fn load_plan_identity(plan: &DllLoadPlan) -> Value {
    Value::Array(
        plan.entries
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "statusId": entry.status_id, "path": entry.path, "mode": entry.mode,
                })
            })
            .collect(),
    )
}
#[derive(Default)]
struct PackagePlan {
    entries: Vec<(DllLoadEntry, DllProfileTarget)>,
    issues: Vec<(DllPlanIssue, DllProfileTarget)>,
}

pub fn discover_dll_candidates(root: &Path) -> Vec<DllCandidate> {
    inspect_packages(root).0
}
pub fn resolve_load_plan(root: &Path, is_server: bool) -> DllLoadPlan {
    if !enabled(root) {
        return DllLoadPlan::default();
    }
    let (_, mut plan) = inspect_packages(root);
    plan.entries.retain(|entry| entry.1.applies_to(is_server));
    let mut result = DllLoadPlan {
        entries: plan.entries.into_iter().map(|entry| entry.0).collect(),
        issues: plan
            .issues
            .into_iter()
            .filter(|(_, target)| target.applies_to(is_server))
            .map(|(issue, _)| issue)
            .collect(),
    };
    let mut chain = DllLoadPlan::default();
    append_chain(root, &mut chain);
    chain.entries.append(&mut result.entries);
    result.entries = chain.entries;
    result.issues.extend(chain.issues);
    let selections = read_selections(root);
    let profiles = mod_profiles::read_profiles(root).unwrap_or_default();
    result.entries.retain(|entry| {
        selections
            .iter()
            .find(|selection| {
                selection
                    .package_directory
                    .eq_ignore_ascii_case(&entry.package_directory)
            })
            .map_or_else(
                || {
                    entry.source != "chain"
                        || mod_profiles::select_profile(
                            &profiles,
                            &entry.package_directory,
                            entry.mod_id.as_deref(),
                            None,
                        )
                        .is_none_or(|profile| profile.target.applies_to(is_server))
                },
                |selection| {
                    selection.enabled
                        && selection.approved
                        && selection.target.applies_to(is_server)
                        && (is_v2_declaration(&read_plugin_ini_declaration(
                            &crate::paths::mods_dir(root).join(&entry.package_directory),
                        )) || selection
                            .compatibility_profile
                            .as_deref()
                            .filter(|id| !id.is_empty() && *id != "none")
                            .is_none_or(|id| profiles.iter().any(|profile| profile.id == id)))
                },
            )
    });
    // Native components follow the same activation, dependency and conflict plan
    // as their Lua package. Folders without a manifest need only their DLL selection.
    if let Some(path) = camino::Utf8Path::from_path(root) {
        match crate::ModEnvironment::load(path) {
            Ok(environment) => {
                let (mods, issues) =
                    environment.plan_report_detailed(is_server, env!("CARGO_PKG_VERSION"));
                let ranks: HashMap<_, _> = mods
                    .iter()
                    .enumerate()
                    .map(|(rank, item)| (item.info().id.clone(), rank))
                    .collect();
                result.entries.retain(|entry| {
                    let allowed = entry
                        .mod_id
                        .as_ref()
                        .is_none_or(|id| ranks.contains_key(id));
                    if !allowed {
                        let detail = issues
                            .iter()
                            .find(|issue| Some(&issue.mod_id) == entry.mod_id.as_ref())
                            .map(|issue| issue.detail.clone())
                            .unwrap_or_else(|| {
                                "Mod is disabled or unavailable in the execution plan".into()
                            });
                        result.issues.push(DllPlanIssue {
                            status_id: entry.status_id.clone(),
                            detail,
                        });
                    }
                    allowed
                });
                // Stable sorting keeps the declared DLL order inside each package.
                result.entries.sort_by_key(|entry| {
                    (
                        entry.source != "chain",
                        if entry.source == "chain" {
                            0
                        } else {
                            entry
                                .mod_id
                                .as_ref()
                                .and_then(|id| ranks.get(id))
                                .copied()
                                .unwrap_or(usize::MAX)
                        },
                    )
                });
            }
            Err(error) => {
                result.entries.retain(|entry| entry.mod_id.is_none());
                result.issues.push(DllPlanIssue {
                    status_id: "dll:plan".into(),
                    detail: format!("Could not resolve mod dependencies: {error}"),
                });
            }
        }
    }
    // Keep declaration aliases for UI status. The process executor deduplicates
    // canonical paths, so a chain entry and profile share one loaded handle.
    tracing::debug!(target: "shroudforge::dll_manager", dlls = result.entries.len(), issues = result.issues.len(), server = is_server, "Resolved DLL load plan");
    result
}
fn inspect_packages(root: &Path) -> (Vec<DllCandidate>, PackagePlan) {
    let profiles = mod_profiles::read_profiles(root).unwrap_or_default();
    let selections = read_selections(root);
    let excluded = excluded_dll_names(root);
    let entries = match fs::read_dir(crate::paths::mods_dir(root)) {
        Ok(entries) => entries,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(target: "shroudforge::dll_manager", %error, "Could not scan mods folder");
            }
            return (Vec::new(), PackagePlan::default());
        }
    };
    let mut directories: Vec<_> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .collect();
    directories.sort_by_key(|entry| entry.file_name().to_string_lossy().to_ascii_lowercase());
    let (mut candidates, mut plan) = (Vec::new(), PackagePlan::default());
    for entry in directories {
        let package = entry.path();
        let directory = entry.file_name().to_string_lossy().into_owned();
        let raw = fs::read(package.join("mod.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
        let mod_id = raw
            .as_ref()
            .and_then(|value| value["id"].as_str())
            .map(str::to_owned);
        let selection = selections
            .iter()
            .find(|selection| selection.package_directory.eq_ignore_ascii_case(&directory));
        let selected = selection
            .and_then(|selection| selection.compatibility_profile.as_deref())
            .filter(|id| !id.is_empty());
        let profile =
            mod_profiles::select_profile(&profiles, &directory, mod_id.as_deref(), selected);
        let plugin_declaration = read_plugin_ini_declaration(&package);
        let ini_state = plugin_declaration.state.as_str();
        let declared_dll = plugin_declaration.dll.clone();
        let v2_declared = is_v2_declaration(&plugin_declaration);
        let mut issues = Vec::new();
        if selected.is_some_and(|id| id != "none") && profile.is_none() {
            issues.push("Selected compatibility profile was not found".into());
        }
        let parsed = if package.join("mod.json").exists() {
            Some(crate::read_manifest_path(root, &package))
        } else {
            None
        };
        let manifest_state = match &parsed {
            Some(Ok(_)) => "valid",
            Some(Err(_)) => "invalid",
            None => "missing",
        };
        if let Some(Err(error)) = &parsed {
            issues.push(format!("mod.json: {error}"));
        }
        let manifest = parsed.as_ref().and_then(|value| value.as_ref().ok());
        let name = manifest
            .map(|manifest| manifest.name.clone())
            .unwrap_or_else(|| directory.clone());
        let target = manifest
            .map(|manifest| match manifest.target {
                crate::ModTarget::Client => "client",
                crate::ModTarget::Server => "server",
                crate::ModTarget::ClientServer => "both",
            })
            .unwrap_or("both");
        let (mut dlls, mut filtered_dlls) = (Vec::new(), Vec::new());
        for file in WalkDir::new(&package)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry
                        .path()
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
            })
        {
            let Ok(relative) = file.path().strip_prefix(&package) else {
                continue;
            };
            let path = relative.to_string_lossy().replace('\\', "/");
            if excluded.contains(&file_name(&path).to_ascii_lowercase()) {
                filtered_dlls.push(path);
            } else if let Ok(metadata) = file.metadata() {
                dlls.push(DllCandidateFile {
                    path,
                    size: metadata.len(),
                });
            }
        }
        if dlls.is_empty()
            && filtered_dlls.is_empty()
            && ini_state == "missing"
            && profile.is_none()
        {
            continue;
        }
        dlls.sort_by_key(|file| file.path.to_ascii_lowercase());
        filtered_dlls.sort();
        let (mut files, mut source, mut load_target) = (
            Vec::new(),
            "manual",
            selection
                .map(|selection| selection.target)
                .or_else(|| profile.map(|profile| profile.target))
                .unwrap_or_default(),
        );
        let selection_active = if v2_declared {
            selection.is_none_or(|selection| selection.enabled && selection.approved)
        } else {
            selection.is_none_or(|selection| selection.enabled && selection.approved)
                && selected.is_none_or(|id| id == "none" || profile.is_some())
        };
        let v2_selection_conflict = if v2_declared {
            selection.is_some_and(|selection| {
                selection.files.iter().any(|file| {
                    file.mode != DllLoadMode::Library
                        || declared_dll.as_deref().is_none_or(|declared| {
                            normalize_dll_path(&file.path) != normalize_dll_path(declared)
                        })
                })
            })
        } else {
            false
        };
        if v2_declared {
            // A V2 shim owns this package's native load plan. Compatibility
            // catalog/profile files remain visible but cannot replace it.
            source = "ini";
            if let Some(selection) = selection {
                load_target = selection.target;
            } else if let Some(profile) = profile {
                load_target = profile.target;
            }
            if v2_selection_conflict {
                issues.push("DLL selection conflicts with the V2 native plugin declaration".into());
            } else if plugin_declaration.state == PluginIniState::Enabled {
                if let Some(path) = declared_dll.clone() {
                    files.push(DllProfileFile {
                        path,
                        mode: DllLoadMode::Library,
                    });
                }
            } else if plugin_declaration.state == PluginIniState::Invalid {
                issues.push("native-plugin.ini has an invalid [Plugin] declaration".into());
            }
        } else if let Some(selection) = selection.filter(|selection| !selection.files.is_empty()) {
            files = selection.files.clone();
            load_target = selection.target;
        } else if let Some(profile) = profile.filter(|profile| !profile.files.is_empty()) {
            files = profile.files.clone();
            source = "profile";
            load_target = selection
                .map(|selection| selection.target)
                .unwrap_or(profile.target);
        } else if ini_state == "enabled" {
            source = "ini";
            files.push(DllProfileFile {
                path: declared_dll.clone().unwrap(),
                mode: DllLoadMode::Library,
            });
            if let Some(selection) = selection {
                load_target = selection.target;
            }
        } else if ini_state == "invalid" {
            issues.push("native-plugin.ini has an invalid [Plugin] declaration".into());
        }
        let mut load_entries = Vec::new();
        let active = selection_active
            && manifest_state != "invalid"
            && manifest.is_none_or(|manifest| manifest.enabled);
        let effective_target = match target {
            "client" if load_target.applies_to(false) => Some(DllProfileTarget::Client),
            "server" if load_target.applies_to(true) => Some(DllProfileTarget::Server),
            "both" => Some(load_target),
            _ => None,
        };
        if active && effective_target.is_none() && !files.is_empty() {
            issues.push("DLL selection target conflicts with the mod manifest target".into());
        }
        if active && v2_declared && plugin_declaration.state == PluginIniState::Invalid {
            plan.issues.push((
                DllPlanIssue {
                    status_id: format!("dll:{}:ini", directory.to_ascii_lowercase()),
                    detail: "native-plugin.ini has an invalid V2 [Plugin] declaration".into(),
                },
                load_target,
            ));
        }
        if active && v2_declared && v2_selection_conflict {
            plan.issues.push((
                DllPlanIssue {
                    status_id: format!("dll:{}:selection", directory.to_ascii_lowercase()),
                    detail:
                        "DLL file or mode selection conflicts with the V2 native plugin declaration"
                            .into(),
                },
                load_target,
            ));
        }
        if active && source == "manual" && files.is_empty() && ini_state == "invalid" {
            if let Some(target) = effective_target {
                plan.issues.push((
                    DllPlanIssue {
                        status_id: format!("dll:{}:ini", directory.to_ascii_lowercase()),
                        detail: "native-plugin.ini has an invalid [Plugin] declaration".into(),
                    },
                    target,
                ));
            }
        }
        for file in files {
            let status_id = format!(
                "dll:{}:{}",
                directory.to_ascii_lowercase(),
                file.path.to_ascii_lowercase()
            );
            match resolve_package_dll_path(root, &directory, &file) {
                Ok(path) => {
                    load_entries.push(CandidateLoadEntry {
                        status_id: status_id.clone(),
                        path: file.path.clone(),
                        mode: file.mode,
                        source: source.into(),
                    });
                    if active {
                        if let Some(target) = effective_target {
                            plan.entries.push((
                                DllLoadEntry {
                                    status_id,
                                    mod_id: mod_id.clone(),
                                    package_directory: directory.clone(),
                                    path,
                                    mode: file.mode,
                                    source: source.into(),
                                },
                                target,
                            ));
                        }
                    }
                }
                Err(error) => {
                    issues.push(error.clone());
                    if active {
                        if let Some(target) = effective_target {
                            plan.issues.push((
                                DllPlanIssue {
                                    status_id,
                                    detail: error,
                                },
                                target,
                            ));
                        }
                    }
                }
            }
        }
        let target = match (target, load_target) {
            ("both", DllProfileTarget::Client) => "client",
            ("both", DllProfileTarget::Server) => "server",
            _ => target,
        };
        let capabilities = manifest
            .and_then(|manifest| serde_json::to_value(&manifest.capabilities).ok())
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        tracing::trace!(target: "shroudforge::dll_manager", package = %directory, source, planned = load_entries.len(), "Inspected native components");
        candidates.push(DllCandidate {
            package_directory: directory,
            name,
            shipped_profile: profile.map(|profile| profile.name.clone()),
            shipped_profile_files: profile
                .map(|profile| profile.files.iter().map(|file| file.path.clone()).collect())
                .unwrap_or_default(),
            filtered_dlls,
            manifest_state,
            has_lua: package.join("src/mod.lua").is_file()
                || profile.is_some_and(|profile| profile.package_files.contains_key("src/mod.lua")),
            capabilities,
            target,
            manifest_enabled: manifest.map(|manifest| manifest.enabled),
            ini_state,
            declared_dll,
            dlls,
            available_profiles: profiles
                .iter()
                .map(|profile| ProfileChoice {
                    id: profile.id.clone(),
                    name: profile.name.clone(),
                })
                .collect(),
            selected_profile: profile.map(|profile| profile.id.clone()),
            source: source.into(),
            load_entries,
            issues,
        });
    }
    (candidates, plan)
}

/// The parser owns the single interpretation of native-plugin.ini used by the
/// DLL Manager and the runtime lifecycle adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PluginIniState {
    #[default]
    Missing,
    Invalid,
    Disabled,
    Enabled,
}

impl PluginIniState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Invalid => "invalid",
            Self::Disabled => "disabled",
            Self::Enabled => "enabled",
        }
    }

    pub const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginIniDeclaration {
    pub state: PluginIniState,
    pub dll: Option<String>,
    pub declared_dlls: Vec<String>,
    pub lifecycle: Option<String>,
    pub services: Vec<String>,
}

impl PluginIniDeclaration {
    pub fn declares_service(&self, service: &str) -> bool {
        self.services.iter().any(|declared| declared == service)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginLoadDisposition {
    Ordinary,
    ShroudForgeV1,
    ShroudForgeV2,
}

/// Only [Plugin] is consumed. Other INI sections belong to the mod.
/// Duplicate or malformed declarations fail closed.
pub fn parse_plugin_ini(text: &str) -> PluginIniDeclaration {
    let (
        mut section,
        mut found,
        mut enabled,
        mut dll,
        mut declared_dlls,
        mut lifecycle,
        mut services,
        mut invalid,
    ) = (false, false, None, None, Vec::new(), None, None, false);
    for line in text
        .lines()
        .map(|line| line.trim().trim_start_matches('\u{feff}'))
    {
        if line.is_empty() || line.starts_with([';', '#']) {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1]
                .trim()
                .eq_ignore_ascii_case("plugin");
            found |= section;
            continue;
        }
        if !section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            invalid = true;
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches(['"', '\'']);
        if key.eq_ignore_ascii_case("enabled") {
            invalid |= enabled.is_some();
            enabled = match value.to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => Some(true),
                "0" | "false" | "no" | "off" => Some(false),
                _ => {
                    invalid = true;
                    None
                }
            };
        } else if key.eq_ignore_ascii_case("dll") {
            invalid |= dll.is_some();
            let path = value.replace('\\', "/");
            declared_dlls.push(path.clone());
            dll = Some(path);
        } else if key.eq_ignore_ascii_case("lifecycle") {
            invalid |= lifecycle.is_some();
            lifecycle = Some(value.to_owned());
        } else if key.eq_ignore_ascii_case("services") {
            invalid |= services.is_some();
            services = Some(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|service| !service.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
            );
        }
    }

    let state = if !found
        || invalid
        || (enabled != Some(false) && dll.as_deref().is_none_or(str::is_empty))
    {
        PluginIniState::Invalid
    } else if enabled == Some(false) {
        PluginIniState::Disabled
    } else {
        PluginIniState::Enabled
    };

    PluginIniDeclaration {
        state,
        dll,
        declared_dlls,
        lifecycle,
        services: services.unwrap_or_default(),
    }
}

pub fn read_plugin_ini_declaration(package: &Path) -> PluginIniDeclaration {
    match fs::read_to_string(package.join("native-plugin.ini")) {
        Ok(text) => parse_plugin_ini(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            PluginIniDeclaration::default()
        }
        Err(_) => PluginIniDeclaration {
            state: PluginIniState::Invalid,
            ..PluginIniDeclaration::default()
        },
    }
}

/// Return parsed metadata only when the selected file is named by the declaration.
pub fn selected_plugin_ini_declaration(
    root: &Path,
    directory: &str,
    selected_path: &Path,
) -> Result<Option<PluginIniDeclaration>, String> {
    let package = crate::paths::mods_dir(root).join(directory);
    let declaration = read_plugin_ini_declaration(&package);
    let selected = selected_path
        .canonicalize()
        .map_err(|error| format!("could not resolve selected DLL path: {error}"))?;
    let package = package
        .canonicalize()
        .map_err(|error| format!("could not resolve DLL package path: {error}"))?;
    let selected_relative = selected
        .strip_prefix(&package)
        .map_err(|_| "selected DLL is outside its package".to_owned())?
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let is_declared = declaration.declared_dlls.iter().any(|declared| {
        declared
            .trim_start_matches("./")
            .replace('\\', "/")
            .to_ascii_lowercase()
            == selected_relative
    });
    Ok(is_declared.then_some(declaration))
}

/// Classify an entry using the one canonical INI parser and the exact selected
/// file path. This is shared by bootstrap and runtime registration routes.
pub fn classify_selected_plugin_dll(
    root: &Path,
    directory: &str,
    selected_path: &Path,
) -> Result<PluginLoadDisposition, String> {
    let declaration = read_plugin_ini_declaration(&crate::paths::mods_dir(root).join(directory));
    let selected_declaration = selected_plugin_ini_declaration(root, directory, selected_path)?;
    if is_v2_declaration(&declaration) {
        return classify_plugin_load(&declaration, selected_declaration.is_some());
    }
    let Some(declaration) = selected_declaration else {
        return Ok(PluginLoadDisposition::Ordinary);
    };
    classify_plugin_load(&declaration, true)
}
/// Return the existing status/DLL tuple for UI and compatibility callers.
pub fn read_plugin_ini(package: &Path) -> (&'static str, Option<String>) {
    let declaration = read_plugin_ini_declaration(package);
    (declaration.state.as_str(), declaration.dll)
}

/// Classify the exact selected DLL. V1 and ordinary declarations preserve the
/// profile path; a V2 declaration owns its package plan and refuses other DLLs.
pub fn classify_plugin_load(
    declaration: &PluginIniDeclaration,
    selected_is_declared_dll: bool,
) -> Result<PluginLoadDisposition, String> {
    if !selected_is_declared_dll {
        if is_v2_declaration(declaration) {
            return Err(
                "A V2 native plugin declaration owns its package DLL plan; other DLLs cannot be loaded"
                    .into(),
            );
        }
        return Ok(PluginLoadDisposition::Ordinary);
    }
    match declaration.state {
        PluginIniState::Missing => Ok(PluginLoadDisposition::Ordinary),
        PluginIniState::Invalid => {
            Err("native-plugin.ini has an invalid [Plugin] declaration".into())
        }
        PluginIniState::Disabled | PluginIniState::Enabled => {
            match declaration.lifecycle.as_deref() {
                None => Ok(PluginLoadDisposition::Ordinary),
                Some(lifecycle) if lifecycle.eq_ignore_ascii_case("shroudforge-v1") => {
                    Ok(PluginLoadDisposition::ShroudForgeV1)
                }
                Some(lifecycle) if lifecycle.eq_ignore_ascii_case("shroudforge-v2") => {
                    Ok(PluginLoadDisposition::ShroudForgeV2)
                }
                Some(lifecycle) => {
                    Err(format!("unsupported native plugin lifecycle '{lifecycle}'"))
                }
            }
        }
    }
}
pub fn chain_load_entries(root: &Path) -> Vec<CandidateLoadEntry> {
    let mut plan = DllLoadPlan::default();
    append_chain(root, &mut plan);
    let mut entries: Vec<_> = plan
        .entries
        .into_iter()
        .map(|entry| CandidateLoadEntry {
            status_id: entry.status_id,
            path: entry
                .path
                .strip_prefix(root)
                .unwrap_or(&entry.path)
                .to_string_lossy()
                .into_owned(),
            mode: entry.mode,
            source: entry.source,
        })
        .collect();
    entries.extend(plan.issues.into_iter().map(|issue| CandidateLoadEntry {
        status_id: issue.status_id,
        path: issue.detail,
        mode: DllLoadMode::Dinput8,
        source: "chain".into(),
    }));
    entries
}
fn append_chain(root: &Path, plan: &mut DllLoadPlan) {
    let text = match fs::read_to_string(root.join("dinput8-chain.ini")) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            plan.issues.push(DllPlanIssue {
                status_id: "dll:chain".into(),
                detail: format!("dinput8-chain.ini: {error}"),
            });
            return;
        }
    };
    let Ok(canonical_root) = root.canonicalize() else {
        return;
    };
    let mut section = false;
    let mut index = 0;
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with([';', '#']) {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().eq_ignore_ascii_case("chain");
            continue;
        }
        if !section {
            continue;
        }
        let relative = line.trim_matches('"').replace('\\', "/");
        let status_id = format!("dll:chain:{index}");
        index += 1;
        let path = if !relative_file(&relative) || !relative.to_ascii_lowercase().ends_with(".dll")
        {
            Err("Chain entry must be a DLL path relative to the game folder".into())
        } else if is_excluded_dll_name(root, &relative) {
            Err(format!("Chain DLL is excluded: {relative}"))
        } else {
            root.join(&relative)
                .canonicalize()
                .map_err(|error| format!("Chain DLL {relative}: {error}"))
        };
        match path {
            Ok(path) if path.starts_with(&canonical_root) && path.is_file() => {
                if ["dinput8.dll", "dbghelp.dll", "winmm.dll"]
                    .iter()
                    .any(|name| {
                        canonical_root
                            .join(name)
                            .canonicalize()
                            .is_ok_and(|own| own == path)
                    })
                {
                    tracing::debug!(target: "shroudforge::dll_manager", dll = %path.display(), "Skipped own bootstrap in DLL chain");
                    continue;
                }
                let package_file =
                    crate::paths::mods_dir(root)
                        .canonicalize()
                        .ok()
                        .and_then(|mods| {
                            let relative = path.strip_prefix(mods).ok()?;
                            let mut components = relative.components();
                            let directory = components
                                .next()?
                                .as_os_str()
                                .to_string_lossy()
                                .into_owned();
                            let file = components.as_path().to_string_lossy().replace('\\', "/");
                            (!file.is_empty()).then_some((directory, file))
                        });
                let Some((package_directory, file)) = package_file else {
                    plan.issues.push(DllPlanIssue {
                        status_id,
                        detail: format!("Chain DLL '{relative}' must be inside mods/<mod-folder>/. Move the mod DLL and its companion files into its own mod folder and update dinput8-chain.ini"),
                    });
                    continue;
                };
                let path = match resolve_package_dll_path(
                    root,
                    &package_directory,
                    &DllProfileFile {
                        path: file,
                        mode: DllLoadMode::Dinput8,
                    },
                ) {
                    Ok(path) => path,
                    Err(error) => {
                        plan.issues.push(DllPlanIssue {
                            status_id,
                            detail: format!("Chain DLL '{relative}': {error}"),
                        });
                        continue;
                    }
                };
                let package = crate::paths::mods_dir(root).join(&package_directory);
                let mod_id = if package.join("mod.json").exists() {
                    match crate::read_manifest_path(root, &package) {
                        Ok(manifest) => Some(manifest.id),
                        Err(error) => {
                            plan.issues.push(DllPlanIssue {
                                status_id,
                                detail: format!("Chain mod manifest is invalid: {error}"),
                            });
                            continue;
                        }
                    }
                } else {
                    None
                };
                plan.entries.push(DllLoadEntry {
                    status_id,
                    mod_id,
                    package_directory,
                    path,
                    mode: DllLoadMode::Dinput8,
                    source: "chain".into(),
                });
            }
            Ok(_) => plan.issues.push(DllPlanIssue {
                status_id,
                detail: "Chain DLL is outside the game installation".into(),
            }),
            Err(detail) => plan.issues.push(DllPlanIssue { status_id, detail }),
        }
    }
}

pub fn validate_profiles(root: &Path, value: &Value) -> Result<Value, String> {
    let profiles: Vec<DllProfile> = serde_json::from_value(value.clone())
        .map_err(|error| format!("Invalid DLL selection: {error}"))?;
    if profiles.len() > 256 {
        return Err("At most 256 DLL selections are supported".into());
    }
    let available = mod_profiles::read_profiles(root).map_err(|errors| errors.join("\n"))?;
    let (mut ids, mut directories) = (HashSet::new(), HashSet::new());
    for profile in &profiles {
        if profile.id.is_empty()
            || profile.id.len() > 128
            || profile.name.trim().is_empty()
            || profile.name.len() > 160
            || !single_folder(&profile.package_directory)
            || !ids.insert(&profile.id)
            || !directories.insert(profile.package_directory.to_ascii_lowercase())
        {
            return Err("DLL selection identity is invalid or duplicated".into());
        }
        if let Some(id) = profile
            .compatibility_profile
            .as_deref()
            .filter(|id| !id.is_empty() && *id != "none")
        {
            if !available.iter().any(|profile| profile.id == id) {
                return Err("Selected compatibility profile was not found".into());
            }
        }
        if profile.files.len() > 16 {
            return Err("At most 16 DLLs per selection are supported".into());
        }
        if profile.enabled && !profile.approved {
            return Err("Confirm the DLL selection before enabling it".into());
        }
        let mut paths = HashSet::new();
        for file in &profile.files {
            if !paths.insert(file.path.to_ascii_lowercase()) {
                return Err("Duplicate DLL path".into());
            }
            resolve_package_dll_path(root, &profile.package_directory, file)?;
        }
    }
    serde_json::to_value(profiles).map_err(|error| error.to_string())
}
pub fn resolve_package_dll_path(
    root: &Path,
    directory: &str,
    file: &DllProfileFile,
) -> Result<PathBuf, String> {
    if !single_folder(directory)
        || directory.starts_with('.')
        || !relative_file(&file.path)
        || !file.path.to_ascii_lowercase().ends_with(".dll")
    {
        return Err("DLL path must stay inside one installed mod folder".into());
    }
    if is_excluded_dll_name(root, &file.path) {
        return Err(format!("DLL is excluded: {}", file.path));
    }
    let mods = crate::paths::mods_dir(root)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let package = mods
        .join(directory)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let path = package
        .join(&file.path)
        .canonicalize()
        .map_err(|error| format!("DLL {}: {error}", file.path))?;
    if package.parent() != Some(mods.as_path())
        || package
            .file_name()
            .is_none_or(|name| name.to_string_lossy().starts_with('.'))
        || !package.is_dir()
        || !path.starts_with(package)
        || !path.is_file()
    {
        return Err("DLL is outside its installed mod folder".into());
    }
    Ok(path)
}

#[cfg(test)]
mod plugin_ini_tests {
    use super::*;

    #[test]
    fn ordinary_enabled_dll_keeps_existing_defaults() {
        let declaration = parse_plugin_ini("[Plugin]\nDll=bin\\sample.dll\n");
        assert_eq!(declaration.state, PluginIniState::Enabled);
        assert_eq!(declaration.dll.as_deref(), Some("bin/sample.dll"));
        assert_eq!(declaration.lifecycle, None);
        assert!(declaration.services.is_empty());
        assert_eq!(
            classify_plugin_load(&declaration, true).unwrap(),
            PluginLoadDisposition::Ordinary
        );
    }

    #[test]
    fn explicit_disabled_declaration_needs_no_dll() {
        let declaration = parse_plugin_ini("[Plugin]\nEnabled = false\n");
        assert_eq!(declaration.state, PluginIniState::Disabled);
        assert_eq!(
            classify_plugin_load(&declaration, true).unwrap(),
            PluginLoadDisposition::Ordinary
        );
    }

    #[test]
    fn malformed_and_duplicate_plugin_declarations_are_invalid() {
        for text in [
            "[Other]\nDll=sample.dll",
            "[Plugin]\nEnabled=maybe\nDll=sample.dll",
            "[Plugin]\nDll=one.dll\nDll=two.dll",
            "[Plugin]\nEnabled=true\nEnabled=false\nDll=sample.dll",
            "[Plugin]\nDll=sample.dll\nLifecycle=shroudforge-v1\nLifecycle=shroudforge-v1",
            "[Plugin]\nDll=sample.dll\nServices=shroudforge.overlay\nServices=shroudforge.overlay",
            "[Plugin]\nmalformed",
        ] {
            assert_eq!(
                parse_plugin_ini(text).state,
                PluginIniState::Invalid,
                "{text}"
            );
        }
    }

    #[test]
    fn unsupported_lifecycle_fails_closed_for_its_declared_dll() {
        let declaration =
            parse_plugin_ini("[Plugin]\nEnabled=true\nDll=sample.dll\nLifecycle=other-v9\n");
        assert_eq!(declaration.state, PluginIniState::Enabled);
        assert!(
            classify_plugin_load(&declaration, true)
                .unwrap_err()
                .contains("unsupported native plugin lifecycle")
        );
        assert_eq!(
            classify_plugin_load(&declaration, false).unwrap(),
            PluginLoadDisposition::Ordinary
        );
    }

    #[test]
    fn disabled_lifecycle_plugin_stays_managed_and_cannot_fall_back() {
        let declaration =
            parse_plugin_ini("[Plugin]\nEnabled=false\nDll=sample.dll\nLifecycle=shroudforge-v1\n");
        assert_eq!(declaration.state, PluginIniState::Disabled);
        assert_eq!(
            classify_plugin_load(&declaration, true).unwrap(),
            PluginLoadDisposition::ShroudForgeV1
        );
    }
    #[test]
    fn lifecycle_and_services_require_explicit_opt_in() {
        let ordinary = parse_plugin_ini("[Plugin]\nDll=sample.dll\nServices=shroudforge.overlay\n");
        assert_eq!(
            classify_plugin_load(&ordinary, true).unwrap(),
            PluginLoadDisposition::Ordinary
        );
        assert!(ordinary.declares_service("shroudforge.overlay"));

        let opted_in = parse_plugin_ini(
            "[Plugin]\nEnabled=yes\nDll=sample.dll\nLifecycle=ShroudForge-V1\nServices=shroudforge.overlay, shroudforge.map\n",
        );
        assert_eq!(opted_in.state, PluginIniState::Enabled);
        assert_eq!(
            classify_plugin_load(&opted_in, true).unwrap(),
            PluginLoadDisposition::ShroudForgeV1
        );
        assert!(opted_in.declares_service("shroudforge.overlay"));
        assert!(opted_in.declares_service("shroudforge.map"));
        assert!(!opted_in.declares_service("shroudforge.unknown"));
    }

    #[test]
    fn v2_lifecycle_is_managed_and_unsupported_versions_still_fail_closed() {
        let opted_in = parse_plugin_ini(
            "[Plugin]\nEnabled=true\nDll=bin/provider.dll\nLifecycle=ShroudForge-V2\n",
        );
        assert_eq!(opted_in.state, PluginIniState::Enabled);
        assert_eq!(
            classify_plugin_load(&opted_in, true).unwrap(),
            PluginLoadDisposition::ShroudForgeV2
        );

        let disabled = parse_plugin_ini(
            "[Plugin]\nEnabled=false\nDll=bin/provider.dll\nLifecycle=ShroudForge-V2\n",
        );
        assert_eq!(
            classify_plugin_load(&disabled, true).unwrap(),
            PluginLoadDisposition::ShroudForgeV2
        );

        let unknown =
            parse_plugin_ini("[Plugin]\nDll=bin/provider.dll\nLifecycle=ShroudForge-V3\n");
        assert!(
            classify_plugin_load(&unknown, true)
                .unwrap_err()
                .contains("unsupported native plugin lifecycle")
        );
    }
}

#[cfg(test)]
mod v2_dll_planner_tests {
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            let nonce = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root =
                std::env::temp_dir().join(format!("sf-v2-dll-plan-{}-{nonce}", std::process::id()));
            fs::create_dir_all(&root).unwrap();
            let fixture = Self(root);
            fixture.package(
                "[Plugin]\nEnabled=true\nDll=shim.dll\nLifecycle=shroudforge-v2\n",
                "creative-profile",
            );
            fixture
        }

        fn package(&self, declaration: &str, profile_id: &str) {
            let package = self.0.join("mods/creative");
            fs::create_dir_all(&package).unwrap();
            fs::write(package.join("native-plugin.ini"), declaration).unwrap();
            fs::write(package.join("shim.dll"), b"shim").unwrap();
            fs::write(package.join("catalog.dll"), b"catalog").unwrap();
            fs::write(
                package.join("mod.json"),
                r#"{"id":"creative","name":"Creative","version":"1.0.0","capabilities":["runtime-register-dll"]}"#,
            )
            .unwrap();
            fs::write(
                package.join("extended.mod.json"),
                r#"{"schemaVersion":1,"enabled":true,"targets":["client","server"],"settings":{}}"#,
            )
            .unwrap();

            let catalog = shroudforge_loader_config::paths::mod_profiles_dir(&self.0);
            fs::create_dir_all(&catalog).unwrap();
            fs::write(
                catalog.join(format!("{profile_id}.json")),
                serde_json::to_vec(&serde_json::json!({
                    "schemaVersion": 1,
                    "id": profile_id,
                    "modId": "creative",
                    "packageDirectory": "creative",
                    "name": "Catalog profile",
                    "target": "both",
                    "files": [{ "path": "catalog.dll", "mode": "library" }]
                }))
                .unwrap(),
            )
            .unwrap();
        }

        fn select(&self, target: DllProfileTarget, approved: bool, files: Value) {
            shroudforge_loader_config::config::update_loader(&self.0, |config| {
                config["modules"]["dllManager"]["profiles"] = serde_json::json!([{
                    "id": "creative",
                    "packageDirectory": "creative",
                    "name": "Creative",
                    "target": match target {
                        DllProfileTarget::Client => "client",
                        DllProfileTarget::Server => "server",
                        DllProfileTarget::Both => "both",
                    },
                    "enabled": true,
                    "approved": approved,
                    "compatibilityProfile": "creative-profile",
                    "files": files
                }]);
                Ok(())
            })
            .unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn planned_paths(root: &Path, server: bool) -> Vec<String> {
        resolve_load_plan(root, server)
            .entries
            .into_iter()
            .map(|entry| {
                entry
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn v2_declaration_overrides_catalog_but_keeps_selection_target_and_approval() {
        let fixture = Fixture::new();
        fixture.select(DllProfileTarget::Server, true, serde_json::json!([]));

        assert!(planned_paths(&fixture.0, false).is_empty());
        assert_eq!(planned_paths(&fixture.0, true), ["shim.dll"]);

        fixture.select(DllProfileTarget::Server, false, serde_json::json!([]));
        assert!(planned_paths(&fixture.0, true).is_empty());

        fs::remove_file(
            shroudforge_loader_config::paths::mod_profiles_dir(&fixture.0)
                .join("creative-profile.json"),
        )
        .unwrap();
        fixture.select(DllProfileTarget::Server, true, serde_json::json!([]));
        assert_eq!(planned_paths(&fixture.0, true), ["shim.dll"]);
    }

    #[test]
    fn v2_explicit_catalog_file_conflict_fails_closed_without_profile_fallback() {
        let fixture = Fixture::new();
        fixture.select(
            DllProfileTarget::Client,
            true,
            serde_json::json!([{ "path": "catalog.dll", "mode": "library" }]),
        );

        let plan = resolve_load_plan(&fixture.0, false);
        assert!(plan.entries.is_empty());
        assert!(plan.issues.iter().any(|issue| {
            issue.status_id == "dll:creative:selection"
                && issue
                    .detail
                    .contains("conflicts with the V2 native plugin declaration")
        }));
    }

    #[test]
    fn disabled_v2_declaration_does_not_fall_back_to_catalog_profile() {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join("mods/creative/native-plugin.ini"),
            "[Plugin]\nEnabled=false\nDll=shim.dll\nLifecycle=shroudforge-v2\n",
        )
        .unwrap();

        let candidate = discover_dll_candidates(&fixture.0)
            .into_iter()
            .find(|candidate| candidate.package_directory == "creative")
            .unwrap();
        assert_eq!(candidate.source, "ini");
        assert!(candidate.load_entries.is_empty());
        assert!(planned_paths(&fixture.0, false).is_empty());
    }
}
