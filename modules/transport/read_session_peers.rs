//! The machines of the game session this client process is in, read from the
//! game's own `enshrouded.log`. Enshrouded writes one
//! `[online] Added peer <machine>(<slot>) (<SteamID64>)` line for every other
//! machine of the session and `[online] Removed peer <machine>(<slot>)` when it
//! leaves. `[Session] finished transition from '<old>' to '<new>'` says whether
//! this game hosts (`Host_Online`), joined another game (`Client_Online`) or is
//! back in the lobby. The host of a session is machine 0, a player's game or a
//! Dedicated Server. Only what the game appended since the previous call is
//! read, at most `MAX_READ_BYTES` per call.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

const MAX_READ_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SessionRole {
    #[default]
    None,
    Hosting,
    Joined,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionPeers {
    pub role: SessionRole,
    /// The other machines' SteamID64 by machine index.
    pub peers: BTreeMap<u32, u64>,
}

impl SessionPeers {
    /// The machine that hosts the game this client joined.
    pub fn host(&self) -> Option<u64> {
        (self.role == SessionRole::Joined)
            .then(|| self.peers.get(&0).copied())
            .flatten()
    }

    /// The players connected to the game this client hosts.
    pub fn players(&self) -> Option<Vec<u64>> {
        (self.role == SessionRole::Hosting).then(|| self.peers.values().copied().collect())
    }

    fn apply(&mut self, line: &str) {
        if let Some(rest) = after(line, "[online] Added peer ") {
            if let (Some(machine), Some(id)) = (machine_index(rest), peer_steam_id(rest)) {
                self.peers.insert(machine, id);
            }
        } else if let Some(rest) = after(line, "[online] Removed peer ") {
            if let Some(machine) = machine_index(rest) {
                self.peers.remove(&machine);
            }
        } else if let Some(rest) = after(line, "[Session] finished transition from '") {
            match rest.split("' to '").nth(1).and_then(|to| to.split('\'').next()) {
                Some("Host_Online") => self.role = SessionRole::Hosting,
                Some("Client_Online") => self.role = SessionRole::Joined,
                Some("Lobby") => *self = SessionPeers::default(),
                _ => {}
            }
        }
    }
}

fn after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.find(marker).map(|index| &line[index + marker.len()..])
}

fn machine_index(rest: &str) -> Option<u32> {
    let digits = rest.split('(').next()?;
    digits.parse().ok()
}

fn peer_steam_id(rest: &str) -> Option<u64> {
    let (_, tail) = rest.split_once(") (")?;
    let value = tail.split(')').next()?;
    let value = value.strip_prefix("steamid:").unwrap_or(value).trim();
    value.parse::<u64>().ok().filter(|id| *id != 0)
}

struct LogCursor {
    path: PathBuf,
    offset: u64,
    partial: Vec<u8>,
    session: SessionPeers,
}

static CURSOR: Mutex<Option<LogCursor>> = Mutex::new(None);

pub fn read_session_peers(root: &Path) -> SessionPeers {
    let path = root.join("enshrouded.log");
    let mut guard = CURSOR.lock().unwrap_or_else(PoisonError::into_inner);
    if guard.as_ref().is_none_or(|cursor| cursor.path != path) {
        *guard = Some(LogCursor {
            path,
            offset: 0,
            partial: Vec::new(),
            session: SessionPeers::default(),
        });
    }
    let cursor = guard.as_mut().expect("cursor was just set");
    let Ok(mut file) = std::fs::File::open(&cursor.path) else {
        return SessionPeers::default();
    };
    let length = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    // The game starts a new log at each start: read it from its beginning.
    if length < cursor.offset {
        cursor.offset = 0;
        cursor.partial.clear();
        cursor.session = SessionPeers::default();
    }
    if length > cursor.offset && file.seek(SeekFrom::Start(cursor.offset)).is_ok() {
        let mut bytes = Vec::new();
        if file.take(MAX_READ_BYTES).read_to_end(&mut bytes).is_ok() {
            cursor.offset += bytes.len() as u64;
            cursor.partial.extend_from_slice(&bytes);
            let mut start = 0;
            while let Some(end) = cursor.partial[start..].iter().position(|byte| *byte == b'\n') {
                let line = String::from_utf8_lossy(&cursor.partial[start..start + end]);
                cursor.session.apply(&line);
                start += end + 1;
            }
            cursor.partial.drain(..start);
        }
    }
    cursor.session.clone()
}

#[cfg(test)]
mod tests {
    use super::{SessionPeers, SessionRole};

    fn session(lines: &[&str]) -> SessionPeers {
        let mut session = SessionPeers::default();
        for line in lines {
            session.apply(line);
        }
        session
    }

    #[test]
    fn a_joined_game_names_machine_zero_as_its_host() {
        let session = session(&[
            "[I 01:34:00,795] [Session] started transition from 'Lobby' to 'Client_Online' (current='<invalid>')!",
            "[I 01:34:00,867] [online] Added peer 0(1) (90297598401357828)",
            "[I 01:34:04,729] [Session] finished transition from 'Lobby' to 'Client_Online' (current='Client_Online')!",
        ]);
        assert_eq!(session.role, SessionRole::Joined);
        assert_eq!(session.host(), Some(90297598401357828));
        assert_eq!(session.players(), None);
    }

    #[test]
    fn a_hosting_game_lists_the_players_who_are_still_connected() {
        let session = session(&[
            "[I 00:00:01,410] [Session] finished transition from 'Lobby' to 'Host_Online' (current='Host_Online')!",
            "[I 00:10:00,000] [online] Added peer 1(2) (76561199229658277)",
            "[I 00:11:00,000] [online] Added peer 2(3) (steamid:76561197992902915)",
            "[I 00:12:00,000] [online] Removed peer 1(2)",
        ]);
        assert_eq!(session.role, SessionRole::Hosting);
        assert_eq!(session.players(), Some(vec![76561197992902915]));
        assert_eq!(session.host(), None);
    }

    #[test]
    fn leaving_for_the_lobby_forgets_the_session() {
        let session = session(&[
            "[I 13:15:15,902] [online] Added peer 0(2) (76561199229658277)",
            "[I 13:15:16,545] [Session] finished transition from 'Lobby' to 'Client_Online' (current='Client_Online')!",
            "[I 13:42:34,663] [online] Removed peer 0(2)",
            "[I 13:42:34,684] [Session] finished transition from 'Client_Online' to 'Lobby' (current='Lobby')!",
        ]);
        assert_eq!(session, SessionPeers::default());
    }

    #[test]
    fn the_host_is_unknown_while_the_session_is_still_joining() {
        let session = session(&["[I 01:34:00,867] [online] Added peer 0(1) (90297598401357828)"]);
        assert_eq!(session.host(), None);
    }

    #[test]
    fn the_log_is_read_again_from_its_start_after_the_game_started_again() {
        let root = std::env::temp_dir().join(format!(
            "shroudforge-session-peers-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let log = root.join("enshrouded.log");
        std::fs::write(
            &log,
            "[I 00:00:01,000] [online] Added peer 0(1) (76561199229658277)\n\
             [I 00:00:02,000] [Session] finished transition from 'Lobby' to 'Client_Online' (current='Client_Online')!\n",
        )
        .unwrap();
        assert_eq!(super::read_session_peers(&root).host(), Some(76561199229658277));
        std::fs::write(&log, "[I 00:00:01,000] new log\n").unwrap();
        assert_eq!(super::read_session_peers(&root), SessionPeers::default());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
