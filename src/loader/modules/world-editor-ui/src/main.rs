#[cfg(windows)]
mod windows {
    use std::{
        fs,
        io::{Cursor, Write},
        path::{Path, PathBuf},
        sync::mpsc,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use base64::Engine as _;
    use serde::Serialize;
    use tao::{
        dpi::{LogicalPosition, LogicalSize, PhysicalPosition},
        event::{Event, StartCause, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, RECT, WAIT_OBJECT_0},
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap,
            CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits,
            ReleaseDC, SRCCOPY, SelectObject,
        },
        System::Threading::{GetCurrentProcessId, OpenEventW, OpenProcess, WaitForSingleObject},
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_F1, VK_F2},
            WindowsAndMessaging::{GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId},
        },
    };
    use wry::{WebContext, WebViewBuilder};

    const DEFAULT_WIDTH: u32 = 1180;
    const DEFAULT_HEIGHT: u32 = 190;
    const MIN_WIDTH: u32 = 760;
    const MAX_WIDTH: u32 = 1920;
    const MIN_HEIGHT: u32 = 140;
    const MAX_HEIGHT: u32 = 400;
    const TOP_OFFSET: i32 = 92;
    const MAX_POSITION: i32 = 8192;

    enum Command {
        Ready,
        StateApplied {
            blueprints: usize,
            stage: String,
            selected: String,
        },
        ScriptError(String),
        Hide,
        Drag,
        Action { action: String, value: String },
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct BlueprintCard {
        name: String,
        image: Option<String>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ViewState {
        blueprints: Vec<BlueprintCard>,
        stage: String,
        hint: String,
        selected: String,
        axis: String,
        turns: u8,
        panel_width: u32,
        panel_height: u32,
        panel_position_x: i32,
        panel_position_y: u32,
    }

    struct Arguments {
        root: PathBuf,
        game_pid: u32,
        game_process: HANDLE,
        stop_name: Option<String>,
        desktop: bool,
    }

    pub fn run_module() -> Result<(), Box<dyn std::error::Error>> {
        let arguments = arguments()?;
        let config = shroudforge_package::config::read_loader(&arguments.root)?;
        let preferences = config["modules"]["worldEditor"].clone();
        let toggle_key = preferences["toggleKey"].as_i64().unwrap_or(VK_F2 as i64) as i32;
        let refresh_ms = preferences["refreshMilliseconds"]
            .as_u64()
            .unwrap_or(400)
            .clamp(100, 5000);
        let initial_state = view_state(&arguments.root).unwrap_or_else(|_| empty_state());
        let initial_window_state = shroudforge_package::config::window_state(&arguments.root);
        // Always start hidden for a new game session. A persisted open state must not
        // make the overlay appear before the player asks for it with F1–F8.
        let _ = shroudforge_package::config::request_window_visibility(
            &arguments.root,
            "worldEditor",
            false,
        );
        let mut visibility_request_id = shroudforge_package::config::window_state(&arguments.root)
            ["worldEditor"]["requestId"]
            .as_u64()
            .unwrap_or_else(|| {
                initial_window_state["worldEditor"]["requestId"]
                    .as_u64()
                    .unwrap_or(0)
            });
        let mut window_visible = false;
        let stop_event = arguments
            .stop_name
            .as_deref()
            .map(open_event)
            .unwrap_or(std::ptr::null_mut());
        let event_loop = EventLoop::new();
        let initial_height = initial_state.panel_height as f64;
        let window = WindowBuilder::new()
            .with_title("ShroudForge | World Editor")
            .with_decorations(false)
            .with_always_on_top(true)
            .with_visible(false)
            .with_inner_size(LogicalSize::new(
                initial_state.panel_width as f64,
                initial_height,
            ))
            // Keep the same native style as Modloader UI. Equal min/max bounds
            // preserve a fixed-size overlay without removing WS_SIZEBOX.
            .with_min_inner_size(LogicalSize::new(
                initial_state.panel_width as f64,
                initial_height,
            ))
            .with_max_inner_size(LogicalSize::new(
                initial_state.panel_width as f64,
                initial_height,
            ))
            .build(&event_loop)?;
        position_window(
            &window,
            arguments.game_pid,
            initial_state.panel_width as f64,
            initial_state.panel_position_x,
            initial_state.panel_position_y,
        );
        let _ = append_ui_log(
            &arguments.root,
            "window initialized with opaque ShroudForge panel and undecorated Tao window",
        );

        let (sender, receiver) = mpsc::channel();
        let handler = move |message: wry::http::Request<String>| {
            let command = match serde_json::from_str::<serde_json::Value>(message.body()) {
                Ok(value) => {
                    let kind = value["kind"].as_str().unwrap_or_default();
                    match kind {
                        "ready" => Command::Ready,
                        "state-applied" => Command::StateApplied {
                            blueprints: value["blueprints"].as_u64().unwrap_or(0) as usize,
                            stage: value["stage"].as_str().unwrap_or("unknown").to_owned(),
                            selected: value["selected"].as_str().unwrap_or_default().to_owned(),
                        },
                        "script-error" => Command::ScriptError(
                            value["value"]
                                .as_str()
                                .unwrap_or("unknown UI script error")
                                .to_owned(),
                        ),
                        "hide" => Command::Hide,
                        "drag" => Command::Drag,
                        "action" => {
                            let action = value["action"].as_str().unwrap_or_default().to_owned();
                            let payload = value["value"].as_str().unwrap_or_default().to_owned();
                            Command::Action {
                                action,
                                value: payload,
                            }
                        }
                        _ => return,
                    }
                }
                Err(_) => return,
            };
            let _ = sender.send(command);
        };
        let mut web_context = WebContext::new(Some(
            shroudforge_package::paths::webview_profile(&arguments.root).join("world-editor"),
        ));
        let styles = format!(
            "{}\n{}",
            include_str!("../../ui-shared/tokens.css"),
            include_str!("../ui/styles.css")
        );
        let icon = base64::engine::general_purpose::STANDARD
            .encode(include_bytes!("../../../../../mods/world-editor/icon.svg"));
        let html = include_str!("../ui/index.html")
            .replace("__SHROUDFORGE_STYLES__", &styles)
            .replace(
                "__WORLD_EDITOR_ICON__",
                &format!("data:image/svg+xml;base64,{icon}"),
            );
        let webview = WebViewBuilder::new_with_web_context(&mut web_context)
            .with_html(html)
            .with_ipc_handler(handler)
            .build(&window)?;
        // Keep the same native window and WebView setup as the Modloader UI.
        // The document paints its own opaque surface; per-window background and
        // DWM frame overrides caused this host to diverge from that working path.
        let _ = append_ui_log(&arguments.root, "window initialized with Modloader-style undecorated Tao window; document owns the opaque panel surface");

        let mut panel_width = initial_state.panel_width;
        let mut panel_height = initial_state.panel_height;
        let mut panel_position_x = initial_state.panel_position_x;
        let mut panel_position_y = initial_state.panel_position_y;
        let mut module_enabled = preferences["enabled"] != false;
        let mut native_visible = false;
        let _ = shroudforge_package::config::publish_window_visibility(
            &arguments.root,
            "worldEditor",
            module_enabled && window_visible,
        );
        let mut function_keys_down = [false; 8];
        let mut toggle_down = false;
        let mut next_visibility_poll = Instant::now();
        let mut next_settings_poll = Instant::now();
        let mut next_refresh = Instant::now();
        let mut last_payload = String::new();
        let mut last_ui_ack = String::new();
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(35));
            match event {
                Event::NewEvents(StartCause::ResumeTimeReached { .. }) | Event::NewEvents(StartCause::Init) => {
                    if !stop_event.is_null() && unsafe { WaitForSingleObject(stop_event, 0) } == WAIT_OBJECT_0 {
                        let _ = shroudforge_package::config::publish_window_visibility(&arguments.root, "worldEditor", false);
                        unsafe { CloseHandle(stop_event) };
                        if !arguments.game_process.is_null() { unsafe { CloseHandle(arguments.game_process) }; }
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    if !arguments.game_process.is_null() && unsafe { WaitForSingleObject(arguments.game_process, 0) } == WAIT_OBJECT_0 {
                        let _ = shroudforge_package::config::publish_window_visibility(&arguments.root, "worldEditor", false);
                        if !stop_event.is_null() { unsafe { CloseHandle(stop_event) }; }
                        unsafe { CloseHandle(arguments.game_process) };
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    while let Ok(command) = receiver.try_recv() {
                        match command {
                            Command::Ready => {
                                next_refresh = Instant::now();
                                let _ = append_ui_log(&arguments.root, "UI document ready");
                            }
                            Command::StateApplied { blueprints, stage, selected } => {
                                let acknowledgment = format!("{blueprints}|{stage}|{selected}");
                                if acknowledgment != last_ui_ack {
                                    let _ = append_ui_log(
                                        &arguments.root,
                                        &format!("UI applied state: blueprints={blueprints} stage={stage} selected={selected}"),
                                    );
                                    last_ui_ack = acknowledgment;
                                }
                            }
                            Command::ScriptError(message) => {
                                let _ = append_ui_log(&arguments.root, &message);
                            }
                            Command::Hide => {
                                window_visible = false;
                                window.set_visible(false);
                                native_visible = false;
                                let _ = shroudforge_package::config::request_window_visibility(
                                    &arguments.root,
                                    "worldEditor",
                                    false,
                                );
                            }
                            Command::Drag => {}
                            Command::Action { action, value } => {
                                if let Err(error) = dispatch_action(&arguments.root, &action, &value) {
                                    let _ = webview.evaluate_script(&format!("window.__worldEditorNotice({});", js_string(&format!("Action failed: {error}"))));
                                }
                            }
                        }
                    }

                    if Instant::now() >= next_visibility_poll {
                        let state = shroudforge_package::config::window_state(&arguments.root);
                        let request_id = state["worldEditor"]["requestId"]
                            .as_u64()
                            .unwrap_or(visibility_request_id);
                        if request_id != visibility_request_id {
                            visibility_request_id = request_id;
                            window_visible = state["worldEditor"]["requestedVisible"]
                                .as_bool()
                                .unwrap_or(window_visible);
                            if window_visible {
                                set_fixed_size(&window, panel_width, panel_height as f64);
                                window.set_inner_size(LogicalSize::new(
                                    panel_width as f64,
                                    panel_height as f64,
                                ));
                            }
                            let _ = shroudforge_package::config::publish_window_visibility(
                                &arguments.root,
                                "worldEditor",
                                module_enabled && window_visible,
                            );
                        }
                        next_visibility_poll = Instant::now() + Duration::from_millis(100);
                    }

                    if Instant::now() >= next_settings_poll {
                        if let Ok(config) =
                            shroudforge_package::config::read_loader(&arguments.root)
                        {
                            let next_enabled = config["modules"]["worldEditor"]["enabled"] != false;
                            if next_enabled != module_enabled {
                                module_enabled = next_enabled;
                                if !module_enabled {
                                    window_visible = false;
                                    window.set_visible(false);
                                    native_visible = false;
                                    let _ = shroudforge_package::config::request_window_visibility(
                                        &arguments.root,
                                        "worldEditor",
                                        false,
                                    );
                                }
                                let _ = shroudforge_package::config::publish_window_visibility(
                                    &arguments.root,
                                    "worldEditor",
                                    module_enabled && window_visible,
                                );
                            }
                        }
                        next_settings_poll = Instant::now() + Duration::from_millis(300);
                    }

                    let foreground = unsafe { GetForegroundWindow() };
                    let mut foreground_pid = 0;
                    if !foreground.is_null() { unsafe { GetWindowThreadProcessId(foreground, &mut foreground_pid) }; }
                    let is_game = arguments.desktop || foreground_pid == arguments.game_pid;
                    let is_overlay = foreground_pid == unsafe { GetCurrentProcessId() };
                    let mut function_key_pressed = [false; 8];
                    for (index, was_down) in function_keys_down.iter_mut().enumerate() {
                        let down = unsafe { GetAsyncKeyState(VK_F1 as i32 + index as i32) } < 0;
                        function_key_pressed[index] = down && !*was_down;
                        *was_down = down;
                    }
                    let toggle = unsafe { GetAsyncKeyState(toggle_key) } < 0;
                    let toggle_pressed = toggle && !toggle_down;
                    if module_enabled && (is_game || is_overlay) {
                        let was_visible = window_visible;
                        if toggle_pressed || function_key_pressed[1] {
                            window_visible = !window_visible;
                            if window_visible {
                                set_fixed_size(&window, panel_width, panel_height as f64);
                                window.set_inner_size(LogicalSize::new(
                                    panel_width as f64,
                                    panel_height as f64,
                                ));
                            }
                        }
                        if was_visible != window_visible {
                            let _ = shroudforge_package::config::request_window_visibility(
                                &arguments.root,
                                "worldEditor",
                                window_visible,
                            );
                        }
                    }
                    toggle_down = toggle;
                    let should_show = module_enabled && window_visible && (is_game || is_overlay);
                    if should_show != native_visible {
                        native_visible = should_show;
                        window.set_visible(native_visible);
                    }
                    if function_key_pressed[0] && module_enabled && window_visible && (is_game || is_overlay) {
                        let state = view_state(&arguments.root).unwrap_or_else(|_| empty_state());
                        if state.selected.is_empty() {
                            let _ = webview.evaluate_script("window.__worldEditorNotice('Select a blueprint first to change its image.');");
                        } else {
                            window.set_visible(false);
                            std::thread::sleep(Duration::from_millis(90));
                            let result = capture_game_window(foreground, &arguments.root, &state.selected);
                            window.set_visible(native_visible);
                            match result {
                                Ok(()) => {
                                    let _ = webview.evaluate_script("window.__worldEditorNotice('Screenshot saved.');");
                                    next_refresh = Instant::now();
                                }
                                Err(error) => {
                                    let _ = webview.evaluate_script(&format!("window.__worldEditorNotice({});", js_string(&format!("Screenshot failed: {error}"))));
                                }
                            }
                        }
                    }
                    if Instant::now() >= next_refresh {
                        if let Ok(state) = view_state(&arguments.root) {
                            if let Ok(payload) = serde_json::to_string(&state) {
                                let state_changed = payload != last_payload;
                                if state_changed {
                                    if state.panel_width != panel_width
                                        || state.panel_height != panel_height
                                        || state.panel_position_x != panel_position_x
                                        || state.panel_position_y != panel_position_y
                                    {
                                        panel_width = state.panel_width;
                                        panel_height = state.panel_height;
                                        panel_position_x = state.panel_position_x;
                                        panel_position_y = state.panel_position_y;
                                        set_fixed_size(&window, panel_width, panel_height as f64);
                                        window.set_inner_size(LogicalSize::new(
                                            panel_width as f64,
                                            panel_height as f64,
                                        ));
                                        position_window(
                                            &window,
                                            arguments.game_pid,
                                            panel_width as f64,
                                            panel_position_x,
                                            panel_position_y,
                                        );
                                    }
                                    last_payload = payload.clone();
                                }
                                let script = format!(
                                    r#"(() => {{
                                        const state = {last_payload};
                                        window.__sfPendingState = state;
                                        window.dispatchEvent(new CustomEvent('shroudforge-world-editor-state', {{ detail: state }}));
                                        let rendered = false;
                                        try {{
                                            if (typeof window.__worldEditorUpdate === 'function') {{
                                                window.__worldEditorUpdate(state);
                                                rendered = true;
                                            }}
                                        }} catch (error) {{
                                            console.error('World Editor render failed', error);
                                        }}
                                        const count = document.getElementById('blueprintCount');
                                        const help = document.getElementById('help');
                                        const list = document.getElementById('cards');
                                        if (count) count.textContent = `${{state.blueprints.length}} BLUEPRINT${{state.blueprints.length === 1 ? '' : 'S'}}`;
                                        if (help) help.textContent = state.hint;
                                        if (list && (!rendered || list.querySelectorAll('.card:not(.create)').length !== state.blueprints.length)) {{
                                            list.replaceChildren();
                                            for (const blueprint of state.blueprints) {{
                                                const card = document.createElement('div');
                                                card.className = 'card';
                                                const thumb = document.createElement('div');
                                                thumb.className = 'thumb';
                                                if (blueprint.image) {{
                                                    const image = document.createElement('img');
                                                    image.src = blueprint.image;
                                                    image.alt = `Screenshot of ${{blueprint.name}}`;
                                                    thumb.append(image);
                                                }} else {{
                                                    thumb.textContent = 'BLUEPRINT';
                                                }}
                                                const label = document.createElement('div');
                                                label.className = 'name';
                                                label.textContent = blueprint.name;
                                                card.append(thumb, label);
                                                list.append(card);
                                            }}
                                        }}
                                        if (window.ipc && typeof window.ipc.postMessage === 'function') {{
                                            window.ipc.postMessage(JSON.stringify({{ kind: 'state-applied', blueprints: state.blueprints.length, stage: state.stage, selected: state.selected }}));
                                        }}
                                    }})();"#
                                );
                                if let Err(error) = webview.evaluate_script(&script) {
                                    let _ = append_ui_log(
                                        &arguments.root,
                                        &format!("state update failed: {error}"),
                                    );
                                } else if state_changed {
                                    let _ = append_ui_log(
                                        &arguments.root,
                                        &format!(
                                            "state sent to UI: blueprints={} stage={} selected={} visible={}",
                                            state.blueprints.len(), state.stage, state.selected, window_visible
                                        ),
                                    );
                                }
                            }
                        }
                        next_refresh = Instant::now() + Duration::from_millis(refresh_ms);
                    }
                }
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    window_visible = false;
                    window.set_visible(false);
                    native_visible = false;
                    let _ = shroudforge_package::config::request_window_visibility(
                        &arguments.root,
                        "worldEditor",
                        false,
                    );
                }
                _ => {}
            }
        });
    }

    fn set_fixed_size(window: &tao::window::Window, width: u32, height: f64) {
        let size = LogicalSize::new(width as f64, height);
        window.set_min_inner_size(Some(size));
        window.set_max_inner_size(Some(size));
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
        let desktop = values.iter().any(|arg| arg == "--desktop");
        let game_pid = if desktop {
            0
        } else {
            value("--game-pid").ok_or("missing --game-pid")?.parse()?
        };
        let game_process = if desktop {
            std::ptr::null_mut()
        } else {
            unsafe { OpenProcess(0x0010_0000, 0, game_pid) }
        };
        if !desktop && game_process.is_null() {
            return Err("could not open Enshrouded process for lifetime tracking".into());
        }
        Ok(Arguments {
            root,
            game_pid,
            game_process,
            stop_name: value("--stop-event"),
            desktop,
        })
    }

    fn open_event(name: &str) -> HANDLE {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = std::ffi::OsStr::new(name)
            .encode_wide()
            .chain(Some(0))
            .collect();
        unsafe { OpenEventW(0x0010_0000, 0, wide.as_ptr()) }
    }

    fn position_window(
        window: &tao::window::Window,
        game_pid: u32,
        panel_width: f64,
        panel_position_x: i32,
        panel_position_y: u32,
    ) {
        let physical_width = (panel_width * window.scale_factor()) as i32;
        let scale = window.scale_factor();
        let foreground = unsafe { GetForegroundWindow() };
        if !foreground.is_null() {
            let mut foreground_pid = 0;
            unsafe { GetWindowThreadProcessId(foreground, &mut foreground_pid) };
            let mut rect: RECT = unsafe { std::mem::zeroed() };
            if foreground_pid == game_pid && unsafe { GetWindowRect(foreground, &mut rect) } != 0 {
                let x = if panel_position_x == -1 {
                    rect.left + ((rect.right - rect.left) - physical_width) / 2
                } else {
                    rect.left + (panel_position_x as f64 * scale) as i32
                };
                let y = rect.top + (panel_position_y as f64 * scale) as i32;
                window.set_outer_position(PhysicalPosition::new(x, y));
                return;
            }
        }
        if let Some(monitor) = window.current_monitor() {
            let origin = monitor.position();
            let size = monitor.size();
            let x = if panel_position_x == -1 {
                origin.x + (size.width as i32 - physical_width) / 2
            } else {
                origin.x + (panel_position_x as f64 * scale) as i32
            };
            let y = origin.y + (panel_position_y as f64 * scale) as i32;
            window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
        } else {
            let x = if panel_position_x == -1 {
                0.0
            } else {
                panel_position_x as f64
            };
            window.set_outer_position(LogicalPosition::new(x, panel_position_y as f64));
        }
    }

    fn view_state(root: &Path) -> Result<ViewState, Box<dyn std::error::Error>> {
        let exports = shroudforge_package::paths::export_dir(root);
        let directory = exports.join("world-editor/blueprints");
        let mut stage = "idle".to_owned();
        let mut hint =
            "Select the Building Hammer, choose a Single Voxel, aim at the first corner, then press F5."
                .to_owned();
        let mut selected = String::new();
        let mut axis = "y".to_owned();
        let mut turns = 0;
        let mut panel_width = DEFAULT_WIDTH;
        let mut panel_height = DEFAULT_HEIGHT;
        let mut panel_position_x = -1;
        let mut panel_position_y = TOP_OFFSET as u32;
        if let Ok(contents) = fs::read_to_string(exports.join("world-editor/editor-state.txt")) {
            for line in contents.lines() {
                if let Some(value) = line.strip_prefix("stage=") {
                    stage = value.to_owned();
                } else if let Some(value) = line.strip_prefix("hint=") {
                    hint = value.to_owned();
                } else if let Some(value) = line.strip_prefix("selected=") {
                    selected = value.to_owned();
                } else if let Some(value) = line.strip_prefix("axis=") {
                    axis = value.to_owned();
                } else if let Some(value) = line.strip_prefix("turns=") {
                    turns = value.parse::<u8>().unwrap_or(0) % 4;
                } else if let Some(value) = line.strip_prefix("panelWidth=") {
                    panel_width = value
                        .parse::<u32>()
                        .unwrap_or(DEFAULT_WIDTH)
                        .clamp(MIN_WIDTH, MAX_WIDTH);
                } else if let Some(value) = line.strip_prefix("panelHeight=") {
                    panel_height = value
                        .parse::<u32>()
                        .unwrap_or(DEFAULT_HEIGHT)
                        .clamp(MIN_HEIGHT, MAX_HEIGHT);
                } else if let Some(value) = line.strip_prefix("panelPositionX=") {
                    panel_position_x = value
                        .parse::<i32>()
                        .unwrap_or(-1)
                        .clamp(-1, MAX_POSITION);
                } else if let Some(value) = line.strip_prefix("panelPositionY=") {
                    panel_position_y = value
                        .parse::<u32>()
                        .unwrap_or(TOP_OFFSET as u32)
                        .min(MAX_POSITION as u32);
                }
            }
        }
        let mut blueprints = Vec::new();
        if let Ok(entries) = fs::read_dir(directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case("sfbp"))
                {
                    let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
                        continue;
                    };
                    if !valid_blueprint_name(name) {
                        continue;
                    }
                    let image_path = path.with_file_name(format!("{name}.thumb.png"));
                    let image_path = if image_path.is_file() {
                        image_path
                    } else {
                        path.with_extension("png")
                    };
                    let image = fs::read(image_path).ok().map(|bytes| {
                        format!(
                            "data:image/png;base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(bytes)
                        )
                    });
                    blueprints.push(BlueprintCard {
                        name: name.to_owned(),
                        image,
                    });
                }
            }
        }
        blueprints.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        Ok(ViewState {
            blueprints,
            stage,
            hint,
            selected,
            axis,
            turns,
            panel_width,
            panel_height,
            panel_position_x,
            panel_position_y,
        })
    }

    fn empty_state() -> ViewState {
        ViewState {
            blueprints: Vec::new(),
            stage: "idle".into(),
            hint: "Select the Building Hammer, choose a Single Voxel, aim at the first corner, then press F5."
                .into(),
            selected: String::new(),
            axis: "y".into(),
            turns: 0,
            panel_width: DEFAULT_WIDTH,
            panel_height: DEFAULT_HEIGHT,
            panel_position_x: -1,
            panel_position_y: TOP_OFFSET as u32,
        }
    }

    fn dispatch_action(
        root: &Path,
        action: &str,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !valid_identifier(action) {
            return Err("Invalid mod action".into());
        }
        let directory = shroudforge_package::paths::ui_data_dir(root).join("actions");
        fs::create_dir_all(&directory)?;
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = directory.join(format!("world-editor-{}-{nonce}.json", std::process::id()));
        let payload =
            serde_json::json!({ "modId": "world-editor", "action": action, "value": value });
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, serde_json::to_vec(&payload)?)?;
        fs::rename(temporary, path)?;
        Ok(())
    }

    fn valid_identifier(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 80
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    }

    fn append_ui_log(root: &Path, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        let directory = shroudforge_package::paths::logs_dir(root);
        fs::create_dir_all(&directory)?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("world-editor-ui.log"))?;
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        writeln!(file, "{timestamp} {message}")?;
        Ok(())
    }

    fn js_string(value: &str) -> String {
        serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
    }

    fn capture_game_window(
        hwnd: HWND,
        root: &Path,
        blueprint: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if hwnd.is_null() {
            return Err("Game window is unavailable".into());
        }
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
            return Err("Could not read the game window size".into());
        }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            return Err("Game window has an invalid size".into());
        }
        let screen = unsafe { GetDC(std::ptr::null_mut()) };
        if screen.is_null() {
            return Err("Could not open the game screen".into());
        }
        let memory = unsafe { CreateCompatibleDC(screen) };
        let bitmap = unsafe { CreateCompatibleBitmap(screen, width, height) };
        if memory.is_null() || bitmap.is_null() {
            if !memory.is_null() {
                unsafe { DeleteDC(memory) };
            }
            if !bitmap.is_null() {
                unsafe { DeleteObject(bitmap) };
            }
            unsafe { ReleaseDC(std::ptr::null_mut(), screen) };
            return Err("Could not allocate the screenshot buffer".into());
        }
        let previous = unsafe { SelectObject(memory, bitmap) };
        let copied = unsafe {
            BitBlt(
                memory, 0, 0, width, height, screen, rect.left, rect.top, SRCCOPY,
            )
        };
        unsafe { SelectObject(memory, previous) };
        let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..unsafe { std::mem::zeroed() }
        };
        let mut bgra = vec![0u8; width as usize * height as usize * 4];
        let read = if copied != 0 {
            unsafe {
                GetDIBits(
                    memory,
                    bitmap,
                    0,
                    height as u32,
                    bgra.as_mut_ptr().cast(),
                    &mut info,
                    DIB_RGB_COLORS,
                )
            }
        } else {
            0
        };
        unsafe {
            DeleteObject(bitmap);
            DeleteDC(memory);
            ReleaseDC(std::ptr::null_mut(), screen);
        }
        if read == 0 {
            return Err("Could not capture the game screen (exclusive fullscreen mode may be enabled)".into());
        }
        for pixel in bgra.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        let image = image::RgbaImage::from_raw(width as u32, height as u32, bgra)
            .ok_or("Screenshot pixels do not match the window size")?;
        let name = if valid_blueprint_name(blueprint) {
            blueprint
        } else {
            return Err("Invalid blueprint name".into());
        };
        let directory =
            shroudforge_package::paths::export_dir(root).join("world-editor/blueprints");
        fs::create_dir_all(&directory)?;
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png)?;
        fs::write(directory.join(format!("{name}.png")), png.into_inner())?;
        let thumbnail = image::imageops::thumbnail(&image, 960, 540);
        let mut png = Cursor::new(Vec::new());
        thumbnail.write_to(&mut png, image::ImageFormat::Png)?;
        fs::write(
            directory.join(format!("{name}.thumb.png")),
            png.into_inner(),
        )?;
        Ok(())
    }

    fn valid_blueprint_name(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    }
}

#[cfg(windows)]
pub use windows::run_module;

#[cfg(not(windows))]
pub fn run_module() -> Result<(), Box<dyn std::error::Error>> {
    Err("World Editor UI is supported on Windows only".into())
}
