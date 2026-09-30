use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModManifest {
    // mandatory
    pub id: String,
    pub name: String,
    pub version: Version,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "empty_settings", rename = "settingValues")]
    pub setting_values: serde_json::Value,
    #[serde(default)]
    pub capabilities: Vec<Capability>,

    // optional
    #[serde(default)]
    pub dependencies: Vec<Dependency>,

    // optional descriptive
    #[serde(default)]
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub icon: Option<String>,
    #[serde(default)]
    pub target: ModTarget,
    #[serde(default)]
    pub changelog: Vec<String>,
    #[serde(default)]
    pub links: ModLinks,
    #[serde(default)]
    pub groups: Vec<SettingGroup>,
    #[serde(default)]
    pub actions: Vec<ModAction>,
    #[serde(default)]
    pub settings: Vec<SettingDefinition>,
}

impl ModManifest {
    /// Export is a permission to write files; it only needs the startup pass
    /// for export-only mods. Runtime mods can use the same permission live.
    pub fn requires_pregame(&self) -> bool {
        let has_runtime = self
            .capabilities
            .iter()
            .any(|capability| capability.requires_runtime());
        self.capabilities
            .iter()
            .any(|capability| *capability == Capability::Patch)
            || (!has_runtime
                && self
                    .capabilities
                    .iter()
                    .any(|capability| *capability == Capability::Export))
    }
}

fn empty_settings() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModTarget {
    Client,
    Server,
    #[default]
    #[serde(rename = "client, server")]
    ClientServer,
}

impl ModTarget {
    pub fn supports_process(self, is_server: bool) -> bool {
        matches!(
            (self, is_server),
            (Self::Client, false) | (Self::Server, true) | (Self::ClientServer, _)
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModLinks(pub std::collections::BTreeMap<String, String>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingGroup {
    pub label: String,
    pub description: Option<String>,
    #[serde(default)]
    pub settings: Vec<String>,
    #[serde(default)]
    pub actions: Vec<ModAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModAction {
    pub id: String,
    pub label: String,
    pub style: Option<String>,
    pub confirm: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingDefinition {
    pub key: String,
    #[serde(rename = "type")]
    pub value_type: SettingValueType,
    pub control: Option<SettingControl>,
    pub label: String,
    pub description: Option<String>,
    pub default: serde_json::Value,
    pub group: Option<String>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub step: Option<f64>,
    pub minimum_length: Option<usize>,
    pub maximum_length: Option<usize>,
    #[serde(default)]
    pub options: Vec<SettingOption>,
    #[serde(default)]
    pub restart_required: bool,
    #[serde(default = "default_apply_at")]
    pub apply_at: String,
}

fn default_apply_at() -> String {
    "restart".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingValueType {
    Boolean,
    String,
    Integer,
    Number,
    Array,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingControl {
    Toggle,
    Checkbox,
    Text,
    Textarea,
    Number,
    Slider,
    Select,
    Radio,
    Segmented,
    Multiselect,
    Keybind,
    Color,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingOption {
    pub value: serde_json::Value,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub id: String,
    pub version: VersionReq,
    pub optional: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    Export,
    #[serde(rename = "patch")]
    Patch,
    Runtime,
    RuntimeRegisterDll,
}

impl Capability {
    /// EML asset edits and exports are executed by the early pregame pass.
    pub fn requires_pregame(self) -> bool {
        matches!(self, Self::Patch | Self::Export)
    }

    /// EML native DLL registration participates in the live runtime lifecycle.
    pub fn requires_runtime(self) -> bool {
        matches!(self, Self::Runtime | Self::RuntimeRegisterDll)
    }
}
