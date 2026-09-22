#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("ShroudForge Modloader UI is available on Windows only");
}

#[cfg(windows)]
mod windows {
    use std::{
        fs::{self, File, OpenOptions},
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
    use wry::WebViewBuilder;

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
        compact_mode: bool,
        #[serde(default)]
        reduced_motion: bool,
        logging_enabled: bool,
        log_level: String,
        update_enabled: bool,
        base_url: String,
        project_id: String,
        check_minutes: u64,
    }

    #[derive(Deserialize)]
    struct UiCommand {
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
    }

    enum Command {
        Hide,
        Drag,
        Refresh,
        CheckUpdates,
        StageUpdate,
        OpenUrl(String),
        SetModEnabled(String, bool),
        SaveModSettings(String, serde_json::Value),
        SaveSettings(UiSettings),
        SearchCatalog(String),
        InstallMod(String),
        SaveLanguage(String),
        RunModAction(String, String),
        RemoveMod(String),
        MarkNewsRead(Vec<String>),
    }

    struct Arguments {
        root: PathBuf,
        game_pid: u32,
        stop_name: Option<String>,
        standalone: bool,
    }

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ModInfo {
        id: String,
        name: String,
        version: String,
        target: String,
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
        time: u64,
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
        connected: bool,
        mode: &'static str,
        game_version: String,
        version: String,
        mods: Vec<ModInfo>,
        activity: Vec<Activity>,
        notices: Vec<Notice>,
        read_notice_ids: Vec<String>,
        release: ReleaseState,
        settings: SnapshotSettings,
        catalog: CatalogState,
        locale: String,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SnapshotSettings {
        compact_mode: bool,
        reduced_motion: bool,
        logging_enabled: bool,
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
        let module_config = load_module_config();
        let user_config = load_user_config(&arguments.root);
        let provider = effective_provider(&module_config, &user_config);
        let installed = installed_release(&arguments.root);
        let staged = arguments
            .root
            .join("Shroudforge_Updates/pending.ready")
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

        let (sender, receiver) = mpsc::channel();
        let handler = move |message: wry::http::Request<String>| {
            let Ok(value) = serde_json::from_str::<UiCommand>(message.body()) else {
                return;
            };
            let command = match value.command.as_str() {
                "hide" => Command::Hide,
                "drag" => Command::Drag,
                "refresh" => Command::Refresh,
                "check-updates" => Command::CheckUpdates,
                "stage-update" => Command::StageUpdate,
                "search-catalog" => match value.query {
                    Some(query) => Command::SearchCatalog(query),
                    None => return,
                },
                "install-mod" => match value.project_id {
                    Some(project_id) => Command::InstallMod(project_id),
                    None => return,
                },
                "save-language" => match value.locale {
                    Some(locale) => Command::SaveLanguage(locale),
                    None => return,
                },
                "open-url" => match value.url {
                    Some(url) => Command::OpenUrl(url),
                    None => return,
                },
                "save-mod-settings" => match (value.mod_id, value.values) {
                    (Some(id), Some(values)) => Command::SaveModSettings(id, values),
                    _ => return,
                },
                "set-mod-enabled" => match (value.mod_id, value.enabled) {
                    (Some(id), Some(enabled)) => Command::SetModEnabled(id, enabled),
                    _ => return,
                },
                "save-settings" => match value.settings {
                    Some(settings) => Command::SaveSettings(settings),
                    None => return,
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
                _ => return,
            };
            let _ = sender.send(command);
        };
        let webview = WebViewBuilder::new()
            .with_html(include_str!("../ui/dist/index.html"))
            .with_ipc_handler(handler)
            .build(&window)?;

        let mut shown = arguments.standalone;
        let mut key_down = false;
        let mut next_refresh = Instant::now();
        let mut next_update_check = if module_config.system_update_provider.enabled {
            Instant::now()
        } else {
            Instant::now() + Duration::from_secs(86400)
        };
        let mut active_provider = provider;
        let system_provider = module_config.system_update_provider.clone();
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
                            Command::Refresh => next_refresh = Instant::now(),
                            Command::CheckUpdates => {
                                start_update_check(system_provider.clone(), release_state.clone())
                            }
                            Command::StageUpdate => {
                                start_staging(arguments.root.clone(), release_state.clone())
                            }
                            Command::SearchCatalog(query) => start_catalog_search(
                                active_provider.clone(),
                                query,
                                catalog_state.clone(),
                            ),
                            Command::InstallMod(project_id) => start_mod_install(
                                arguments.root.clone(),
                                active_provider.clone(),
                                project_id,
                            ),
                            Command::SaveLanguage(locale) => {
                                let _ = save_language(&arguments.root, &locale);
                            }
                            Command::OpenUrl(url) => open_url(&url),
                            Command::SetModEnabled(id, enabled) => {
                                let result = set_mod_enabled(&arguments.root, &id, enabled);
                                let success = result.is_ok();
                                let error = result.err();
                                write_activity(
                                    &arguments.root,
                                    &id,
                                    if enabled {
                                        "Mod enabled"
                                    } else {
                                        "Mod disabled"
                                    },
                                    if success { "Successful" } else { "Failed" },
                                    error.as_deref(),
                                    if success { "success" } else { "error" },
                                );
                                next_refresh = Instant::now();
                            }
                            Command::SaveModSettings(id, values) => {
                                if save_mod_settings(&arguments.root, &id, &values).is_ok() {
                                    write_activity(
                                        &arguments.root,
                                        &id,
                                        "Einstellungen gespeichert",
                                        "Erfolgreich",
                                        None,
                                        "success",
                                    );
                                }
                                next_refresh = Instant::now();
                            }
                            Command::SaveSettings(settings) => {
                                if save_settings(&arguments.root, &settings).is_ok() {
                                    active_provider.enabled = settings.update_enabled;
                                    active_provider.base_url = settings.base_url;
                                    active_provider.project_id = settings.project_id;
                                    active_provider.check_minutes =
                                        settings.check_minutes.clamp(5, 1440);
                                    next_refresh = Instant::now();
                                    write_activity(
                                        &arguments.root,
                                        "Modloader",
                                        "Einstellungen gespeichert",
                                        "Erfolgreich",
                                        None,
                                        "success",
                                    );
                                }
                            }
                            Command::RunModAction(id, action) => {
                                let result = queue_mod_action(&arguments.root, &id, &action);
                                let success = result.is_ok();
                                let error = result.err();
                                write_activity(
                                    &arguments.root,
                                    &id,
                                    "Aktion ausgeführt",
                                    if success {
                                        "Übergeben"
                                    } else {
                                        "Fehlgeschlagen"
                                    },
                                    error.as_deref(),
                                    if success { "success" } else { "error" },
                                );
                                next_refresh = Instant::now();
                            }
                            Command::RemoveMod(id) => {
                                let result = remove_mod(&arguments.root, &id);
                                let success = result.is_ok();
                                let error = result.err();
                                write_activity(
                                    &arguments.root,
                                    &id,
                                    "Mod entfernt",
                                    if success {
                                        "Erfolgreich"
                                    } else {
                                        "Fehlgeschlagen"
                                    },
                                    error.as_deref(),
                                    if success { "success" } else { "error" },
                                );
                                next_refresh = Instant::now();
                            }
                            Command::MarkNewsRead(ids) => {
                                let _ = mark_news_read(&arguments.root, &ids);
                                next_refresh = Instant::now();
                            }
                        }
                    }

                    let foreground = foreground_process();
                    let game_focused = arguments.standalone || foreground == arguments.game_pid;
                    let ui_focused = foreground == unsafe { GetCurrentProcessId() };
                    let down = unsafe { GetAsyncKeyState(module_config.toggle_key as i32) } < 0;
                    if game_focused && down && !key_down {
                        shown = !shown;
                        window.set_visible(shown);
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
                        start_update_check(system_provider.clone(), release_state.clone());
                        next_update_check = Instant::now()
                            + Duration::from_secs(
                                system_provider.check_minutes.clamp(5, 1440) * 60,
                            );
                    }
                    if Instant::now() >= next_refresh {
                        next_refresh = Instant::now()
                            + Duration::from_millis(
                                module_config.refresh_milliseconds.clamp(250, 10_000),
                            );
                        let snapshot =
                            snapshot(&arguments, &active_provider, &release_state, &catalog_state);
                        if let Ok(payload) = serde_json::to_string(&snapshot) {
                            let _ = webview.evaluate_script(&format!(
                                "window.__shroudforgeUpdate({payload});"
                            ));
                        }
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

    fn load_module_config() -> ModuleConfig {
        let path = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|parent| parent.join("module.json")));
        let mut config = path
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(ModuleConfig {
                toggle_key: default_toggle_key(),
                refresh_milliseconds: default_refresh(),
                update_provider: ProviderConfig::default(),
                system_update_provider: default_system_provider(),
            });
        if !(1..=255).contains(&config.toggle_key) {
            config.toggle_key = default_toggle_key();
        }
        config
    }

    fn central_config_path(root: &Path) -> PathBuf {
        root.join("config/shroudforge.json")
    }

    fn read_central_config(root: &Path) -> serde_json::Value {
        let path = central_config_path(root);
        if let Some(value) = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .filter(|value| value.is_object())
        {
            return value;
        }

        let value = serde_json::json!({
            "schemaVersion": 1,
            "general": {
                "locale": "de"
            },
            "logging": { "enabled": true, "level": "INFO" },
            "catalog": {
                "provider": null
            },
            "mods": {}
        });
        let _ = write_json(&path, &value);
        value
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
        let value = fs::read(root.join("version.json"))
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
        let logging = read_logging(&arguments.root);
        let mods = read_mods(&arguments.root);
        sync_mod_events(&arguments.root, &mods);
        Snapshot {
            connected: !arguments.standalone,
            mode: if arguments.standalone {
                "STANDALONE"
            } else if arguments.root.join("enshrouded.exe").is_file() {
                "CLIENT"
            } else {
                "SERVER"
            },
            game_version: read_game_version(&arguments.root),
            version: installed_version(&arguments.root),
            mods,
            activity: read_activity(&arguments.root),
            notices: read_notifications(&arguments.root),
            read_notice_ids: read_news_state(&arguments.root),
            release: release
                .lock()
                .map(|value| value.clone())
                .unwrap_or_else(|_| ReleaseState::idle(installed_release(&arguments.root), false)),
            settings: SnapshotSettings {
                compact_mode: read_central_config(&arguments.root)
                    .pointer("/general/compactMode")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                reduced_motion: read_central_config(&arguments.root)
                    .pointer("/general/reducedMotion")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                logging_enabled: logging.0,
                log_level: logging.1,
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
        fs::read(root.join("version.json"))
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

    fn read_logging(root: &Path) -> (bool, String) {
        let value = read_central_config(root);
        let enabled = value
            .pointer("/logging/enabled")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let level = value
            .pointer("/logging/level")
            .and_then(|value| value.as_str())
            .unwrap_or("INFO")
            .to_ascii_uppercase();
        (enabled, level)
    }

    fn read_mods(root: &Path) -> Vec<ModInfo> {
        let Ok(entries) = fs::read_dir(root.join("mods")) else {
            return Vec::new();
        };
        let config = read_central_config(root);
        let catalog_mods = catalog_installed_mod_ids(root);
        let mut mods = Vec::new();
        for entry in entries.flatten() {
            let package = entry.path();
            let Some(value) = read_package_file(&package, "mod.json")
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            else {
                continue;
            };
            let Some(id) = value.get("id").and_then(|value| value.as_str()) else {
                continue;
            };
            let ui = value
                .get("ui")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let assets = read_mod_assets(&package, &ui);
            mods.push(ModInfo {
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
                    .get("mods")
                    .and_then(|value| value.get(id))
                    .and_then(|value| value.get("enabled"))
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true),
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
                    .get("mods")
                    .and_then(|value| value.get(id))
                    .and_then(|value| value.get("settings"))
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
        let Ok(file) = File::open(root.join("Shroudforge_UI/activity.jsonl")) else {
            return Vec::new();
        };
        let mut items = BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str::<Activity>(&line).ok())
            .collect::<Vec<_>>();
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
        let directory = root.join("Shroudforge_UI");
        if fs::create_dir_all(&directory).is_err() {
            return;
        }
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let time = timestamp.as_secs();
        let item = Activity {
            id: format!("{}-{}", timestamp.as_nanos(), std::process::id()),
            time,
            source: source.chars().take(80).collect(),
            action: action.chars().take(120).collect(),
            result: result.chars().take(80).collect(),
            details: details.map(|value| value.chars().take(500).collect()),
            level: level.to_owned(),
        };
        let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("activity.jsonl"))
        else {
            return;
        };
        if let Ok(line) = serde_json::to_string(&item) {
            let _ = writeln!(file, "{line}");
        }
    }

    fn read_notifications(root: &Path) -> Vec<Notice> {
        let Ok(entries) = fs::read_dir(root.join("Shroudforge_UI/notifications")) else {
            return Vec::new();
        };
        let mut notices = entries
            .flatten()
            .filter_map(|entry| fs::read(entry.path()).ok())
            .filter_map(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .filter_map(|value| {
                Some(Notice {
                    id: value.get("id")?.as_str()?.to_owned(),
                    mod_id: value.get("modId")?.as_str()?.to_owned(),
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

    fn news_state_path(root: &Path) -> PathBuf {
        root.join("Shroudforge_UI/news-state.json")
    }

    fn read_news_state(root: &Path) -> Vec<String> {
        fs::read(news_state_path(root))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| {
                value
                    .get("read")
                    .and_then(|value| value.as_array())
                    .cloned()
            })
            .map(|items| {
                items
                    .into_iter()
                    .filter_map(|value| {
                        value
                            .as_str()
                            .filter(|id| valid_news_id(id))
                            .map(str::to_owned)
                    })
                    .take(1000)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn mark_news_read(root: &Path, ids: &[String]) -> Result<(), String> {
        let mut read = read_news_state(root);
        for id in ids.iter().filter(|id| valid_news_id(id)) {
            if !read.contains(id) {
                read.push(id.clone());
            }
        }
        if read.len() > 1000 {
            read.drain(..read.len() - 1000);
        }
        write_json(&news_state_path(root), &serde_json::json!({ "read": read }))
    }

    fn mod_state_path(root: &Path) -> PathBuf {
        root.join("Shroudforge_UI/mod-state.json")
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
        let path = mod_state_path(root);
        let previous = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| value.as_object().cloned());
        let Some(previous) = previous else {
            let _ = write_json(&path, &serde_json::Value::Object(current));
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
        let _ = write_json(&path, &serde_json::Value::Object(current));
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
        let directory = root.join("Shroudforge_UI/notifications");
        if fs::create_dir_all(&directory).is_err() {
            return;
        }
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
        let _ = write_json(&directory.join(format!("event-{id}.json")), &payload);
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
            .get("ui")
            .is_some_and(|ui| ui_declares_action(ui, action));
        if !declared {
            return Err("mod action is not declared by the package".into());
        }
        let actions = root.join("Shroudforge_UI/actions");
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
        let trash = root.join("Shroudforge_UI/trash");
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

    fn save_mod_settings(root: &Path, id: &str, values: &serde_json::Value) -> Result<(), String> {
        if !values.is_object() || !valid_identifier(id) {
            return Err("invalid mod settings target".into());
        }
        let package = find_mod_package(root, id).ok_or("mod package not found")?;
        let manifest = read_package_file(&package, "mod.json").and_then(|bytes| {
            serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|error| error.to_string())
        })?;
        let definitions = manifest
            .get("settings")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let object = values.as_object().ok_or("settings must be an object")?;
        let mut validated = serde_json::Map::new();
        for definition in &definitions {
            let Some(key) = definition.get("key").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(value) = object.get(key) else {
                continue;
            };
            validate_setting_value(definition, value)?;
            validated.insert(key.to_owned(), value.clone());
        }
        if validated.len() != object.len() {
            return Err("settings contain unknown keys".into());
        }
        let mut config = read_central_config(root);
        let root_object = config
            .as_object_mut()
            .ok_or("central config must be an object")?;
        let mods = root_object
            .entry("mods")
            .or_insert_with(|| serde_json::json!({}));
        let mods = mods
            .as_object_mut()
            .ok_or("mods config must be an object")?;
        let entry = mods
            .entry(id)
            .or_insert_with(|| serde_json::json!({ "enabled": true, "settings": {} }));
        let entry = entry
            .as_object_mut()
            .ok_or("mod config must be an object")?;
        entry.insert("settings".into(), serde_json::Value::Object(validated));
        write_json(&central_config_path(root), &config)
    }

    fn set_mod_enabled(root: &Path, id: &str, enabled: bool) -> Result<(), String> {
        if !valid_identifier(id) || find_mod_package(root, id).is_none() {
            return Err("invalid mod activation target".into());
        }
        let mut config = read_central_config(root);
        let root_object = config
            .as_object_mut()
            .ok_or("central config must be an object")?;
        let mods = root_object
            .entry("mods")
            .or_insert_with(|| serde_json::json!({}));
        let mods = mods
            .as_object_mut()
            .ok_or("mods config must be an object")?;
        let entry = mods
            .entry(id)
            .or_insert_with(|| serde_json::json!({ "enabled": true, "settings": {} }));
        let entry = entry
            .as_object_mut()
            .ok_or("mod config must be an object")?;
        entry.insert("enabled".into(), serde_json::Value::Bool(enabled));
        write_json(&central_config_path(root), &config)
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

    fn validate_setting_value(
        definition: &serde_json::Value,
        value: &serde_json::Value,
    ) -> Result<(), String> {
        let key = definition
            .get("key")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let kind = definition
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or("string");
        let type_ok = match kind {
            "boolean" => value.is_boolean(),
            "string" => value.is_string(),
            "integer" => value.as_i64().is_some(),
            "number" => value.is_number(),
            "array" => value.is_array(),
            _ => false,
        };
        if !type_ok {
            return Err(format!("setting '{key}' has the wrong type"));
        }
        if let Some(number) = value.as_f64() {
            if definition
                .get("minimum")
                .and_then(|value| value.as_f64())
                .is_some_and(|minimum| number < minimum)
            {
                return Err(format!("setting '{key}' is below its minimum"));
            }
            if definition
                .get("maximum")
                .and_then(|value| value.as_f64())
                .is_some_and(|maximum| number > maximum)
            {
                return Err(format!("setting '{key}' is above its maximum"));
            }
        }
        if let Some(text) = value.as_str() {
            if definition
                .get("minimumLength")
                .and_then(|value| value.as_u64())
                .is_some_and(|minimum| text.chars().count() < minimum as usize)
            {
                return Err(format!("setting '{key}' is too short"));
            }
            if definition
                .get("maximumLength")
                .and_then(|value| value.as_u64())
                .is_some_and(|maximum| text.chars().count() > maximum as usize)
            {
                return Err(format!("setting '{key}' is too long"));
            }
        }
        if let Some(options) = definition.get("options").and_then(|value| value.as_array()) {
            let valid = if value.is_array() {
                value.as_array().is_some_and(|selected| {
                    selected.iter().all(|item| {
                        options
                            .iter()
                            .any(|option| option.get("value") == Some(item))
                    })
                })
            } else {
                options
                    .iter()
                    .any(|option| option.get("value") == Some(value))
            };
            if !valid {
                return Err(format!("setting '{key}' contains an unknown option"));
            }
        }
        Ok(())
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
        let path = central_config_path(root);
        let mut value = read_central_config(root);
        let object = value
            .as_object_mut()
            .ok_or("config/shroudforge.json must contain an object")?;
        object.insert("logging".into(), serde_json::json!({ "enabled": settings.logging_enabled, "level": &settings.log_level }));
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
        write_json(&path, &value)
    }

    fn save_language(root: &Path, locale: &str) -> Result<(), String> {
        if !is_valid_locale(locale) {
            return Err("invalid locale".into());
        }
        let mut value = read_central_config(root);
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
        write_json(&central_config_path(root), &value)
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
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let temporary = path.with_extension("tmp");
        let backup = path.with_extension("bak");
        let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
        fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
        let _ = fs::remove_file(&backup);
        if path.exists() {
            fs::rename(path, &backup).map_err(|error| error.to_string())?;
        }
        if let Err(error) = fs::rename(&temporary, path) {
            if backup.exists() {
                let _ = fs::rename(&backup, path);
            }
            return Err(error.to_string());
        }
        let _ = fs::remove_file(backup);
        Ok(())
    }

    fn start_catalog_search(
        provider: ProviderConfig,
        query: String,
        state: Arc<Mutex<CatalogState>>,
    ) {
        let query = query.trim().chars().take(100).collect::<String>();
        if let Ok(mut value) = state.lock() {
            value.query = query.clone();
            value.items.clear();
            value.message = None;
            if !provider.enabled {
                value.state = "idle".into();
                value.message = Some("ShroudEdit-Zugriff ist deaktiviert.".into());
                return;
            }
            value.state = "loading".into();
        }
        thread::spawn(move || {
            let result = fetch_catalog(&provider, &query);
            if let Ok(mut value) = state.lock() {
                match result {
                    Ok(items) => {
                        value.state = "ready".into();
                        value.message = if items.is_empty() {
                            Some("Keine passenden ShroudForge-Mods gefunden.".into())
                        } else {
                            None
                        };
                        value.items = items;
                    }
                    Err(error) => {
                        value.state = "error".into();
                        value.message = Some(error);
                    }
                }
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
            .map_err(|error| format!("ShroudEdit ist nicht erreichbar: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "ShroudEdit antwortete mit HTTP {}",
                response.status()
            ));
        }
        let payload: serde_json::Value = response
            .json()
            .map_err(|error| format!("Ungültige ShroudEdit-Antwort: {error}"))?;
        let values = payload
            .as_array()
            .or_else(|| payload.get("hits").and_then(|value| value.as_array()))
            .or_else(|| payload.get("projects").and_then(|value| value.as_array()))
            .or_else(|| payload.get("data").and_then(|value| value.as_array()))
            .ok_or("ShroudEdit-Suche enthält keine Ergebnisliste")?;
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

    fn start_mod_install(root: PathBuf, provider: ProviderConfig, project_id: String) {
        if !valid_identifier(&project_id) {
            write_activity(
                &root,
                "ShroudEdit",
                "Mod installieren",
                "Fehlgeschlagen",
                Some("Ungültige Projekt-ID"),
                "error",
            );
            return;
        }
        write_activity(
            &root,
            "ShroudEdit",
            "Mod installieren",
            "Gestartet",
            Some(&project_id),
            "info",
        );
        thread::spawn(move || {
            let result = match INSTALL_LOCK.try_lock() {
                Ok(_guard) => install_mod_from_catalog(&root, &provider, &project_id),
                Err(_) => Err("Eine Mod-Installation läuft bereits".to_owned()),
            };
            let (result_label, details, level) = match &result {
                Ok(installed) => (
                    "Erfolgreich",
                    Some(format!("{} Paket(e) installiert", installed.len())),
                    "success",
                ),
                Err(error) => ("Fehlgeschlagen", Some(error.clone()), "error"),
            };
            write_activity(
                &root,
                "ShroudEdit",
                "Mod installieren",
                result_label,
                details.as_deref(),
                level,
            );
        });
    }

    fn install_mod_from_catalog(
        root: &Path,
        provider: &ProviderConfig,
        project_id: &str,
    ) -> Result<Vec<String>, String> {
        if !provider.enabled || provider.kind != "shroudedit" {
            return Err("ShroudEdit-Zugriff ist deaktiviert".into());
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
            .map_err(|error| format!("ShroudEdit ist nicht erreichbar: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "ShroudEdit-Auflösung antwortete mit HTTP {}",
                response.status()
            ));
        }
        let plan: ResolveContentPlan = response
            .json()
            .map_err(|error| format!("Ungültiger Installationsplan: {error}"))?;
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
                let _ = fs::remove_file(path);
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
            .map_err(|error| format!("Versionsdaten konnten nicht geladen werden: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Versionsdaten antworteten mit HTTP {}",
                response.status()
            ));
        }
        let version: ApiVersion = response
            .json()
            .map_err(|error| format!("Ungültige Versionsdaten: {error}"))?;
        if version.id != resolved.version_id || version.project_id != resolved.project_id {
            return Err("Versionsdaten gehören nicht zum aufgelösten Projekt".into());
        }
        let file = version
            .files
            .iter()
            .find(|file| file.primary)
            .or_else(|| version.files.first())
            .ok_or("Version enthält kein Mod-Paket")?;
        if !file.filename.to_ascii_lowercase().ends_with(".zip")
            || !file.url.starts_with("https://")
            || file.size > MAX_MOD_BYTES
        {
            return Err("Mod-Paket ist kein zulässiges ZIP-Archiv".into());
        }
        let response = client
            .get(&file.url)
            .send()
            .map_err(|error| format!("Mod-Paket konnte nicht geladen werden: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Mod-Download antwortete mit HTTP {}",
                response.status()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_MOD_BYTES)
        {
            return Err("Mod-Paket ist größer als 512 MiB".into());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_MOD_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_MOD_BYTES {
            return Err("Mod-Paket ist größer als 512 MiB".into());
        }
        verify_mod_hash(&bytes, &file.hashes)?;

        let downloads = root.join("Shroudforge_UI/downloads");
        fs::create_dir_all(&downloads).map_err(|error| error.to_string())?;
        let temporary = downloads.join(format!(
            "{}-{}-{}.zip",
            resolved.project_id,
            resolved.version_id,
            std::process::id()
        ));
        fs::write(&temporary, &bytes).map_err(|error| error.to_string())?;
        let manifest = read_package_file(&temporary, "mod.json").and_then(|bytes| {
            serde_json::from_slice::<shroudforge_package::ModManifest>(&bytes)
                .map_err(|error| error.to_string())
        });
        let manifest = match manifest {
            Ok(manifest) => manifest,
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                return Err(format!("Ungültiges mod.json: {error}"));
            }
        };
        if let Err(error) = shroudforge_package::validate_manifest(&manifest) {
            let _ = fs::remove_file(&temporary);
            return Err(format!("Ungültiges Mod-Manifest: {error}"));
        }
        if find_mod_package(root, &manifest.id).is_some() {
            let _ = fs::remove_file(&temporary);
            return Err(format!("Mod '{}' ist bereits installiert", manifest.name));
        }
        let mods = root.join("mods");
        fs::create_dir_all(&mods).map_err(|error| error.to_string())?;
        let destination = mods.join(format!("{}.zip", manifest.id));
        fs::rename(&temporary, &destination).map_err(|error| error.to_string())?;
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
            .ok_or("Mod-Paket besitzt keine SHA-512- oder SHA-256-Prüfsumme")?;
        let actual = match expected.0 {
            "sha512" => format!("{:x}", Sha512::digest(bytes)),
            _ => format!("{:x}", Sha256::digest(bytes)),
        };
        if !actual.eq_ignore_ascii_case(expected.1) {
            return Err("Prüfsumme des Mod-Pakets stimmt nicht".into());
        }
        Ok(())
    }

    fn catalog_install_registry_path(root: &Path) -> PathBuf {
        root.join("Shroudforge_UI/catalog-installs.json")
    }

    fn read_catalog_install_registry(root: &Path) -> serde_json::Value {
        fs::read(catalog_install_registry_path(root))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .filter(|value: &serde_json::Value| value.get("projects").is_some())
            .unwrap_or_else(|| serde_json::json!({ "schemaVersion": 1, "projects": {} }))
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
        let mut registry = read_catalog_install_registry(root);
        let projects = registry
            .get_mut("projects")
            .and_then(|value| value.as_object_mut())
            .ok_or("invalid catalog install registry")?;
        projects.insert(
            project_id.to_owned(),
            serde_json::json!({
                "modId": mod_id,
                "versionId": version_id,
                "file": package.file_name().and_then(|value| value.to_str()).unwrap_or_default()
            }),
        );
        write_json(&catalog_install_registry_path(root), &registry)
    }

    fn forget_catalog_install(root: &Path, mod_id: &str) -> Result<(), String> {
        let mut registry = read_catalog_install_registry(root);
        let projects = registry
            .get_mut("projects")
            .and_then(|value| value.as_object_mut())
            .ok_or("invalid catalog install registry")?;
        projects
            .retain(|_, value| value.get("modId").and_then(|value| value.as_str()) != Some(mod_id));
        write_json(&catalog_install_registry_path(root), &registry)
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

    fn start_update_check(provider: ProviderConfig, state: Arc<Mutex<ReleaseState>>) {
        if let Ok(mut value) = state.lock() {
            if value.state == "checking" || value.state == "downloading" {
                return;
            }
            if !provider.enabled {
                value.state = "idle".into();
                value.message = Some("GitHub-Systemupdates sind deaktiviert.".into());
                return;
            }
            value.state = "checking".into();
            value.message = Some("Veröffentlichungen werden geprüft …".into());
        }
        thread::spawn(move || {
            let (current, current_build) = state
                .lock()
                .map(|value| (value.current_version.clone(), value.current_build))
                .unwrap_or_default();
            let result = fetch_latest(&provider, &current);
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
                            "ShroudForge ist auf dem aktuellen Stand.".into()
                        });
                        value.release = Some(release);
                        value.state = "ready".into();
                    }
                    Ok(None) => {
                        value.update_available = false;
                        value.latest_version = None;
                        value.release_url = None;
                        value.release = None;
                        value.state = "ready".into();
                        value.message =
                            Some("Noch kein ShroudForge-Release auf GitHub veröffentlicht.".into());
                    }
                    Err(error) => {
                        value.state = "error".into();
                        value.message = Some(error);
                    }
                }
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
            .map_err(|error| format!("GitHub ist nicht erreichbar: {error}"))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!("GitHub antwortete mit HTTP {}", response.status()));
        }
        let release: GithubRelease = response
            .json()
            .map_err(|error| format!("Ungültige GitHub-Antwort: {error}"))?;
        let archive = release
            .assets
            .iter()
            .find(|asset| asset.name.starts_with("shroudforge-") && asset.name.ends_with(".zip"))
            .ok_or("GitHub-Release enthält kein ShroudForge-ZIP")?;
        let checksum_name = format!("{}.sha256", archive.name);
        let checksum_asset = release
            .assets
            .iter()
            .find(|asset| asset.name == checksum_name)
            .ok_or("GitHub-Release enthält keine passende .zip.sha256-Datei")?;
        let checksum_response = client
            .get(&checksum_asset.browser_download_url)
            .send()
            .map_err(|error| format!("GitHub-Prüfsumme konnte nicht geladen werden: {error}"))?;
        if !checksum_response.status().is_success() {
            return Err(format!(
                "GitHub-Prüfsumme antwortete mit HTTP {}",
                checksum_response.status()
            ));
        }
        let checksum_text = checksum_response
            .text()
            .map_err(|error| format!("GitHub-Prüfsumme konnte nicht gelesen werden: {error}"))?;
        let checksum = checksum_text
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("GitHub-Release enthält keine gültige SHA-256-Prüfsumme".into());
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
                "Eine neue ShroudForge-Version ist auf GitHub verfügbar.".into()
            } else {
                release.body
            },
        }))
    }

    fn parse_release_tag(tag: &str) -> Result<(String, u64), String> {
        let tag = tag.trim_start_matches('v');
        let (version, build) = tag
            .rsplit_once("-build.")
            .ok_or_else(|| format!("Ungültiger ShroudForge-Release-Tag: {tag}"))?;
        Version::parse(version)
            .map_err(|error| format!("Ungültige ShroudForge-Version '{version}': {error}"))?;
        let build = build
            .parse::<u64>()
            .map_err(|_| format!("Ungültige ShroudForge-Build-ID: {build}"))?;
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

    fn start_staging(root: PathBuf, state: Arc<Mutex<ReleaseState>>) {
        let release = {
            let Ok(mut value) = state.lock() else { return };
            if value.state == "downloading" || value.staged {
                return;
            }
            let Some(release) = value.release.clone() else {
                value.state = "error".into();
                value.message = Some("Vor dem Download muss nach Updates gesucht werden.".into());
                return;
            };
            value.state = "downloading".into();
            value.message = Some("Update wird heruntergeladen und geprüft …".into());
            release
        };
        thread::spawn(move || {
            let result = stage_release(&root, &release);
            if let Ok(mut value) = state.lock() {
                match result {
                    Ok(()) => {
                        value.state = "staged".into();
                        value.staged = true;
                        value.message = Some(
                            "Update geprüft und für das Beenden des Spiels vorgemerkt.".into(),
                        );
                    }
                    Err(error) => {
                        value.state = "error".into();
                        value.message = Some(error);
                    }
                }
            }
        });
    }

    fn stage_release(root: &Path, release: &Release) -> Result<(), String> {
        let updates = root.join("Shroudforge_Updates");
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
            .map_err(|error| format!("Download fehlgeschlagen: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Download antwortete mit HTTP {}",
                response.status()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_UPDATE_BYTES)
        {
            return Err("Updatepaket ist größer als 2 GiB".into());
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
                return Err("Updatepaket ist größer als 2 GiB".into());
            }
            file.write_all(&buffer[..count])
                .map_err(|error| error.to_string())?;
            hasher.update(&buffer[..count]);
        }
        file.flush().map_err(|error| error.to_string())?;
        let actual = format!("{:x}", hasher.finalize());
        if actual != release.checksum {
            return Err("SHA-256-Prüfsumme des Updatepakets stimmt nicht".into());
        }
        extract_zip(&archive_path, &pending)?;
        for required in [
            "game/version.json",
            "game/Shroudforge_Updater/shroudforge-updater.exe",
        ] {
            if !pending.join(required).is_file() {
                return Err(format!("Updatepaket enthält {required} nicht"));
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
            return Err("Updatearchiv enthält zu viele Einträge".into());
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
                    "Symlink im Updatearchiv ist nicht erlaubt: {}",
                    entry.name()
                ));
            }
            expanded = expanded.saturating_add(entry.size());
            if expanded > MAX_UPDATE_BYTES {
                return Err("Entpacktes Update ist größer als 2 GiB".into());
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
            return Err(format!("Pfad liegt außerhalb von {}", root.display()));
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
fn main() {
    if let Err(error) = windows::run() {
        eprintln!("ShroudForge Modloader UI failed: {error}");
        std::process::exit(1);
    }
}
