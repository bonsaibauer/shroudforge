//! Single process-wide ShroudForge logging pipeline.

use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tracing::{Event, Subscriber, field::Visit};
use tracing_subscriber::{
    filter::LevelFilter,
    fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
    registry::LookupSpan,
    layer::SubscriberExt,
    Layer,
    util::SubscriberInitExt,
};

#[derive(Clone)]
struct LogWriter(Arc<PathBuf>);

struct Output { writer: LogWriter, buffer: Vec<u8> }

impl Output {
    fn commit(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() { return Ok(()); }
        let bytes = std::mem::take(&mut self.buffer);
        with_log_lock(&self.writer.0, || {
            let mut file = OpenOptions::new().create(true).append(true).open(&*self.writer.0)?;
            file.write_all(&bytes)?;
            file.flush()
        })
    }
}

impl Drop for Output {
    fn drop(&mut self) { let _ = self.commit(); }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogWriter {
    type Writer = Output;

    fn make_writer(&'a self) -> Self::Writer {
        Output { writer: self.clone(), buffer: Vec::new() }
    }
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        io::stdout().write_all(bytes)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.commit()?;
        io::stdout().flush()
    }
}

struct LineFormat;

static STARTED: OnceLock<Instant> = OnceLock::new();

/// Returns elapsed process time using the canonical ShroudForge log layout.
pub fn timestamp() -> String {
    let elapsed = STARTED.get_or_init(Instant::now).elapsed();
    let total_seconds = elapsed.as_secs();
    format!(
        "{:02}:{:02}:{:02},{:03}",
        total_seconds / 3600,
        total_seconds / 60 % 60,
        total_seconds % 60,
        elapsed.subsec_millis()
    )
}

fn single_line(value: &str) -> String {
    value.replace('\r', "\\r").replace('\n', "\\n")
}

/// Formats a complete log line using the canonical ShroudForge layout.
pub fn format_line(level: char, source: &str, message: &str) -> String {
    let source = single_line(source).replace(']', "_");
    format!(
        "[{level} {}] [{source}] {}",
        timestamp(),
        single_line(message)
    )
}

#[derive(Default)]
struct Fields {
    message: String,
    mod_id: Option<String>,
    extra: Vec<String>,
}

impl Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        let value = format!("{value:?}");
        match field.name() {
            "message" => self.message = value,
            "mod_id" => self.mod_id = Some(value.trim_matches('"').to_owned()),
            _ => self.extra.push(format!("{}={value}", field.name())),
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "message" => self.message = value.to_owned(),
            "mod_id" => self.mod_id = Some(value.to_owned()),
            _ => self.extra.push(format!("{}={value}", field.name())),
        }
    }
}

impl<S, N> FormatEvent<S, N> for LineFormat
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        _context: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let level = match *event.metadata().level() {
            tracing::Level::TRACE => 'T',
            tracing::Level::DEBUG => 'D',
            tracing::Level::INFO => 'I',
            tracing::Level::WARN => 'W',
            tracing::Level::ERROR => 'E',
        };
        let mut fields = Fields::default();
        event.record(&mut fields);
        let target = event.metadata().target();
        let source = fields.mod_id.as_deref().unwrap_or_else(|| match target {
            target if target.starts_with("shroudforge_api") => "api",
            target if target.starts_with("shroudforge_package") => "package",
            target if target.starts_with("shroudforge_modloader") => "runtime",
            _ => target.strip_prefix("shroudforge::").unwrap_or(target),
        });
        write!(writer, "{}", format_line(level, source, &fields.message))?;
        if !fields.extra.is_empty() {
            write!(writer, " {}", single_line(&fields.extra.join(" ")))?;
        }
        writeln!(writer)
    }
}

pub fn initialize(root: impl AsRef<Path>, archive_existing: bool) -> io::Result<()> {
    STARTED.get_or_init(Instant::now);
    let root = root.as_ref();
    fs::create_dir_all(root)?;
    let current = root.join("shroudforge.log");
    with_log_lock(root, || {
        if archive_existing && current.is_file() && current.metadata()?.len() > 0 {
            let archive = root.join("logs");
            fs::create_dir_all(&archive)?;
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            let mut destination = archive.join(format!("shroudforge-{timestamp}.log"));
            for suffix in 2.. {
                if !destination.exists() {
                    break;
                }
                destination = archive.join(format!("shroudforge-{timestamp}-{suffix}.log"));
            }
            fs::rename(&current, destination)?;
        }
        let _ = OpenOptions::new().create(true).append(true).open(&current)?;
        Ok(())
    })?;
    let config_root = root.to_path_buf();
    let cached = Mutex::new((Instant::now(), configured_level(root)));
    let filter = tracing_subscriber::filter::filter_fn(move |metadata| {
        let Ok(mut cached) = cached.lock() else { return false };
        if cached.0.elapsed() >= Duration::from_millis(500) {
            *cached = (Instant::now(), configured_level(&config_root));
        }
        *metadata.level() <= cached.1
    });
    let layer = tracing_subscriber::fmt::layer()
        .event_format(LineFormat)
        .with_writer(LogWriter(Arc::new(current)))
        .with_filter(filter);
    let subscriber = tracing_subscriber::registry().with(layer);
    let _ = subscriber.try_init();
    Ok(())
}

/// Append one canonical ShroudForge log entry, honoring the shared loader settings.
pub fn append(root: impl AsRef<Path>, level: char, source: &str, message: &str) -> io::Result<bool> {
    let root = root.as_ref();
    if !allows(root, level) { return Ok(false); }
    append_line(root, level, source, message)?;
    Ok(true)
}

/// Append a durable user-facing activity event even when ordinary log output is disabled.
/// These events back the Activity view and are intentionally independent of the debug log filter.
pub fn append_event(root: impl AsRef<Path>, level: char, source: &str, message: &str) -> io::Result<()> {
    append_line(root.as_ref(), level, source, message)
}

fn append_line(root: &Path, level: char, source: &str, message: &str) -> io::Result<()> {
    let line = format!("{}\n", format_line(level, source, message));
    with_log_lock(root, || {
        let mut file = OpenOptions::new().create(true).append(true).open(root.join("shroudforge.log"))?;
        file.write_all(line.as_bytes())?;
        file.flush()
    })
}

#[cfg(windows)]
fn with_log_lock<T>(_root: &Path, operation: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::{Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_ABANDONED}, System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject}};
    let name = "Local\\ShroudForgeLog";
    let wide: Vec<u16> = OsStr::new(&name).encode_wide().chain(Some(0)).collect();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
    if handle.is_null() { return Err(io::Error::last_os_error()); }
    let wait = unsafe { WaitForSingleObject(handle, u32::MAX) };
    if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
        unsafe { CloseHandle(handle); }
        return Err(io::Error::last_os_error());
    }
    let result = operation();
    unsafe { ReleaseMutex(handle); CloseHandle(handle); }
    result
}

#[cfg(not(windows))]
fn with_log_lock<T>(_root: &Path, operation: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = LOCK.get_or_init(|| Mutex::new(())).lock().map_err(|_| io::Error::other("logging lock poisoned"))?;
    operation()
}

fn configured_level(root: &Path) -> LevelFilter {
    let Ok(value) = crate::config::read_loader(root) else {
        return LevelFilter::INFO;
    };
    let enabled = value
        .pointer("/logging/enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    if !enabled {
        return LevelFilter::OFF;
    }
    match value
        .pointer("/logging/minimumLevel")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("INFO")
        .to_ascii_uppercase()
        .as_str()
    {
        "ALL" | "TRACE" => LevelFilter::TRACE,
        "DEBUG" => LevelFilter::DEBUG,
        "WARNING" | "WARN" => LevelFilter::WARN,
        "ERROR" => LevelFilter::ERROR,
        _ => LevelFilter::INFO,
    }
}

pub fn allows(root: &Path, level: char) -> bool {
    let level = match level { 'T' => LevelFilter::TRACE, 'D' => LevelFilter::DEBUG, 'W' => LevelFilter::WARN, 'E' => LevelFilter::ERROR, _ => LevelFilter::INFO };
    level <= configured_level(root)
}
