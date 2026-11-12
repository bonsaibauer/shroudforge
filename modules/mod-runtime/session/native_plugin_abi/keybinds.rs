//! Owner-thread native access to the package's declared keybind settings.
use super::*;
use std::collections::HashMap;

pub(super) const SERVICE: &str = "shroudforge.keybinds";
const MAX_SETTING_KEY_BYTES: usize = 256;
const KEYBIND_DOWN: u32 = 1;
const KEYBIND_PRESSED: u32 = 2;

#[derive(Default)]
pub(super) struct Keybinds(HashMap<String, Binding>);

struct Binding {
    hotkey: Option<shroudforge_hotkeys::Hotkey>,
    previous_revision: Option<u64>,
}

impl Binding {
    fn sample(&mut self, revision: u64, down: bool, allowed: bool) -> u32 {
        let pressed = self
            .previous_revision
            .is_some_and(|previous| revision > previous);
        self.previous_revision = Some(revision);
        if allowed {
            (u32::from(down) * KEYBIND_DOWN) | (u32::from(pressed) * KEYBIND_PRESSED)
        } else {
            0
        }
    }
}

impl Keybinds {
    pub(super) fn update(
        &mut self,
        declared: &[mod_loader::SettingDefinition],
        json: &[u8],
    ) -> Result<(), String> {
        let current: serde_json::Value =
            serde_json::from_slice(json).map_err(|error| error.to_string())?;
        let mut values = serde_json::Map::new();
        for setting in declared {
            values.insert(
                setting.key.clone(),
                current
                    .get(&setting.key)
                    .unwrap_or(&setting.default)
                    .clone(),
            );
        }
        let values = serde_json::Value::Object(values);
        let mut next = HashMap::new();
        for setting in declared
            .iter()
            .filter(|setting| matches!(setting.control, Some(mod_loader::SettingControl::Keybind)))
        {
            let hotkey = if mod_loader::setting_is_enabled(&setting.key, declared, &values) {
                shroudforge_hotkeys::parse_value(&values[&setting.key])?
            } else {
                None
            };
            let previous_revision = self
                .0
                .get(&setting.key)
                .filter(|old| old.hotkey == hotkey)
                .and_then(|old| old.previous_revision);
            next.insert(
                setting.key.clone(),
                Binding {
                    hotkey,
                    previous_revision,
                },
            );
        }
        self.0 = next;
        Ok(())
    }

    fn read(&mut self, key: &str, allowed: bool) -> Result<u32, i32> {
        let binding = self.0.get_mut(key).ok_or(NATIVE_STATUS_NOT_FOUND)?;
        let Some(hotkey) = &binding.hotkey else {
            return Ok(0);
        };
        let revision = shroudforge_hotkeys::hotkey_press_revision(hotkey);
        let down = shroudforge_hotkeys::is_hotkey_down(hotkey);
        Ok(binding.sample(revision, down, allowed))
    }
}

#[repr(C)]
struct KeybindStateV1 {
    size: u32,
    flags: u32,
}

#[repr(C)]
struct KeybindServiceV1 {
    size: u32,
    version: u32,
    context: *mut c_void,
    read_keybind:
        Option<unsafe extern "C" fn(*mut c_void, *const u8, u32, u32, *mut KeybindStateV1) -> i32>,
}

pub(super) unsafe fn query_v1(context: *mut c_void, capacity: u32, output: *mut c_void) -> i32 {
    if capacity < size_of::<KeybindServiceV1>() as u32 {
        return NATIVE_STATUS_BUFFER_TOO_SMALL;
    }
    if output.is_null() {
        return NATIVE_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        output.cast::<KeybindServiceV1>().write(KeybindServiceV1 {
            size: size_of::<KeybindServiceV1>() as u32,
            version: 1,
            context,
            read_keybind: Some(read_keybind_v1),
        });
    }
    NATIVE_STATUS_OK
}

unsafe extern "C" fn read_keybind_v1(
    package_context: *mut c_void,
    setting_key_utf8: *const u8,
    setting_key_length: u32,
    out_size: u32,
    out_state: *mut KeybindStateV1,
) -> i32 {
    let Some(context) = (unsafe { host_context(package_context) }) else {
        return NATIVE_STATUS_INVALID_ARGUMENT;
    };
    if !context.is_owner_thread() {
        return NATIVE_STATUS_WRONG_THREAD;
    }
    if context.services_revoked.get() {
        return NATIVE_STATUS_UNAVAILABLE;
    }
    if !context.settings_service_available() {
        return NATIVE_STATUS_UNAVAILABLE;
    }
    if out_size < size_of::<KeybindStateV1>() as u32 {
        return NATIVE_STATUS_BUFFER_TOO_SMALL;
    }
    if out_state.is_null()
        || setting_key_utf8.is_null()
        || setting_key_length == 0
        || setting_key_length as usize > MAX_SETTING_KEY_BYTES
    {
        return NATIVE_STATUS_INVALID_ARGUMENT;
    }
    let key_bytes =
        unsafe { std::slice::from_raw_parts(setting_key_utf8, setting_key_length as usize) };
    let Ok(setting_key) = std::str::from_utf8(key_bytes) else {
        return NATIVE_STATUS_INVALID_ARGUMENT;
    };
    let Ok(mut keybinds) = context.keybinds.try_borrow_mut() else {
        return NATIVE_STATUS_BUSY;
    };
    let allowed = game_is_focused() && !shroudforge_hotkeys::is_capture_active();
    match keybinds.read(setting_key, allowed) {
        Ok(flags) => {
            unsafe {
                out_state.write(KeybindStateV1 {
                    size: size_of::<KeybindStateV1>() as u32,
                    flags,
                });
            }
            NATIVE_STATUS_OK
        }
        Err(status) => status,
    }
}

#[cfg(windows)]
fn game_is_focused() -> bool {
    use windows_sys::Win32::{
        System::Threading::GetCurrentProcessId,
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    };

    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_null() {
        return false;
    }
    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(foreground, &mut process_id) };
    process_id == unsafe { GetCurrentProcessId() }
}

#[cfg(not(windows))]
fn game_is_focused() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared_keybind(default: &str) -> Vec<mod_loader::SettingDefinition> {
        serde_json::from_value(serde_json::json!([{
            "key": "toggle",
            "type": "string",
            "control": "keybind",
            "label": "Toggle",
            "default": default
        }]))
        .unwrap()
    }

    #[test]
    fn keybind_press_edges_are_one_shot_and_blocked_edges_are_consumed() {
        let mut binding = Binding {
            hotkey: None,
            previous_revision: None,
        };
        assert_eq!(binding.sample(4, false, true), 0);
        assert_eq!(binding.sample(5, false, true), KEYBIND_PRESSED);
        assert_eq!(binding.sample(5, true, true), KEYBIND_DOWN);
        assert_eq!(binding.sample(6, true, false), 0);
        assert_eq!(binding.sample(6, false, true), 0);
        assert_eq!(
            binding.sample(7, true, true),
            KEYBIND_DOWN | KEYBIND_PRESSED
        );
    }

    #[test]
    fn settings_reparse_is_atomic_and_binding_changes_reset_the_baseline() {
        let declared = declared_keybind("SDL:67");
        let mut keybinds = Keybinds::default();
        keybinds
            .update(&declared, br#"{"toggle":"SDL:67"}"#)
            .unwrap();
        keybinds.0.get_mut("toggle").unwrap().previous_revision = Some(12);

        assert!(keybinds.update(&declared, br#"{"toggle":"F10"}"#).is_err());
        assert_eq!(
            keybinds.0.get("toggle").unwrap().previous_revision,
            Some(12)
        );

        keybinds
            .update(&declared, br#"{"toggle":"SDL:67"}"#)
            .unwrap();
        assert_eq!(
            keybinds.0.get("toggle").unwrap().previous_revision,
            Some(12)
        );

        keybinds
            .update(&declared, br#"{"toggle":"SDL:68"}"#)
            .unwrap();
        assert_eq!(keybinds.0.get("toggle").unwrap().previous_revision, None);
    }
}
