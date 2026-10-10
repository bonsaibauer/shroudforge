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
    use image::{DynamicImage, ImageReader, RgbaImage};
    use serde::Serialize;
    use tao::{
        dpi::{LogicalPosition, LogicalSize, PhysicalPosition},
        event::{Event, StartCause, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
    };
    use windows::{
        Win32::{
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoTaskMemFree, CoUninitialize,
            },
            UI::Shell::{
                FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog,
                SIGDN_FILESYSPATH,
            },
        },
        core::PCWSTR,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, RECT, WAIT_OBJECT_0},
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
        OpenModSettings,
        Drag,
        Action {
            action: String,
            value: String,
        },
        Manager(bool),
        LibraryExpanded {
            expanded: bool,
            height: u32,
        },
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct BlueprintCard {
        name: String,
        image: Option<String>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ScreenshotCard {
        name: String,
        path: String,
        modified: u64,
        thumbnail: String,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ScreenshotList {
        folder: String,
        screenshots: Vec<ScreenshotCard>,
        truncated: bool,
        can_undo: bool,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ViewState {
        blueprints: Vec<BlueprintCard>,
        process_state: String,
        process_operation: String,
        process_phase: String,
        process_completed: u8,
        process_total: u8,
        process_sequence: u64,
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
                        "open-mod-settings" => Command::OpenModSettings,
                        "drag" => Command::Drag,
                        "manager-open" => Command::Manager(true),
                        "manager-close" => Command::Manager(false),
                        "library-expand" => Command::LibraryExpanded {
                            expanded: value["expanded"].as_bool().unwrap_or(false),
                            height: value["height"].as_u64().unwrap_or(DEFAULT_HEIGHT as u64)
                                as u32,
                        },
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
        let _ = append_ui_log(
            &arguments.root,
            "window initialized with Modloader-style undecorated Tao window; document owns the opaque panel surface",
        );

        let mut panel_width = initial_state.panel_width;
        let mut panel_height = initial_state.panel_height;
        let mut panel_position_x = initial_state.panel_position_x;
        let mut panel_position_y = initial_state.panel_position_y;
        let mut manager_open = false;
        let mut library_expanded = false;
        let mut library_expanded_height = initial_state.panel_height;
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
        // Keep disk enumeration and thumbnail reads off the window thread.
        // Bound the channel so slow rendering cannot accumulate snapshots.
        let (state_sender, state_updates) = mpsc::sync_channel(1);
        let state_root = arguments.root.clone();
        std::thread::spawn(move || loop {
            if let Ok(state) = view_state(&state_root) {
                match state_sender.try_send(state) {
                    Ok(()) | Err(mpsc::TrySendError::Full(_)) => {}
                    Err(mpsc::TrySendError::Disconnected(_)) => break,
                }
            }
            std::thread::sleep(Duration::from_millis(refresh_ms));
        });
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
                            Command::OpenModSettings => {
                                if let Err(error) = shroudforge_package::config::request_world_editor_module_settings(&arguments.root) {
                                    let _ = append_ui_log(&arguments.root, &format!("Could not open World Editor module settings in Modloader UI: {error}"));
                                }
                            }
                            Command::Drag => {}
                            Command::Manager(open) => {
                                if manager_open != open {
                                    manager_open = open;
                                    let height = if manager_open {
                                        560.0
                                    } else if library_expanded {
                                        library_expanded_height as f64
                                    } else {
                                        panel_height as f64
                                    };
                                    set_fixed_size(&window, panel_width, height);
                                    window.set_inner_size(LogicalSize::new(
                                        panel_width as f64,
                                        height,
                                    ));
                                    position_window(
                                        &window,
                                        arguments.game_pid,
                                        panel_width as f64,
                                        panel_position_x,
                                        panel_position_y,
                                    );
                                }
                            }
                            Command::LibraryExpanded { expanded, height } => {
                                library_expanded = expanded;
                                library_expanded_height = height.clamp(DEFAULT_HEIGHT, 720);
                                if !manager_open {
                                    let height = if library_expanded {
                                        library_expanded_height as f64
                                    } else {
                                        panel_height as f64
                                    };
                                    set_fixed_size(&window, panel_width, height);
                                    window.set_inner_size(LogicalSize::new(
                                        panel_width as f64,
                                        height,
                                    ));
                                    position_window(
                                        &window,
                                        arguments.game_pid,
                                        panel_width as f64,
                                        panel_position_x,
                                        panel_position_y,
                                    );
                                }
                            }
                            Command::Action { action, value } => {
                                let result = match action.as_str() {
                                    "browseScreenshots" => {
                                        browse_screenshots(&arguments.root, &value)
                                    }
                                    "refreshScreenshots" => {
                                        load_screenshot_list(&arguments.root, &value).map(Some)
                                    }
                                    "applyScreenshot" => apply_screenshot(&arguments.root, &value)
                                        .map(|()| None),
                                    "undoScreenshot" => undo_screenshot(&arguments.root, &value)
                                        .map(|()| None),
                                    _ => dispatch_action(&arguments.root, &action, &value)
                                        .map(|()| None),
                                };
                                match result {
                                    Ok(Some(list)) => {
                                        if let Ok(payload) = serde_json::to_string(&list) {
                                            let _ = webview.evaluate_script(&format!(
                                                "window.__worldEditorScreenshots({payload});"
                                            ));
                                        }
                                    }
                                    Ok(None) => {
                                        if action == "applyScreenshot" {
                                            let name = value.split_once('\t').map(|(name, _)| name).unwrap_or("unknown");
                                            let _ = append_ui_log(
                                                &arguments.root,
                                                &format!("screenshot cover saved for blueprint={name}"),
                                            );
                                            let _ = webview.evaluate_script(
                                                "window.__worldEditorScreenshotApplied(false);",
                                            );
                                            next_refresh = Instant::now();
                                        } else if action == "undoScreenshot" {
                                            let _ = append_ui_log(
                                                &arguments.root,
                                                &format!("screenshot cover undo completed for blueprint={value}"),
                                            );
                                            let _ = webview.evaluate_script(
                                                "window.__worldEditorScreenshotApplied(true);",
                                            );
                                            next_refresh = Instant::now();
                                        }
                                    }
                                    Err(error) => {
                                        let _ = append_ui_log(
                                            &arguments.root,
                                            &format!("action failed action={action}: {error}"),
                                        );
                                        let _ = webview.evaluate_script(&format!(
                                            "window.__worldEditorScreenshotError({});",
                                            js_string(&error.to_string())
                                        ));
                                    }
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
                                let height = if manager_open {
                                    560.0
                                } else if library_expanded {
                                    library_expanded_height as f64
                                } else {
                                    panel_height as f64
                                };
                                set_fixed_size(&window, panel_width, height);
                                window.set_inner_size(LogicalSize::new(
                                    panel_width as f64,
                                    height,
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
                                let height = if manager_open {
                                    560.0
                                } else if library_expanded {
                                    library_expanded_height as f64
                                } else {
                                    panel_height as f64
                                };
                                set_fixed_size(&window, panel_width, height);
                                window.set_inner_size(LogicalSize::new(
                                    panel_width as f64,
                                    height,
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
                    if Instant::now() >= next_refresh {
                        if let Some(state) = state_updates.try_iter().last() {
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
                                        let height = if manager_open {
                                            560.0
                                        } else if library_expanded {
                                            library_expanded_height as f64
                                        } else {
                                            panel_height as f64
                                        };
                                        set_fixed_size(&window, panel_width, height);
                                        window.set_inner_size(LogicalSize::new(
                                            panel_width as f64,
                                            height,
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
                                        // Keep the process indicator working in the native fallback renderer too.
                                        // A state-applied acknowledgement does not prove the page script ran.
                                        const processPanel = document.getElementById('processProgress');
                                        const processBar = document.getElementById('processBar');
                                        const processLabel = document.getElementById('processLabel');
                                        const processElapsed = document.getElementById('processElapsed');
                                        if (processPanel && processBar && processLabel && processElapsed) {{
                                            const status = state.processState || 'idle';
                                            const completed = Math.max(0, Number(state.processCompleted) || 0);
                                            const total = Math.max(0, Number(state.processTotal) || 0);
                                            processPanel.hidden = false;
                                            processPanel.dataset.status = status;
                                            processBar.max = Math.max(1, total);
                                            if (total > 0) processBar.value = Math.min(total, completed);
                                            else processBar.removeAttribute('value');
                                            processLabel.textContent = [state.processOperation, state.processPhase].filter(Boolean).join(' · ') || 'World Editor · Ready';
                                            const detail = total > 0 ? `${{completed}}/${{total}} steps complete` : 'Working';
                                            processPanel.title = status === 'error' ? `${{detail}}. ${{state.hint}}` : detail;
                                            processBar.setAttribute('aria-valuetext', `${{processLabel.textContent}}; ${{detail}}`);
                                            if (window.__sfProcessSequence !== state.processSequence) {{
                                                window.__sfProcessSequence = state.processSequence;
                                                window.__sfProcessStarted = performance.now();
                                            }}
                                            processElapsed.textContent = status === 'running'
                                                ? `${{total > 0 ? `${{completed}}/${{total}} · ` : ''}}${{Math.floor((performance.now() - window.__sfProcessStarted) / 1000)}}s`
                                                : status === 'complete' ? '✓' : status === 'idle' ? '' : `${{completed}}/${{total || '—'}}`;
                                        }}
                                        const count = document.getElementById('blueprintCount');
                                        const help = document.getElementById('help');
                                        const list = document.getElementById('cards');
                                        window.__sfPendingState = state;
                                        const renderSignature = JSON.stringify([state.stage, state.selected, state.blueprints.map(item => [item.name, item.image])]);
                                        if (count) count.textContent = `${{state.blueprints.length}} BLUEPRINT${{state.blueprints.length === 1 ? '' : 'S'}}`;
                                        if (help) help.textContent = state.hint;
                                        const cardCountMismatch = list && list.querySelectorAll('.card:not(.create)').length !== state.blueprints.length;
                                        const fallbackNeedsRender = !rendered && (window.__sfFallbackRenderSignature !== renderSignature || cardCountMismatch);
                                        if (list && (fallbackNeedsRender || (rendered && cardCountMismatch))) {{
                                            list.replaceChildren();
                                            window.__sfFallbackRenderSignature = renderSignature;
                                            const create = document.createElement('button');
                                            create.type = 'button';
                                            const createActive = !state.selected;
                                            create.className = 'card create' + (createActive ? ' active' : '');
                                            create.innerHTML = `<span class="create-art" aria-hidden="true"><svg class="new-blueprint-icon" viewBox="0 0 44 54"><path class="page" d="M5 2h24l10 10v38H5zM29 2v10h10"/><path class="plus" d="M16 31h12M22 25v12"/></svg></span><span class="create-label">New blueprint</span><span class="card-state">${{createActive ? 'ACTIVE' : 'CREATE'}}</span>`;
                                            create.title = 'Start a new capture. F5 marks the first corner.';
                                            create.onclick = () => window.ipc?.postMessage(JSON.stringify({{kind:'action', action:'newBlueprint', value:''}}));
                                            list.append(create);
                                            for (const blueprint of state.blueprints) {{
                                                const card = document.createElement('button');
                                                card.type = 'button';
                                                const active = state.selected === blueprint.name;
                                                card.className = 'card blueprint-card' + (active ? ' active' : '');
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
                                                const badge = document.createElement('span');
                                                badge.className = 'card-state';
                                                badge.textContent = active ? 'ACTIVE' : '';
                                                card.append(thumb, label, badge);
                                                card.onclick = event => {{
                                                    clearTimeout(window.__sfFallbackCardClickTimer);
                                                    if (event.detail >= 2) {{
                                                        window.__worldEditorManageSelected?.(blueprint.name);
                                                        return;
                                                    }}
                                                    window.__sfFallbackCardClickTimer = setTimeout(() => window.ipc?.postMessage(JSON.stringify({{kind:'action', action:'selectBlueprint', value:blueprint.name}})), 260);
                                                }};
                                                list.append(card);
                                            }}
                                        }}
                                        if (typeof window.__worldEditorManageSelected !== 'function') {{
                                            window.__worldEditorManageSelected = name => {{
                                                const current = window.__sfPendingState || state;
                                                const target = current.blueprints.find(item => item.name === name) || current.blueprints.find(item => item.name === current.selected);
                                                if (!target) {{
                                                    const help = document.getElementById('help');
                                                    if (help) help.textContent = 'Select a saved blueprint before opening its manager.';
                                                    return;
                                                }}
                                                if (current.selected !== target.name) window.ipc?.postMessage(JSON.stringify({{kind:'action', action:'selectBlueprint', value:target.name}}));
                                                const panel = document.getElementById('manage');
                                                if (!panel) return;
                                                const esc = value => String(value).replace(/[&<>"']/g, ch => ({{'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}}[ch]));
                                                panel.innerHTML = `<div class="manage-head"><div><strong>Manage blueprint · ${{esc(target.name)}}</strong><p>Take a Steam screenshot with F12, then select it here. You can also browse another folder.</p></div><button id="fallbackDone">Back to library</button></div><div class="manage-actions"><input id="fallbackRename" maxlength="64" aria-label="Blueprint name"><button id="fallbackRenameButton">Rename</button><button id="fallbackDuplicateButton">Duplicate as…</button><button class="danger" id="fallbackDeleteButton">Delete blueprint</button></div><div class="screenshot-toolbar"><span id="fallbackFolder">Finding screenshots…</span><button id="fallbackBrowse">Choose folder…</button><button id="fallbackReload">Reload screenshots</button><button id="fallbackUse" disabled>Use selected screenshot</button><button id="fallbackUndo" hidden>Undo cover change</button></div><div id="fallbackQueue" class="screenshot-queue"><div class="screenshot-empty">Loading screenshots…</div></div>`;
                                                panel.classList.add('show');
                                                const send = (kind, fields = {{}}) => window.ipc?.postMessage(JSON.stringify({{kind, ...fields}}));
                                                const action = (id, value = '') => send('action', {{action:id, value}});
                                                const input = panel.querySelector('#fallbackRename');
                                                input.value = target.name;
                                                let selectedShot = null;
                                                const refresh = () => action('refreshScreenshots', target.name);
                                                send('manager-open'); refresh();
                                                panel.querySelector('#fallbackDone').onclick = () => {{ panel.classList.remove('show'); send('manager-close'); }};
                                                panel.querySelector('#fallbackBrowse').onclick = () => action('browseScreenshots', target.name);
                                                panel.querySelector('#fallbackReload').onclick = refresh;
                                                panel.querySelector('#fallbackUse').onclick = () => selectedShot && action('applyScreenshot', target.name + String.fromCharCode(9) + selectedShot.path);
                                                panel.querySelector('#fallbackUndo').onclick = () => action('undoScreenshot', target.name);
                                                panel.querySelector('#fallbackRenameButton').onclick = () => input.value.trim() && action('renameLibraryBlueprint', target.name + String.fromCharCode(9) + input.value.trim());
                                                panel.querySelector('#fallbackDuplicateButton').onclick = () => input.value.trim() && action('duplicateLibraryBlueprint', target.name + String.fromCharCode(9) + input.value.trim());
                                                panel.querySelector('#fallbackDeleteButton').onclick = () => window.confirm(`Delete blueprint “${{target.name}}” and its image?`) && action('deleteLibraryBlueprint', target.name);
                                                window.__sfFallbackSelectedShot = shot => {{ selectedShot = shot; panel.querySelector('#fallbackUse').disabled = !shot; }};
                                            }};
                                        }}
                                        if (typeof window.__worldEditorScreenshots !== 'function') window.__worldEditorScreenshots = data => {{
                                            const panel = document.getElementById('manage');
                                            if (!panel?.classList.contains('show')) return;
                                            panel.querySelector('#fallbackFolder').textContent = data.folder || 'Screenshot folder unavailable';
                                            panel.querySelector('#fallbackUndo').hidden = !data.canUndo;
                                            const queue = panel.querySelector('#fallbackQueue'); queue.replaceChildren();
                                            if (!data.screenshots.length) {{ const empty = document.createElement('div'); empty.className = 'screenshot-empty'; empty.textContent = 'No SVG, PNG, JPG, or JPEG images found. Choose another folder, or take a Steam screenshot with F12 and reload.'; queue.append(empty); return; }}
                                            for (const shot of data.screenshots) {{ const item = document.createElement('button'); item.className = 'screenshot-item'; const image = document.createElement('img'); image.src = shot.thumbnail; image.alt = ''; const label = document.createElement('span'); label.textContent = shot.name; const date = document.createElement('small'); date.textContent = new Date(shot.modified * 1000).toLocaleString(); item.append(image, label, date); item.onclick = () => {{ queue.querySelectorAll('.screenshot-item').forEach(node => node.classList.remove('selected')); item.classList.add('selected'); window.__sfFallbackSelectedShot?.(shot); }}; queue.append(item); }}
                                        }};
                                        if (typeof window.__worldEditorScreenshotApplied !== 'function') window.__worldEditorScreenshotApplied = undone => {{ const help = document.getElementById('help'); if (help) help.textContent = undone ? 'Previous screenshot cover restored.' : 'Screenshot cover updated.'; }};
                                        if (typeof window.__worldEditorScreenshotError !== 'function') window.__worldEditorScreenshotError = message => {{ const help = document.getElementById('help'); if (help) help.textContent = `Screenshot update failed: ${{message}}`; }};
                                        if (window.__worldEditorUpdate !== undefined && typeof window.__worldEditorUpdate !== 'function') console.error('World Editor UI entrypoint is unavailable');
                                        if (typeof window.__worldEditorUpdate !== 'function' && !window.__sfWorldEditorFallbackEvents) {{
                                            window.__sfWorldEditorFallbackEvents = true;
                                            document.addEventListener('click', event => {{
                                                const target = event.target instanceof Element ? event.target.closest('#close, #manageButton, #expandLibrary') : null;
                                                if (!target || !window.ipc || typeof window.ipc.postMessage !== 'function') return;
                                                if (target.id === 'close') window.ipc.postMessage(JSON.stringify({{kind:'hide'}}));
                                                if (target.id === 'manageButton') window.__worldEditorManageSelected?.();
                                                if (target.id === 'expandLibrary') {{
                                                    const expanded = !document.getElementById('cards')?.classList.contains('expanded');
                                                    const list = document.getElementById('cards');
                                                    list?.classList.toggle('expanded', expanded);
                                                    target.textContent = expanded ? '⌃' : '⌄';
                                                    target.setAttribute('aria-expanded', String(expanded));
                                                    window.ipc.postMessage(JSON.stringify({{kind:'library-expand', expanded, height: 400}}));
                                                }}
                                            }});
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
        let mut process_state = "idle".to_owned();
        let mut process_operation = String::new();
        let mut process_phase = String::new();
        let mut process_completed = 0;
        let mut process_total = 0;
        let mut process_sequence = 0;
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
                if let Some(value) = line.strip_prefix("processState=") {
                    process_state = value.to_owned();
                } else if let Some(value) = line.strip_prefix("processOperation=") {
                    process_operation = value.to_owned();
                } else if let Some(value) = line.strip_prefix("processPhase=") {
                    process_phase = value.to_owned();
                } else if let Some(value) = line.strip_prefix("processCompleted=") {
                    process_completed = value.parse::<u8>().unwrap_or(0);
                } else if let Some(value) = line.strip_prefix("processTotal=") {
                    process_total = value.parse::<u8>().unwrap_or(0);
                } else if let Some(value) = line.strip_prefix("processSequence=") {
                    process_sequence = value.parse::<u64>().unwrap_or(0);
                } else if let Some(value) = line.strip_prefix("stage=") {
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
                    panel_position_x = value.parse::<i32>().unwrap_or(-1).clamp(-1, MAX_POSITION);
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
            process_state,
            process_operation,
            process_phase,
            process_completed,
            process_total,
            process_sequence,
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
            process_state: "idle".into(),
            process_operation: String::new(),
            process_phase: String::new(),
            process_completed: 0,
            process_total: 0,
            process_sequence: 0,
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

    fn browse_screenshots(
        root: &Path,
        blueprint: &str,
    ) -> Result<Option<ScreenshotList>, Box<dyn std::error::Error>> {
        let folder = choose_screenshot_folder()?;
        save_screenshot_folder(root, &folder)?;
        load_screenshot_list(root, blueprint).map(Some)
    }

    fn choose_screenshot_folder() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        initialized.ok().map_err(|error| {
            format!("could not initialize the Windows screenshot-folder picker: {error}")
        })?;
        struct ComUninitialize;
        impl Drop for ComUninitialize {
            fn drop(&mut self) {
                unsafe { CoUninitialize() };
            }
        }
        let _com = ComUninitialize;

        let title: Vec<u16> = "Choose the folder containing your screenshots"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let dialog = unsafe {
            CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
        }?;
        unsafe {
            dialog
                .SetTitle(PCWSTR(title.as_ptr()))
                .and_then(|()| dialog.SetOptions(FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM))?;
            dialog.Show(None)?;
        }
        let selected = unsafe { dialog.GetResult() }?;
        let display_name = unsafe { selected.GetDisplayName(SIGDN_FILESYSPATH) }?;
        let path_string: Result<String, std::string::FromUtf16Error> =
            unsafe { display_name.to_string() };
        unsafe { CoTaskMemFree(Some(display_name.0.cast())) };
        let folder = PathBuf::from(path_string?);
        if !folder.is_dir() {
            return Err("The selected screenshot folder is unavailable".into());
        }
        Ok(folder)
    }

    fn screenshot_folder_file(root: &Path) -> PathBuf {
        shroudforge_package::paths::export_dir(root)
            .join("world-editor")
            .join("screenshot-source.txt")
    }

    fn save_screenshot_folder(
        root: &Path,
        folder: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !folder.is_dir() {
            return Err("The selected screenshot folder is unavailable".into());
        }
        let path = screenshot_folder_file(root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, folder.to_string_lossy().as_bytes())?;
        Ok(())
    }

    fn current_screenshot_folder(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let saved = screenshot_folder_file(root);
        if let Ok(value) = fs::read_to_string(&saved) {
            let folder = PathBuf::from(value.trim());
            if folder.is_dir() {
                return Ok(folder);
            }
        }
        if let Some(folder) = discover_screenshot_folder() {
            save_screenshot_folder(root, &folder)?;
            return Ok(folder);
        }
        Err("Choose the folder where your screenshots are stored".into())
    }

    fn discover_screenshot_folder() -> Option<PathBuf> {
        let mut candidates = Vec::new();
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            candidates.push(PathBuf::from(profile).join("Pictures/Screenshots/Enshrouded"));
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
        for steam in steam_roots {
            let userdata = steam.join("userdata");
            let Ok(accounts) = fs::read_dir(userdata) else {
                continue;
            };
            for account in accounts.flatten() {
                candidates.push(account.path().join("760/remote/1203620/screenshots"));
            }
        }
        candidates.into_iter().find(|path| {
            path.is_dir()
                && fs::read_dir(path).ok().is_some_and(|entries| {
                    entries.flatten().any(|entry| {
                        entry.file_type().is_ok_and(|kind| kind.is_file())
                            && is_supported_image(&entry.path())
                    })
                })
        })
    }

    fn load_screenshot_list(
        root: &Path,
        blueprint: &str,
    ) -> Result<ScreenshotList, Box<dyn std::error::Error>> {
        const MAX_SCREENSHOTS: usize = 120;
        if !valid_blueprint_name(blueprint) {
            return Err("Select a blueprint before choosing a screenshot".into());
        }
        let folder = current_screenshot_folder(root)?;
        let mut files = fs::read_dir(&folder)?
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if !entry.file_type().ok()?.is_file() || !is_supported_image(&path) {
                    return None;
                }
                let modified = entry
                    .metadata()
                    .ok()?
                    .modified()
                    .ok()?
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                Some((path, modified))
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| right.1.cmp(&left.1));
        let truncated = files.len() > MAX_SCREENSHOTS;
        files.truncate(MAX_SCREENSHOTS);
        let mut screenshots = Vec::with_capacity(files.len());
        for (path, modified) in files {
            let Ok(image) = load_screenshot_image(&path, 320) else {
                continue;
            };
            let thumbnail = image.thumbnail(320, 180);
            let mut bytes = Cursor::new(Vec::new());
            thumbnail.write_to(&mut bytes, image::ImageFormat::Png)?;
            screenshots.push(ScreenshotCard {
                name: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Screenshot")
                    .to_owned(),
                path: path.to_string_lossy().into_owned(),
                modified,
                thumbnail: format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())
                ),
            });
        }
        Ok(ScreenshotList {
            folder: folder.to_string_lossy().into_owned(),
            screenshots,
            truncated,
            can_undo: shroudforge_package::paths::export_dir(root)
                .join("world-editor/blueprints")
                .join(format!("{blueprint}.cover-undo.json"))
                .is_file(),
        })
    }

    fn is_supported_image(path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(extension.to_ascii_lowercase().as_str(), "svg" | "png" | "jpg" | "jpeg")
            })
    }

    fn load_screenshot_image(
        path: &Path,
        max_dimension: u32,
    ) -> Result<DynamicImage, Box<dyn std::error::Error>> {
        const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
        const MAX_RASTER_DIMENSION: u32 = 8192;
        const MAX_RASTER_PIXELS: u64 = 32 * 1024 * 1024;
        let metadata = fs::metadata(path)?;
        if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err("Image file is not a supported size".into());
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if extension == "svg" {
            let data = fs::read(path)?;
            let mut options = resvg::usvg::Options::default();
            options.fontdb_mut().load_system_fonts();
            let tree = resvg::usvg::Tree::from_data(&data, &options)?;
            let size = tree.size();
            let source_width = size.width();
            let source_height = size.height();
            let longest = source_width.max(source_height);
            if !longest.is_finite() || longest <= 0.0 {
                return Err("SVG has invalid dimensions".into());
            }
            let scale = (max_dimension.max(1) as f32 / longest).min(1.0);
            let width = (source_width * scale).ceil().max(1.0) as u32;
            let height = (source_height * scale).ceil().max(1.0) as u32;
            let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
                .ok_or("Could not allocate the SVG preview")?;
            let transform = resvg::tiny_skia::Transform::from_scale(
                width as f32 / source_width,
                height as f32 / source_height,
            );
            resvg::render(&tree, transform, &mut pixmap.as_mut());
            let pixels = RgbaImage::from_raw(width, height, pixmap.take())
                .ok_or("Could not read the rendered SVG pixels")?;
            return Ok(DynamicImage::ImageRgba8(pixels));
        }

        let reader = ImageReader::open(path)?.with_guessed_format()?;
        let format = reader.format().ok_or("Image format could not be identified")?;
        let expected_format = match extension.as_str() {
            "png" => image::ImageFormat::Png,
            "jpg" | "jpeg" => image::ImageFormat::Jpeg,
            _ => return Err("Choose an SVG, PNG, JPG, or JPEG image".into()),
        };
        if format != expected_format {
            return Err("Image contents do not match the file extension".into());
        }
        let (width, height) = reader.into_dimensions()?;
        if width == 0
            || height == 0
            || width > MAX_RASTER_DIMENSION
            || height > MAX_RASTER_DIMENSION
            || u64::from(width) * u64::from(height) > MAX_RASTER_PIXELS
        {
            return Err("Image dimensions exceed the 8192 by 8192 limit".into());
        }
        Ok(image::open(path)?)
    }

    fn apply_screenshot(root: &Path, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        let (name, source) = value
            .split_once('\t')
            .ok_or("Screenshot selection is incomplete")?;
        if !valid_blueprint_name(name) {
            return Err("The selected blueprint name is invalid".into());
        }
        let exports = shroudforge_package::paths::export_dir(root);
        let blueprint_dir = exports.join("world-editor/blueprints");
        if !blueprint_dir.join(format!("{name}.sfbp")).is_file() {
            return Err("The selected blueprint no longer exists".into());
        }
        let folder = current_screenshot_folder(root)?.canonicalize()?;
        let source = PathBuf::from(source).canonicalize()?;
        if !source.starts_with(&folder) || !source.is_file() || !is_supported_image(&source) {
            return Err("Choose an image from the selected screenshot folder".into());
        }
        let image = load_screenshot_image(&source, 4096)?;
        fs::create_dir_all(&blueprint_dir)?;
        backup_cover(&blueprint_dir, name)?;
        let mut full = Cursor::new(Vec::new());
        image.write_to(&mut full, image::ImageFormat::Png)?;
        fs::write(blueprint_dir.join(format!("{name}.png")), full.into_inner())?;
        let thumbnail = image.thumbnail(960, 540);
        let mut thumb = Cursor::new(Vec::new());
        thumbnail.write_to(&mut thumb, image::ImageFormat::Png)?;
        fs::write(
            blueprint_dir.join(format!("{name}.thumb.png")),
            thumb.into_inner(),
        )?;
        Ok(())
    }

    fn backup_cover(directory: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let cover = directory.join(format!("{name}.png"));
        let thumbnail = directory.join(format!("{name}.thumb.png"));
        let cover_backup = directory.join(format!("{name}.cover-undo.png"));
        let thumbnail_backup = directory.join(format!("{name}.cover-undo.thumb.png"));
        let had_cover = cover.is_file();
        let had_thumbnail = thumbnail.is_file();
        if had_cover {
            fs::copy(&cover, &cover_backup)?;
        } else {
            let _ = fs::remove_file(&cover_backup);
        }
        if had_thumbnail {
            fs::copy(&thumbnail, &thumbnail_backup)?;
        } else {
            let _ = fs::remove_file(&thumbnail_backup);
        }
        fs::write(
            directory.join(format!("{name}.cover-undo.json")),
            serde_json::to_vec(&serde_json::json!({
                "hadCover": had_cover,
                "hadThumbnail": had_thumbnail,
            }))?,
        )?;
        Ok(())
    }

    fn undo_screenshot(root: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !valid_blueprint_name(name) {
            return Err("The selected blueprint name is invalid".into());
        }
        let directory =
            shroudforge_package::paths::export_dir(root).join("world-editor/blueprints");
        if !directory.join(format!("{name}.sfbp")).is_file() {
            return Err("The selected blueprint no longer exists".into());
        }
        let undo_path = directory.join(format!("{name}.cover-undo.json"));
        let undo: serde_json::Value = serde_json::from_slice(&fs::read(&undo_path)?)?;
        for (key, current_name, backup_name) in [
            (
                "hadCover",
                format!("{name}.png"),
                format!("{name}.cover-undo.png"),
            ),
            (
                "hadThumbnail",
                format!("{name}.thumb.png"),
                format!("{name}.cover-undo.thumb.png"),
            ),
        ] {
            let current = directory.join(current_name);
            let backup = directory.join(backup_name);
            if undo[key].as_bool().unwrap_or(false) {
                if !backup.is_file() {
                    return Err("The previous cover backup is missing".into());
                }
                fs::copy(&backup, &current)?;
            } else if current.exists() {
                fs::remove_file(current)?;
            }
            let _ = fs::remove_file(backup);
        }
        fs::remove_file(undo_path)?;
        Ok(())
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
