use std::time::Instant;
#[derive(Clone, Copy, Default)]
pub(super) struct Batch {
    pub toggle: Option<bool>,
    pub radius: Option<u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Event {
    Toggle(bool),
    Radius(u32),
}
pub(super) struct Pump {
    started: Instant,
    pending: Batch,
    next: u64,
}
impl Default for Pump {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            pending: Batch::default(),
            next: 0,
        }
    }
}
impl Pump {
    pub(super) fn tick(
        &mut self,
        client: bool,
        take: impl FnOnce() -> Option<Batch>,
        persist: impl FnMut(Event) -> bool,
    ) {
        self.at(
            self.started.elapsed().as_millis().min(u64::MAX as u128) as u64,
            client,
            take,
            persist,
        );
    }
    fn at(
        &mut self,
        now: u64,
        client: bool,
        take: impl FnOnce() -> Option<Batch>,
        mut persist: impl FnMut(Event) -> bool,
    ) {
        if !client {
            self.pending = Batch::default();
            self.next = 0;
            return;
        }
        if now < self.next {
            return;
        }
        if let Some(batch) = take() {
            if batch.toggle.is_some() {
                self.pending.toggle = batch.toggle;
            }
            if batch.radius.is_some() {
                self.pending.radius = batch.radius;
            }
        }
        if let Some(value) = self.pending.toggle {
            if persist(Event::Toggle(value)) {
                self.pending.toggle = None;
            }
        }
        if let Some(value) = self.pending.radius {
            if persist(Event::Radius(value)) {
                self.pending.radius = None;
            }
        }
        // Coalesce genuine newer events, retaining at most one per kind on an IO failure.
        let retry = self.pending.toggle.is_some() || self.pending.radius.is_some();
        self.next = now.saturating_add(if retry { 500 } else { 100 });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vein_feed_failure_retains_and_retries_without_repolling_early() {
        let mut p = Pump::default();
        let batch = Batch {
            toggle: Some(false),
            radius: Some(3),
        };
        p.at(0, true, || Some(batch), |_| false);
        assert_eq!(p.pending.toggle, Some(false));
        assert_eq!(p.pending.radius, Some(3));
        p.at(
            499,
            true,
            || panic!("early poll"),
            |_| panic!("early write"),
        );
        let mut delivered = Vec::new();
        p.at(
            500,
            true,
            || None,
            |e| {
                delivered.push(e);
                true
            },
        );
        assert_eq!(delivered, vec![Event::Toggle(false), Event::Radius(3)]);
        assert!(p.pending.toggle.is_none() && p.pending.radius.is_none());
    }
    #[test]
    fn vein_latest_typed_event_coalesces_without_inventing_state() {
        let mut p = Pump::default();
        p.at(
            0,
            true,
            || {
                Some(Batch {
                    toggle: Some(true),
                    radius: None,
                })
            },
            |_| false,
        );
        let mut seen = Vec::new();
        p.at(
            500,
            true,
            || {
                Some(Batch {
                    toggle: Some(false),
                    radius: Some(2),
                })
            },
            |e| {
                seen.push(e);
                true
            },
        );
        assert_eq!(seen, vec![Event::Toggle(false), Event::Radius(2)]);
        p.at(600, true, || None, |_| panic!("duplicate event"));
    }
    #[test]
    fn vein_server_gate_does_not_poll_or_persist_and_clears_pending() {
        let mut p = Pump::default();
        p.at(
            0,
            true,
            || {
                Some(Batch {
                    toggle: Some(true),
                    radius: None,
                })
            },
            |_| false,
        );
        p.at(
            10,
            false,
            || panic!("server poll"),
            |_| panic!("server write"),
        );
        assert!(p.pending.toggle.is_none());
    }
}
