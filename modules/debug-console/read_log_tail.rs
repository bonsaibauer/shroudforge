use std::io::{Read, Seek};
use std::fs::File;
use std::io::SeekFrom;
use std::path::Path;

/// What the console page should do with one log: replace its text, append
/// complete lines to it, or keep it (`None` from `LogFollower::read`).
#[derive(serde::Serialize, Debug, PartialEq)]
pub(crate) struct LogChunk {
    pub(crate) mode: &'static str,
    pub(crate) text: String,
    pub(crate) state: &'static str,
}

/// Follows one log file for the console, so that only new complete lines
/// travel to the page once it holds the tail.
#[derive(Default)]
pub(crate) struct LogFollower {
    /// Bytes already handed to the page: always just after a newline.
    offset: Option<u64>,
    /// State last reported for an unreadable or missing file.
    unreadable: Option<&'static str>,
}

impl LogFollower {
    /// Forget what the page holds: the next read replaces its text.
    pub(crate) fn reset(&mut self) {
        self.offset = None;
        self.unreadable = None;
    }

    /// Complete lines written since the previous read. The first read, a
    /// shrunk file (a new session) or more than `maximum` new bytes replace
    /// the page's text with the last `maximum` bytes instead. A line still
    /// being written waits for its newline.
    pub(crate) fn read(&mut self, path: &Path, maximum: u64) -> Option<LogChunk> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(_) => return self.report_unreadable(if path.exists() { "error" } else { "missing" }),
        };
        let Ok(end) = file.seek(SeekFrom::End(0)) else {
            return self.report_unreadable("error");
        };
        self.unreadable = None;
        if let Some(offset) = self.offset {
            if end == offset {
                return None;
            }
            if end > offset && end - offset <= maximum {
                let bytes = read_range(&mut file, offset, end)?;
                let complete = bytes.iter().rposition(|byte| *byte == b'\n')? + 1;
                self.offset = Some(offset + complete as u64);
                return Some(LogChunk {
                    mode: "append",
                    text: String::from_utf8_lossy(&bytes[..complete]).into_owned(),
                    state: "ready",
                });
            }
        }
        let start = end.saturating_sub(maximum);
        let Some(mut bytes) = read_range(&mut file, start, end) else {
            return self.report_unreadable("error");
        };
        let mut first = 0;
        if start > 0 {
            // The tail starts inside a line: drop that fragment.
            first = bytes.iter().position(|byte| *byte == b'\n').map_or(bytes.len(), |n| n + 1);
        }
        let complete = bytes.iter().rposition(|byte| *byte == b'\n').map_or(first, |n| (n + 1).max(first));
        bytes.truncate(complete);
        self.offset = Some(start + complete as u64);
        Some(LogChunk {
            mode: "replace",
            text: String::from_utf8_lossy(&bytes[first..]).into_owned(),
            state: "ready",
        })
    }

    fn report_unreadable(&mut self, state: &'static str) -> Option<LogChunk> {
        self.offset = None;
        if self.unreadable == Some(state) {
            return None;
        }
        self.unreadable = Some(state);
        Some(LogChunk { mode: "replace", text: String::new(), state })
    }
}

fn read_range(file: &mut File, start: u64, end: u64) -> Option<Vec<u8>> {
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::with_capacity((end - start) as usize);
    file.take(end - start).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn append(path: &Path, text: &str) {
        std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap().write_all(text.as_bytes()).unwrap();
    }

    #[test]
    fn follows_complete_lines_and_replaces_when_needed() {
        let directory = std::env::temp_dir().join(format!("sf-console-follow-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("client.log");
        let _ = std::fs::remove_file(&path);
        let mut follower = LogFollower::default();

        let missing = follower.read(&path, 64).unwrap();
        assert_eq!((missing.mode, missing.state), ("replace", "missing"));
        assert!(follower.read(&path, 64).is_none(), "a missing file is reported once");

        append(&path, "one\ntwo\npart");
        let first = follower.read(&path, 64).unwrap();
        assert_eq!((first.mode, first.text.as_str()), ("replace", "one\ntwo\n"));
        assert!(follower.read(&path, 64).is_none(), "an unfinished line waits");

        append(&path, "ial\nthree\n");
        let next = follower.read(&path, 64).unwrap();
        assert_eq!((next.mode, next.text.as_str()), ("append", "partial\nthree\n"));
        assert!(follower.read(&path, 64).is_none(), "nothing new");

        append(&path, &"x".repeat(80));
        append(&path, "\nlast\n");
        let jump = follower.read(&path, 64).unwrap();
        assert_eq!((jump.mode, jump.text.as_str()), ("replace", "last\n"), "too much new text: tail again");

        std::fs::write(&path, "new session\n").unwrap();
        let rotated = follower.read(&path, 64).unwrap();
        assert_eq!((rotated.mode, rotated.text.as_str()), ("replace", "new session\n"));

        follower.reset();
        assert_eq!(follower.read(&path, 64).unwrap().mode, "replace", "reset resends the tail");
        let _ = std::fs::remove_dir_all(&directory);
    }
}
