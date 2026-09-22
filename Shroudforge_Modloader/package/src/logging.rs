//! Single process-wide ShroudForge logging pipeline.

use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tracing::{Event, Subscriber, field::Visit};
use tracing_subscriber::{
    filter::LevelFilter,
    fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
    registry::LookupSpan,
    util::SubscriberInitExt,
};

#[derive(Clone)]
struct LogWriter(Arc<Mutex<File>>);

struct Output(LogWriter);

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogWriter {
    type Writer = Output;

    fn make_writer(&'a self) -> Self::Writer {
        Output(self.clone())
    }
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Ok(mut file) = self.0.0.lock() {
            file.write_all(bytes)?;
            file.flush()?;
        }
        io::stdout().write_all(bytes)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
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
    let file = OpenOptions::new().create(true).append(true).open(current)?;
    let level = configured_level(root);
    let subscriber = tracing_subscriber::fmt()
        .event_format(LineFormat)
        .with_writer(LogWriter(Arc::new(Mutex::new(file))))
        .with_max_level(level)
        .finish();
    let _ = subscriber.try_init();
    Ok(())
}

fn configured_level(root: &Path) -> LevelFilter {
    let bytes = fs::read(root.join("config").join("shroudforge.json"))
        .or_else(|_| fs::read(root.join("shroudforge.json")));
    let Ok(bytes) = bytes else {
        return LevelFilter::INFO;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return LevelFilter::INFO;
    };
    let enabled = value
        .pointer("/logging/enabled")
        .or_else(|| value.get("enableLogging"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    if !enabled {
        return LevelFilter::OFF;
    }
    match value
        .pointer("/logging/level")
        .or_else(|| value.get("logLevel"))
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
