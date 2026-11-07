use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    #[serde(default)]
    pub locales: BTreeMap<String, ModLocaleCatalog>,
    /// Situations a mod reports while it runs ("hammer": "Hammer in hand"),
    /// named by `reserve.when` and `wheel.when` of its settings.
    #[serde(
        default,
        rename = "inputContexts",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub input_contexts: BTreeMap<String, String>,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModLocaleCatalog {
    #[serde(default, rename = "mod")]
    pub mod_info: ModLocaleMetadata,
    #[serde(default)]
    pub settings: BTreeMap<String, ModSettingLocale>,
    #[serde(default)]
    pub groups: Vec<ModGroupLocale>,
    #[serde(default)]
    pub changelog: Option<Vec<String>>,
    #[serde(default)]
    pub strings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModLocaleMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModSettingLocale {
    pub label: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub options: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModGroupLocale {
    pub label: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub actions: BTreeMap<String, ModActionLocale>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModActionLocale {
    pub label: Option<String>,
    pub confirm: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled_when: Option<SettingCondition>,
    #[serde(default)]
    pub asset_preparation: bool,
    #[serde(default)]
    pub restart_required: bool,
    #[serde(default = "default_apply_at")]
    pub apply_at: String,
    #[serde(skip)]
    pub apply_at_explicit: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reserve: Option<KeyReservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reserved: Option<ReservedDevices>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wheel: Option<WheelReservation>,
}

impl SettingDefinition {
    /// Whether the game is denied this keybind on `device` while the mod's
    /// context is active: the player's padlock, else the mod's default.
    pub fn reserves(&self, device: shroudforge_hotkeys::InputDevice) -> bool {
        let Some(reserve) = &self.reserve else {
            return false;
        };
        let chosen = self.reserved.as_ref().and_then(|reserved| match device {
            shroudforge_hotkeys::InputDevice::Keyboard => reserved.keyboard,
            shroudforge_hotkeys::InputDevice::Mouse => reserved.mouse,
        });
        chosen.unwrap_or(match device {
            shroudforge_hotkeys::InputDevice::Keyboard => reserve.keyboard,
            shroudforge_hotkeys::InputDevice::Mouse => reserve.mouse,
        })
    }
}

/// A keybind the player may keep from the game while the context `when` is
/// active: the mod still reads it, the game does not. `keyboard` and `mouse`
/// are the mod's defaults for each device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyReservation {
    pub when: String,
    #[serde(default)]
    pub keyboard: bool,
    #[serde(default)]
    pub mouse: bool,
}

/// The player's padlocks of one keybind, saved by the Modloader: a device set
/// here overrides the mod's default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReservedDevices {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouse: Option<bool>,
}

/// A boolean setting through which the mod reads the mouse wheel while the
/// context `when` is active. Its value is the padlock: true keeps the wheel
/// from the game, false shares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WheelReservation {
    pub when: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingCondition {
    pub setting: String,
    pub equals: serde_json::Value,
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
