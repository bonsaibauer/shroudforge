use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use kfc::guid::ContentHash;
use mlua::{Table, UserData, UserDataMethods, Value};

use crate::env::AppState;
use crate::lua::{CastLuaExt, LuaValue};

/// A part of a game content being read on its own thread: `result()` gives
/// nil until it is done, then {data = bytes, seconds = read time} or
/// {error = text}.
struct ContentReadJob {
    done: Arc<Mutex<Option<Result<(Vec<u8>, f64), String>>>>,
}

impl UserData for ContentReadJob {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("result", |lua, job, ()| {
            let done = job.done.lock().map_err(|_| mlua::Error::runtime("content read: the reader failed"))?;
            let Some(outcome) = done.as_ref() else { return Ok(Value::Nil) };
            let result = lua.create_table()?;
            match outcome {
                Ok((data, seconds)) => {
                    result.raw_set("data", lua.create_string(data)?)?;
                    result.raw_set("seconds", *seconds)?;
                }
                Err(error) => result.raw_set("error", error.as_str())?,
            }
            Ok(Value::Table(result))
        });
    }
}

/// `count` bytes of the file at `path`, from `offset`.
fn read_range(path: &Path, offset: u64, count: u64) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut data = vec![0u8; count as usize];
    file.read_exact(&mut data)?;
    Ok(data)
}

/// Where the bytes `start`..`start + count` of a content are: its container
/// file and the offset there (a content is stored as is, not compressed).
fn locate(app_state: &AppState, hash: &ContentHash, start: Option<u64>, count: Option<u64>) -> mlua::Result<(PathBuf, u64, u64)> {
    let entry = app_state
        .kfc_file()
        .contents()
        .get(hash)
        .ok_or_else(|| mlua::Error::runtime(format!("content read: no content {hash}")))?;
    let (offset, index) = (entry.offset, entry.container_index);
    let size = u64::from(hash.size());
    let start = start.unwrap_or(0).min(size);
    let count = count.unwrap_or(size - start).min(size - start);
    Ok((app_state.reader().dat_path(index), offset + start, count))
}

/// `shroudforge.contents`: `read(params)` reads bytes of a game content (the
/// levels of a texture, for instance) on a thread of its own and returns a
/// job to ask at the next updates, so a mod never waits for the game files
/// (31/10: `game.assets.get_content(...):read_data()` took 30 ms for a 2048 x
/// 2048 texture, on the mods loop). params: {hash = a content hash (as a
/// resource holds it, or {size, hash0, hash1, hash2}), start = first byte
/// (0), count = bytes (to the end)}. Contents of the game files only (not
/// those a mod created).
pub(crate) fn create(lua: &mlua::Lua) -> mlua::Result<Table> {
    let contents = lua.create_table()?;
    contents.raw_set(
        "read",
        lua.create_function(|lua, params: Table| {
            let value: LuaValue = params.raw_get("hash")?;
            let hash = value
                .try_cast::<ContentHash>()?
                .ok_or_else(|| mlua::Error::runtime("content read: hash: expected a content hash"))?;
            let (path, offset, count) = {
                let app_state = lua
                    .app_data_ref::<AppState>()
                    .ok_or_else(|| mlua::Error::runtime("content read: the game data is not loaded"))?;
                locate(&app_state, &hash, params.raw_get("start")?, params.raw_get("count")?)?
            };
            let done = Arc::new(Mutex::new(None));
            let result = Arc::clone(&done);
            std::thread::Builder::new()
                .name("shroudforge-content-read".into())
                .spawn(move || {
                    let started = Instant::now();
                    let outcome = read_range(&path, offset, count)
                        .map(|data| (data, started.elapsed().as_secs_f64()))
                        .map_err(|error| format!("content read: {}: {error}", path.display()));
                    if let Ok(mut slot) = result.lock() {
                        *slot = Some(outcome);
                    }
                })
                .map_err(|error| mlua::Error::runtime(format!("content read: no thread: {error}")))?;
            Ok(ContentReadJob { done })
        })?,
    )?;
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_of_a_file_is_read() {
        let path = std::env::temp_dir().join(format!("shroudforge-content-read-{}.dat", std::process::id()));
        std::fs::write(&path, (0u8..=255).collect::<Vec<_>>()).unwrap();
        assert_eq!(read_range(&path, 10, 4).unwrap(), vec![10, 11, 12, 13]);
        assert!(read_range(&path, 250, 10).is_err(), "past the end");
        std::fs::remove_file(&path).ok();
    }
}
