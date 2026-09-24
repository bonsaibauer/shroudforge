#[cfg(windows)]
mod windows {
    use std::{
        fs::{self, File},
        io::{BufRead, BufReader, Read, Write},
        os::windows::ffi::OsStrExt,
        path::{Component, Path, PathBuf},
        sync::{Arc, Mutex, mpsc},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use base64::Engine as _;
    use semver::Version;
    use serde::{Deserialize, Serialize};
    use sha2::{Digest, Sha256, Sha512};
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
            Input::KeyboardAndMouse::GetAsyncKeyState,
            Shell::ShellExecuteW,
            WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
        },
    };
    use wry::{WebContext, WebViewBuilder};

    const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
    const DEFAULT_TOGGLE_KEY: u32 = 120; // F9
    const MAX_UPDATE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
    const MAX_MOD_BYTES: u64 = 512 * 1024 * 1024;
    const MAX_ARCHIVE_ENTRIES: usize = 4096;
    static INSTALL_LOCK: Mutex<()> = Mutex::new(());

    #[derive(Clone, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ProviderConfig {
        #[serde(default)]
        enabled: bool,
        #[serde(default = "default_provider_kind")]
        kind: String,
        #[serde(default = "default_base_url")]
        base_url: String,
        #[serde(default = "default_project_id")]
        project_id: String,
        #[serde(default = "default_loader")]
        loader: String,
        #[serde(default = "default_check_minutes")]
        check_minutes: u64,
    }

    impl Default for ProviderConfig {
        fn default() -> Self {
            Self {
                enabled: false,
                kind: default_provider_kind(),
                base_url: default_base_url(),
                project_id: default_project_id(),
                loader: default_loader(),
                check_minutes: default_check_minutes(),
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ModuleConfig {
        #[serde(default = "default_toggle_key")]
        toggle_key: u32,
        #[serde(default = "default_refresh")]
        refresh_milliseconds: u64,
        #[serde(default)]
        update_provider: ProviderConfig,
        #[serde(default = "default_system_provider")]
        system_update_provider: ProviderConfig,
    }

    #[derive(Clone, Default, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct UserConfig {
        #[serde(default)]
        update_provider: Option<ProviderConfig>,
        #[serde(default)]
        locale: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct UiSettings {
        #[serde(default)]
        module_preferences: Option<serde_json::Value>,
        #[serde(default)]
        compact_mode: bool,
        #[serde(default)]
        reduced_motion: bool,
        log_level: String,
        update_enabled: bool,
        base_url: String,
        project_id: String,
        check_minutes: u64,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct UiCommand {
        #[serde(default)]
        revision: Option<String>,
        command: String,
        #[serde(default)]
        settings: Option<UiSettings>,
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        mod_id: Option<String>,
        #[serde(default)]
        values: Option<serde_json::Value>,
        #[serde(default)]
        query: Option<String>,
        #[serde(default)]
        locale: Option<String>,
        #[serde(default)]
        action: Option<String>,
        #[serde(default)]
        enabled: Option<bool>,
        #[serde(default)]
        ids: Option<Vec<String>>,
        #[serde(default)]
        project_id: Option<String>,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        scope: Option<String>,
        #[serde(default)]
        module: Option<String>,
        #[serde(default)]
        key: Option<String>,
        #[serde(default)]
        value: serde_json::Value,
        #[serde(default)]
        request_id: Option<String>,
        #[serde(default)]
        visible: Option<bool>,
    }

    enum Command {
        Hide,
        Drag,
        Refresh,
        RefreshMod(String, String),
        ReloadSettings(String),
        CheckUpdates(String),
        StageUpdate(String),
        OpenUrl(String),
        SetModEnabled(String, bool, String, String),
        SaveModSettings(String, serde_json::Value, String, String),
        SaveSettings(UiSettings, String),
        SaveSetting(String, Option<String>, String, serde_json::Value, String),
        SetWindowVisibility(String, bool, String),
        SearchCatalog(String, String),
        InstallMod(String, String),
        SaveLanguage(String, String),
        RunModAction(String, String),
        RemoveMod(String),
        MarkNewsRead(Vec<String>),
        Diagnostics(String, String),
        UiReady,
        UiError(String),
    }

    struct AsyncUiResult {
        request_id: String,
        source: String,
        action: String,
        success_message: String,
        result: Result<String, String>,
    }

    struct Arguments {
        root: PathBuf,
        game_pid: u32,
        stop_name: Option<String>,
        standalone: bool,
        server: bool,
    }

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ModInfo {
        revision: String,
        id: String,
        name: String,
        version: String,
        target: String,
        runtime: bool,
        description: Option<String>,
        source: &'static str,
        enabled: bool,
        settings: Vec<serde_json::Value>,
        setting_groups: Vec<serde_json::Value>,
        ui: serde_json::Value,
        changelog: Vec<String>,
        assets: std::collections::HashMap<String, String>,
        setting_values: serde_json::Value,
    }

    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Activity {
        id: String,
        time: String,
        source: String,
        action: String,
        result: String,
        details: Option<String>,
        level: String,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Notice {
        id: String,
        mod_id: String,
        title: String,
        message: String,
        level: String,
        action_url: Option<String>,
        updated_at: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        kind: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        values: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        changelog: Vec<String>,
    }

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ReleaseState {
        current_version: String,
        #[serde(skip)]
        current_build: u64,
        latest_version: Option<String>,
        update_available: bool,
        state: String,
        message: Option<String>,
        release_url: Option<String>,
        staged: bool,
        #[serde(skip)]
        release: Option<Release>,
    }

    impl ReleaseState {
        fn idle(installed: InstalledRelease, staged: bool) -> Self {
            Self {
                current_version: installed.version,
                current_build: installed.build,
                latest_version: None,
                update_available: false,
                state: "idle".into(),
                message: None,
                release_url: None,
                staged,
                release: None,
            }
        }
    }

    #[derive(Clone)]
    struct Release {
        version: String,
        base_version: String,
        build: u64,
        download_url: String,
        checksum: String,
        release_url: Option<String>,
        message: String,
    }

    struct InstalledRelease {
        version: String,
        build: u64,
    }

    #[derive(Deserialize)]
    struct GithubRelease {
        tag_name: String,
        #[serde(default)]
        body: String,
        html_url: String,
        #[serde(default)]
        assets: Vec<GithubAsset>,
    }

    #[derive(Deserialize)]
    struct GithubAsset {
        name: String,
        browser_download_url: String,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Snapshot {
        diagnostics: serde_json::Value,
        windows: serde_json::Value,
        configuration: serde_json::Value,
        connected: bool,
        mode: &'static str,
        game_version: String,
        version: String,
        mods: Vec<ModInfo>,
        activity: Vec<Activity>,
        notices: Vec<Notice>,
        news_templates: serde_json::Value,
        read_notice_ids: Vec<String>,
        release: ReleaseState,
        settings: SnapshotSettings,
        catalog: CatalogState,
        locale: String,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SnapshotSettings {
        config_revision: String,
        module_preferences: serde_json::Value,
        compact_mode: bool,
        reduced_motion: bool,
        log_level: String,
        update_enabled: bool,
        base_url: String,
        project_id: String,
        check_minutes: u64,
    }

    #[derive(Clone, Default, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct CatalogState {
        state: String,
        query: String,
        message: Option<String>,
        items: Vec<CatalogItem>,
    }

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct CatalogItem {
        id: String,
        slug: String,
        name: String,
        summary: String,
        icon_url: Option<String>,
        author: Option<String>,
        downloads: Option<u64>,
        version: Option<String>,
    }

    #[derive(Deserialize)]
    struct ResolveContentPlan {
        primary: ResolvedContent,
        #[serde(default)]
        dependencies: Vec<ResolvedContent>,
    }

    #[derive(Deserialize)]
    struct ResolvedContent {
        project_id: String,
        version_id: String,
    }

    #[derive(Deserialize)]
    struct ApiVersion {
        id: String,
        project_id: String,
        #[serde(rename = "version_number")]
        _version_number: String,
        #[serde(default)]
        files: Vec<ApiVersionFile>,
    }

    #[derive(Deserialize)]
    struct ApiVersionFile {
        url: String,
        filename: String,
        #[serde(default)]
        hashes: std::collections::HashMap<String, String>,
        #[serde(default)]
        primary: bool,
        size: u64,
    }

    fn default_toggle_key() -> u32 {
        DEFAULT_TOGGLE_KEY
    }
    fn default_refresh() -> u64 {
        1000
    }
    fn default_provider_kind() -> String {
        "shroudedit".into()
    }
    fn default_base_url() -> String {
        "https://api.shroudedit.com".into()
    }
    fn default_project_id() -> String {
        "shroudforge".into()
    }
    fn default_loader() -> String {
        "shroudforge".into()
    }
    fn default_check_minutes() -> u64 {
        30
    }

    fn default_system_provider() -> ProviderConfig {
        ProviderConfig {
            enabled: true,
            kind: "github-releases".into(),
            base_url: "https://api.github.com".into(),
            project_id: "bonsaibauer/shroudforge".into(),
            loader: default_loader(),
            check_minutes: 60,
        }
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let arguments = arguments()?;
        match shroudforge_package::migration::migrate_installation(&arguments.root) {
            Ok(errors)=>for error in errors {write_activity(&arguments.root,"mods","Migration","Failed",Some(&error),"error");},
            Err(error)=>write_activity(&arguments.root,"state","Migration","Failed",Some(&error),"error"),
        }
        if let Err(error)=shroudforge_package::news::migrate(&arguments.root) { write_activity(&arguments.root,"news","Migration","Failed",Some(&error),"error"); }
        let mut module_config = load_module_config(&arguments.root).unwrap_or_else(|_| ModuleConfig { toggle_key:default_toggle_key(),refresh_milliseconds:default_refresh(),update_provider:ProviderConfig::default(),system_update_provider:default_system_provider() });
        if shroudforge_package::config::read_loader(&arguments.root).is_ok_and(|value|value["modules"]["modloaderUi"]["enabled"] == false) { return Ok(()); }
        let user_config = load_user_config(&arguments.root);
        let provider = effective_provider(&module_config, &user_config);
        let installed = installed_release(&arguments.root);
        let staged = shroudforge_package::paths::updates_dir(&arguments.root)
            .join("pending.ready")
            .is_file();
        let release_state = Arc::new(Mutex::new(ReleaseState::idle(installed, staged)));
        let catalog_state = Arc::new(Mutex::new(CatalogState {
            state: "idle".into(),
            ..CatalogState::default()
        }));
        let stop_event = arguments
            .stop_name
            .as_deref()
            .map(open_event)
            .unwrap_or(std::ptr::null_mut());
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("ShroudForge | Modloader")
            .with_decorations(false)
            .with_always_on_top(true)
            .with_visible(arguments.standalone)
            .with_inner_size(LogicalSize::new(1180.0, 760.0))
            .with_min_inner_size(LogicalSize::new(860.0, 560.0))
            .build(&event_loop)?;

        let mut saved_position = read_central_config(&arguments.root)["modules"]["modloaderUi"]["window"]["position"].clone();
        position_window(&window, &saved_position, false);
        let (sender, receiver) = mpsc::channel();
        let (async_sender, async_receiver) = mpsc::channel::<AsyncUiResult>();
        let handler = move |message: wry::http::Request<String>| {
            let Ok(value) = serde_json::from_str::<UiCommand>(message.body()) else {
                return;
            };
            let command = match value.command.as_str() {
                "diagnostics" => Command::Diagnostics(value.action.unwrap_or_default(), value.request_id.unwrap_or_default()),
                "hide" => Command::Hide,
                "drag" => Command::Drag,
                "refresh" => Command::Refresh,
                "refresh-mod" => match value.mod_id {
                    Some(id) => Command::RefreshMod(id, value.request_id.unwrap_or_default()),
                    None => return,
                },
                "reload-settings" => Command::ReloadSettings(value.request_id.unwrap_or_default()),
                "check-updates" => Command::CheckUpdates(value.request_id.unwrap_or_default()),
                "stage-update" => Command::StageUpdate(value.request_id.unwrap_or_default()),
                "search-catalog" => match value.query {
                    Some(query) => Command::SearchCatalog(query, value.request_id.unwrap_or_default()),
                    None => return,
                },
                "install-mod" => match value.project_id {
                    Some(project_id) => Command::InstallMod(project_id, value.request_id.unwrap_or_default()),
                    None => return,
                },
                "save-language" => match value.locale {
                    Some(locale) => Command::SaveLanguage(locale, value.request_id.unwrap_or_default()),
                    None => return,
                },
                "open-url" => match value.url {
                    Some(url) => Command::OpenUrl(url),
                    None => return,
                },
                "save-mod-settings" => match (value.mod_id, value.values) {
                    (Some(id), Some(values)) => Command::SaveModSettings(id, values, value.revision.unwrap_or_default(), value.request_id.unwrap_or_default()),
                    _ => return,
                },
                "set-mod-enabled" => match (value.mod_id, value.enabled) {
                    (Some(id), Some(enabled)) => Command::SetModEnabled(id, enabled, value.revision.unwrap_or_default(), value.request_id.unwrap_or_default()),
                    _ => return,
                },
                "save-settings" => match value.settings {
                    Some(settings) => Command::SaveSettings(settings, value.request_id.unwrap_or_default()),
                    None => return,
                },
                "save-setting" => match (value.scope, value.key) {
                    (Some(scope), Some(key)) => Command::SaveSetting(scope, value.module, key, value.value, value.request_id.unwrap_or_default()),
                    _ => return,
                },
                "set-window-visibility" => match (value.module, value.visible) {
                    (Some(module), Some(visible)) => Command::SetWindowVisibility(module, visible, value.request_id.unwrap_or_default()),
                    _ => return,
                },
                "run-mod-action" => match (value.mod_id, value.action) {
                    (Some(id), Some(action)) => Command::RunModAction(id, action),
                    _ => return,
                },
                "remove-mod" => match value.mod_id {
                    Some(id) => Command::RemoveMod(id),
                    None => return,
                },
                "mark-news-read" => Command::MarkNewsRead(value.ids.unwrap_or_default()),
                "ui-ready" => Command::UiReady,
                "ui-error" => Command::UiError(value.message.unwrap_or_else(|| "Unknown WebView error".into())),
                _ => return,
            };
            let _ = sender.send(command);
        };
        let profile = shroudforge_package::paths::webview_profile(&arguments.root);
        fs::create_dir_all(&profile)?;
        let mut web_context = WebContext::new(Some(profile));
        let webview = WebViewBuilder::new_with_web_context(&mut web_context)
            .with_html(include_str!("../ui/dist/index.html"))
            .with_ipc_handler(handler)
            .build(&window)?;

        let mut shown = arguments.standalone;
        let mut previous_configuration_errors = std::collections::HashSet::<String>::new();
        let initial_window_state = shroudforge_package::config::window_state(&arguments.root);
        let mut visibility_request_id = initial_window_state["modloaderUi"]["requestId"].as_u64().unwrap_or(0);
        let _ = shroudforge_package::config::publish_window_visibility(&arguments.root, "modloaderUi", shown);
        if read_central_config(&arguments.root)["modules"]["debugConsole"]["enabled"] == false {
            let _ = shroudforge_package::config::publish_window_visibility(&arguments.root, "debugConsole", false);
        }
        let mut key_down = false;
        let mut next_refresh = Instant::now();
        let mut next_visibility_poll = Instant::now();
        let mut next_window_state_refresh = Instant::now();
        let mut next_update_check = if module_config.system_update_provider.enabled {
            Instant::now()
        } else {
            Instant::now() + Duration::from_secs(86400)
        };
        let mut active_provider = provider;
        let mut system_provider = module_config.system_update_provider.clone();
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50));
            match event {
                Event::NewEvents(StartCause::ResumeTimeReached { .. })
                | Event::NewEvents(StartCause::Init) => {
                    if !stop_event.is_null()
                        && unsafe { WaitForSingleObject(stop_event, 0) } == WAIT_OBJECT_0
                    {
                        let _ = shroudforge_package::config::publish_window_visibility(&arguments.root, "modloaderUi", false);
                        unsafe { CloseHandle(stop_event) };
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    while let Ok(command) = receiver.try_recv() {
                        match command {
                            Command::Hide => {
                                shown = false;
                                window.set_visible(false);
                                let _ = shroudforge_package::config::publish_window_visibility(&arguments.root, "modloaderUi", shown);
                            }
                            Command::Drag => {
                                let _ = window.drag_window();
                            }
                            Command::Refresh => next_refresh = Instant::now(),
                            Command::RefreshMod(id, request_id) => {
                                let result = if read_mods(&arguments.root).iter().any(|mod_info| mod_info.id == id) {
                                    Ok(())
                                } else {
                                    Err(format!("Mod '{id}' is no longer installed"))
                                };
                                report_command_result(&arguments.root, &webview, &request_id, &id, "Refresh mod", result, "mod.refreshed", None, false);
                                next_refresh = Instant::now();
                            }
                            Command::ReloadSettings(request_id) => {
                                write_activity(&arguments.root, "Settings", "Reload settings", "Succeeded", None, "success");
                                notify_command_result(&webview, &request_id, Ok(()), "settings.reloaded", None, false);
                                next_refresh = Instant::now();
                            }
                            Command::CheckUpdates(request_id) => {
                                start_update_check(arguments.root.clone(), system_provider.clone(), release_state.clone(), async_sender.clone(), Some(request_id))
                            }
                            Command::StageUpdate(request_id) => {
                                start_staging(arguments.root.clone(), release_state.clone(), async_sender.clone(), request_id)
                            }
                            Command::SearchCatalog(query, request_id) => start_catalog_search(
                                arguments.root.clone(),
                                active_provider.clone(),
                                query,
                                catalog_state.clone(),
                                async_sender.clone(),
                                request_id,
                            ),
                            Command::InstallMod(project_id, request_id) => start_mod_install(
                                arguments.root.clone(),
                                active_provider.clone(),
                                project_id,
                                async_sender.clone(),
                                request_id,
                            ),
                            Command::SaveLanguage(locale, request_id) => {
                                let result = save_language(&arguments.root, &locale);
                                report_command_result(&arguments.root, &webview, &request_id, "Modloader", "Save language", result, "settings.languageSaved", None, false);
                                next_refresh = Instant::now();
                            }
                            Command::OpenUrl(url) => open_url(&url),
                            Command::SetModEnabled(id, enabled, revision, request_id) => {
                                let result = set_mod_enabled(&arguments.root, &id, enabled, &revision);
                                let action = if enabled { "Enable mod" } else { "Disable mod" };
                                let message = if enabled { "mod.enabledToast" } else { "mod.disabledToast" };
                                report_command_result(&arguments.root, &webview, &request_id, &id, action, result, message, None, false);
                                next_refresh = Instant::now();
                            }
                            Command::SaveModSettings(id, values, revision, request_id) => {
                                let result = save_mod_settings(&arguments.root, &id, &values, &revision);
                                report_command_result(&arguments.root, &webview, &request_id, &id, "Save mod settings", result, "settings.saved", None, false);
                                next_refresh = Instant::now();
                            }
                            Command::SaveSettings(settings, request_id) => {
                                let result = save_settings(&arguments.root, &settings);
                                next_refresh = Instant::now();
                                if result.is_ok() {
                                    active_provider.enabled = settings.update_enabled;
                                    active_provider.base_url = settings.base_url;
                                    active_provider.project_id = settings.project_id;
                                    active_provider.check_minutes =
                                        settings.check_minutes.clamp(5, 1440);
                                    next_refresh = Instant::now();
                                }
                                report_command_result(&arguments.root, &webview, &request_id, "Modloader", "Save settings", result, "settings.saved", None, true);
                            }
                            Command::SaveSetting(scope, module, key, setting, request_id) => {
                                let result = save_setting(&arguments.root, &scope, module.as_deref(), &key, &setting);
                                let target = module.as_deref().map_or_else(|| format!("{scope}.{key}"), |module| format!("{module}.{key}"));
                                report_command_result(&arguments.root, &webview, &request_id, "Settings", &format!("Save {target}"), result, "settings.saved", Some("settings"), true);
                                next_refresh = Instant::now();
                            }
                            Command::SetWindowVisibility(module, visible, request_id) => {
                                let result = if module == "debugConsole" && visible && arguments.standalone {
                                    Err("Debug Console visibility requires a running game loader.".into())
                                } else if module == "debugConsole" && visible
                                    && shroudforge_package::config::read_loader(&arguments.root).is_ok_and(|config| config["modules"]["debugConsole"]["enabled"] == false) {
                                    Err("Debug Console is disabled for game startup.".into())
                                } else {
                                    shroudforge_package::config::request_window_visibility(&arguments.root, &module, visible)
                                };
                                let action = if visible { "Show window" } else { "Hide window" };
                                report_command_result(&arguments.root, &webview, &request_id, &module, action, result, "settings.visibilityRequested", None, false);
                                next_refresh = Instant::now();
                            }
                            Command::RunModAction(id, action) => {
                                let result = queue_mod_action(&arguments.root, &id, &action);
                                report_command_result(&arguments.root, &webview, "", &id, "Queue mod action", result, "mod.actionQueued", None, false);
                                next_refresh = Instant::now();
                            }
                            Command::RemoveMod(id) => {
                                let result = remove_mod(&arguments.root, &id);
                                report_command_result(&arguments.root, &webview, "", &id, "Remove mod", result, "mod.removedToast", None, false);
                                next_refresh = Instant::now();
                            }
                            Command::MarkNewsRead(ids) => {
                                let result = mark_news_read(&arguments.root, &ids);
                                report_command_result(&arguments.root, &webview, "", "Modloader", "Mark notices as read", result, "news.markedReadToast", None, false);
                                next_refresh = Instant::now();
                            }
                            Command::Diagnostics(action, request_id) => {
                                let result=shroudforge_runtime_diagnostics::request(&arguments.root,&action);
                                let message = match action.as_str() { "start" => "settings.diagnosticStarted", "stop" => "settings.diagnosticStopped", _ => "settings.snapshotRequested" };
                                let action_name = match action.as_str() { "start" => "Start diagnostics", "stop" => "Stop diagnostics", _ => "Request diagnostic snapshot" };
                                report_command_result(&arguments.root, &webview, &request_id, "diagnostics", action_name, result, message, None, false);
                                next_refresh=Instant::now();
                            }
                            Command::UiReady => write_activity(&arguments.root, "modloader-ui", "WebView rendered", "Ready", None, "info"),
                            Command::UiError(message) => write_activity(&arguments.root, "modloader-ui", "WebView JavaScript error", "Failed", Some(&message), "error"),
                        }
                    }
                    while let Ok(result) = async_receiver.try_recv() {
                        report_async_result(&arguments.root, &webview, result);
                        next_refresh = Instant::now();
                    }

                    if Instant::now() >= next_visibility_poll {
                        let window_state = shroudforge_package::config::window_state(&arguments.root);
                        let requested_id = window_state["modloaderUi"]["requestId"].as_u64().unwrap_or(0);
                        if requested_id != visibility_request_id {
                            visibility_request_id = requested_id;
                            shown = window_state["modloaderUi"]["requestedVisible"].as_bool().unwrap_or(shown);
                            window.set_visible(shown);
                            if shown { window.set_focus(); }
                            let _ = shroudforge_package::config::publish_window_visibility(&arguments.root, "modloaderUi", shown);
                        }
                        next_visibility_poll = Instant::now() + Duration::from_millis(100);
                    }
                    if Instant::now() >= next_window_state_refresh {
                        let windows = shroudforge_package::config::window_state(&arguments.root);
                        if let Ok(payload) = serde_json::to_string(&windows) {
                            let _ = webview.evaluate_script(&format!("window.__shroudforgeWindowState({payload});"));
                        }
                        next_window_state_refresh = Instant::now() + Duration::from_millis(250);
                    }
                    let foreground = foreground_process();
                    let game_focused = arguments.standalone || foreground == arguments.game_pid;
                    let ui_focused = foreground == unsafe { GetCurrentProcessId() };
                    let down = unsafe { GetAsyncKeyState(module_config.toggle_key as i32) } < 0;
                    if (game_focused || ui_focused) && down && !key_down {
                        shown = !shown;
                        window.set_visible(shown);
                        let _ = shroudforge_package::config::publish_window_visibility(&arguments.root, "modloaderUi", shown);
                        next_refresh = Instant::now();
                        if shown {
                            window.set_focus();
                        }
                    }
                    key_down = down;
                    if !arguments.standalone && shown && !game_focused && !ui_focused {
                        window.set_visible(false);
                    } else if shown && (game_focused || ui_focused) && !window.is_visible() {
                        window.set_visible(true);
                    }

                    if system_provider.enabled && Instant::now() >= next_update_check {
                        start_update_check(arguments.root.clone(), system_provider.clone(), release_state.clone(), async_sender.clone(), None);
                        next_update_check = Instant::now()
                            + Duration::from_secs(
                                system_provider.check_minutes.clamp(5, 1440) * 60,
                            );
                    }
                    if Instant::now() >= next_refresh {
                        let next_position = read_central_config(&arguments.root)["modules"]["modloaderUi"]["window"]["position"].clone();
                        if next_position != saved_position {
                            position_window(&window, &next_position, false);
                            saved_position = next_position;
                        }
                        if let Ok(next) = load_module_config(&arguments.root) {
                            if next.system_update_provider.enabled != system_provider.enabled || next.system_update_provider.check_minutes != system_provider.check_minutes {
                                next_update_check = Instant::now();
                            }
                            system_provider = next.system_update_provider.clone();
                            active_provider = effective_provider(&next, &load_user_config(&arguments.root));
                            module_config = next;
                        }
                        next_refresh = Instant::now()
                            + Duration::from_millis(
                                module_config.refresh_milliseconds.clamp(250, 10_000),
                            );
                        let snapshot =
                            snapshot(&arguments, &active_provider, &release_state, &catalog_state);
                        let current_errors: std::collections::HashSet<String> = snapshot.configuration.get("errors").and_then(serde_json::Value::as_array).into_iter().flatten().filter_map(serde_json::Value::as_str).map(str::to_owned).collect();
                        for error in current_errors.difference(&previous_configuration_errors) {
                            write_activity(&arguments.root, "Configuration", "Configuration issue", "Failed", Some(error), "error");
                        }
                        for error in previous_configuration_errors.difference(&current_errors) {
                            write_activity(&arguments.root, "Configuration", "Configuration issue resolved", "Succeeded", Some(error), "success");
                        }
                        previous_configuration_errors = current_errors;
                        if let Ok(payload) = serde_json::to_string(&snapshot) {
                            let _ = webview.evaluate_script(&format!(
                                "window.__shroudforgeUpdate({payload});"
                            ));
                        }
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::Moved(position),
                    ..
                } => {
                    saved_position = serde_json::json!({"x":position.x,"y":position.y});
                    let _ = shroudforge_package::config::update_loader(&arguments.root, |value| {
                        value["modules"]["modloaderUi"]["window"]["position"] = saved_position.clone();
                        Ok(())
                    });
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    if arguments.standalone {
                        let _ = shroudforge_package::config::publish_window_visibility(&arguments.root, "modloaderUi", false);
                        *control_flow = ControlFlow::Exit;
                    } else {
                        shown = false;
                        window.set_visible(false);
                        let _ = shroudforge_package::config::publish_window_visibility(&arguments.root, "modloaderUi", shown);
                    }
                }
                _ => {}
            }
        });
    }

    fn position_window(window: &tao::window::Window, saved: &serde_json::Value, right: bool) {
        if let Some(monitor) = window.current_monitor() {
            let origin = monitor.position();
            let size = monitor.size();
            let extent = window.outer_size();
            let available_x = size.width.saturating_sub(extent.width) as i32;
            let available_y = size.height.saturating_sub(extent.height) as i32;
            let x = saved["x"].as_i64().unwrap_or((origin.x + if right { available_x.saturating_sub(20) } else { available_x / 2 }) as i64);
            let y = saved["y"].as_i64().unwrap_or((origin.y + available_y / 2) as i64);
            window.set_outer_position(tao::dpi::PhysicalPosition::new(
                (x as i32).clamp(origin.x, origin.x + available_x),
                (y as i32).clamp(origin.y, origin.y + available_y)));
        }
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
        let server = if standalone {
            match value("--target").as_deref() {
                Some("server") => true,
                Some("client") => false,
                Some(_) => return Err("--target must be client or server".into()),
                None => match (root.join("enshrouded.exe").is_file(), root.join("enshrouded_server.exe").is_file()) {
                    (true, false) => false,
                    (false, true) => true,
                    _ => return Err("Select an installation target with --target client or --target server".into()),
                },
            }
        } else {
            use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
            let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, game_pid) };
            if process.is_null() { return Err(std::io::Error::last_os_error().into()); }
            let mut buffer = vec![0u16; 32768];
            let mut length = buffer.len() as u32;
            let success = unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) };
            let error = std::io::Error::last_os_error();
            unsafe { CloseHandle(process); }
            if success == 0 { return Err(error.into()); }
            let executable = PathBuf::from(String::from_utf16(&buffer[..length as usize])?);
            if executable.parent().ok_or("target has no installation directory")?.canonicalize()? != root.canonicalize()? {
                return Err("Target process does not belong to the selected installation".into());
            }
            match executable.file_name().and_then(|name| name.to_str()).map(str::to_ascii_lowercase).as_deref() {
                Some("enshrouded_server.exe") => true,
                Some("enshrouded.exe") => false,
                _ => return Err("Target process is not an Enshrouded client or dedicated server".into()),
            }
        };
        Ok(Arguments {
            root,
            game_pid,
            stop_name,
            standalone,
            server,
        })
    }

    fn load_module_config(root: &Path) -> Result<ModuleConfig, String> {
        let value = shroudforge_package::config::read_loader(root)?;
        let mut config: ModuleConfig = serde_json::from_value(value["modules"]["modloaderUi"].clone()).map_err(|e| e.to_string())?;
        if let Some(enabled) = value.pointer("/modules/updates/system/enabled").and_then(|v| v.as_bool()) { config.system_update_provider.enabled = enabled; }
        if let Some(minutes) = value.pointer("/modules/updates/system/checkMinutes").and_then(|v| v.as_u64()) { config.system_update_provider.check_minutes = minutes; }
        Ok(config)
    }


    fn read_central_config(root: &Path) -> serde_json::Value {
        shroudforge_package::config::read_loader(root).unwrap_or_else(|error| {
            eprintln!("loader configuration: {error}");
            serde_json::json!({"configurationError": error})
        })
    }


    fn load_user_config(root: &Path) -> UserConfig {
        let value = read_central_config(root);
        UserConfig {
            update_provider: value
                .pointer("/catalog/provider")
                .cloned()
                .and_then(|provider| serde_json::from_value(provider).ok()),
            locale: value
                .pointer("/general/locale")
                .and_then(|value| value.as_str())
                .map(str::to_owned),
        }
    }

    fn effective_provider(module: &ModuleConfig, user: &UserConfig) -> ProviderConfig {
        user.update_provider
            .clone()
            .unwrap_or_else(|| module.update_provider.clone())
    }

    fn installed_release(root: &Path) -> InstalledRelease {
        let value = fs::read(shroudforge_package::paths::version_file(root))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        let version = value
            .as_ref()
            .and_then(|value| value.get("version"))
            .and_then(|value| value.as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").into());
        let build = value
            .as_ref()
            .and_then(|value| value.get("build"))
            .and_then(|value| {
                value
                    .as_u64()
                    .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
            })
            .unwrap_or(0);
        InstalledRelease { version, build }
    }

    fn installed_version(root: &Path) -> String {
        installed_release(root).version
    }

    fn snapshot(
        arguments: &Arguments,
        provider: &ProviderConfig,
        release: &Arc<Mutex<ReleaseState>>,
        catalog: &Arc<Mutex<CatalogState>>,
    ) -> Snapshot {
        let central = read_central_config(&arguments.root);
        let logging_level = central.pointer("/logging/minimumLevel").and_then(|v| v.as_str()).unwrap_or("INFO").to_owned();
        let mods = read_mods(&arguments.root);
        sync_mod_events(&arguments.root, &mods);
        Snapshot {
            diagnostics: shroudforge_runtime_diagnostics::status(&arguments.root),
            windows: shroudforge_package::config::window_state(&arguments.root),
            configuration: shroudforge_package::status::configuration(&arguments.root, arguments.server, env!("CARGO_PKG_VERSION")),
            connected: !arguments.standalone,
            mode: if arguments.standalone {
                if arguments.server { "STANDALONE SERVER" } else { "STANDALONE CLIENT" }
            } else if !arguments.server {
                "CLIENT"
            } else {
                "SERVER"
            },
            game_version: read_game_version(&arguments.root),
            version: installed_version(&arguments.root),
            mods,
            activity: read_activity(&arguments.root),
            notices: read_notifications(&arguments.root),
            news_templates: shroudforge_package::news::read(&arguments.root)
                .ok().and_then(|value| value.get("templates").cloned()).unwrap_or_else(|| serde_json::json!({})),
            read_notice_ids: read_news_state(&arguments.root),
            release: release
                .lock()
                .map(|value| value.clone())
                .unwrap_or_else(|_| ReleaseState::idle(installed_release(&arguments.root), false)),
            settings: SnapshotSettings {
                config_revision: config_revision(&central),
                module_preferences: central["modules"].clone(),
                compact_mode: central
                    .pointer("/general/compactMode")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                reduced_motion: central
                    .pointer("/general/reducedMotion")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                log_level: logging_level,
                update_enabled: provider.enabled,
                base_url: provider.base_url.clone(),
                project_id: provider.project_id.clone(),
                check_minutes: provider.check_minutes,
            },
            catalog: catalog
                .lock()
                .map(|value| value.clone())
                .unwrap_or_default(),
            locale: load_user_config(&arguments.root)
                .locale
                .filter(|locale| is_valid_locale(locale))
                .unwrap_or_else(|| "de".into()),
        }
    }

    fn read_game_version(root: &Path) -> String {
        fs::read(shroudforge_package::paths::version_file(root))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| {
                value
                    .get("gameVersion")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "Automatisch erkannt".into())
    }

    fn read_mods(root: &Path) -> Vec<ModInfo> {
        let Ok(entries) = fs::read_dir(root.join("mods")) else {
            return Vec::new();
        };
        let catalog_mods = catalog_installed_mod_ids(root);
        let mut mods = Vec::new();
        for entry in entries.flatten() {
            let package = entry.path();
            let Ok(bytes)=read_package_file(&package,"mod.json") else {continue};
            let revision=shroudforge_package::config::revision(&bytes);
            let Some(value) = shroudforge_package::config::read_manifest_path(root, &package)
                .and_then(|manifest| serde_json::to_value(manifest).map_err(|e| e.to_string()))
                .ok()
            else {
                continue;
            };
            let Some(id) = value.get("id").and_then(|value| value.as_str()) else {
                continue;
            };
            let config=serde_json::json!({"enabled":value["enabled"],"settings":value["settingValues"]});
            let ui = value
                .get("ui")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let assets = read_mod_assets(&package, &ui);
            mods.push(ModInfo {
                revision,
                id: id.into(),
                name: value
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or(id)
                    .into(),
                version: value
                    .get("version")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown")
                    .into(),
                target: value
                    .get("target")
                    .and_then(|value| value.as_str())
                    .unwrap_or("both")
                    .into(),
                runtime: value.get("capabilities").and_then(serde_json::Value::as_array)
                    .is_some_and(|items| items.iter().any(|item| item == "runtime"))
                    && !value.get("capabilities").and_then(serde_json::Value::as_array)
                        .is_some_and(|items| items.iter().any(|item| item == "patch" || item == "assets-write")),
                description: value
                    .get("description")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned),
                source: if catalog_mods.contains(id) {
                    "shroudedit"
                } else {
                    "local"
                },
                enabled: config
                    .get("enabled")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                settings: value
                    .get("settings")
                    .and_then(|value| value.as_array())
                    .cloned()
                    .unwrap_or_default(),
                setting_groups: value
                    .get("settingGroups")
                    .and_then(|value| value.as_array())
                    .cloned()
                    .unwrap_or_default(),
                ui,
                changelog: value
                    .pointer("/release/changelog")
                    .and_then(|value| value.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.as_str().map(str::to_owned))
                            .take(50)
                            .collect()
                    })
                    .unwrap_or_default(),
                assets,
                setting_values: config
                    .get("settings")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({})),
            });
        }
        mods.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        mods
    }


    fn read_mod_assets(
        package: &Path,
        ui: &serde_json::Value,
    ) -> std::collections::HashMap<String, String> {
        fn collect(value: &serde_json::Value, paths: &mut Vec<String>) {
            match value {
                serde_json::Value::Object(object) => {
                    if object.get("type").and_then(|value| value.as_str()) == Some("image") {
                        if let Some(path) = object
                            .get("src")
                            .and_then(|value| value.as_str())
                            .filter(|path| path.starts_with("assets/"))
                        {
                            paths.push(path.to_owned());
                        }
                    }
                    for value in object.values() {
                        collect(value, paths);
                    }
                }
                serde_json::Value::Array(values) => {
                    for value in values {
                        collect(value, paths);
                    }
                }
                _ => {}
            }
        }
        let mut paths = Vec::new();
        collect(ui, &mut paths);
        paths.sort();
        paths.dedup();
        paths
            .into_iter()
            .filter_map(|relative| {
                if relative.contains("..") || relative.contains('\\') {
                    return None;
                }
                let bytes = read_package_file(package, &relative)
                    .ok()
                    .filter(|bytes| bytes.len() <= 2 * 1024 * 1024)?;
                let mime = match Path::new(&relative)
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "webp" => "image/webp",
                    "svg" => "image/svg+xml",
                    _ => return None,
                };
                let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                Some((relative, format!("data:{mime};base64,{encoded}")))
            })
            .collect()
    }

    fn read_package_file(package: &Path, relative: &str) -> Result<Vec<u8>, String> {
        if relative.is_empty()
            || relative.contains("..")
            || relative.contains('\\')
            || Path::new(relative).is_absolute()
        {
            return Err("invalid package path".into());
        }
        if package.is_dir() {
            return fs::read(package.join(relative)).map_err(|error| error.to_string());
        }
        if !package
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
        {
            return Err("unsupported mod package".into());
        }
        let file = File::open(package).map_err(|error| error.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
        let mut entry = archive
            .by_name(relative)
            .map_err(|error| error.to_string())?;
        if entry.size() > 2 * 1024 * 1024 {
            return Err("package file is too large".into());
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        Ok(bytes)
    }

    fn read_activity(root: &Path) -> Vec<Activity> {
        let Ok(file) = File::open(shroudforge_package::paths::current_log(root)) else {
            return Vec::new();
        };
        let mut items = BufReader::new(file).lines().map_while(Result::ok).enumerate().filter_map(|(index, line)| {
            let close = line.find("] [")?;
            let severity = match line.as_bytes().first().copied()? { b'[' => line.chars().nth(1)?, _ => return None };
            if !shroudforge_package::logging::allows(root, severity) { return None; }
            let level = match severity { 'T' | 'D' | 'I' => "info", 'W' => "warn", 'E' => "error", _ => return None };
            let time_text = line.get(3..close)?;
            let source_end = line.get(close + 3..)?.find("] ")? + close + 3;
            let source = line.get(close + 3..source_end)?.to_owned();
            if source != "activity" { return None; }
            let message = line.get(source_end + 2..)?.to_owned();
            let event: serde_json::Value = serde_json::from_str(&message).ok()?;
            let source = event.get("source")?.as_str()?.to_owned();
            let action = event.get("action")?.as_str()?.to_owned();
            let result = event.get("result")?.as_str()?.to_owned();
            let details = event.get("details").and_then(serde_json::Value::as_str).map(str::to_owned);
            Some(Activity { id: format!("{}-{}-{}-{}", index, time_text, source, action), time: time_text.to_owned(), source, action, result, details, level: level.to_owned() })
        }).collect::<Vec<_>>();
        items.reverse();
        items.truncate(100);
        items
    }

    fn write_activity(
        root: &Path,
        source: &str,
        action: &str,
        result: &str,
        details: Option<&str>,
        level: &str,
    ) {
        let severity = match level { "warn" => 'W', "error" => 'E', "debug" => 'D', _ => 'I' };
        let event = serde_json::json!({
            "source": source,
            "action": action,
            "result": result,
            "details": details.map(|value| value.chars().take(500).collect::<String>())
        });
        let _ = shroudforge_package::logging::append_event(root, severity, "activity", &event.to_string());
    }

    fn read_notifications(root: &Path) -> Vec<Notice> {
        let events=shroudforge_package::config::read_document(root,"events-state").unwrap_or_default();
        let news = shroudforge_package::news::read(root)
            .ok()
            .and_then(|value| value.get("messages").and_then(|v| v.as_array()).cloned())
            .unwrap_or_default();
        let mut notices = events.as_object().into_iter().flat_map(|values|values.values().cloned())
            .filter(|value| shroudforge_package::news::validate_event(root, value).is_ok())
            .chain(news)
            .filter_map(|value| {
                Some(Notice {
                    id: value.get("id")?.as_str()?.to_owned(),
                    mod_id: value
                        .get("modId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("shroudforge")
                        .to_owned(),
                    title: value.get("title")?.as_str()?.chars().take(120).collect(),
                    message: value
                        .get("message")?
                        .as_str()?
                        .chars()
                        .take(2_000)
                        .collect(),
                    level: value
                        .get("level")
                        .and_then(|value| value.as_str())
                        .unwrap_or("info")
                        .to_owned(),
                    action_url: value
                        .get("actionUrl")
                        .and_then(|value| value.as_str())
                        .filter(|url| url.starts_with("https://"))
                        .map(str::to_owned),
                    updated_at: value
                        .get("updatedAt")
                        .and_then(|value| value.as_u64())
                        .unwrap_or_default(),
                    kind: value
                        .get("kind")
                        .and_then(|value| value.as_str())
                        .map(str::to_owned),
                    values: value.get("values").cloned(),
                    changelog: value
                        .get("changelog")
                        .and_then(|value| value.as_array())
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| item.as_str().map(str::to_owned))
                                .take(50)
                                .collect()
                        })
                        .unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>();
        notices.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        notices.truncate(100);
        notices
    }

    fn valid_identifier(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 80
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    }

    fn valid_news_id(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 200
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'+')
            })
    }


    fn read_news_state(root: &Path) -> Vec<String> {
        shroudforge_package::news::read_ids(root).unwrap_or_else(|error| {
            eprintln!("news state: {error}");
            Vec::new()
        })
    }

    fn mark_news_read(root: &Path, ids: &[String]) -> Result<(), String> {
        let ids = ids.iter().filter(|id| valid_news_id(id)).cloned().collect::<Vec<_>>();
        shroudforge_package::news::mark_read(root, &ids)
    }

    fn sync_mod_events(root: &Path, mods: &[ModInfo]) {
        let current = mods
            .iter()
            .map(|item| {
                (
                    item.id.clone(),
                    serde_json::json!({
                        "name": item.name,
                        "version": item.version,
                        "changelog": item.changelog
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let previous = shroudforge_package::config::read_document(root,"mod-state")
            .ok().and_then(|value|value.as_object().cloned());
        let Some(previous) = previous else {
            let _=shroudforge_package::config::update_state_section(root,"mods", |_| Ok(serde_json::Value::Object(current)));
            return;
        };
        for (id, item) in &current {
            let name = item
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or(id);
            let version = item
                .get("version")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            match previous.get(id) {
                None => write_event_notice(
                    root,
                    &format!("mod-install-{id}-{version}"),
                    "mod.install.success",
                    "success",
                    name,
                    version,
                    None,
                    item.get("changelog"),
                ),
                Some(old) if old.get("version") != item.get("version") => {
                    let old_version = old
                        .get("version")
                        .and_then(|value| value.as_str())
                        .unwrap_or("unknown");
                    write_event_notice(
                        root,
                        &format!("mod-update-{id}-{version}"),
                        "mod.update.success",
                        "update",
                        name,
                        version,
                        Some(old_version),
                        item.get("changelog"),
                    );
                }
                _ => {}
            }
        }
        for (id, item) in &previous {
            if !current.contains_key(id) {
                let name = item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or(id);
                let version = item
                    .get("version")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown");
                write_event_notice(
                    root,
                    &format!("mod-remove-{id}-{version}"),
                    "mod.remove.success",
                    "info",
                    name,
                    version,
                    None,
                    None,
                );
            }
        }
        let _=shroudforge_package::config::update_state_section(root,"mods", |_| Ok(serde_json::Value::Object(current)));
    }

    fn write_event_notice(
        root: &Path,
        id: &str,
        kind: &str,
        level: &str,
        name: &str,
        version: &str,
        previous_version: Option<&str>,
        changelog: Option<&serde_json::Value>,
    ) {
        let values = serde_json::json!({
            "name": name,
            "version": version,
            "previousVersion": previous_version
        });
        let payload = serde_json::json!({
            "id": id,
            "modId": "shroudforge.modloader",
            "title": "",
            "message": "",
            "level": level,
            "kind": kind,
            "values": values,
            "changelog": changelog.cloned().unwrap_or_else(|| serde_json::json!([])),
            "updatedAt": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
        });
        if let Err(error) = shroudforge_package::news::write_event(root, &format!("event-{id}"), &payload) {
            write_activity(root, "news", "Publish notification", "Failed", Some(&error), "error");
        }
    }

    fn queue_mod_action(root: &Path, id: &str, action: &str) -> Result<(), String> {
        if !valid_identifier(id) || !valid_identifier(action) {
            return Err("invalid mod action".into());
        }
        let package = find_mod_package(root, id).ok_or("mod package not found")?;
        let manifest = read_package_file(&package, "mod.json")?;
        let manifest = serde_json::from_slice::<serde_json::Value>(&manifest)
            .map_err(|error| error.to_string())?;
        let declared = manifest
            .pointer("/shroudforge/ui")
            .is_some_and(|ui| ui_declares_action(ui, action));
        if !declared {
            return Err("mod action is not declared by the package".into());
        }
        let actions = shroudforge_package::paths::ui_data_dir(root).join("actions");
        fs::create_dir_all(&actions).map_err(|error| error.to_string())?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        write_json(
            &actions.join(format!(
                "{}-{}-{}.json",
                id,
                timestamp.as_nanos(),
                std::process::id()
            )),
            &serde_json::json!({
                "modId": id,
                "action": action,
                "createdAt": timestamp.as_secs()
            }),
        )
    }

    fn ui_declares_action(ui: &serde_json::Value, action: &str) -> bool {
        fn sections_declare(sections: Option<&Vec<serde_json::Value>>, action: &str) -> bool {
            sections.is_some_and(|sections| {
                sections.iter().any(|section| {
                    section
                        .get("components")
                        .and_then(|value| value.as_array())
                        .is_some_and(|components| {
                            components.iter().any(|component| {
                                component.get("type").and_then(|value| value.as_str())
                                    == Some("button")
                                    && component.get("action").and_then(|value| value.as_str())
                                        == Some(action)
                            })
                        })
                })
            })
        }
        if sections_declare(
            ui.get("sections").and_then(|value| value.as_array()),
            action,
        ) {
            return true;
        }
        ui.get("tabs")
            .and_then(|value| value.as_array())
            .is_some_and(|tabs| {
                tabs.iter().any(|tab| {
                    sections_declare(
                        tab.get("sections").and_then(|value| value.as_array()),
                        action,
                    )
                })
            })
    }

    fn remove_mod(root: &Path, id: &str) -> Result<(), String> {
        if !valid_identifier(id) {
            return Err("invalid mod id".into());
        }
        let source = find_mod_package(root, id).ok_or("mod package not found")?;
        let trash = shroudforge_package::paths::ui_data_dir(root).join("trash");
        fs::create_dir_all(&trash).map_err(|error| error.to_string())?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let suffix = source.extension().and_then(|value| value.to_str());
        let destination = trash.join(match suffix {
            Some(extension) if source.is_file() => format!("{id}-{timestamp}.{extension}"),
            _ => format!("{id}-{timestamp}"),
        });
        fs::rename(source, destination).map_err(|error| error.to_string())?;
        forget_catalog_install(root, id)
    }

    fn save_mod_settings(root: &Path, id: &str, values: &serde_json::Value, revision: &str) -> Result<(), String> {
        let package = find_mod_package(root, id).ok_or("mod package not found")?;
        let manifest = shroudforge_package::config::read_manifest_path(root, &package)?;
        shroudforge_package::config::validate_settings(&manifest.settings, values)?;
        shroudforge_package::config::update_mod_config_revision(root, id, Some(revision), |config| {
            config["settings"] = values.clone();
            Ok(())
        })
    }

    fn set_mod_enabled(root: &Path, id: &str, enabled: bool, revision: &str) -> Result<(), String> {
        if find_mod_package(root, id).is_none() {
            return Err("mod not found".into());
        }
        shroudforge_package::config::update_mod_config_revision(root, id, Some(revision), |config| {
            config["enabled"] = enabled.into();
            Ok(())
        })
    }

    fn find_mod_package(root: &Path, id: &str) -> Option<PathBuf> {
        fs::read_dir(root.join("mods"))
            .ok()?
            .flatten()
            .find_map(|entry| {
                let package = entry.path();
                let manifest = read_package_file(&package, "mod.json").ok()?;
                let value = serde_json::from_slice::<serde_json::Value>(&manifest).ok()?;
                (value.get("id").and_then(|value| value.as_str()) == Some(id)).then_some(package)
            })
    }

    fn config_revision(value: &serde_json::Value) -> String {
        shroudforge_package::config::revision(&serde_json::to_vec(value).expect("JSON value is serializable"))
    }

    fn notify_command_result(webview: &wry::WebView, request_id: &str, result: Result<(), String>, success_message: &str, batch_key: Option<&str>, restore_settings: bool) {
        let (success, message) = match result {
            Ok(()) => (true, success_message.to_owned()),
            Err(error) => (false, error),
        };
        let payload = serde_json::json!({"requestId":request_id,"success":success,"message":message,"batchKey":batch_key,"restoreSettings":restore_settings});
        if let Ok(payload) = serde_json::to_string(&payload) {
            let _ = webview.evaluate_script(&format!("window.__shroudforgeCommandResult({payload});"));
        }
    }

    fn report_command_result(
        root: &Path,
        webview: &wry::WebView,
        request_id: &str,
        source: &str,
        action: &str,
        result: Result<(), String>,
        success_message: &str,
        batch_key: Option<&str>,
        restore_settings: bool,
    ) {
        let (activity_result, details, level) = match &result {
            Ok(()) => ("Succeeded", None, "success"),
            Err(error) => ("Failed", Some(error.as_str()), "error"),
        };
        write_activity(root, source, action, activity_result, details, level);
        notify_command_result(webview, request_id, result, success_message, batch_key, restore_settings);
    }

    fn report_async_result(root: &Path, webview: &wry::WebView, command: AsyncUiResult) {
        let (result, activity_result, details, level) = match command.result {
            Ok(details) => (Ok(()), "Succeeded", Some(details), "success"),
            Err(error) => (Err(error.clone()), "Failed", Some(error), "error"),
        };
        write_activity(root, &command.source, &command.action, activity_result, details.as_deref(), level);
        notify_command_result(webview, &command.request_id, result, &command.success_message, None, false);
    }

    fn save_setting(root: &Path, scope: &str, module: Option<&str>, key: &str, setting: &serde_json::Value) -> Result<(), String> {
        if scope == "general" {
            if !matches!(key, "compactMode" | "reducedMotion") { return Err("unsupported general setting".into()); }
            return shroudforge_package::config::update_loader(root, |value| {
                value["general"][key] = setting.clone();
                Ok(())
            });
        }
        if scope == "logging" {
            if key != "minimumLevel" { return Err("unsupported logging setting".into()); }
            return shroudforge_package::config::update_loader(root, |value| {
                value["logging"][key] = setting.clone();
                Ok(())
            });
        }
        if scope != "module" { return Err("unsupported settings scope".into()); }
        let module = module.ok_or("missing module setting target")?;
        let valid = match module {
            "debugConsole" => matches!(key, "defaultSource" | "levelFilter" | "autoScroll" | "toggleKey" | "refreshMilliseconds" | "tailBytes") || key == "window.position",
            "modloaderUi" => matches!(key, "startPage" | "toggleKey" | "refreshMilliseconds") || key == "window.position",
            "runtimeDiagnostics" => matches!(key, "intervalMilliseconds" | "maximumDurationSeconds" | "slowCallbackMilliseconds" | "onlyChanges" | "areas"),
            "updates.system" => matches!(key, "enabled" | "checkMinutes" | "channel"),
            _ => false,
        };
        if !valid { return Err("unsupported module setting".into()); }
        shroudforge_package::config::update_loader(root, |config| {
            if key == "window.position" {
                config["modules"][module]["window"]["position"] = setting.clone();
            } else if module == "updates.system" {
                config["modules"]["updates"]["system"][key] = setting.clone();
            } else {
                config["modules"][module][key] = setting.clone();
            }
            Ok(())
        })
    }

    fn save_settings(root: &Path, settings: &UiSettings) -> Result<(), String> {
        let provider = ProviderConfig {
            enabled: settings.update_enabled,
            kind: default_provider_kind(),
            base_url: settings.base_url.trim_end_matches('/').to_owned(),
            project_id: settings.project_id.trim().to_owned(),
            loader: default_loader(),
            check_minutes: settings.check_minutes.clamp(5, 1440),
        };
        if provider.base_url.is_empty() || provider.project_id.is_empty() {
            return Err("update provider fields must not be empty".into());
        }
        shroudforge_package::config::update_loader(root, |value| {
        if let Some(preferences) = &settings.module_preferences {
            for (module, keys) in [
                ("debugConsole", &["defaultSource", "levelFilter", "autoScroll", "toggleKey", "refreshMilliseconds", "tailBytes"][..]),
                ("modloaderUi", &["startPage", "toggleKey", "refreshMilliseconds"][..]),
                ("runtimeDiagnostics", &["intervalMilliseconds", "maximumDurationSeconds", "slowCallbackMilliseconds", "onlyChanges", "areas"][..]),
            ] {
                for key in keys {
                    if let Some(setting) = preferences.get(module).and_then(|value| value.get(*key)) {
                        value["modules"][module][*key] = setting.clone();
                    }
                }
                if let Some(position) = preferences.get(module).and_then(|value| value.pointer("/window/position")) {
                    value["modules"][module]["window"]["position"] = position.clone();
                }
            }
            for key in ["enabled", "checkMinutes", "channel"] {
                if let Some(setting) = preferences.pointer(&format!("/updates/system/{key}")) {
                    value["modules"]["updates"]["system"][key] = setting.clone();
                }
            }
        }
        let object = value
            .as_object_mut()
            .ok_or("shroudforge/config/shroudforge.json must contain an object")?;
        object.insert("logging".into(), serde_json::json!({ "minimumLevel": &settings.log_level }));
        object.insert(
            "catalog".into(),
            serde_json::json!({ "provider": provider }),
        );
        let general = object
            .entry("general")
            .or_insert_with(|| serde_json::json!({}));
        let general = general
            .as_object_mut()
            .ok_or("general config must be an object")?;
        general.insert("compactMode".into(), settings.compact_mode.into());
        general.insert("reducedMotion".into(), settings.reduced_motion.into());
        Ok(())
        })
    }

    fn save_language(root: &Path, locale: &str) -> Result<(), String> {
        if !is_valid_locale(locale) {
            return Err("invalid locale".into());
        }
        shroudforge_package::config::update_loader(root, |value| {
        let object = value
            .as_object_mut()
            .ok_or("central config must be an object")?;
        let general = object
            .entry("general")
            .or_insert_with(|| serde_json::json!({}));
        let general = general
            .as_object_mut()
            .ok_or("general config must be an object")?;
        general.insert(
            "locale".into(),
            serde_json::Value::String(locale.to_owned()),
        );
        Ok(())
        })
    }

    fn is_valid_locale(locale: &str) -> bool {
        (2..=35).contains(&locale.len())
            && !locale.starts_with('-')
            && !locale.ends_with('-')
            && !locale.contains("--")
            && locale
                .bytes()
                .all(|character| character.is_ascii_alphanumeric() || character == b'-')
    }

    fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
        let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
        shroudforge_package::config::write_json(path, &value)
    }

    fn start_catalog_search(
        root: PathBuf,
        provider: ProviderConfig,
        query: String,
        state: Arc<Mutex<CatalogState>>,
        results: mpsc::Sender<AsyncUiResult>,
        request_id: String,
    ) {
        let query = query.trim().chars().take(100).collect::<String>();
        if let Ok(mut value) = state.lock() {
            value.query = query.clone();
            value.items.clear();
            value.message = None;
            if !provider.enabled {
                let error = "ShroudEdit access is disabled.".to_owned();
                value.state = "error".into();
                value.message = Some(error.clone());
                let _ = results.send(AsyncUiResult { request_id, source: "ShroudEdit".into(), action: "Search catalog".into(), success_message: "catalog.searchFailed".into(), result: Err(error) });
                return;
            }
            value.state = "loading".into();
        }
        thread::spawn(move || {
            let result = fetch_catalog(&provider, &query);
            let mut failure = None;
            let mut success_detail = None;
            if let Ok(mut value) = state.lock() {
                match result {
                    Ok(items) => {
                        success_detail = Some(format!("Catalog search completed: {} result(s).", items.len()));
                        value.state = "ready".into();
                        value.message = if items.is_empty() {
                            Some("No matching ShroudForge mods found.".into())
                        } else {
                            None
                        };
                        value.items = items;
                    }
                    Err(error) => {
                        value.state = "error".into();
                        value.message = Some(error.clone());
                        failure = Some(error);
                    }
                }
            }
            if let Some(detail) = success_detail {
                write_activity(&root, "ShroudEdit", "Search catalog", "Succeeded", Some(&detail), "info");
            }
            if let Some(error) = failure {
                let _ = results.send(AsyncUiResult { request_id, source: "ShroudEdit".into(), action: "Search catalog".into(), success_message: "catalog.searchFailed".into(), result: Err(error) });
            }
        });
    }

    fn fetch_catalog(provider: &ProviderConfig, query: &str) -> Result<Vec<CatalogItem>, String> {
        if provider.kind != "shroudedit" {
            return Err(format!("unsupported catalog provider: {}", provider.kind));
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .user_agent(format!(
                "ShroudForge/{} (catalog)",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .map_err(|error| error.to_string())?;
        let response = client
            .get(format!(
                "{}/v3/search",
                provider.base_url.trim_end_matches('/')
            ))
            .query(&[("query", query), ("limit", "30"), ("loader", "shroudforge")])
            .send()
            .map_err(|error| format!("ShroudEdit is unreachable: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "ShroudEdit returned HTTP {}",
                response.status()
            ));
        }
        let payload: serde_json::Value = response
            .json()
            .map_err(|error| format!("Invalid ShroudEdit response: {error}"))?;
        let values = payload
            .as_array()
            .or_else(|| payload.get("hits").and_then(|value| value.as_array()))
            .or_else(|| payload.get("projects").and_then(|value| value.as_array()))
            .or_else(|| payload.get("data").and_then(|value| value.as_array()))
            .ok_or("ShroudEdit search response contains no result list")?;
        let mut items = Vec::new();
        for value in values {
            let loaders = value
                .get("loaders")
                .and_then(|value| value.as_array())
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if !loaders.is_empty()
                && !loaders
                    .iter()
                    .any(|loader| loader.eq_ignore_ascii_case("shroudforge"))
            {
                continue;
            }
            let id = string_field(value, &["project_id", "projectId", "id"]);
            let slug = string_field(value, &["slug", "project_slug"]);
            let name = string_field(value, &["name", "title"]);
            if id.is_empty() || name.is_empty() {
                continue;
            }
            items.push(CatalogItem {
                id,
                slug: if slug.is_empty() {
                    string_field(value, &["project_id", "projectId", "id"])
                } else {
                    slug
                },
                name,
                summary: string_field(value, &["summary", "description"]),
                icon_url: optional_string_field(value, &["icon_url", "iconUrl"]),
                author: optional_string_field(value, &["author", "owner_name", "ownerName"]),
                downloads: value
                    .get("downloads")
                    .or_else(|| value.get("download_count"))
                    .and_then(|value| value.as_u64()),
                version: optional_string_field(
                    value,
                    &["latest_version", "latestVersion", "version"],
                ),
            });
        }
        Ok(items)
    }

    fn start_mod_install(root: PathBuf, provider: ProviderConfig, project_id: String, results: mpsc::Sender<AsyncUiResult>, request_id: String) {
        if !valid_identifier(&project_id) {
            let _ = results.send(AsyncUiResult { request_id, source: "ShroudEdit".into(), action: "Install mod".into(), success_message: "mod.install.succeeded".into(), result: Err("Invalid project ID".into()) });
            return;
        }
        write_activity(
            &root,
            "ShroudEdit",
            "Install mod",
            "Started",
            Some(&project_id),
            "info",
        );
        thread::spawn(move || {
            let result = match INSTALL_LOCK.try_lock() {
                Ok(_guard) => install_mod_from_catalog(&root, &provider, &project_id),
                Err(_) => Err("A mod installation is already in progress".to_owned()),
            };
            let result = match result {
                Ok(installed) => Ok(format!("Installed {} package(s)", installed.len())),
                Err(error) => Err(error),
            };
            let _ = results.send(AsyncUiResult { request_id, source: "ShroudEdit".into(), action: "Install mod".into(), success_message: "mod.install.succeeded".into(), result });
        });
    }

    fn install_mod_from_catalog(
        root: &Path,
        provider: &ProviderConfig,
        project_id: &str,
    ) -> Result<Vec<String>, String> {
        if !provider.enabled || provider.kind != "shroudedit" {
            return Err("ShroudEdit access is disabled".into());
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent(format!(
                "ShroudForge/{} (mod-installer)",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .map_err(|error| error.to_string())?;
        let registry = read_catalog_install_registry(root);
        let existing_project_ids = registry
            .get("projects")
            .and_then(|value| value.as_object())
            .map(|projects| projects.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let response = client
            .post(format!(
                "{}/v3/content/resolve",
                provider.base_url.trim_end_matches('/')
            ))
            .json(&serde_json::json!({
                "project_id": project_id,
                "version_id": null,
                "content_type": "mod",
                "selected": { "game_versions": [], "loaders": [provider.loader] },
                "target": { "game_versions": [], "loaders": [provider.loader] },
                "existing_project_ids": existing_project_ids
            }))
            .send()
            .map_err(|error| format!("ShroudEdit is unreachable: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "ShroudEdit resolution returned HTTP {}",
                response.status()
            ));
        }
        let plan: ResolveContentPlan = response
            .json()
            .map_err(|error| format!("Invalid installation plan: {error}"))?;
        let mut resolved = plan.dependencies;
        resolved.push(plan.primary);
        let mut installed_paths = Vec::new();
        let mut installed = Vec::new();
        let result = (|| {
            for item in resolved {
                let installed_mod = download_catalog_version(root, provider, &client, &item)?;
                installed_paths.push(installed_mod.1.clone());
                remember_catalog_install(
                    root,
                    &item.project_id,
                    &item.version_id,
                    &installed_mod.0,
                    &installed_mod.1,
                )?;
                installed.push(installed_mod.0);
            }
            Ok(())
        })();
        if let Err(error) = result {
            for path in installed_paths {
                // Failed multi-package installs remain recoverable in the loader trash.
                let trash = shroudforge_package::paths::ui_data_dir(root).join("trash");
                let _ = fs::create_dir_all(&trash);
                if let Some(name) = path.file_name() {
                    let destination = trash.join(format!("failed-{}-{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(), name.to_string_lossy()));
                    let _ = fs::rename(&path, destination);
                }
            }
            for mod_id in &installed {
                let _ = forget_catalog_install(root, mod_id);
            }
            return Err(error);
        }
        Ok(installed)
    }

    fn download_catalog_version(
        root: &Path,
        provider: &ProviderConfig,
        client: &reqwest::blocking::Client,
        resolved: &ResolvedContent,
    ) -> Result<(String, PathBuf), String> {
        let response = client
            .get(format!(
                "{}/v3/version/{}",
                provider.base_url.trim_end_matches('/'),
                resolved.version_id
            ))
            .send()
            .map_err(|error| format!("Could not load version metadata: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Version metadata endpoint returned HTTP {}",
                response.status()
            ));
        }
        let version: ApiVersion = response
            .json()
            .map_err(|error| format!("Invalid version metadata: {error}"))?;
        if version.id != resolved.version_id || version.project_id != resolved.project_id {
            return Err("Version metadata does not belong to the resolved project".into());
        }
        let file = version
            .files
            .iter()
            .find(|file| file.primary)
            .or_else(|| version.files.first())
            .ok_or("Version contains no mod package")?;
        if !file.filename.to_ascii_lowercase().ends_with(".zip")
            || !file.url.starts_with("https://")
            || file.size > MAX_MOD_BYTES
        {
            return Err("Mod package is not a valid ZIP archive".into());
        }
        let response = client
            .get(&file.url)
            .send()
            .map_err(|error| format!("Could not read mod package: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Mod download returned HTTP {}",
                response.status()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_MOD_BYTES)
        {
            return Err("Mod package exceeds 512 MiB".into());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_MOD_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_MOD_BYTES {
            return Err("Mod package exceeds 512 MiB".into());
        }
        verify_mod_hash(&bytes, &file.hashes)?;

        let downloads = shroudforge_package::paths::ui_data_dir(root).join("downloads");
        fs::create_dir_all(&downloads).map_err(|error| error.to_string())?;
        let temporary = downloads.join(format!(
            "{}-{}-{}.zip",
            resolved.project_id,
            resolved.version_id,
            std::process::id()
        ));
        fs::write(&temporary, &bytes).map_err(|error| error.to_string())?;
        let manifest = shroudforge_package::config::read_manifest_path(root, &temporary);
        let manifest = match manifest {
            Ok(manifest) => manifest,
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                return Err(format!("Invalid mod.json: {error}"));
            }
        };
        if let Err(error) = shroudforge_package::validate_manifest(&manifest) {
            let _ = fs::remove_file(&temporary);
            return Err(format!("Invalid mod manifest: {error}"));
        }
        if find_mod_package(root, &manifest.id).is_some() {
            let _ = fs::remove_file(&temporary);
            return Err(format!("Mod '{}' is already installed", manifest.name));
        }
        let mods = root.join("mods");
        fs::create_dir_all(&mods).map_err(|error| error.to_string())?;
        let staging = downloads.join(format!("{}-{}-extract", manifest.id, SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()));
        fs::create_dir(&staging).map_err(|error| error.to_string())?;
        extract_zip(&temporary, &staging)?;
        let extracted = shroudforge_package::config::read_manifest_path(root, &staging)?;
        if extracted.id != manifest.id || !staging.join("src/mod.lua").is_file() {
            return Err("extracted mod must have the same id and a src/mod.lua entrypoint".into());
        }
        let destination = mods.join(&manifest.id);
        if destination.exists() { return Err("mod destination already exists".into()); }
        fs::rename(&staging, &destination).map_err(|error| error.to_string())?;
        Ok((manifest.id, destination))
    }

    fn verify_mod_hash(
        bytes: &[u8],
        hashes: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        let expected = hashes
            .get("sha512")
            .map(|value| ("sha512", value))
            .or_else(|| hashes.get("sha256").map(|value| ("sha256", value)))
            .ok_or("Mod package has no SHA-512 or SHA-256 checksum")?;
        let actual = match expected.0 {
            "sha512" => format!("{:x}", Sha512::digest(bytes)),
            _ => format!("{:x}", Sha256::digest(bytes)),
        };
        if !actual.eq_ignore_ascii_case(expected.1) {
            return Err("Mod package checksum does not match".into());
        }
        Ok(())
    }

    fn read_catalog_install_registry(root: &Path) -> serde_json::Value {
        shroudforge_package::config::read_document(root,"catalog-state")
            .unwrap_or_else(|_|serde_json::json!({ "schemaVersion": 1, "projects": {} }))
    }

    fn catalog_installed_mod_ids(root: &Path) -> std::collections::HashSet<String> {
        read_catalog_install_registry(root)
            .get("projects")
            .and_then(|value| value.as_object())
            .into_iter()
            .flat_map(|projects| projects.values())
            .filter_map(|value| value.get("modId").and_then(|value| value.as_str()))
            .map(str::to_owned)
            .collect()
    }

    fn remember_catalog_install(
        root: &Path,
        project_id: &str,
        version_id: &str,
        mod_id: &str,
        package: &Path,
    ) -> Result<(), String> {
        shroudforge_package::config::update_state_section(root,"catalog",|stored| {
            let mut registry=stored.cloned().unwrap_or_else(||serde_json::json!({"schemaVersion":1,"projects":{}}));
            registry["projects"][project_id]=serde_json::json!({"modId":mod_id,"versionId":version_id,"file":package.file_name().and_then(|value|value.to_str()).unwrap_or_default()});
            shroudforge_package::config::validate_document(root,"catalog-state",&registry)?;
            Ok(registry)
        })
    }

    fn forget_catalog_install(root: &Path, mod_id: &str) -> Result<(), String> {
        shroudforge_package::config::update_state_section(root,"catalog",|stored| {
            let mut registry=stored.cloned().unwrap_or_else(||serde_json::json!({"schemaVersion":1,"projects":{}}));
            registry["projects"].as_object_mut().ok_or("invalid catalog install registry")?.retain(|_,value|value.get("modId").and_then(|value|value.as_str())!=Some(mod_id));
            shroudforge_package::config::validate_document(root,"catalog-state",&registry)?;
            Ok(registry)
        })
    }

    fn string_field(value: &serde_json::Value, names: &[&str]) -> String {
        optional_string_field(value, names).unwrap_or_default()
    }

    fn optional_string_field(value: &serde_json::Value, names: &[&str]) -> Option<String> {
        names.iter().find_map(|name| {
            value
                .get(name)
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        })
    }

    fn start_update_check(root: PathBuf, provider: ProviderConfig, state: Arc<Mutex<ReleaseState>>, results: mpsc::Sender<AsyncUiResult>, request_id: Option<String>) {
        if let Ok(mut value) = state.lock() {
            if value.state == "checking" || value.state == "downloading" {
                if let Some(request_id) = request_id {
                    let _ = results.send(AsyncUiResult { request_id, source: "Updates".into(), action: "Check for updates".into(), success_message: "updates.check.upToDate".into(), result: Err("An update operation is already in progress.".into()) });
                }
                return;
            }
            if !provider.enabled {
                value.state = "idle".into();
                value.message = Some("GitHub system updates are disabled.".into());
                if let Some(request_id) = request_id {
                    let _ = results.send(AsyncUiResult { request_id, source: "Updates".into(), action: "Check for updates".into(), success_message: "updates.check.upToDate".into(), result: Err("System update checks are disabled in settings.".into()) });
                }
                return;
            }
            value.state = "checking".into();
            value.message = Some("Checking releases…".into());
        } else {
            if let Some(request_id) = request_id {
                let _ = results.send(AsyncUiResult { request_id, source: "Updates".into(), action: "Check for updates".into(), success_message: "updates.check.upToDate".into(), result: Err("Update status is unavailable.".into()) });
            }
            return;
        }
        thread::spawn(move || {
            let (current, current_build) = state
                .lock()
                .map(|value| (value.current_version.clone(), value.current_build))
                .unwrap_or_default();
            let result = fetch_latest(&provider, &current);
            let mut outcome = Err("Update status is unavailable.".to_owned());
            if let Ok(mut value) = state.lock() {
                match result {
                    Ok(Some(release)) => {
                        value.update_available = is_update_available(
                            &current,
                            current_build,
                            &release.base_version,
                            release.build,
                        );
                        value.latest_version = Some(release.version.clone());
                        value.release_url = release.release_url.clone();
                        value.message = Some(if value.update_available {
                            release.message.clone()
                        } else {
                            "ShroudForge is up to date.".into()
                        });
                        let update_available = value.update_available;
                        let latest_version = release.version.clone();
                        value.release = Some(release);
                        value.state = "ready".into();
                        outcome = Ok(if update_available { format!("Update {latest_version} is available.") } else { "ShroudForge is up to date.".into() });
                    }
                    Ok(None) => {
                        value.update_available = false;
                        value.latest_version = None;
                        value.release_url = None;
                        value.release = None;
                        value.state = "ready".into();
                        value.message =
                            Some("No ShroudForge release has been published on GitHub yet.".into());
                        outcome = Ok("No ShroudForge release has been published yet.".into());
                    }
                    Err(error) => {
                        value.state = "error".into();
                        value.message = Some(error.clone());
                        outcome = Err(error);
                    }
                }
            }
            if let Some(request_id) = request_id {
                let success_message = match outcome {
                    Ok(ref detail) if detail == "ShroudForge is up to date." => "updates.check.upToDate",
                    Ok(ref detail) if detail == "No ShroudForge release has been published yet." => "updates.check.none",
                    Ok(_) => "updates.check.available",
                    Err(_) => "updates.check.upToDate",
                };
                let _ = results.send(AsyncUiResult { request_id, source: "Updates".into(), action: "Check for updates".into(), success_message: success_message.into(), result: outcome });
            } else if let Err(error) = outcome {
                let _ = shroudforge_package::logging::append(&root, 'W', "updates", &format!("Automatic update check failed: {error}"));
            }
        });
    }

    fn fetch_latest(provider: &ProviderConfig, current: &str) -> Result<Option<Release>, String> {
        if provider.kind != "github-releases" {
            return Err(format!("unsupported update provider: {}", provider.kind));
        }
        fetch_latest_github(provider, current)
    }

    fn fetch_latest_github(
        provider: &ProviderConfig,
        current: &str,
    ) -> Result<Option<Release>, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(format!("ShroudForge/{current} (modloader-ui)"))
            .build()
            .map_err(|error| error.to_string())?;
        let url = format!(
            "{}/repos/{}/releases/latest",
            provider.base_url.trim_end_matches('/'),
            provider.project_id.trim_matches('/')
        );
        let response = client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .map_err(|error| format!("GitHub is unreachable: {error}"))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!("GitHub returned HTTP {}", response.status()));
        }
        let release: GithubRelease = response
            .json()
            .map_err(|error| format!("Invalid GitHub response: {error}"))?;
        let archive = release
            .assets
            .iter()
            .find(|asset| asset.name.starts_with("shroudforge-") && asset.name.ends_with(".zip"))
            .ok_or("GitHub release does not contain a ShroudForge ZIP")?;
        let checksum_name = format!("{}.sha256", archive.name);
        let checksum_asset = release
            .assets
            .iter()
            .find(|asset| asset.name == checksum_name)
            .ok_or("GitHub release does not contain a matching .zip.sha256 file")?;
        let checksum_response = client
            .get(&checksum_asset.browser_download_url)
            .send()
            .map_err(|error| format!("Could not download GitHub checksum: {error}"))?;
        if !checksum_response.status().is_success() {
            return Err(format!(
                "GitHub checksum endpoint returned HTTP {}",
                checksum_response.status()
            ));
        }
        let checksum_text = checksum_response
            .text()
            .map_err(|error| format!("Could not read GitHub checksum: {error}"))?;
        let checksum = checksum_text
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("GitHub release does not contain a valid SHA-256 checksum".into());
        }
        let version = release.tag_name.trim_start_matches('v').to_owned();
        let (base_version, build) = parse_release_tag(&version)?;
        Ok(Some(Release {
            version,
            base_version,
            build,
            download_url: archive.browser_download_url.clone(),
            checksum,
            release_url: Some(release.html_url),
            message: if release.body.trim().is_empty() {
                "A new ShroudForge version is available on GitHub.".into()
            } else {
                release.body
            },
        }))
    }

    fn parse_release_tag(tag: &str) -> Result<(String, u64), String> {
        let tag = tag.trim_start_matches('v');
        let (version, build) = tag
            .rsplit_once("-build.")
            .ok_or_else(|| format!("Invalid ShroudForge release tag: {tag}"))?;
        Version::parse(version)
            .map_err(|error| format!("Invalid ShroudForge version '{version}': {error}"))?;
        let build = build
            .parse::<u64>()
            .map_err(|_| format!("Invalid ShroudForge build ID: {build}"))?;
        Ok((version.to_owned(), build))
    }

    fn is_update_available(
        current_version: &str,
        current_build: u64,
        latest_version: &str,
        latest_build: u64,
    ) -> bool {
        let Ok(current) = Version::parse(current_version.trim_start_matches('v')) else {
            return false;
        };
        let Ok(latest) = Version::parse(latest_version.trim_start_matches('v')) else {
            return false;
        };
        latest > current || (latest == current && latest_build > current_build)
    }

    fn start_staging(root: PathBuf, state: Arc<Mutex<ReleaseState>>, results: mpsc::Sender<AsyncUiResult>, request_id: String) {
        let release = {
            let Ok(mut value) = state.lock() else {
                let _ = results.send(AsyncUiResult { request_id, source: "Updates".into(), action: "Stage update".into(), success_message: "updates.stagedToast".into(), result: Err("Update status is unavailable.".into()) });
                return;
            };
            if value.state == "downloading" || value.staged {
                let error = if value.staged { "An update is already staged." } else { "An update is already downloading." };
                let _ = results.send(AsyncUiResult { request_id, source: "Updates".into(), action: "Stage update".into(), success_message: "updates.stagedToast".into(), result: Err(error.into()) });
                return;
            }
            let Some(release) = value.release.clone() else {
                value.state = "error".into();
                value.message = Some("Check for updates before downloading.".into());
                let _ = results.send(AsyncUiResult { request_id, source: "Updates".into(), action: "Stage update".into(), success_message: "updates.stagedToast".into(), result: Err("Check for updates before downloading.".into()) });
                return;
            };
            value.state = "downloading".into();
            value.message = Some("Downloading and verifying update…".into());
            release
        };
        let version = release.version.clone();
        thread::spawn(move || {
            let result = stage_release(&root, &release);
            let feedback = match result {
                Ok(()) => Ok(format!("Verified and staged version {version}.")),
                Err(error) => Err(error),
            };
            if let Ok(mut value) = state.lock() {
                match &feedback {
                    Ok(_) => {
                        value.state = "staged".into();
                        value.staged = true;
                        value.message = Some(
                            "Update verified and staged for installation after the game exits.".into(),
                        );
                    }
                    Err(error) => {
                        value.state = "error".into();
                        value.message = Some(error.clone());
                    }
                }
            }
            let _ = results.send(AsyncUiResult { request_id, source: "Updates".into(), action: "Stage update".into(), success_message: "updates.stagedToast".into(), result: feedback });
        });
    }

    fn stage_release(root: &Path, release: &Release) -> Result<(), String> {
        let updates = shroudforge_package::paths::updates_dir(root);
        let pending = updates.join("pending");
        ensure_child(&updates, &pending)?;
        fs::create_dir_all(&updates).map_err(|error| error.to_string())?;
        let _ = fs::remove_file(updates.join("pending.ready"));
        if pending.exists() {
            fs::remove_dir_all(&pending).map_err(|error| error.to_string())?;
        }
        fs::create_dir_all(&pending).map_err(|error| error.to_string())?;
        let archive_path = updates.join("download.zip");

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent(format!("ShroudForge/{} updater", release.version))
            .build()
            .map_err(|error| error.to_string())?;
        let mut response = client
            .get(&release.download_url)
            .send()
            .map_err(|error| format!("Download failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Download endpoint returned HTTP {}",
                response.status()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_UPDATE_BYTES)
        {
            return Err("Update package exceeds 2 GiB".into());
        }
        let mut file = File::create(&archive_path).map_err(|error| error.to_string())?;
        let mut hasher = Sha256::new();
        let mut total = 0_u64;
        let mut buffer = [0_u8; 1024 * 1024];
        loop {
            let count = response
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > MAX_UPDATE_BYTES {
                return Err("Update package exceeds 2 GiB".into());
            }
            file.write_all(&buffer[..count])
                .map_err(|error| error.to_string())?;
            hasher.update(&buffer[..count]);
        }
        file.flush().map_err(|error| error.to_string())?;
        let actual = format!("{:x}", hasher.finalize());
        if actual != release.checksum {
            return Err("Update package SHA-256 checksum does not match".into());
        }
        extract_zip(&archive_path, &pending)?;
        let package = pending.clone();
        for required in ["shroudforge/version.json", "shroudforge.exe"] {
            if !package.join(required).is_file() {
                return Err(format!("Update package is missing {required}"));
            }
        }
        let marker = serde_json::json!({
            "version": release.version,
            "checksumAlgorithm": "SHA-256",
            "checksum": release.checksum,
            "stagedAt": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
        });
        write_json(&updates.join("pending.ready"), &marker)?;
        let _ = fs::remove_file(archive_path);
        Ok(())
    }

    fn extract_zip(archive_path: &Path, target: &Path) -> Result<(), String> {
        let file = File::open(archive_path).map_err(|error| error.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
        if archive.len() > MAX_ARCHIVE_ENTRIES {
            return Err("Update archive contains too many entries".into());
        }
        let mut expanded = 0_u64;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
            let Some(relative) = entry.enclosed_name() else {
                return Err(format!("Unsicherer Archivpfad: {}", entry.name()));
            };
            if relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            }) {
                return Err(format!("Unsicherer Archivpfad: {}", entry.name()));
            }
            if entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                return Err(format!(
                    "Update archive contains a disallowed symbolic link: {}",
                    entry.name()
                ));
            }
            expanded = expanded.saturating_add(entry.size());
            if expanded > MAX_UPDATE_BYTES {
                return Err("Extracted update exceeds 2 GiB".into());
            }
            let output = target.join(relative);
            ensure_child(target, &output)?;
            if entry.is_dir() {
                fs::create_dir_all(&output).map_err(|error| error.to_string())?;
                continue;
            }
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let mut destination = File::create(output).map_err(|error| error.to_string())?;
            std::io::copy(&mut entry, &mut destination).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn ensure_child(root: &Path, child: &Path) -> Result<(), String> {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let candidate = child.canonicalize().unwrap_or_else(|_| child.to_path_buf());
        if !candidate.starts_with(&root) {
            return Err(format!("Path resolves outside {}", root.display()));
        }
        Ok(())
    }

    fn open_event(name: &str) -> HANDLE {
        let mut wide: Vec<u16> = name.encode_utf16().collect();
        wide.push(0);
        unsafe { OpenEventW(SYNCHRONIZE_ACCESS, 0, wide.as_ptr()) }
    }

    fn foreground_process() -> u32 {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid) };
        pid
    }

    fn open_url(url: &str) {
        if !url.starts_with("https://") {
            return;
        }
        let operation: Vec<u16> = std::ffi::OsStr::new("open")
            .encode_wide()
            .chain(Some(0))
            .collect();
        let target: Vec<u16> = std::ffi::OsStr::new(url)
            .encode_wide()
            .chain(Some(0))
            .collect();
        unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            );
        }
    }
}

#[cfg(windows)]
pub fn run_module() -> Result<(), Box<dyn std::error::Error>> {
    windows::run()
}

#[cfg(not(windows))]
pub fn run_module() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::other("ShroudForge Modloader UI is available on Windows only").into())
}
