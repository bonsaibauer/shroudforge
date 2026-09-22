use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModManifest {
    // mandatory
    pub id: String,
    pub name: String,
    pub version: Version,
    pub api: Option<String>,
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
    pub release: ReleaseMetadata,
    #[serde(default, rename = "settingGroups")]
    pub setting_groups: Vec<SettingGroup>,
    #[serde(default)]
    pub settings: Vec<SettingDefinition>,
    #[serde(default)]
    pub ui: ModUi,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModTarget {
    Client,
    Server,
    #[default]
    Both,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseMetadata {
    #[serde(default)]
    pub changelog: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingGroup {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModUi {
    #[serde(default)]
    pub sections: Vec<UiSection>,
    #[serde(default)]
    pub tabs: Vec<UiTab>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiTab {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub sections: Vec<UiSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiSection {
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub components: Vec<UiComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiComponent {
    #[serde(rename = "type")]
    pub component_type: UiComponentType,
    pub key: Option<String>,
    pub text: Option<String>,
    pub label: Option<String>,
    pub level: Option<String>,
    pub binding: Option<String>,
    pub action: Option<String>,
    pub style: Option<String>,
    pub confirmation: Option<String>,
    pub url: Option<String>,
    pub src: Option<String>,
    pub alt: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiComponentType {
    Setting,
    Text,
    Notice,
    Status,
    Button,
    Link,
    Separator,
    Image,
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
    AssetsWrite,
    Runtime,
}
