use crate::LogLevel;
use serde::Serialize;
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

const MAX_ACTIVITY_ENTRIES: usize = 1000;
const MAX_ACTIVITY_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityLogEntry {
    pub id: String,
    pub time: String,
    pub source: String,
    pub action: String,
    pub result: String,
    pub level: LogLevel,
}

pub fn read_activity(root: &Path) -> Result<Vec<ActivityLogEntry>, String> {
    read_activity_from_path(&shroudforge_loader_config::paths::current_log(root))
}

pub fn read_activity_from_path(path: &Path) -> Result<Vec<ActivityLogEntry>, String> {
    let mut file = File::open(path)
        .map_err(|error| error.to_string())?;
    let length = file.metadata().map_err(|error| error.to_string())?.len();
    let offset = length.saturating_sub(MAX_ACTIVITY_BYTES);
    file.seek(SeekFrom::Start(offset)).map_err(|error| error.to_string())?;
    let mut contents = String::new();
    BufReader::new(file)
        .read_to_string(&mut contents)
        .map_err(|error| error.to_string())?;
    if offset > 0 {
        if let Some(line_end) = contents.find('\n') {
            contents.drain(..=line_end);
        } else {
            contents.clear();
        }
    }
    let lines = contents.lines().rev().take(MAX_ACTIVITY_ENTRIES).collect::<Vec<_>>();
    let mut items = lines
        .into_iter()
        .rev()
        .enumerate()
        .filter_map(|(index, line)| {
            let close = line.find("] [")?;
            let severity = match line.as_bytes().first().copied()? {
                b'[' => line.chars().nth(1)?,
                _ => return None,
            };
            let level = LogLevel::from_char(severity)?;
            let time = line.get(3..close)?.to_owned();
            let source_end = line.get(close + 3..)?.find("] ")? + close + 3;
            let source = line.get(close + 3..source_end)?.to_owned();
            let action = line.get(source_end + 2..)?.to_owned();
            Some(ActivityLogEntry {
                id: format!("{}-{}-{}", index, time, source),
                time,
                source,
                action,
                result: level.label().to_owned(),
                level,
            })
        })
        .collect::<Vec<_>>();
    items.reverse();
    Ok(items)
}
