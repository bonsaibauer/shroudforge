use crate::panel_layout::DEFAULT_HEIGHT;
use crate::panel_layout::DEFAULT_WIDTH;
use crate::panel_layout::MAX_HEIGHT;
use crate::panel_layout::MAX_POSITION;
use crate::panel_layout::MAX_WIDTH;
use crate::panel_layout::MIN_HEIGHT;
use crate::panel_layout::MIN_WIDTH;
use crate::panel_layout::TOP_OFFSET;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlueprintCard {
    pub(crate) name: String,
    pub(crate) image: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KeybindLabel {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewState {
    pub(crate) blueprints: Vec<BlueprintCard>,
    pub(crate) keybinds: Vec<KeybindLabel>,
    pub(crate) window_toggle_label: String,
    pub(crate) process_state: String,
    pub(crate) process_operation: String,
    pub(crate) process_phase: String,
    pub(crate) process_completed: u8,
    pub(crate) process_total: u8,
    pub(crate) process_sequence: u64,
    pub(crate) stage: String,
    pub(crate) hint: String,
    pub(crate) selected: String,
    pub(crate) axis: String,
    pub(crate) turns: u8,
    pub(crate) panel_width: u32,
    pub(crate) panel_height: u32,
    pub(crate) panel_position_x: i32,
    pub(crate) panel_position_y: u32,
}

pub(crate) fn view_state(root: &Path) -> Result<ViewState, Box<dyn std::error::Error>> {
    let exports = shroudforge_loader_config::paths::export_dir(root);
    let mut process_state = "idle".to_owned();
    let mut process_operation = String::new();
    let mut process_phase = String::new();
    let mut process_completed = 0;
    let mut process_total = 0;
    let mut process_sequence = 0;
    let mut stage = "idle".to_owned();
    let mut hint = String::new();
    let mut applied_keybinds = BTreeMap::new();
    let mut selected = String::new();
    let mut axis = "y".to_owned();
    let mut turns = 0;
    let mut panel_width = DEFAULT_WIDTH;
    let mut panel_height = DEFAULT_HEIGHT;
    let mut panel_position_x = -1;
    let mut panel_position_y = TOP_OFFSET as u32;
    let mut blueprints = Vec::new();
    if let Ok(contents) = fs::read_to_string(exports.join("world-editor/editor-state.txt")) {
        for line in contents.lines() {
            if let Some(value) = line.strip_prefix("keybind.") {
                if let Some((key, binding)) = value.split_once('=') {
                    applied_keybinds.insert(key.to_owned(), binding.to_owned());
                }
            } else if let Some(value) = line.strip_prefix("blueprint.") {
                if let Some((name, image)) = value.split_once('=') {
                    if valid_blueprint_name(name) && image.starts_with("data:image/png;base64,") {
                        blueprints.push(BlueprintCard {
                            name: name.to_owned(),
                            image: Some(image.to_owned()),
                        });
                    }
                }
            } else if let Some(value) = line.strip_prefix("processState=") {
                process_state = value.to_owned();
            } else if let Some(value) = line.strip_prefix("processOperation=") {
                process_operation = value.to_owned();
            } else if let Some(value) = line.strip_prefix("processPhase=") {
                process_phase = value.to_owned();
            } else if let Some(value) = line.strip_prefix("processCompleted=") {
                process_completed = value.parse::<u8>().unwrap_or(0);
            } else if let Some(value) = line.strip_prefix("processTotal=") {
                process_total = value.parse::<u8>().unwrap_or(0);
            } else if let Some(value) = line.strip_prefix("processSequence=") {
                process_sequence = value.parse::<u64>().unwrap_or(0);
            } else if let Some(value) = line.strip_prefix("stage=") {
                stage = value.to_owned();
            } else if let Some(value) = line.strip_prefix("hint=") {
                hint = value.to_owned();
            } else if let Some(value) = line.strip_prefix("selected=") {
                selected = value.to_owned();
            } else if let Some(value) = line.strip_prefix("axis=") {
                axis = value.to_owned();
            } else if let Some(value) = line.strip_prefix("turns=") {
                turns = value.parse::<u8>().unwrap_or(0) % 4;
            } else if let Some(value) = line.strip_prefix("panelWidth=") {
                panel_width = value
                    .parse::<u32>()
                    .unwrap_or(DEFAULT_WIDTH)
                    .clamp(MIN_WIDTH, MAX_WIDTH);
            } else if let Some(value) = line.strip_prefix("panelHeight=") {
                panel_height = value
                    .parse::<u32>()
                    .unwrap_or(DEFAULT_HEIGHT)
                    .clamp(MIN_HEIGHT, MAX_HEIGHT);
            } else if let Some(value) = line.strip_prefix("panelPositionX=") {
                panel_position_x = value.parse::<i32>().unwrap_or(-1).clamp(-1, MAX_POSITION);
            } else if let Some(value) = line.strip_prefix("panelPositionY=") {
                panel_position_y = value
                    .parse::<u32>()
                    .unwrap_or(TOP_OFFSET as u32)
                    .min(MAX_POSITION as u32);
            }
        }
    }
    let keybinds = keybind_labels(root, &applied_keybinds);
    if hint.is_empty() {
        if let Some(mark) = keybinds.iter().find(|binding| binding.id == "markCursorNext") {
            hint = format!("Select the Building Hammer, choose a Single Voxel, aim at the first corner, then press {}.", mark.key);
        }
    }
    let window_toggle_label = shroudforge_loader_config::config::read_loader(root)
        .ok()
        .and_then(|config| {
            shroudforge_hotkeys::parse_value(&config["modules"]["worldEditor"]["toggleKey"])
                .ok()
                .flatten()
        })
        .map(|binding| binding.label())
        .unwrap_or_default();
    blueprints.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(ViewState {
        keybinds,
        window_toggle_label,
        process_state,
        process_operation,
        process_phase,
        process_completed,
        process_total,
        process_sequence,
        blueprints,
        stage,
        hint,
        selected,
        axis,
        turns,
        panel_width,
        panel_height,
        panel_position_x,
        panel_position_y,
    })
}

pub(crate) fn empty_state() -> ViewState {
    ViewState {
        blueprints: Vec::new(),
        keybinds: Vec::new(),
        window_toggle_label: String::new(),
        process_state: "idle".into(),
        process_operation: String::new(),
        process_phase: String::new(),
        process_completed: 0,
        process_total: 0,
        process_sequence: 0,
        stage: "idle".into(),
        hint: String::new(),
        selected: String::new(),
        axis: "y".into(),
        turns: 0,
        panel_width: DEFAULT_WIDTH,
        panel_height: DEFAULT_HEIGHT,
        panel_position_x: -1,
        panel_position_y: TOP_OFFSET as u32,
    }
}

fn keybind_labels(root: &Path, applied: &BTreeMap<String, String>) -> Vec<KeybindLabel> {
    let package = shroudforge_loader_config::paths::mods_dir(root).join("world-editor");
    let Ok(manifest) = shroudforge_package::read_manifest_path(root, &package) else {
        return Vec::new();
    };
    let mut bindings = manifest
        .settings
        .iter()
        .filter(|setting| {
            matches!(setting.control, Some(shroudforge_package::SettingControl::Keybind))
        })
        .map(|setting| {
            let value = applied
                .get(&setting.key)
                .map(String::as_str)
                .or_else(|| manifest.setting_values[&setting.key].as_str())
                .unwrap_or("");
            let active = manifest.enabled && shroudforge_package::setting_is_enabled(
                &setting.key,
                &manifest.settings,
                &manifest.setting_values,
            );
            let key = if active {
                shroudforge_hotkeys::parse_hotkey(value)
                    .ok()
                    .flatten()
                    .map(|binding| binding.label())
            } else {
                None
            }
            .unwrap_or_else(|| "Disabled".to_owned());
            KeybindLabel {
                id: setting.key.clone(),
                label: setting.label.clone(),
                key,
            }
        })
        .collect::<Vec<_>>();
    bindings.sort_by_key(|binding| {
        manifest
            .groups
            .iter()
            .flat_map(|group| &group.settings)
            .position(|key| key == &binding.id)
            .unwrap_or(usize::MAX)
    });
    bindings
}

pub(crate) fn valid_blueprint_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
