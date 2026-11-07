//! Keys, mouse buttons and the mouse wheel a mod keeps from the game while it
//! reports one of its contexts (extended.mod.json `inputReservations`). The
//! game is denied them by filter_game_input; ShroudForge still reads the keys
//! in its own keyboard process, so the mods do too. The wheel, which
//! ShroudForge does not read, is counted here for the mods.
//!
//! A context is a lease: it ends CONTEXT_LEASE after the mod last reported
//! it, so a mod that stops, fails or stalls gives the game its input back.

use crate::Hotkey;
use crate::parse_hotkey::Trigger;
use std::{
    collections::{BTreeSet, HashMap},
    sync::{
        RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// How long a context stays active after the mod last reported it.
pub const CONTEXT_LEASE: Duration = Duration::from_millis(1000);

/// SDL scancodes the game always gets: Escape, F9 (Modloader) and F10 (console).
const NEVER_RESERVED: [u16; 3] = [41, 66, 67];

/// A keybind kept from the game while `context` is active: only the chords of
/// the devices the player left padlocked.
#[derive(Clone, Debug)]
pub struct ReservedKey {
    pub context: String,
    pub hotkey: Hotkey,
}

/// A mouse wheel read by a mod through the boolean setting `setting` while
/// `context` is active; `reserved` keeps it from the game meanwhile.
#[derive(Clone, Debug)]
pub struct ReadWheel {
    pub setting: String,
    pub context: String,
    pub reserved: bool,
}

#[derive(Default)]
struct Owner {
    keys: Vec<ReservedKey>,
    wheels: Vec<ReadWheel>,
    contexts: HashMap<String, Instant>,
    wheel_steps: HashMap<String, i32>,
}

impl Owner {
    fn active(&self, context: &str, now: Instant) -> bool {
        self.contexts.get(context).is_some_and(|until| *until > now)
    }
}

#[derive(Default)]
struct Reservations {
    owners: HashMap<String, Owner>,
}

static RESERVATIONS: RwLock<Option<Reservations>> = RwLock::new(None);
/// False while no mod declares anything: the filter returns at once.
static ANY: AtomicBool = AtomicBool::new(false);

/// What one path of the game's input (Raw Input, window messages) kept, so
/// that the release of a kept press is kept too, and only that.
#[derive(Default)]
pub(crate) struct KeptPresses {
    keys: BTreeSet<u16>,
    buttons: BTreeSet<u8>,
}

/// Replaces what the mod `owner` reserves (its keybinds as they are now
/// bound, its wheels). Called again when the player changes a binding or a
/// padlock.
pub fn set_reserved_input(owner: &str, keys: Vec<ReservedKey>, wheels: Vec<ReadWheel>) {
    let Ok(mut guard) = RESERVATIONS.write() else {
        return;
    };
    let reservations = guard.get_or_insert_with(Reservations::default);
    let entry = reservations.owners.entry(owner.to_owned()).or_default();
    entry.keys = keys
        .into_iter()
        .filter(|key| {
            key.hotkey
                .keyboard_scancodes()
                .all(|code| !NEVER_RESERVED.contains(&code))
        })
        .collect();
    entry.wheels = wheels;
    entry
        .wheel_steps
        .retain(|setting, _| entry.wheels.iter().any(|wheel| wheel.setting == *setting));
    ANY.store(
        reservations
            .owners
            .values()
            .any(|owner| !owner.keys.is_empty() || !owner.wheels.is_empty()),
        Ordering::Release,
    );
}

/// The mod `owner` reports whether `context` is active: renew it at every
/// update while it is (it ends CONTEXT_LEASE later otherwise), end it at once
/// with `false`.
pub fn set_input_context(owner: &str, context: &str, active: bool) {
    set_input_context_at(owner, context, active, Instant::now());
}

fn set_input_context_at(owner: &str, context: &str, active: bool, now: Instant) {
    let Ok(mut guard) = RESERVATIONS.write() else {
        return;
    };
    let entry = guard
        .get_or_insert_with(Reservations::default)
        .owners
        .entry(owner.to_owned())
        .or_default();
    if active {
        entry
            .contexts
            .insert(context.to_owned(), now + CONTEXT_LEASE);
    } else {
        entry.contexts.remove(context);
        // Steps counted while active are not carried into the next time.
        for wheel in entry.wheels.iter().filter(|wheel| wheel.context == context) {
            entry.wheel_steps.remove(&wheel.setting);
        }
    }
}

/// The wheel steps counted for the mod `owner` through `setting` since the
/// last call (positive: away from the player), then zero.
pub fn take_wheel_steps(owner: &str, setting: &str) -> i32 {
    RESERVATIONS
        .write()
        .ok()
        .and_then(|mut guard| {
            guard
                .as_mut()?
                .owners
                .get_mut(owner)?
                .wheel_steps
                .remove(setting)
        })
        .unwrap_or(0)
}

/// Forgets the mod `owner` (unloaded): the game gets its input back.
pub fn release_reserved_input(owner: &str) {
    set_reserved_input(owner, Vec::new(), Vec::new());
    if let Ok(mut guard) = RESERVATIONS.write()
        && let Some(reservations) = guard.as_mut()
    {
        reservations.owners.remove(owner);
    }
}

pub(crate) fn anything_reserved() -> bool {
    ANY.load(Ordering::Acquire)
}

fn reserved_now(trigger: Trigger, modifiers: u8, now: Instant) -> bool {
    if let Trigger::Keyboard(code) = trigger
        && NEVER_RESERVED.contains(&code)
    {
        return false;
    }
    let Ok(guard) = RESERVATIONS.read() else {
        return false;
    };
    guard.as_ref().is_some_and(|reservations| {
        reservations.owners.values().any(|owner| {
            owner.keys.iter().any(|key| {
                owner.active(&key.context, now)
                    && key.hotkey.chords.iter().any(|chord| {
                        chord.trigger == trigger && chord.required_modifiers() == modifiers
                    })
            })
        })
    })
}

impl KeptPresses {
    pub(crate) const fn new() -> Self {
        Self {
            keys: BTreeSet::new(),
            buttons: BTreeSet::new(),
        }
    }

    /// A key event of SDL scancode `code` with `modifiers` held (1 Ctrl,
    /// 2 Alt, 4 Shift, 8 Meta): whether the game must not get it.
    pub(crate) fn key(&mut self, code: u16, down: bool, modifiers: u8) -> bool {
        self.key_at(code, down, modifiers, Instant::now())
    }

    fn key_at(&mut self, code: u16, down: bool, modifiers: u8, now: Instant) -> bool {
        if !down {
            return self.keys.remove(&code);
        }
        if self.keys.contains(&code) {
            return true; // held: its repeats are kept too
        }
        let kept = reserved_now(Trigger::Keyboard(code), modifiers, now);
        if kept {
            self.keys.insert(code);
        }
        kept
    }

    /// Whether a later message of a kept key (its character) is kept.
    pub(crate) fn holds_key(&self, code: u16) -> bool {
        self.keys.contains(&code)
    }

    /// A mouse button event (SDL numbering: 1 left, 2 middle, 3 right, 4 and
    /// 5 the side buttons): whether the game must not get it.
    pub(crate) fn button(&mut self, button: u8, down: bool, modifiers: u8) -> bool {
        if !down {
            return self.buttons.remove(&button);
        }
        if self.buttons.contains(&button) {
            return true;
        }
        let kept = reserved_now(Trigger::Mouse(button), modifiers, Instant::now());
        if kept {
            self.buttons.insert(button);
        }
        kept
    }
}

/// Wheel `steps` turned: counted for each mod reading the wheel now, and
/// whether the game must not get them. `count` is false for the second path
/// of the same turn (the window message after its Raw Input).
pub(crate) fn wheel(steps: i32, count: bool) -> bool {
    wheel_at(steps, count, Instant::now())
}

fn wheel_at(steps: i32, count: bool, now: Instant) -> bool {
    let Ok(mut guard) = RESERVATIONS.write() else {
        return false;
    };
    let Some(reservations) = guard.as_mut() else {
        return false;
    };
    let mut kept = false;
    for owner in reservations.owners.values_mut() {
        let reading: Vec<_> = owner
            .wheels
            .iter()
            .filter(|wheel| owner.active(&wheel.context, now))
            .map(|wheel| (wheel.setting.clone(), wheel.reserved))
            .collect();
        for (setting, reserved) in reading {
            kept |= reserved;
            if count {
                *owner.wheel_steps.entry(setting).or_default() += steps;
            }
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_hotkey;

    fn key(owner: &str, binding: &str, context: &str) {
        set_reserved_input(
            owner,
            vec![ReservedKey {
                context: context.into(),
                hotkey: parse_hotkey(binding).unwrap().unwrap(),
            }],
            vec![ReadWheel {
                setting: "wheelDistance".into(),
                context: "free".into(),
                reserved: true,
            }],
        );
    }

    // One test: the reservations are process-wide.
    #[test]
    fn keeps_reserved_input_while_its_context_is_leased() {
        key("a", "SDL:30|Mouse:4", "hammer");
        let start = Instant::now();
        let mut raw = KeptPresses::default();
        assert!(
            !raw.key_at(30, true, 0, start),
            "no context yet: the game gets it"
        );
        assert!(!raw.key_at(30, false, 0, start), "its release too");

        set_input_context_at("a", "hammer", true, start);
        assert!(
            raw.key_at(30, true, 0, start),
            "kept while the context is active"
        );
        assert!(raw.key_at(30, true, 0, start), "its repeats too");
        assert!(raw.holds_key(30), "its character message is kept too");
        assert!(raw.key_at(30, false, 0, start), "and its release");
        assert!(
            !raw.key_at(30, true, 4, start),
            "Shift+key is another binding"
        );
        assert!(!raw.key_at(31, true, 0, start), "another key");

        // A press the game got keeps its release for the game, even if the
        // context starts in between.
        let mut messages = KeptPresses::default();
        set_input_context_at("a", "hammer", false, start);
        assert!(!messages.key_at(30, true, 0, start));
        set_input_context_at("a", "hammer", true, start);
        assert!(
            !messages.key_at(30, false, 0, start),
            "release of a press the game got"
        );

        // The lease ends without a renewal.
        assert!(!raw.key_at(
            30,
            true,
            0,
            start + CONTEXT_LEASE + Duration::from_millis(1)
        ));
        raw.key_at(30, false, 0, start);

        // Mouse button 4 of the same binding.
        set_input_context_at("a", "hammer", true, Instant::now());
        assert!(raw.button(4, true, 0) && raw.button(4, false, 0));
        assert!(!raw.button(1, true, 0));

        // Escape is never kept, whatever the mod declares.
        key("a", "SDL:41", "hammer");
        set_input_context_at("a", "hammer", true, start);
        assert!(!raw.key_at(41, true, 0, start));

        // The wheel: counted and kept only while its context is active.
        assert!(!wheel_at(1, true, start), "context free not active");
        assert_eq!(take_wheel_steps("a", "wheelDistance"), 0);
        set_input_context_at("a", "free", true, start);
        assert!(wheel_at(2, true, start) && wheel_at(2, false, start));
        assert!(wheel_at(-1, true, start));
        assert_eq!(
            take_wheel_steps("a", "wheelDistance"),
            1,
            "the message path is not counted twice"
        );
        assert_eq!(take_wheel_steps("a", "wheelDistance"), 0);

        release_reserved_input("a");
        assert!(!anything_reserved());
        assert!(!raw.key_at(30, true, 0, start));
    }
}
