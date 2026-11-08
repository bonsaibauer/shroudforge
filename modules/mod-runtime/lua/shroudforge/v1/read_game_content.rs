use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Take};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use kfc::guid::ContentHash;
use mlua::{String as LuaString, Table, UserData, UserDataMethods, Value};

use crate::env::AppState;
use crate::lua::{CastLuaExt, LuaValue};
use crate::session::{ContentReadPermit, MAX_CONTENT_READ_BYTES, MAX_CONTENT_READ_JOBS};

type ContentReader = Take<BufReader<File>>;

/// A bounded background read. The permit remains held while the worker runs
/// and until Lua consumes or drops the result.
struct ContentReadJob {
    done: Arc<Mutex<Option<Result<(Vec<u8>, f64), String>>>>,
    cached: Option<Result<(LuaString, f64), String>>,
    permit: Option<ContentReadPermit>,
}

impl UserData for ContentReadJob {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("result", |lua, job, ()| {
            if job.cached.is_none() {
                let outcome = job
                    .done
                    .lock()
                    .map_err(|_| mlua::Error::runtime("content read: the reader failed"))?
                    .take();
                let Some(outcome) = outcome else {
                    return Ok(Value::Nil);
                };

                job.cached = Some(match outcome {
                    Ok((data, seconds)) => match lua.create_string(&data) {
                        Ok(data) => Ok((data, seconds)),
                        Err(error) => {
                            if let Ok(mut done) = job.done.lock() {
                                *done = Some(Ok((data, seconds)));
                            }
                            return Err(error);
                        }
                    },
                    Err(error) => Err(error),
                });
                job.permit.take();
            }

            let result = lua.create_table()?;
            match job.cached.as_ref().unwrap() {
                Ok((data, seconds)) => {
                    result.raw_set("data", data.clone())?;
                    result.raw_set("seconds", *seconds)?;
                }
                Err(error) => result.raw_set("error", error.as_str())?,
            }
            Ok(Value::Table(result))
        });
    }
}

/// Reads one validated byte range from the parser-owned content stream.
fn read_range(mut content: ContentReader, start: u64, count: u64) -> std::io::Result<Vec<u8>> {
    let relative_start = i64::try_from(start).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "content range start is too large",
        )
    })?;
    content.get_mut().seek(SeekFrom::Current(relative_start))?;

    let length = usize::try_from(count).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "content range is too large",
        )
    })?;
    let mut data = Vec::new();
    data.try_reserve_exact(length)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
    data.resize(length, 0);
    content.take(count).read_exact(&mut data)?;
    Ok(data)
}

fn validate_range(size: u64, start: Option<u64>, count: Option<u64>) -> mlua::Result<(u64, u64)> {
    let start = start.unwrap_or(0);
    if start > size {
        return Err(mlua::Error::runtime(
            "content read: start exceeds the content size",
        ));
    }

    let count = count.unwrap_or(size - start);
    if count > size - start {
        return Err(mlua::Error::runtime(
            "content read: range exceeds the content size",
        ));
    }
    if count > MAX_CONTENT_READ_BYTES {
        return Err(mlua::Error::runtime(format!(
            "content read: each request is limited to {} MiB. Split larger reads into ranges",
            MAX_CONTENT_READ_BYTES / (1024 * 1024)
        )));
    }

    Ok((start, count))
}

/// `shroudforge.contents`: reads a validated game-content range in the
/// background. Each request is limited to 32 MiB and at most four jobs may be
/// outstanding per runtime. Contents created by mods are not included.
pub(crate) fn create(lua: &mlua::Lua) -> mlua::Result<Table> {
    let contents = lua.create_table()?;
    contents.raw_set(
        "read",
        lua.create_function(|lua, params: Table| {
            let value: LuaValue = params.raw_get("hash")?;
            let hash = value
                .try_cast::<ContentHash>()?
                .ok_or_else(|| mlua::Error::runtime("content read: hash: expected a content hash"))?;
            let (content, start, count, permit) = {
                let app_state = lua
                    .app_data_ref::<AppState>()
                    .ok_or_else(|| mlua::Error::runtime("content read: the game data is not loaded"))?;
                let size = u64::from(hash.size());
                let (start, count) =
                    validate_range(size, params.raw_get("start")?, params.raw_get("count")?)?;
                let content = app_state
                    .reader()
                    .open_content(&hash)
                    .map_err(|error| mlua::Error::runtime(format!("content read: {error}")))?
                    .ok_or_else(|| mlua::Error::runtime(format!("content read: no content {hash}")))?;
                let permit = app_state.reserve_content_read().ok_or_else(|| {
                    mlua::Error::runtime(format!(
                        "content read: at most {MAX_CONTENT_READ_JOBS} reads may be outstanding. Wait for a read to finish and consume its result before starting another read"
                    ))
                })?;
                (content, start, count, permit)
            };

            let done = Arc::new(Mutex::new(None));
            let result = Arc::clone(&done);
            let worker_permit = permit.clone();
            std::thread::Builder::new()
                .name("shroudforge-content-read".into())
                .spawn(move || {
                    let _permit = worker_permit;
                    let started = Instant::now();
                    let outcome = read_range(content, start, count)
                        .map(|data| (data, started.elapsed().as_secs_f64()))
                        .map_err(|error| format!("content read: {error}"));
                    if let Ok(mut slot) = result.lock() {
                        *slot = Some(outcome);
                    }
                })
                .map_err(|error| mlua::Error::runtime(format!("content read: no thread: {error}")))?;

            Ok(ContentReadJob {
                done,
                cached: None,
                permit: Some(permit),
            })
        })?,
    )?;
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_of_a_file_is_read() {
        let path = std::env::temp_dir().join(format!(
            "shroudforge-content-read-{}.dat",
            std::process::id()
        ));
        std::fs::write(&path, (0u8..=255).collect::<Vec<_>>()).unwrap();
        let content = BufReader::new(File::open(&path).unwrap()).take(256);
        assert_eq!(read_range(content, 10, 4).unwrap(), vec![10, 11, 12, 13]);
        let content = BufReader::new(File::open(&path).unwrap()).take(256);
        assert!(read_range(content, 250, 10).is_err(), "past the end");
        std::fs::remove_file(&path).ok();
    }
}
