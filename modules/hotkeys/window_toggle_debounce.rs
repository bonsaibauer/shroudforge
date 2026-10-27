use std::time::{Duration, Instant};

const WINDOW_TOGGLE_DEBOUNCE: Duration = Duration::from_millis(250);

/// Suppresses duplicate presses for configured native UI window toggles.
/// Each window owns its own instance. Mod keybind queries do not use this gate.
pub struct WindowToggleDebouncer {
    last_press_revision: u64,
    last_toggle_at: Option<Instant>,
}

impl WindowToggleDebouncer {
    pub fn new(current_press_revision: u64) -> Self {
        Self {
            last_press_revision: current_press_revision,
            last_toggle_at: None,
        }
    }

    /// Reset the baseline after a configured window binding changes.
    pub fn reset(&mut self, current_press_revision: u64) {
        self.last_press_revision = current_press_revision;
        self.last_toggle_at = None;
    }

    /// Consume a new press revision and allow a toggle when it is eligible
    /// and at least 250 ms have passed since the last accepted toggle.
    pub fn should_toggle(
        &mut self,
        current_press_revision: u64,
        eligible: impl FnOnce() -> bool,
    ) -> bool {
        if current_press_revision < self.last_press_revision {
            self.reset(current_press_revision);
            return false;
        }
        if current_press_revision == self.last_press_revision {
            return false;
        }

        self.last_press_revision = current_press_revision;
        let now = Instant::now();
        if self
            .last_toggle_at
            .is_some_and(|last| now.saturating_duration_since(last) < WINDOW_TOGGLE_DEBOUNCE)
        {
            return false;
        }
        if !eligible() {
            return false;
        }

        self.last_toggle_at = Some(now);
        true
    }
}
