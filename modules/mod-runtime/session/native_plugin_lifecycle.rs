pub(crate) const NATIVE_STATUS_OK: i32 = 0;
const DRAIN_TIMEOUT_MS: u32 = 250;

pub(crate) trait LifecycleCallbacks {
    fn initialize(&mut self) -> i32;
    fn revoke_services(&mut self) -> i32;
    fn drain_services(&mut self, timeout_ms: u32) -> i32;
    fn stop(&mut self) -> i32;
    fn shutdown(&mut self) -> i32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TeardownOutcome {
    Quiescent,
    Retain { stage: &'static str, status: i32 },
}

impl TeardownOutcome {
    pub(crate) fn description(self) -> String {
        match self {
            Self::Quiescent => "partial initialization was rolled back and services drained".into(),
            Self::Retain { stage, status } => {
                format!("module retained at {stage} with status {status}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InitializationFailure {
    pub(crate) status: i32,
    pub(crate) cleanup: TeardownOutcome,
}

pub(crate) fn initialize_or_rollback(
    callbacks: &mut impl LifecycleCallbacks,
) -> Result<(), InitializationFailure> {
    let status = callbacks.initialize();
    if status == NATIVE_STATUS_OK {
        return Ok(());
    }

    Err(InitializationFailure {
        status,
        cleanup: teardown(callbacks),
    })
}

pub(crate) fn teardown(callbacks: &mut impl LifecycleCallbacks) -> TeardownOutcome {
    let status = callbacks.revoke_services();
    if status != NATIVE_STATUS_OK {
        return TeardownOutcome::Retain {
            stage: "service revocation",
            status,
        };
    }
    let status = callbacks.drain_services(DRAIN_TIMEOUT_MS);
    if status != NATIVE_STATUS_OK {
        return TeardownOutcome::Retain {
            stage: "initial service drain",
            status,
        };
    }
    let status = callbacks.stop();
    if status != NATIVE_STATUS_OK {
        return TeardownOutcome::Retain {
            stage: "plugin stop",
            status,
        };
    }
    let status = callbacks.shutdown();
    if status != NATIVE_STATUS_OK {
        return TeardownOutcome::Retain {
            stage: "plugin shutdown",
            status,
        };
    }
    let status = callbacks.drain_services(DRAIN_TIMEOUT_MS);
    if status != NATIVE_STATUS_OK {
        return TeardownOutcome::Retain {
            stage: "final service drain",
            status,
        };
    }
    TeardownOutcome::Quiescent
}

#[cfg(test)]
mod tests {
    use super::{
        LifecycleCallbacks, NATIVE_STATUS_OK, TeardownOutcome, initialize_or_rollback, teardown,
    };
    use std::collections::VecDeque;

    const BUSY: i32 = 6;
    const FAILED: i32 = 7;
    const DRAIN_TIMEOUT_MS: u32 = 250;

    struct FakePlugin {
        events: Vec<&'static str>,
        initialize: i32,
        revoke: i32,
        drains: VecDeque<i32>,
        stop: i32,
        shutdown: i32,
    }

    impl LifecycleCallbacks for FakePlugin {
        fn initialize(&mut self) -> i32 {
            self.events.push("initialize");
            self.initialize
        }

        fn revoke_services(&mut self) -> i32 {
            self.events.push("revoke");
            self.revoke
        }

        fn drain_services(&mut self, timeout_ms: u32) -> i32 {
            assert_eq!(timeout_ms, DRAIN_TIMEOUT_MS);
            self.events.push("drain");
            self.drains.pop_front().unwrap_or(NATIVE_STATUS_OK)
        }

        fn stop(&mut self) -> i32 {
            self.events.push("stop");
            self.stop
        }

        fn shutdown(&mut self) -> i32 {
            self.events.push("shutdown");
            self.shutdown
        }
    }

    fn plugin() -> FakePlugin {
        FakePlugin {
            events: Vec::new(),
            initialize: NATIVE_STATUS_OK,
            revoke: NATIVE_STATUS_OK,
            drains: [NATIVE_STATUS_OK, NATIVE_STATUS_OK].into(),
            stop: NATIVE_STATUS_OK,
            shutdown: NATIVE_STATUS_OK,
        }
    }

    #[test]
    fn successful_teardown_revokes_and_drains_around_plugin_shutdown() {
        let mut plugin = plugin();

        assert_eq!(teardown(&mut plugin), TeardownOutcome::Quiescent);
        assert_eq!(
            plugin.events,
            ["revoke", "drain", "stop", "shutdown", "drain"]
        );
    }

    #[test]
    fn busy_service_drain_retains_module_even_when_plugin_stop_succeeds() {
        let mut plugin = plugin();
        plugin.drains = [NATIVE_STATUS_OK, BUSY].into();

        assert_eq!(
            teardown(&mut plugin),
            TeardownOutcome::Retain {
                stage: "final service drain",
                status: BUSY,
            }
        );
        assert_eq!(
            plugin.events,
            ["revoke", "drain", "stop", "shutdown", "drain"]
        );
    }

    #[test]
    fn pre_stop_busy_drain_never_calls_plugin_or_allows_unload() {
        let mut plugin = plugin();
        plugin.drains = [BUSY].into();

        assert_eq!(
            teardown(&mut plugin),
            TeardownOutcome::Retain {
                stage: "initial service drain",
                status: BUSY,
            }
        );
        assert_eq!(plugin.events, ["revoke", "drain"]);
    }

    #[test]
    fn failed_revoke_retains_module_without_entering_plugin_code() {
        let mut plugin = plugin();
        plugin.revoke = FAILED;

        assert_eq!(
            teardown(&mut plugin),
            TeardownOutcome::Retain {
                stage: "service revocation",
                status: FAILED,
            }
        );
        assert_eq!(plugin.events, ["revoke"]);
    }

    #[test]
    fn failed_plugin_stop_skips_shutdown_and_retains_module() {
        let mut plugin = plugin();
        plugin.stop = FAILED;

        assert_eq!(
            teardown(&mut plugin),
            TeardownOutcome::Retain {
                stage: "plugin stop",
                status: FAILED,
            }
        );
        assert_eq!(plugin.events, ["revoke", "drain", "stop"]);
    }

    #[test]
    fn failed_initialize_runs_the_same_partial_registration_rollback() {
        let mut plugin = plugin();
        plugin.initialize = FAILED;

        let failure = initialize_or_rollback(&mut plugin).unwrap_err();
        assert_eq!(failure.status, FAILED);
        assert_eq!(failure.cleanup, TeardownOutcome::Quiescent);
        assert_eq!(
            plugin.events,
            ["initialize", "revoke", "drain", "stop", "shutdown", "drain"]
        );
    }
}
