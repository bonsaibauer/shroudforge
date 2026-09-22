#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("ShroudForge Debug Console is available on Windows only");
}

#[cfg(windows)]
mod windows {
    use std::{
        fs::File,
        io::{Read, Seek, SeekFrom},
        path::{Path, PathBuf},
        sync::mpsc,
        time::{Duration, Instant},
    };

    use tao::{
        dpi::LogicalSize,
        event::{Event, StartCause, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
        System::Threading::{GetCurrentProcessId, OpenEventW, WaitForSingleObject},
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_F10},
            WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
        },
    };
    use wry::WebViewBuilder;

    const MAX_TAIL: u64 = 1024 * 1024;
    const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;

    enum Command {
        Hide,
        Drag,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Config {
        #[serde(default = "default_toggle_key")]
        toggle_key: u32,
        #[serde(default = "default_refresh")]
        refresh_milliseconds: u64,
        #[serde(default = "default_tail")]
        tail_bytes: u64,
    }

    struct Arguments {
        root: PathBuf,
        game_pid: u32,
        stop_name: Option<String>,
        standalone: bool,
    }

    #[derive(serde::Serialize)]
    struct LogTail {
        text: String,
        state: &'static str,
    }

    fn default_toggle_key() -> u32 {
        VK_F10 as u32
    }
    fn default_refresh() -> u64 {
        500
    }
    fn default_tail() -> u64 {
        MAX_TAIL
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let arguments = arguments()?;
        let config = load_config();
        let stop_event = arguments
            .stop_name
            .as_deref()
            .map(open_event)
            .unwrap_or(std::ptr::null_mut());
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("ShroudForge | Debug Console")
            .with_decorations(false)
            .with_always_on_top(true)
            .with_visible(arguments.standalone)
            .with_inner_size(LogicalSize::new(960.0, 580.0))
            .with_min_inner_size(LogicalSize::new(760.0, 420.0))
            .build(&event_loop)?;

        let (sender, receiver) = mpsc::channel();
        let handler = move |message: wry::http::Request<String>| {
            let command = match message.body().as_str() {
                "hide" => Command::Hide,
                "drag" => Command::Drag,
                _ => return,
            };
            let _ = sender.send(command);
        };
        let styles = format!(
            "{}\n{}",
            include_str!("../../ui-shared/tokens.css"),
            include_str!("../ui/styles.css")
        );
        let html = include_str!("../ui/index.html")
            .replace("__SHROUDFORGE_STYLES__", &styles)
            .replace("__SHROUDFORGE_SCRIPT__", include_str!("../ui/app.ts"));
        let webview = WebViewBuilder::new()
            .with_html(html)
            .with_ipc_handler(handler)
            .build(&window)?;

        let mut shown = arguments.standalone;
        let mut key_down = false;
        let mut next_refresh = Instant::now();
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50));
            match event {
                Event::NewEvents(StartCause::ResumeTimeReached { .. })
                | Event::NewEvents(StartCause::Init) => {
                    if !stop_event.is_null()
                        && unsafe { WaitForSingleObject(stop_event, 0) } == WAIT_OBJECT_0
                    {
                        unsafe { CloseHandle(stop_event) };
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    while let Ok(command) = receiver.try_recv() {
                        match command {
                            Command::Hide => {
                                shown = false;
                                window.set_visible(false);
                            }
                            Command::Drag => {
                                let _ = window.drag_window();
                            }
                        }
                    }
                    let foreground = foreground_process();
                    let game_focused = arguments.standalone || foreground == arguments.game_pid;
                    let console_focused = foreground == unsafe { GetCurrentProcessId() };
                    let down = unsafe { GetAsyncKeyState(config.toggle_key as i32) } < 0;
                    if game_focused && down && !key_down {
                        shown = !shown;
                        window.set_visible(shown);
                        if shown {
                            window.set_focus();
                        }
                    }
                    key_down = down;
                    if !arguments.standalone && shown && !game_focused && !console_focused {
                        window.set_visible(false);
                    } else if shown && (game_focused || console_focused) && !window.is_visible() {
                        window.set_visible(true);
                    }
                    if Instant::now() >= next_refresh {
                        next_refresh =
                            Instant::now() + Duration::from_millis(config.refresh_milliseconds);
                        let payload = serde_json::json!({
                            "game": read_tail(&arguments.root.join("enshrouded.log"), config.tail_bytes),
                            "loader": read_tail(&arguments.root.join("shroudforge.log"), config.tail_bytes),
                            "paths": {
                                "game": arguments.root.join("enshrouded.log").display().to_string(),
                                "loader": arguments.root.join("shroudforge.log").display().to_string(),
                            },
                            "connected": true,
                        });
                        let _ = webview
                            .evaluate_script(&format!("window.__shroudforgeUpdate({});", payload));
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    shown = false;
                    window.set_visible(false);
                }
                _ => {}
            }
        });
    }

    fn arguments() -> Result<Arguments, Box<dyn std::error::Error>> {
        let values: Vec<String> = std::env::args().collect();
        let value = |name: &str| {
            values
                .windows(2)
                .find(|pair| pair[0] == name)
                .map(|pair| pair[1].clone())
        };
        let root = PathBuf::from(value("--root").ok_or("missing --root")?);
        let standalone = values.iter().any(|value| value == "--standalone");
        let game_pid = if standalone {
            unsafe { GetCurrentProcessId() }
        } else {
            value("--game-pid").ok_or("missing --game-pid")?.parse()?
        };
        let stop_name = if standalone {
            None
        } else {
            Some(value("--stop-event").ok_or("missing --stop-event")?)
        };
        Ok(Arguments {
            root,
            game_pid,
            stop_name,
            standalone,
        })
    }

    fn open_event(name: &str) -> HANDLE {
        let mut wide: Vec<u16> = name.encode_utf16().collect();
        wide.push(0);
        unsafe { OpenEventW(SYNCHRONIZE_ACCESS, 0, wide.as_ptr()) }
    }

    fn load_config() -> Config {
        let path = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|parent| parent.join("module.json")));
        let mut config = path
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Config {
                toggle_key: default_toggle_key(),
                refresh_milliseconds: default_refresh(),
                tail_bytes: default_tail(),
            });
        if !(1..=255).contains(&config.toggle_key) {
            config.toggle_key = default_toggle_key();
        }
        config.refresh_milliseconds = config.refresh_milliseconds.clamp(100, 10_000);
        config.tail_bytes = config.tail_bytes.clamp(1024, MAX_TAIL);
        config
    }

    fn foreground_process() -> u32 {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid) };
        pid
    }

    fn read_tail(path: &Path, maximum: u64) -> LogTail {
        let Ok(mut file) = File::open(path) else {
            return LogTail {
                text: String::new(),
                state: if path.exists() { "error" } else { "missing" },
            };
        };
        let Ok(end) = file.seek(SeekFrom::End(0)) else {
            return LogTail {
                text: String::new(),
                state: "error",
            };
        };
        let count = end.min(maximum);
        let start = end - count;
        let mut partial = false;
        if start > 0 {
            let mut previous = [0_u8; 1];
            if file.seek(SeekFrom::Start(start - 1)).is_err()
                || file.read_exact(&mut previous).is_err()
            {
                return LogTail {
                    text: String::new(),
                    state: "error",
                };
            }
            partial = previous[0] != b'\n';
        }
        if file.seek(SeekFrom::Start(start)).is_err() {
            return LogTail {
                text: String::new(),
                state: "error",
            };
        }
        let mut bytes = Vec::with_capacity(count as usize);
        if file.take(count).read_to_end(&mut bytes).is_err() {
            return LogTail {
                text: String::new(),
                state: "error",
            };
        }
        if partial {
            if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
                bytes.drain(..=newline);
            } else {
                bytes.clear();
            }
        }
        LogTail {
            text: String::from_utf8_lossy(&bytes).into_owned(),
            state: "ready",
        }
    }
}

#[cfg(windows)]
fn main() {
    if let Err(error) = windows::run() {
        eprintln!("ShroudForge Debug Console failed: {error}");
        std::process::exit(1);
    }
}
