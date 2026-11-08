use crate::config::write_document;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};

type Key = (PathBuf, &'static str);

struct Pending {
    document: crate::Document,
    value: Value,
    sequence: u64,
    on_error: fn(&str),
}

#[derive(Default)]
struct State {
    pending: HashMap<Key, Pending>,
    // The newest write asked for each document: an older one is skipped.
    latest: HashMap<Key, u64>,
    next: u64,
}

struct Writer {
    state: Mutex<State>,
    ready: Condvar,
    // Held while a document is written, so a write made now and a queued
    // one of the same document never cross.
    writing: Mutex<()>,
    threaded: AtomicBool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn writer() -> &'static Writer {
    static WRITER: OnceLock<&'static Writer> = OnceLock::new();
    WRITER.get_or_init(|| {
        // Lives as long as the process, like its thread.
        let writer: &'static Writer = Box::leak(Box::new(Writer {
            state: Mutex::new(State::default()),
            ready: Condvar::new(),
            writing: Mutex::new(()),
            threaded: AtomicBool::new(true),
        }));
        let spawned = std::thread::Builder::new()
            .name("shroudforge-document-writer".into())
            .spawn(move || write_pending_forever(writer));
        writer.threaded.store(spawned.is_ok(), Ordering::Release);
        writer
    })
}

fn write_pending_forever(writer: &Writer) {
    loop {
        let batch: Vec<(Key, Pending)> = {
            let mut state = lock(&writer.state);
            while state.pending.is_empty() {
                state = writer
                    .ready
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            state.pending.drain().collect()
        };
        for (key, item) in batch {
            let _writing = lock(&writer.writing);
            if lock(&writer.state).latest.get(&key) != Some(&item.sequence) {
                continue;
            }
            if let Err(error) = write_document(&key.0, item.document, &item.value) {
                (item.on_error)(&error);
            }
        }
    }
}

/// Writes `value` to `document` on a background thread, for callers that must
/// not wait on the disk: the in-game mods' loop, where one write (validated,
/// flushed and renamed) stalled every mod for 100 to 270 ms at times. Only the
/// newest value of each document is written; `on_error` is called on the
/// writer thread when a write fails. Without a thread, writes at once.
pub fn write_document_in_background(
    root: &Path,
    document: crate::Document,
    value: Value,
    on_error: fn(&str),
) {
    let writer = writer();
    if !writer.threaded.load(Ordering::Acquire) {
        if let Err(error) = write_document(root, document, &value) {
            on_error(&error);
        }
        return;
    }
    let mut state = lock(&writer.state);
    let key = (root.to_path_buf(), document.name);
    state.next += 1;
    let sequence = state.next;
    state.latest.insert(key.clone(), sequence);
    state.pending.insert(
        key,
        Pending {
            document,
            value,
            sequence,
            on_error,
        },
    );
    writer.ready.notify_one();
}

/// Writes `value` to `document` now, after any background write of it under
/// way, and drops a queued older one (a final status, for instance).
pub fn write_document_now(
    root: &Path,
    document: crate::Document,
    value: &Value,
) -> Result<(), String> {
    let writer = writer();
    let _writing = lock(&writer.writing);
    {
        let mut state = lock(&writer.state);
        let key = (root.to_path_buf(), document.name);
        state.next += 1;
        let sequence = state.next;
        state.latest.insert(key.clone(), sequence);
        state.pending.remove(&key);
    }
    write_document(root, document, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DOCUMENT: crate::Document = crate::Document {
        name: "background-write-test.json",
        schema: "{}",
        storage: crate::Storage::Config("background-write-test.json"),
    };

    fn read(root: &Path) -> Option<Value> {
        let path = crate::config::document_path(root, TEST_DOCUMENT);
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    }

    #[test]
    fn writes_the_newest_value_in_background() {
        let root = tempfile::tempdir().unwrap();
        for index in 0..20 {
            write_document_in_background(
                root.path(),
                TEST_DOCUMENT,
                serde_json::json!({"index": index}),
                |_| {},
            );
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while read(root.path()) != Some(serde_json::json!({"index": 19})) {
            assert!(
                std::time::Instant::now() < deadline,
                "newest value not written"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn a_write_made_now_is_not_overwritten_by_an_older_queued_one() {
        let root = tempfile::tempdir().unwrap();
        write_document_in_background(
            root.path(),
            TEST_DOCUMENT,
            serde_json::json!({"running": true}),
            |_| {},
        );
        write_document_now(
            root.path(),
            TEST_DOCUMENT,
            &serde_json::json!({"running": false}),
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(
            read(root.path()),
            Some(serde_json::json!({"running": false}))
        );
    }
}
