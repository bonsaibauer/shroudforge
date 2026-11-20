//! The machines of the game session this client process is in, read from the
//! game's own `enshrouded.log`. Enshrouded writes one
//! `[online] Added peer <machine>(<slot>) (<SteamID64>)` line for every other
//! machine of the session and `[online] Removed peer <machine>(<slot>)` when it
//! leaves. `[Session] finished transition from '<old>' to '<new>'` says whether
//! this game hosts (`Host_Online`), joined another game (`Client_Online`) or is
//! back in the lobby. The host of a session is machine 0, a player's game or a
//! Dedicated Server. Only what the game appended since the previous call is
//! read, at most `MAX_READ_BYTES` per call.
//!
//! The game that hosts, a player's game or a Dedicated Server (whose log is
//! `logs/enshrouded_server.log`), also writes each player's permissions when
//! the player logs in: `[server] Player '<name>' logged in with Permissions:`
//! followed by one `- <Permission>` line each. The login follows the player's
//! `Added peer` line, so the permissions go to the oldest added peer whose
//! permissions are not known yet. The hosting game's own login comes before
//! any peer and is left aside.

use std::collections::{BTreeMap, VecDeque};
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
    /// The game name and permissions learned for each connected Steam peer.
    pub player_records: BTreeMap<u64, SessionPlayer>,
    awaiting_login: VecDeque<u64>,
    reading_permissions: Option<(u64, String, Vec<String>)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionPlayer {
    pub name: Option<String>,
    pub permissions: Vec<String>,
}

impl SessionPeers {
    /// The permissions a player of the game this process hosts logged in with.
    pub fn player_permissions(&self, steam_id: u64) -> Option<&[String]> {
        if self.role != SessionRole::Hosting || !self.peers.values().any(|id| *id == steam_id) {
            return None;
        }
        self.player_records
            .get(&steam_id)
            .map(|player| player.permissions.as_slice())
    }

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
        if let Some((steam_id, player_name, mut names)) = self.reading_permissions.take() {
            if let Some(name) = permission_name(line) {
                names.push(name.to_owned());
                self.reading_permissions = Some((steam_id, player_name, names));
                return;
            }
            if steam_id != 0 {
                self.player_records.insert(
                    steam_id,
                    SessionPlayer {
                        name: Some(player_name),
                        permissions: names,
                    },
                );
            }
        }
        if let Some(rest) = after(line, "[online] Added peer ") {
            if let (Some(machine), Some(id)) = (machine_index(rest), peer_steam_id(rest)) {
                self.peers.insert(machine, id);
                self.awaiting_login.push_back(id);
            }
        } else if let Some(rest) = after(line, "[online] Removed peer ") {
            if let Some(id) = machine_index(rest).and_then(|machine| self.peers.remove(&machine)) {
                self.player_records.remove(&id);
                self.awaiting_login.retain(|waiting| *waiting != id);
            }
        } else if line.contains("[server] Player '") && line.contains("' logged in with Permissions:") {
            // 0: a login without a peer waiting, the hosting game's own.
            let steam_id = self.awaiting_login.pop_front().unwrap_or(0);
            let player_name = after(line, "[server] Player '")
                .and_then(|rest| {
                    rest.split_once("' logged in with Permissions:")
                        .map(|(name, _)| name)
                })
                .unwrap_or_default()
                .to_owned();
            self.reading_permissions = Some((steam_id, player_name, Vec::new()));
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

/// `CanEditBase` from a `[I 00:01:01,586] \t - CanEditBase` line.
fn permission_name(line: &str) -> Option<&str> {
    let (_, rest) = line.split_once("] ")?;
    let name = rest.trim_start().strip_prefix("- ")?.trim();
    (!name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric())).then_some(name)
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

/// The session of this process: a client's `enshrouded.log`, or the
/// Dedicated Server's `logs/enshrouded_server.log` when `server` is true.
pub fn read_session_peers(root: &Path, server: bool) -> SessionPeers {
    let path = if server {
        root.join("logs").join("enshrouded_server.log")
    } else {
        root.join("enshrouded.log")
    };
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
        assert_eq!(super::read_session_peers(&root, false).host(), Some(76561199229658277));
        std::fs::write(&log, "[I 00:00:01,000] new log\n").unwrap();
        assert_eq!(super::read_session_peers(&root, false), SessionPeers::default());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_dedicated_server_knows_each_player_permissions() {
        let session = session(&[
            "[I 00:00:01,410] [Session] finished transition from 'Lobby' to 'Host_Online' (current='Host_Online')!",
            "[I 00:00:53,596] [online] Added peer 0(1) (steamid:76561197992902915)",
            "[I 00:00:55,221] [online] Client '76561197992902915' authenticated by steam",
            "[I 00:00:55,580] [session] Remote player added. Player handle: 0(0)",
            "[I 00:01:01,584] [server] Machine '1': Player '0(0)' logged in",
            "[I 00:01:01,586] [server] Player 'Duke' logged in with Permissions:",
            "[I 00:01:01,586] \t - CanKickBan",
            "[I 00:01:01,586] \t - CanEditBase",
            "[I 00:01:01,586] \t - CanEditWorld",
            "[I 00:01:01,586] ",
            "[I 00:12:23,424] [online] Added peer 1(1) (steamid:76561199229658277)",
            "[I 00:12:28,588] [server] Player 'Guest' logged in with Permissions:",
            "[I 00:12:28,588] \t - CanReceiveEXP",
            "[I 00:12:28,619] [ecss] Replication Checkpoint 1 Set for Player 1 in Tick 44,869",
        ]);
        assert_eq!(
            session.player_permissions(76561197992902915),
            Some(&["CanKickBan".to_owned(), "CanEditBase".to_owned(), "CanEditWorld".to_owned()][..])
        );
        assert_eq!(
            session.player_permissions(76561199229658277),
            Some(&["CanReceiveEXP".to_owned()][..])
        );
        assert_eq!(session.player_permissions(76561198000000099), None);
    }

    #[test]
    fn a_hosting_player_leaves_its_own_login_aside_and_forgets_who_left() {
        let mut session = session(&[
            "[I 00:08:15,000] [Session] finished transition from 'Lobby' to 'Host_Online' (current='Host_Online')!",
            "[I 00:08:15,771] [server] Player 'laufeyson.' logged in with Permissions:",
            "[I 00:08:15,771] \t - CanKickBan",
            "[I 00:08:15,771] ",
            "[I 00:08:34,858] [online] Added peer 0(2) (steamid:76561197992902915)",
            "[I 00:08:42,296] [server] Player 'Duke' logged in with Permissions:",
            "[I 00:08:42,296] \t - CanEditWorld",
            "[I 00:08:42,296] ",
        ]);
        assert_eq!(
            session.player_permissions(76561197992902915),
            Some(&["CanEditWorld".to_owned()][..])
        );
        session.apply("[I 00:13:10,208] [online] Removed peer 0(2)");
        assert_eq!(session.player_permissions(76561197992902915), None);
        assert_eq!(session.players(), Some(Vec::new()));
    }

    #[test]
    fn a_player_who_has_not_logged_in_yet_has_no_permissions() {
        let session = session(&[
            "[I 00:00:01,410] [Session] finished transition from 'Lobby' to 'Host_Online' (current='Host_Online')!",
            "[I 00:00:53,596] [online] Added peer 0(1) (steamid:76561197992902915)",
        ]);
        assert_eq!(session.players(), Some(vec![76561197992902915]));
        assert_eq!(session.player_permissions(76561197992902915), None);
    }
}
