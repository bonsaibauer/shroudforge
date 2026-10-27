use super::log_configuration_changes::log_configuration_check_transitions;
use super::log_configuration_errors::log_configuration_errors;
use super::log_player_name_detection::log_player_name_detection;
use super::read_host_arguments::Arguments;
use super::read_installed_mods::read_mods;
use super::read_ui_configuration::read_central_config;
use super::read_ui_snapshot::read_ui_snapshot;
use std::collections::{BTreeMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

type SharedValue = Arc<Mutex<serde_json::Value>>;

pub(crate) struct RefreshedSnapshot {
    pub(crate) central: serde_json::Value,
    pub(crate) payload: Option<String>,
}

pub(crate) struct SnapshotRefresher {
    requests: Sender<serde_json::Value>,
    results: Receiver<RefreshedSnapshot>,
    running: bool,
    queued: Option<serde_json::Value>,
}

#[derive(Default)]
struct PreviousStatus {
    configuration_errors: HashSet<String>,
    player_name_detection: Option<(
        shroudforge_player_name::DetectionReport,
        shroudforge_player_name::ResolvedName,
    )>,
    configuration_checks: BTreeMap<String, (String, String)>,
    asset_status: BTreeMap<String, (String, String)>,
    mod_statuses: BTreeMap<String, (String, String)>,
}

impl SnapshotRefresher {
    pub(crate) fn start(
        arguments: &Arguments,
        release: &SharedValue,
        release_announcement: &SharedValue,
        catalog: &SharedValue,
    ) -> Self {
        let (requests, request_receiver) = mpsc::channel::<serde_json::Value>();
        let (result_sender, results) = mpsc::channel();
        let root = arguments.root.clone();
        let (game_pid, desktop, server) = (arguments.game_pid, arguments.desktop, arguments.server);
        let (release, release_announcement, catalog) = (
            release.clone(),
            release_announcement.clone(),
            catalog.clone(),
        );

        std::thread::spawn(move || {
            let mut previous = PreviousStatus::default();
            for provider in request_receiver {
                let refreshed = refresh(
                    &root,
                    game_pid,
                    desktop,
                    server,
                    &provider,
                    &release,
                    &release_announcement,
                    &catalog,
                    &mut previous,
                );
                if result_sender.send(refreshed).is_err() {
                    break;
                }
            }
        });

        Self {
            requests,
            results,
            running: false,
            queued: None,
        }
    }

    pub(crate) fn request(&mut self, provider: &serde_json::Value) {
        if self.running {
            self.queued = Some(provider.clone());
            return;
        }
        self.running = self.requests.send(provider.clone()).is_ok();
    }

    pub(crate) fn take(&mut self) -> Option<RefreshedSnapshot> {
        let mut latest = None;
        while let Ok(refreshed) = self.results.try_recv() {
            latest = Some(refreshed);
            self.running = false;
        }
        if !self.running
            && let Some(provider) = self.queued.take()
        {
            self.request(&provider);
        }
        latest
    }
}

fn refresh(
    root: &std::path::Path,
    game_pid: u32,
    desktop: bool,
    server: bool,
    provider: &serde_json::Value,
    release: &SharedValue,
    release_announcement: &SharedValue,
    catalog: &SharedValue,
    previous: &mut PreviousStatus,
) -> RefreshedSnapshot {
    let central = read_central_config(root);
    let locale = central
        .pointer("/general/locale")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("en-GB");
    let mods = read_mods(root, locale);
    let notification_mods: Vec<_> = mods
        .iter()
        .map(|item| shroudforge_news::ModNotification {
            id: &item.id,
            name: &item.name,
            version: &item.version,
            ecosystem: item.ecosystem,
            changelog: &item.changelog,
        })
        .collect();
    shroudforge_news::sync_mod_events(root, &notification_mods);
    let game_process_id = (!desktop && !server).then_some(game_pid);
    let player_name_detection =
        shroudforge_player_name::detect_active_character_for_process(root, game_process_id);
    let (snapshot, resolved_player_name) = read_ui_snapshot(
        root,
        game_pid,
        desktop,
        server,
        provider,
        release,
        release_announcement,
        catalog,
        central.clone(),
        mods,
        &player_name_detection,
    );
    log_player_name_detection(
        root,
        &player_name_detection,
        &resolved_player_name,
        &mut previous.player_name_detection,
    );
    log_configuration_check_transitions(
        root,
        &snapshot.configuration,
        &mut previous.configuration_checks,
        &mut previous.asset_status,
        &mut previous.mod_statuses,
    );
    log_configuration_errors(root, &snapshot, &mut previous.configuration_errors);
    RefreshedSnapshot {
        central,
        payload: serde_json::to_string(&snapshot).ok(),
    }
}
