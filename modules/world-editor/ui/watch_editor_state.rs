use crate::blueprint_library::ViewState;
use crate::blueprint_library::view_state;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

pub(crate) fn watch_editor_state(
    state_root: PathBuf,
    refresh_ms: u64,
) -> (mpsc::Receiver<ViewState>, mpsc::Sender<bool>) {
    let (state_sender, state_updates) = mpsc::sync_channel(1);
    let (visibility_sender, visibility_updates) = mpsc::channel();
    std::thread::spawn(move || {
        let refresh_interval = Duration::from_millis(refresh_ms);
        let mut visible = false;
        loop {
            if !visible {
                // No panel can use a snapshot while hidden, so wait instead of scanning files.
                match visibility_updates.recv() {
                    Ok(next_visible) => visible = next_visible,
                    Err(_) => break,
                }
                continue;
            }

            if let Ok(state) = view_state(&state_root) {
                match state_sender.try_send(state) {
                    Ok(()) | Err(mpsc::TrySendError::Full(_)) => {}
                    Err(mpsc::TrySendError::Disconnected(_)) => break,
                }
            }

            match visibility_updates.recv_timeout(refresh_interval) {
                Ok(next_visible) => visible = next_visible,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    (state_updates, visibility_sender)
}
