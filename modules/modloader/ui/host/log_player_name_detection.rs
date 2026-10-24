use shroudforge_logging::write_activity;
use shroudforge_player_name::{DetectionReport, ResolvedName};

pub(crate) fn log_player_name_detection(
    root: &std::path::Path,
    report: &DetectionReport,
    resolved: &ResolvedName,
    previous_report: &mut Option<(DetectionReport, ResolvedName)>,
) {
    let current = (report.clone(), resolved.clone());
    if previous_report.as_ref() == Some(&current) {
        return;
    }

    let details = format!(
        "{}. {}. Effective name: {}. Runtime candidate: {}. Process ID: {}.{}",
        report.status.description(),
        resolved.source.description(),
        resolved.name,
        report.name.as_deref().unwrap_or("unavailable"),
        report
            .process_id
            .map(|process_id| process_id.to_string())
            .unwrap_or_else(|| "unavailable".into()),
        report
            .detail
            .as_deref()
            .map(|detail| format!(" Selection detail: {detail}."))
            .unwrap_or_default(),
    );
    let (title, level) = match resolved.source {
        shroudforge_player_name::NameSource::Live => ("Runtime character name used", "info"),
        shroudforge_player_name::NameSource::Fallback => ("Configured fallback used", "info"),
    };
    write_activity(
        root,
        "Modloader",
        "Player name detection",
        title,
        Some(&details),
        level,
    );
    *previous_report = Some(current);
}
