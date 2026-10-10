use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const KSC_HEADER_SIZE: usize = 24;
const KSC_ENTRY_SIZE: usize = 12;
const MAX_KSC_ENTRIES: usize = 4096;
const MAX_CHARACTER_BLOB_SIZE: usize = 16 * 1024 * 1024;
const DETECTION_CACHE_DURATION: Duration = Duration::from_secs(5);

static DETECTION_CACHE: OnceLock<Mutex<(Option<Instant>, Option<String>)>> = OnceLock::new();

#[derive(Debug)]
struct CharacterName {
    name: String,
    last_play_time: Option<u32>,
}

pub(super) fn detect_active_character_name() -> Option<String> {
    let cache = DETECTION_CACHE.get_or_init(|| Mutex::new((None, None)));
    let Ok(mut cache) = cache.lock() else {
        return None;
    };
    if cache
        .0
        .is_some_and(|sampled_at| sampled_at.elapsed() < DETECTION_CACHE_DURATION)
    {
        return cache.1.clone();
    }

    let detected = detect_active_character_name_from_saves();
    *cache = (Some(Instant::now()), detected.clone());
    detected
}

fn detect_active_character_name_from_saves() -> Option<String> {
    let mut candidates = Vec::new();
    for directory in character_save_directories() {
        if let Some(character) = read_latest_character(&directory) {
            candidates.push(character);
        }
    }

    let newest_time = candidates
        .iter()
        .filter_map(|candidate| candidate.last_play_time)
        .max();
    let Some(newest_time) = newest_time else {
        return if candidates.len() == 1 {
            candidates.pop().map(|candidate| candidate.name)
        } else {
            None
        };
    };

    let newest: Vec<_> = candidates
        .into_iter()
        .filter(|candidate| candidate.last_play_time == Some(newest_time))
        .collect();
    let name = newest.first()?.name.clone();
    newest
        .iter()
        .all(|candidate| candidate.name == name)
        .then_some(name)
}

fn character_save_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        directories.push(PathBuf::from(profile).join("Saved Games/Enshrouded"));
    }

    let mut steam_roots = Vec::new();
    for variable in ["PROGRAMFILES(X86)", "PROGRAMFILES"] {
        if let Some(value) = std::env::var_os(variable) {
            steam_roots.push(PathBuf::from(value).join("Steam"));
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        steam_roots.push(PathBuf::from(local).join("Programs/Steam"));
    }
    if let Some(steam) = std::env::var_os("STEAM_COMPAT_CLIENT_INSTALL_PATH") {
        steam_roots.push(PathBuf::from(steam));
    }

    for steam in steam_roots {
        let Ok(accounts) = fs::read_dir(steam.join("userdata")) else {
            continue;
        };
        for account in accounts.flatten() {
            directories.push(account.path().join("1203620/remote"));
        }
    }
    directories.sort();
    directories.dedup();
    directories
}

fn read_latest_character(directory: &Path) -> Option<CharacterName> {
    let index_path = directory.join("characters-index");
    let index_before = fs::read(&index_path).ok()?;
    let index: serde_json::Value = serde_json::from_slice(&index_before).ok()?;
    let latest = index.get("latest")?.as_u64()?;
    if latest > 9 || index.get("deleted").and_then(serde_json::Value::as_bool) == Some(true) {
        return None;
    }

    let save_path = if latest == 0 {
        directory.join("characters")
    } else {
        directory.join(format!("characters-{latest}"))
    };
    let save_before = fs::metadata(&save_path).ok()?;
    let bytes = fs::read(&save_path).ok()?;
    let save_after = fs::metadata(&save_path).ok()?;
    let index_after = fs::read(&index_path).ok()?;
    if index_before != index_after
        || save_before.len() != save_after.len()
        || save_after.len() != bytes.len() as u64
        || save_before.modified().ok() != save_after.modified().ok()
    {
        return None;
    }

    let characters = parse_ksc1_characters(&bytes)?;
    select_most_recent_character(characters)
}

fn parse_ksc1_characters(bytes: &[u8]) -> Option<Vec<CharacterName>> {
    if bytes.len() < KSC_HEADER_SIZE || &bytes[..4] != b"KSC1" {
        return None;
    }
    let entry_count = read_u32(bytes, 4)? as usize;
    if entry_count == 0 || entry_count > MAX_KSC_ENTRIES {
        return None;
    }
    let table_end = KSC_HEADER_SIZE.checked_add(entry_count.checked_mul(KSC_ENTRY_SIZE)?)?;
    if table_end > bytes.len() {
        return None;
    }

    let mut data_offset = table_end;
    let mut characters = Vec::new();
    for index in 0..entry_count {
        let entry_offset = KSC_HEADER_SIZE + index * KSC_ENTRY_SIZE;
        let owner_id = read_u32(bytes, entry_offset)?;
        let blob_type = bytes.get(entry_offset + 4..entry_offset + 8)?;
        let compressed_size = read_u32(bytes, entry_offset + 8)? as usize;
        let data_end = data_offset.checked_add(compressed_size)?;
        let compressed = bytes.get(data_offset..data_end)?;
        data_offset = data_end;

        if blob_type != b"CHAR" || owner_id == 0 || compressed.is_empty() {
            continue;
        }
        let raw = zstd::bulk::decompress(compressed, MAX_CHARACTER_BLOB_SIZE).ok()?;
        if raw.get(..4)? != b"BDB1" {
            continue;
        }
        if let Some(name) = extract_character_name(&raw) {
            characters.push(CharacterName {
                name,
                last_play_time: extract_last_play_time(&raw),
            });
        }
    }
    (data_offset == bytes.len()).then_some(characters)
}

fn extract_character_name(data: &[u8]) -> Option<String> {
    for offset in 2..data.len().saturating_sub(8) {
        if read_u16(data, offset - 2) != Some(4) || data.get(offset..offset + 4)? != b"name" {
            continue;
        }
        let length = read_u16(data, offset + 4)? as usize;
        if length == 0 || length > 128 {
            continue;
        }
        let end = offset.checked_add(6)?.checked_add(length)?;
        let Ok(name) = std::str::from_utf8(data.get(offset + 6..end)?) else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || name.chars().any(char::is_control) {
            continue;
        }
        return Some(name.to_owned());
    }
    None
}

fn extract_last_play_time(data: &[u8]) -> Option<u32> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let maximum = u32::try_from(now.saturating_add(24 * 60 * 60)).ok()?;
    for offset in (0..data.len().saturating_sub(31)).step_by(4) {
        if read_u32(data, offset) != Some(12)
            || read_u32(data, offset + 4) != Some(1)
            || read_u32(data, offset + 8) != Some(1)
            || read_u32(data, offset + 12) != Some(13)
            || read_u32(data, offset + 20) != Some(0)
            || read_u32(data, offset + 24) != Some(1)
            || read_u32(data, offset + 28) != Some(14)
        {
            continue;
        }
        let timestamp = read_u32(data, offset + 16)?;
        if (1_500_000_000..=maximum).contains(&timestamp) {
            return Some(timestamp);
        }
    }
    None
}

fn select_most_recent_character(mut characters: Vec<CharacterName>) -> Option<CharacterName> {
    if characters.len() == 1 {
        return characters.pop();
    }
    let newest_time = characters
        .iter()
        .filter_map(|character| character.last_play_time)
        .max()?;
    let mut newest = characters
        .into_iter()
        .filter(|character| character.last_play_time == Some(newest_time));
    let selected = newest.next()?;
    newest
        .all(|character| character.name == selected.name)
        .then_some(selected)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?))
}
