use crate::Session;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub(super) fn defaults() -> Value {
    json!({"enabled":false,"sessionActive":false,"continuous":true,"intervalMilliseconds":2000,"maximumDurationSeconds":120,"slowCallbackMilliseconds":20,"onlyChanges":true,"jobId":"runtime.overview","requestId":0})
}

impl Session {
    pub(super) fn refresh(&mut self) {
        if Instant::now() < self.next_config {
            return;
        }
        self.next_config = Instant::now() + Duration::from_millis(500);
        let read = if self.background_io {
            shroudforge_loader_config::config::read_loader_in_background(&self.root)
        } else {
            shroudforge_loader_config::config::read_loader(&self.root)
        };
        let config = match read {
            Ok(value) => value,
            Err(error) => {
                if error != self.last_error {
                    tracing::error!(target:"shroudforge::diagnostics","Configuration rejected: {error}");
                    self.last_error = error;
                }
                return;
            }
        };
        self.last_error.clear();
        let mut next = defaults();
        if let Some(values) = config["modules"]["runtimeDiagnostics"].as_object() {
            for (key, value) in values {
                next[key] = value.clone();
            }
        }
        let active = next["enabled"] == true && next["sessionActive"] == true;
        let start = active
            && (self.settings["sessionActive"] != true
                || next["requestId"] != self.settings["requestId"]);
        if !active && self.active {
            self.stop_diagnostic_job();
        }
        next["sessionActive"] = active.into();
        self.settings = next;
        if start {
            self.start_diagnostic_job();
        }
    }
}
