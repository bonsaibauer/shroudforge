use crate::{Session, seconds};
use serde_json::json;

impl Session {
    pub(super) fn publish(&self) {
        let value = json!({"schemaVersion":1,"pid":std::process::id(),"updatedAt":seconds(),"active":self.active,
            "jobId":self.job.as_ref().map(|job| job.id.as_str()),
            "reason":self.reason,"remainingSeconds":if self.active {self.limit().saturating_sub(self.started.elapsed().as_secs())}else{0},
            "lastSampleAt":self.last_sample,"report":self.last_report.clone()});
        // Off the mods' loop in game: a write could stall every mod at times.
        if self.background_io {
            shroudforge_loader_config::config::write_document_in_background(
                &self.root,
                crate::documents::STATUS,
                value,
                |error| tracing::error!(target:"shroudforge::diagnostics","Status publication failed: {error}"),
            );
        } else if let Err(error) =
            shroudforge_loader_config::config::write_document(&self.root, crate::documents::STATUS, &value)
        {
            tracing::error!(target:"shroudforge::diagnostics","Status publication failed: {error}");
        }
    }
}
