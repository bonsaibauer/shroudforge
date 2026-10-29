use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum InputDevice {
    Keyboard,
    Mouse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Trigger {
    Keyboard(u16),
    Mouse(u8),
}

impl Trigger {
    pub(crate) fn device(self) -> InputDevice {
        match self {
            Self::Keyboard(_) => InputDevice::Keyboard,
            Self::Mouse(_) => InputDevice::Mouse,
        }
    }
    pub(crate) fn primary_modifier(self) -> u8 {
        match self {
            Self::Keyboard(code) => crate::observe_keyboard::primary_modifier(code),
            Self::Mouse(_) => 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Chord {
    pub(crate) trigger: Trigger,
    pub(crate) modifiers: u8,
}

impl Chord {
    pub(crate) fn required_modifiers(&self) -> u8 {
        self.modifiers | self.trigger.primary_modifier()
    }
    fn parts(&self, trigger: String) -> String {
        let mut parts = Vec::with_capacity(5);
        for (bit, name) in [(1, "Ctrl"), (2, "Alt"), (4, "Shift"), (8, "Meta")] {
            if self.modifiers & bit != 0 {
                parts.push(name.to_owned());
            }
        }
        parts.push(trigger);
        parts.join("+")
    }
    fn normalized(&self) -> String {
        self.parts(match self.trigger {
            Trigger::Keyboard(code) => format!("SDL:{code}"),
            Trigger::Mouse(button) => format!("Mouse:{button}"),
        })
    }
    fn label(&self) -> String {
        self.parts(match self.trigger {
            Trigger::Keyboard(code) => crate::observe_keyboard::key_label(code),
            Trigger::Mouse(button) => crate::observe_keyboard::mouse_label(button),
        })
    }
}

/// One setting, with an optional keyboard chord and an optional mouse chord.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub(crate) chords: Vec<Chord>,
}

impl Hotkey {
    pub(crate) fn single(chord: Chord) -> Self {
        Self {
            chords: vec![chord],
        }
    }
    pub fn normalized(&self) -> String {
        self.chords
            .iter()
            .map(Chord::normalized)
            .collect::<Vec<_>>()
            .join("|")
    }
    pub fn label(&self) -> String {
        self.chords
            .iter()
            .map(Chord::label)
            .collect::<Vec<_>>()
            .join(" / ")
    }
    pub fn keyboard_scancodes(&self) -> impl Iterator<Item = u16> + '_ {
        self.chords.iter().filter_map(|chord| match chord.trigger {
            Trigger::Keyboard(scancode) => Some(scancode),
            Trigger::Mouse(_) => None,
        })
    }
    /// The chords of this setting on `device` only, or None without one.
    pub fn on_device(&self, device: InputDevice) -> Option<Self> {
        let chords: Vec<_> = self
            .chords
            .iter()
            .filter(|chord| chord.trigger.device() == device)
            .cloned()
            .collect();
        (!chords.is_empty()).then_some(Self { chords })
    }
    pub fn overlaps(&self, other: &Self) -> bool {
        self.chords.iter().any(|left| {
            other.chords.iter().any(|right| {
                left.trigger == right.trigger
                    && left.required_modifiers() == right.required_modifiers()
            })
        })
    }
}

/// All physical key translation is supplied by SDL. No VK or browser mapping.
pub fn parse_hotkey(value: &str) -> Result<Option<Hotkey>, String> {
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 100 {
        return Err("Hotkey is longer than 100 characters".into());
    }
    let mut chords = Vec::with_capacity(2);
    for alternative in value.split('|') {
        let parts: Vec<_> = alternative.split('+').collect();
        let (key, prefixes) = parts.split_last().unwrap();
        let trigger = if let Some(code) = key.strip_prefix("SDL:") {
            Trigger::Keyboard(
                code.parse::<u16>()
                    .ok()
                    .filter(|value| crate::observe_keyboard::valid_scancode(*value))
                    .ok_or("Hotkey requires a valid SDL scancode")?,
            )
        } else if let Some(button) = key.strip_prefix("Mouse:") {
            Trigger::Mouse(
                button
                    .parse::<u8>()
                    .ok()
                    .filter(|value| (1..=32).contains(value))
                    .ok_or("Mouse buttons must be between 1 and 32")?,
            )
        } else {
            return Err("Hotkey requires an SDL scancode or a mouse button".into());
        };
        if chords
            .iter()
            .any(|chord: &Chord| chord.trigger.device() == trigger.device())
        {
            return Err("A shortcut has at most one keyboard and one mouse binding".into());
        }
        let mut modifiers = 0;
        for prefix in prefixes {
            let bit = match *prefix {
                "Ctrl" => 1,
                "Alt" => 2,
                "Shift" => 4,
                "Meta" => 8,
                _ => return Err(format!("Unknown hotkey modifier '{prefix}'")),
            };
            if modifiers & bit != 0 {
                return Err(format!("Duplicate hotkey modifier '{prefix}'"));
            }
            modifiers |= bit;
        }
        chords.push(Chord {
            trigger,
            modifiers: modifiers & !trigger.primary_modifier(),
        });
    }
    chords.sort_by_key(|chord| chord.trigger.device());
    let binding = Hotkey { chords };
    if binding.normalized() != value {
        return Err(format!(
            "Use the canonical hotkey '{}'",
            binding.normalized()
        ));
    }
    Ok(Some(binding))
}

pub fn parse_value(value: &Value) -> Result<Option<Hotkey>, String> {
    parse_hotkey(
        value
            .as_str()
            .ok_or("Hotkey must be an SDL binding string")?,
    )
}
