use super::create_console_window::create_console_window;
use super::open_alert_console::open_for_alert;
use super::open_alert_console::suppress_reopen_until;
use super::parse_console_message::Command;
use super::parse_console_message::parse_console_message;
use super::position_console_window::position_window;
use super::read_console_arguments::arguments;
use super::read_console_settings::Config;
use super::read_console_snapshot::read_console_snapshot;
use super::read_window_events::foreground_process;
use super::read_window_events::open_event;
use super::refresh_console_settings::refresh_console_settings;
use super::render_console_document::render_console_document;
use crate::save_console_preferences::save_console_preferences;
use crate::scan_log_for_alert::scan_log_for_alert;
use shroudforge_window_management::current_window_geometry;
use shroudforge_window_management::save_window_geometry;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;
use tao::event::Event;
use tao::event::StartCause;
use tao::event::WindowEvent;
use tao::event_loop::ControlFlow;
use tao::event_loop::EventLoop;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::System::Threading::GetExitCodeProcess;
use windows_sys::Win32::System::Threading::WaitForSingleObject;
use wry::WebContext;
use wry::WebViewBuilder;

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = arguments()?;
    if let Err(error) = shroudforge_logging::initialize(&arguments.root, false) {
        eprintln!("Could not initialize Debug Console logging: {error}");
    }
    let mut preferences =
        shroudforge_loader_config::config::read_loader(&arguments.root)?["modules"]["debugConsole"]
            .clone();
    if preferences["enabled"] == false && !arguments.startup_watch {
        return Ok(());
    }
    let mut config: Config = serde_json::from_value(preferences.clone())?;
    let stop_event = arguments
        .stop_name
        .as_deref()
        .map(open_event)
        .unwrap_or(std::ptr::null_mut());
    let show_event = arguments
        .show_name
        .as_deref()
        .map(open_event)
        .unwrap_or(std::ptr::null_mut());
    let event_loop = EventLoop::new();
    event_loop.set_device_event_filter(tao::event_loop::DeviceEventFilter::Always);
    if let Err(error) = shroudforge_hotkeys::initialize() {
        let _ = shroudforge_logging::append(&arguments.root, 'W', "hotkeys", &error);
    }
    let window = create_console_window(&event_loop, &arguments)?;
    let mut effective_window_geometry =
        position_window(&window, &preferences["window"]["position"], true);

    let (sender, receiver) = mpsc::channel();
    let handler = move |message: wry::http::Request<String>| {
        let Some(command) = parse_console_message(message.body()) else {
            return;
        };
        let _ = sender.send(command);
    };
    let html = render_console_document();
    // This window runs concurrently with Modloader and World Editor. Give
    // its WebView2 context a private user-data directory so WebView2's
    // profile lock cannot prevent another ShroudForge window from opening.
    let profile =
        shroudforge_loader_config::paths::webview_profile(&arguments.root).join("debug-console");
    std::fs::create_dir_all(&profile)?;
    let mut web_context = WebContext::new(Some(profile));
    let webview = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_html(html)
        .with_ipc_handler(handler)
        .build(&window)?;
    let mut shown = arguments.desktop;
    let mut window_topmost = false;
    let initial_window_state = shroudforge_loader_config::config::window_state(&arguments.root);
    let mut visibility_request_id = initial_window_state["debugConsole"]["requestId"]
        .as_u64()
        .unwrap_or(0);
    let _ = shroudforge_loader_config::config::publish_window_visibility(
        &arguments.root,
        "debugConsole",
        shown,
    );
    let mut previous_toggle_key = config.toggle_key.clone();
    let initial_toggle_press_revision = shroudforge_hotkeys::parse_hotkey(&config.toggle_key)
        .ok()
        .flatten()
        .as_ref()
        .map_or(0, shroudforge_hotkeys::hotkey_press_revision);
    let mut toggle_debouncer =
        shroudforge_hotkeys::WindowToggleDebouncer::new(initial_toggle_press_revision);
    let mut fatal_seen = false;
    let mut failure_review = false;
    let mut game_exited = false;
    let mut game_exit_deadline: Option<Instant> = None;
    let mut show_event_seen = false;
    let mut auto_open_alert = false;
    let mut auto_open_suppressed_until: Option<Instant> = None;
    let mut log_offset = arguments.log_start_offset;
    let mut bootstrap_complete = false;
    let mut bootstrap_popup_shown = false;
    let log_path = shroudforge_loader_config::paths::current_log(&arguments.root);
    let mut user_enabled = preferences["enabled"] != false;
    let mut next_alert_scan = Instant::now();
    let mut next_activation_poll = Instant::now();
    let mut next_visibility_poll = Instant::now();
    let mut next_refresh = Instant::now();
    let mut was_refresh_window_visible = window.is_visible() && !window.is_minimized();
    let mut last_config_error: Option<String> = None;
    let mut pending_window_geometry_save: Option<Instant> = None;
    // Last snapshot handed to the page and the WebView's own visibility: a
    // hidden console neither reads the logs nor renders, and an unchanged
    // snapshot is not sent again (the page rebuilds its whole list for one).
    let mut last_payload = String::new();
    let mut webview_visible = window.is_visible();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50));
        match event {
            Event::NewEvents(StartCause::ResumeTimeReached { .. })
            | Event::NewEvents(StartCause::Init) => {
                if pending_window_geometry_save
                    .is_some_and(|deadline| Instant::now() >= deadline)
                {
                    pending_window_geometry_save = None;
                    if current_window_geometry(&window)
                        .is_some_and(|geometry| geometry != effective_window_geometry)
                    {
                        if let Ok(Some(geometry)) = save_window_geometry(
                            &arguments.root,
                            "debugConsole",
                            &window,
                        ) {
                            preferences["window"]["position"] = geometry.clone();
                            effective_window_geometry = geometry;
                        }
                    }
                }
                if game_exit_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    let _ = shroudforge_loader_config::config::publish_window_visibility(&arguments.root, "debugConsole", false);
                    if !stop_event.is_null() { unsafe { CloseHandle(stop_event) }; }
                    if !show_event.is_null() { unsafe { CloseHandle(show_event) }; }
                    if !arguments.owner_process.is_null() { unsafe { CloseHandle(arguments.owner_process) }; }
                    if !arguments.game_process.is_null() { unsafe { CloseHandle(arguments.game_process) }; }
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                let stop_requested = !stop_event.is_null()
                    && unsafe { WaitForSingleObject(stop_event, 0) } == WAIT_OBJECT_0;
                let owner_exited = !arguments.owner_process.is_null()
                    && unsafe { WaitForSingleObject(arguments.owner_process, 0) } == WAIT_OBJECT_0;
                if stop_requested || owner_exited {
                    let _ = shroudforge_loader_config::config::publish_window_visibility(&arguments.root, "debugConsole", false);
                    if !stop_event.is_null() { unsafe { CloseHandle(stop_event) }; }
                    if !show_event.is_null() { unsafe { CloseHandle(show_event) }; }
                    if !arguments.owner_process.is_null() { unsafe { CloseHandle(arguments.owner_process) }; }
                    if !arguments.game_process.is_null() { unsafe { CloseHandle(arguments.game_process) }; }
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                if Instant::now() >= next_activation_poll {
                    if let Ok(settings) =
                        shroudforge_loader_config::config::read_loader(&arguments.root)
                    {
                        let debug_settings = &settings["modules"]["debugConsole"];
                        if let Some(toggle_key) = debug_settings["toggleKey"].as_str()
                            && config.toggle_key != toggle_key
                        {
                            config.toggle_key = toggle_key.to_owned();
                        }
                        let enabled = debug_settings["enabled"] != false;
                        for key in ["popupMode", "popupCooldownSeconds"] {
                            if let Some(value) = debug_settings.get(key) {
                                preferences[key] = value.clone();
                            }
                        }
                        if enabled != user_enabled {
                            user_enabled = enabled;
                            preferences["enabled"] = serde_json::json!(enabled);
                            if enabled {
                                next_refresh = Instant::now();
                            } else {
                                shown = false;
                                window.set_visible(false);
                                auto_open_alert = false;
                                let _ = shroudforge_loader_config::config::publish_window_visibility(
                                    &arguments.root,
                                    "debugConsole",
                                    false,
                                );
                            }
                        }
                    }
                    next_activation_poll = Instant::now() + Duration::from_secs(1);
                }
                let popup_mode = if user_enabled && arguments.startup_watch {
                    preferences["popupMode"].as_str().unwrap_or("bootstrap")
                } else {
                    "off"
                };
                if Instant::now() >= next_alert_scan {
                    next_alert_scan = Instant::now() + Duration::from_millis(500);
                    if scan_log_for_alert(
                        &log_path,
                        &mut log_offset,
                        popup_mode,
                        &mut bootstrap_complete,
                        &mut bootstrap_popup_shown,
                    ) && user_enabled
                    {
                        fatal_seen = true;
                        failure_review = true;
                        open_for_alert(
                            &window,
                            &arguments.root,
                            &mut shown,
                            &mut auto_open_alert,
                            auto_open_suppressed_until,
                        );
                    }
                }
                if !game_exited && !arguments.game_process.is_null()
                    && unsafe { WaitForSingleObject(arguments.game_process, 0) } == WAIT_OBJECT_0
                {
                    game_exited = true;
                    let mut exit_code = 0;
                    let exit_code_known = unsafe { GetExitCodeProcess(arguments.game_process, &mut exit_code) } != 0;
                    if exit_code_known && exit_code != 0 {
                        let detail = format!("Enshrouded exited unexpectedly with process exit code 0x{exit_code:08X}");
                        let _ = shroudforge_logging::append(&arguments.root, 'E', "bootstrap-watch", &detail);
                        fatal_seen = true;
                        if user_enabled && popup_mode == "all" {
                            open_for_alert(
                                &window,
                                &arguments.root,
                                &mut shown,
                                &mut auto_open_alert,
                                auto_open_suppressed_until,
                            );
                        }
                    }
                    if fatal_seen {
                        // Keep the captured startup error visible briefly, but never
                        // let the debug helper outlive Enshrouded indefinitely.
                        failure_review = true;
                        game_exit_deadline = Some(Instant::now() + Duration::from_secs(2));
                    } else {
                        let _ = shroudforge_loader_config::config::publish_window_visibility(&arguments.root, "debugConsole", false);
                        if !stop_event.is_null() { unsafe { CloseHandle(stop_event) }; }
                        if !show_event.is_null() { unsafe { CloseHandle(show_event) }; }
                        if !arguments.game_process.is_null() { unsafe { CloseHandle(arguments.game_process) }; }
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                }
                if !show_event_seen && !show_event.is_null()
                    && unsafe { WaitForSingleObject(show_event, 0) } == WAIT_OBJECT_0
                {
                    show_event_seen = true;
                    if user_enabled && popup_mode != "off" {
                        if popup_mode == "bootstrap" {
                            bootstrap_popup_shown = true;
                        }
                        fatal_seen = true;
                        failure_review = true;
                        open_for_alert(
                            &window,
                            &arguments.root,
                            &mut shown,
                            &mut auto_open_alert,
                            auto_open_suppressed_until,
                        );
                    }
                }
                while let Ok(command) = receiver.try_recv() {
                    match command {
                        Command::MinimizeWindow => window.set_minimized(true),
                        Command::ToggleMaximizeWindow => {
                            window.set_maximized(!window.is_maximized());
                        }
                        Command::Hide => {
                            auto_open_suppressed_until = suppress_reopen_until(&preferences);
                            shown = false;
                            window.set_visible(false);
                            let _ = shroudforge_loader_config::config::publish_window_visibility(&arguments.root, "debugConsole", shown);
                        }
                        Command::Drag => {
                            let _ = window.drag_window();
                        }
                        Command::Preferences(patch) => save_console_preferences(&arguments.root, patch, &mut next_refresh),
                    }
                }
                if Instant::now() >= next_visibility_poll {
                    let window_state = shroudforge_loader_config::config::window_state(&arguments.root);
                    let requested_id = window_state["debugConsole"]["requestId"].as_u64().unwrap_or(0);
                    if requested_id != visibility_request_id {
                        visibility_request_id = requested_id;
                        let requested_visible = window_state["debugConsole"]["requestedVisible"]
                            .as_bool()
                            .unwrap_or(shown)
                            && user_enabled;
                        if shown && !requested_visible {
                            auto_open_suppressed_until = suppress_reopen_until(&preferences);
                        }
                        shown = requested_visible;
                        if shown {
                            window.set_minimized(false);
                        }
                        window.set_visible(shown);
                        if shown { window.set_focus(); }
                        let _ = shroudforge_loader_config::config::publish_window_visibility(&arguments.root, "debugConsole", shown);
                    }
                    next_visibility_poll = Instant::now() + Duration::from_millis(100);
                }
                let foreground = foreground_process();
                let game_focused = !arguments.desktop && foreground == arguments.game_pid;
                let console_focused = foreground == unsafe { GetCurrentProcessId() };
                let next_topmost = !arguments.desktop
                    && shown
                    && (game_focused || console_focused);
                if next_topmost != window_topmost {
                    window.set_always_on_top(next_topmost);
                    window_topmost = next_topmost;
                }
                let binding = shroudforge_hotkeys::parse_hotkey(&config.toggle_key)
                    .ok()
                    .flatten();
                let press_revision = binding
                    .as_ref()
                    .map_or(0, shroudforge_hotkeys::hotkey_press_revision);
                if config.toggle_key != previous_toggle_key {
                    toggle_debouncer.reset(press_revision);
                    previous_toggle_key = config.toggle_key.clone();
                }
                if toggle_debouncer.should_toggle(press_revision, || {
                    !arguments.desktop
                        && user_enabled
                        && (game_focused || console_focused)
                        && !shroudforge_hotkeys::is_capture_active()
                }) {
                    shown = !shown;
                    if !shown {
                        auto_open_suppressed_until = suppress_reopen_until(&preferences);
                    }
                    if shown {
                        window.set_minimized(false);
                    }
                    window.set_visible(shown);
                    let _ = shroudforge_loader_config::config::publish_window_visibility(&arguments.root, "debugConsole", shown);
                    if shown {
                        window.set_focus();
                    }
                }
                if !user_enabled && shown {
                    shown = false;
                    window.set_visible(false);
                    auto_open_alert = false;
                    let _ = shroudforge_loader_config::config::publish_window_visibility(
                        &arguments.root,
                        "debugConsole",
                        false,
                    );
                }
                if !failure_review && !arguments.desktop && shown && !game_focused && !console_focused {
                    window.set_visible(false);
                } else if shown && (game_focused || console_focused) && !window.is_visible() {
                    window.set_visible(true);
                }
                let refresh_window_visible = window.is_visible() && !window.is_minimized();
                if refresh_window_visible != webview_visible {
                    // Lets WebView2 suspend rendering while the console is hidden.
                    let _ = webview.set_visible(refresh_window_visible);
                    webview_visible = refresh_window_visible;
                }
                if refresh_window_visible && !was_refresh_window_visible {
                    next_refresh = Instant::now();
                }
                was_refresh_window_visible = refresh_window_visible;
                if refresh_window_visible && Instant::now() >= next_refresh {
                    let Some(minimum_level) = refresh_console_settings(&arguments, &window, &mut preferences, &mut effective_window_geometry, &mut config, &mut user_enabled, &mut last_config_error, control_flow) else { return; };
                    next_refresh =
                        Instant::now() + Duration::from_millis(config.refresh_milliseconds);
                    if visible {
                        let payload = read_console_snapshot(&arguments, &config, &preferences, minimum_level, auto_open_alert).to_string();
                        if payload != last_payload
                            && webview
                                .evaluate_script(&format!("window.__shroudforgeUpdate({payload});"))
                                .is_ok()
                        {
                            auto_open_alert = false;
                            last_payload = payload;
                        }
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Moved(_),
                ..
            }
            | Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                if !window.is_maximized() && !window.is_minimized() {
                    pending_window_geometry_save = current_window_geometry(&window)
                        .filter(|geometry| geometry != &effective_window_geometry)
                        .map(|_| Instant::now() + Duration::from_millis(250));
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                auto_open_suppressed_until = suppress_reopen_until(&preferences);
                shown = false;
                window.set_visible(false);
                let _ = shroudforge_loader_config::config::publish_window_visibility(&arguments.root, "debugConsole", shown);
            }
            Event::LoopDestroyed => {
                if current_window_geometry(&window)
                    .is_some_and(|geometry| geometry != effective_window_geometry)
                {
                    let _ = save_window_geometry(&arguments.root, "debugConsole", &window);
                }
            }
            _ => {}
        }
    });
}
