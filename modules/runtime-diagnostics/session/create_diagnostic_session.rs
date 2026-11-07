use crate::refresh_session_settings::defaults;
use crate::{DiagnosticJob, collectors::mod_callbacks::Measurement};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Instant,
};

pub struct Session {
    pub(super) root: PathBuf,
    // Settings read and status written off the caller's thread (in game).
    pub(super) background_io: bool,
    pub(super) settings: Value,
    pub(super) active: bool,
    pub(super) started: Instant,
    pub(super) next_config: Instant,
    pub(super) next_sample: Instant,
    pub(super) last_report: Value,
    pub(super) last_signature: Value,
    pub(super) last_error: String,
    pub(super) reason: &'static str,
    pub(super) measurements: BTreeMap<String, Measurement>,
    pub(super) job: Option<DiagnosticJob>,
    pub(super) last_sample: u64,
    pub(crate) job_execution: Option<Box<dyn crate::RunningJob>>,
    pub(crate) job_report: Value,
    pub(crate) job_exports: Option<String>,
    pub(crate) job_last_tick: Instant,
}

impl Session {
    pub fn new(root: &Path) -> Self {
        let now = Instant::now();
        let mut session = Self {
            root: root.into(),
            background_io: false,
            settings: defaults(),
            active: false,
            started: now,
            next_config: now,
            next_sample: now,
            last_report: Value::Null,
            last_signature: Value::Null,
            last_error: String::new(),
            reason: "disabled",
            measurements: BTreeMap::new(),
            job: None,
            last_sample: 0,
            job_execution: None,
            job_report: Value::Null,
            job_exports: None,
            job_last_tick: now,
        };
        session.refresh();
        session.publish();
        session
    }

    /// From now on, reads the loader settings and writes the status off the
    /// caller's thread: in game, the mods' loop, which one disk access could
    /// stall for 100 ms at times. Settings are then up to 250 ms old.
    pub fn with_background_io(mut self) -> Self {
        self.background_io = true;
        self
    }
}
