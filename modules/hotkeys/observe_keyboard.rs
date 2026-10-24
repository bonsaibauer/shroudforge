use crate::{Hotkey, InputDevice};
use std::path::Path;

pub fn initialize() -> Result<(), String> {
    #[cfg(windows)]
    return windows::initialize(None);
    #[cfg(not(windows))]
    Err("Background keyboard input is implemented for Windows".into())
}

/// The injected runtime must not replace the game's Raw Input registration.
/// Its SDL event loop runs in the existing executable's hidden process mode.
pub fn initialize_runtime(executable: &Path) -> Result<(), String> {
    #[cfg(windows)]
    return windows::initialize(Some(executable));
    #[cfg(not(windows))]
    {
        let _ = executable;
        initialize()
    }
}

pub fn run_input_process(parent_pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    return windows::run_input_process(parent_pid);
    #[cfg(not(windows))]
    {
        let _ = parent_pid;
        initialize()
    }
}

pub fn set_capture_active(active: bool, device: InputDevice) -> Result<(), String> {
    #[cfg(windows)]
    return windows::set_capture_active(active, device);
    #[cfg(not(windows))]
    {
        let _ = (active, device);
        Ok(())
    }
}

pub fn is_capture_active() -> bool {
    #[cfg(windows)]
    return windows::is_capture_active();
    #[cfg(not(windows))]
    false
}

pub fn take_captured_hotkey() -> Option<Hotkey> {
    #[cfg(windows)]
    return windows::CAPTURE.lock().ok()?.pending.take();
    #[cfg(not(windows))]
    None
}

pub fn layout_revision() -> u64 {
    #[cfg(windows)]
    return windows::LAYOUT_REVISION.load(std::sync::atomic::Ordering::Acquire);
    #[cfg(not(windows))]
    0
}

pub fn is_hotkey_down(hotkey: &Hotkey) -> bool {
    #[cfg(windows)]
    return windows::is_down(hotkey);
    #[cfg(not(windows))]
    {
        let _ = hotkey;
        false
    }
}

pub(crate) fn valid_scancode(scancode: u16) -> bool {
    #[cfg(windows)]
    return unsafe { windows::sf_keyboard_valid_scancode(i32::from(scancode)) != 0 };
    #[cfg(not(windows))]
    (1..512).contains(&scancode)
}

pub(crate) fn primary_modifier(scancode: u16) -> u8 {
    #[cfg(windows)]
    return unsafe { windows::sf_keyboard_modifier(i32::from(scancode)) as u8 };
    #[cfg(not(windows))]
    {
        let _ = scancode;
        0
    }
}

pub(crate) fn key_label(scancode: u16) -> String {
    #[cfg(windows)]
    if let Ok(labels) = windows::LABELS.read()
        && let Some(label) = labels.get(usize::from(scancode))
        && !label.is_empty()
    {
        return label.clone();
    }
    format!("SDL:{scancode}")
}

pub(crate) fn mouse_label(button: u8) -> String {
    #[cfg(windows)]
    {
        let label = unsafe {
            std::ffi::CStr::from_ptr(windows::sf_keyboard_mouse_label(i32::from(button)))
        }
        .to_string_lossy();
        if !label.is_empty() {
            return label.into_owned();
        }
    }
    format!("Mouse {button}")
}

#[cfg(windows)]
mod windows {
    use super::{Hotkey, InputDevice, Path};
    use crate::parse_hotkey::{Chord, Trigger};
    use serde::{Deserialize, Serialize};
    use std::{
        collections::BTreeMap,
        ffi::{CStr, c_char},
        io::{BufRead, BufReader, Write},
        os::windows::process::CommandExt,
        process::{Command, Stdio},
        ptr,
        sync::{
            Mutex, OnceLock, RwLock,
            atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
            mpsc,
        },
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0},
        System::{
            LibraryLoader::{
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN,
                GetModuleHandleExW,
            },
            Threading::{
                CreateEventW, OpenEventW, OpenProcess, SYNCHRONIZATION_SYNCHRONIZE,
                WaitForSingleObject,
            },
        },
    };

    #[repr(C)]
    #[derive(Default)]
    struct SdlKeyEvent {
        scancode: i32,
        down: u8,
        repeat: u8,
        modifiers: u8,
        kind: u8,
        timestamp: u64,
    }

    unsafe extern "C" {
        fn sf_keyboard_initialize() -> i32;
        fn sf_keyboard_error() -> *const c_char;
        fn sf_keyboard_scancode_count() -> i32;
        pub(super) fn sf_keyboard_valid_scancode(scancode: i32) -> i32;
        pub(super) fn sf_keyboard_modifier(scancode: i32) -> i32;
        fn sf_keyboard_label(scancode: i32) -> *const c_char;
        fn sf_keyboard_sync_layout() -> i32;
        fn sf_keyboard_poll(event: *mut SdlKeyEvent, timeout: i32) -> i32;
        fn sf_keyboard_now_ns() -> u64;
        pub(super) fn sf_keyboard_mouse_label(button: i32) -> *const c_char;
        fn sf_keyboard_quit();
    }

    #[derive(Serialize, Deserialize)]
    enum KeyboardMessage {
        Labels(Vec<String>),
        Error(String),
        Key {
            code: u16,
            device: InputDevice,
            down: bool,
            repeat: bool,
            modifiers: u8,
            timestamp: u64,
        },
    }

    #[derive(Clone, Copy, Default)]
    struct KeyState {
        down: bool,
        modifiers: u8,
    }

    pub(super) struct Capture {
        armed: bool,
        armed_at: u64,
        device: InputDevice,
        candidate: Option<Chord>,
        pub(super) pending: Option<Hotkey>,
        guard: Option<usize>,
        ending: bool,
        release_after: Option<Instant>,
    }

    impl Capture {
        fn release_if_idle(&mut self, idle: bool) {
            if !self.ending {
                return;
            }
            // Only the inputs held when the capture ends delay the protection.
            // Once they are released, the next presses are new ones (moving
            // in the game, the window shortcut): they must not restart the
            // three seconds, or the shortcuts stay blocked while the player
            // keeps playing.
            if !idle && self.release_after.is_none() {
                return;
            }
            let release_after = self.release_after.get_or_insert_with(|| {
                tracing::debug!(target: "shroudforge::hotkeys", "Shortcut capture protection waits three seconds after input release");
                Instant::now() + Duration::from_secs(3)
            });
            if Instant::now() >= *release_after {
                self.release();
            }
        }

        fn release(&mut self) {
            if let Some(event) = self.guard.take() {
                unsafe { CloseHandle(event as HANDLE) };
                tracing::debug!(target: "shroudforge::hotkeys", "Shortcut capture protection ended");
            }
            self.ending = false;
            self.release_after = None;
        }
    }

    static ACTIVE: AtomicBool = AtomicBool::new(false);
    static MODIFIERS: AtomicU8 = AtomicU8::new(0);
    static KEYS: RwLock<BTreeMap<Trigger, KeyState>> = RwLock::new(BTreeMap::new());
    pub(super) static LABELS: RwLock<Vec<String>> = RwLock::new(Vec::new());
    pub(super) static LAYOUT_REVISION: AtomicU64 = AtomicU64::new(0);
    pub(super) static CAPTURE: Mutex<Capture> = Mutex::new(Capture {
        armed: false,
        armed_at: 0,
        device: InputDevice::Keyboard,
        candidate: None,
        pending: None,
        guard: None,
        ending: false,
        release_after: None,
    });
    static INITIALIZED: OnceLock<Result<(), String>> = OnceLock::new();

    pub(super) fn initialize(executable: Option<&Path>) -> Result<(), String> {
        INITIALIZED
            .get_or_init(|| {
                tracing::debug!(target: "shroudforge::hotkeys", isolated = executable.is_some(), "Starting SDL keyboard and mouse input");
                // This thread can outlive a runtime handle in the injected DLL.
                let mut module = ptr::null_mut();
                if unsafe {
                    GetModuleHandleExW(
                        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                        initialize as *const () as *const u16,
                        &mut module,
                    )
                } == 0
                {
                    return Err(format!(
                        "Could not retain SDL input reader: {}",
                        std::io::Error::last_os_error()
                    ));
                }
                let (sender, receiver) = mpsc::channel();
                let executable = executable.map(Path::to_path_buf);
                std::thread::Builder::new()
                    .name("sf-sdl-keyboard".into())
                    .spawn(move || {
                        let result = if let Some(executable) = executable {
                            read_input_process(&executable, &sender)
                        } else {
                            run_sdl(
                                || true,
                                |message| {
                                    let ready = !ACTIVE.load(Ordering::Acquire);
                                    record(message);
                                    if ready {
                                        let _ = sender.send(Ok(()));
                                    }
                                    Ok(())
                                },
                            )
                        };
                        if let Err(error) = result {
                            tracing::error!(target: "shroudforge::hotkeys", %error, "SDL input stopped");
                            let _ = sender.send(Err(error));
                        }
                        ACTIVE.store(false, Ordering::Release);
                        if let Ok(mut keys) = KEYS.write() {
                            keys.clear();
                        }
                        if let Ok(mut capture) = CAPTURE.lock() {
                            capture.armed = false;
                            capture.pending = None;
                            capture.release();
                        }
                    })
                    .map_err(|error| format!("Could not start SDL input reader: {error}"))?;
                receiver
                    .recv_timeout(Duration::from_secs(10))
                    .map_err(|error| format!("SDL input did not initialize: {error}"))?
            })
            .clone()?;
        if ACTIVE.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err("SDL keyboard input is no longer running".into())
        }
    }

    fn read_input_process(
        executable: &Path,
        ready: &mpsc::Sender<Result<(), String>>,
    ) -> Result<(), String> {
        let mut child = Command::new(executable)
            .args([
                "--keyboard-input",
                "--parent-pid",
                &std::process::id().to_string(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|error| format!("Could not start isolated SDL input: {error}"))?;
        let output = child
            .stdout
            .take()
            .ok_or("SDL input process has no output")?;
        let result = (|| {
            for line in BufReader::new(output).lines() {
                let message = serde_json::from_str(&line.map_err(|error| error.to_string())?)
                    .map_err(|error| format!("Invalid SDL input message: {error}"))?;
                if let KeyboardMessage::Error(error) = message {
                    return Err(error);
                }
                let first = !ACTIVE.load(Ordering::Acquire);
                record(message);
                if first {
                    let _ = ready.send(Ok(()));
                }
            }
            Err("Isolated SDL input stopped".to_owned())
        })();
        let _ = child.kill();
        let _ = child.wait();
        result
    }

    pub(super) fn run_input_process(parent_pid: u32) -> Result<(), String> {
        if parent_pid == 0 || parent_pid == std::process::id() {
            return Err("SDL input requires a parent process".into());
        }
        let parent = unsafe { OpenProcess(SYNCHRONIZATION_SYNCHRONIZE, 0, parent_pid) };
        if parent.is_null() {
            return Err(format!(
                "Could not observe SDL input owner: {}",
                std::io::Error::last_os_error()
            ));
        }
        let mut output = std::io::BufWriter::new(std::io::stdout().lock());
        let result = run_sdl(
            || unsafe { WaitForSingleObject(parent, 0) } != WAIT_OBJECT_0,
            |message| {
                serde_json::to_writer(&mut output, &message).map_err(|error| error.to_string())?;
                output
                    .write_all(b"\n")
                    .and_then(|_| output.flush())
                    .map_err(|error| error.to_string())
            },
        );
        if let Err(error) = &result {
            let _ = serde_json::to_writer(&mut output, &KeyboardMessage::Error(error.clone()));
            let _ = output.write_all(b"\n").and_then(|_| output.flush());
        }
        unsafe {
            CloseHandle(parent);
        }
        result
    }

    fn labels() -> Vec<String> {
        (0..unsafe { sf_keyboard_scancode_count() })
            .map(|code| {
                unsafe { CStr::from_ptr(sf_keyboard_label(code)) }
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    fn run_sdl(
        mut alive: impl FnMut() -> bool,
        mut send: impl FnMut(KeyboardMessage) -> Result<(), String>,
    ) -> Result<(), String> {
        if unsafe { sf_keyboard_initialize() } == 0 {
            return Err(format!(
                "SDL keyboard initialization failed: {}",
                unsafe { CStr::from_ptr(sf_keyboard_error()) }.to_string_lossy()
            ));
        }
        let result = (|| {
            unsafe {
                sf_keyboard_sync_layout();
            }
            send(KeyboardMessage::Labels(labels()))?;
            let mut next_layout = Instant::now() + Duration::from_millis(200);
            while alive() {
                if Instant::now() >= next_layout {
                    if unsafe { sf_keyboard_sync_layout() } != 0 {
                        send(KeyboardMessage::Labels(labels()))?;
                    }
                    next_layout = Instant::now() + Duration::from_millis(200);
                }
                let mut event = SdlKeyEvent::default();
                match unsafe { sf_keyboard_poll(&mut event, 25) } {
                    1 if event.scancode > 0 => send(KeyboardMessage::Key {
                        code: event.scancode as u16,
                        device: if event.kind == 0 {
                            InputDevice::Keyboard
                        } else {
                            InputDevice::Mouse
                        },
                        down: event.down != 0,
                        repeat: event.repeat != 0,
                        modifiers: event.modifiers,
                        timestamp: event.timestamp,
                    })?,
                    2 => send(KeyboardMessage::Labels(labels()))?,
                    _ => {}
                }
                // No key event is required to finish the post-capture delay.
                let idle = KEYS.read().is_ok_and(|keys| keys.is_empty());
                if let Ok(mut capture) = CAPTURE.lock() {
                    capture.release_if_idle(idle);
                }
            }
            Ok(())
        })();
        unsafe {
            sf_keyboard_quit();
        }
        result
    }

    fn record(message: KeyboardMessage) {
        match message {
            KeyboardMessage::Error(_) => ACTIVE.store(false, Ordering::Release),
            KeyboardMessage::Labels(labels) => {
                if let Ok(mut current) = LABELS.write()
                    && *current != labels
                {
                    *current = labels;
                    LAYOUT_REVISION.fetch_add(1, Ordering::Release);
                    tracing::trace!(target: "shroudforge::hotkeys", count = current.len(), "SDL input labels refreshed");
                }
                if !ACTIVE.swap(true, Ordering::AcqRel) {
                    tracing::info!(target: "shroudforge::hotkeys", "SDL keyboard and mouse input is ready");
                }
            }
            KeyboardMessage::Key {
                code,
                device,
                down,
                repeat,
                modifiers,
                timestamp,
            } => {
                let trigger = match device {
                    InputDevice::Keyboard => Trigger::Keyboard(code),
                    InputDevice::Mouse if code <= 32 => Trigger::Mouse(code as u8),
                    InputDevice::Mouse => return,
                };
                MODIFIERS.store(modifiers, Ordering::Release);
                let idle = if let Ok(mut keys) = KEYS.write() {
                    if down && keys.get(&trigger).is_some_and(|key| key.down) {
                        return;
                    }
                    if down {
                        keys.insert(trigger, KeyState { down, modifiers });
                    } else {
                        keys.remove(&trigger);
                    }
                    keys.is_empty()
                } else {
                    false
                };
                if repeat {
                    return;
                }
                if let Ok(mut capture) = CAPTURE.lock() {
                    capture.release_if_idle(idle);
                    if capture.armed && timestamp >= capture.armed_at && device == capture.device {
                        if down
                            && capture
                                .candidate
                                .as_ref()
                                .is_none_or(|candidate| candidate.trigger.primary_modifier() != 0)
                        {
                            capture.candidate = Some(Chord {
                                trigger,
                                modifiers: modifiers & !trigger.primary_modifier(),
                            });
                            tracing::trace!(target: "shroudforge::hotkeys", ?device, "Shortcut capture candidate received");
                        } else if !down
                            && capture
                                .candidate
                                .as_ref()
                                .is_some_and(|candidate| candidate.trigger == trigger)
                        {
                            // Keep the cross-process guard active for the whole press.
                            capture.pending = capture.candidate.take().map(Hotkey::single);
                            capture.armed = false;
                            tracing::debug!(target: "shroudforge::hotkeys", ?device, "Shortcut capture completed after release");
                        }
                    }
                }
            }
        }
    }

    pub(super) fn is_down(hotkey: &Hotkey) -> bool {
        if !ACTIVE.load(Ordering::Acquire) {
            return false;
        }
        let modifiers = MODIFIERS.load(Ordering::Acquire);
        KEYS.read().is_ok_and(|keys| {
            hotkey.chords.iter().any(|chord| {
                modifiers == chord.required_modifiers()
                    && keys
                        .get(&chord.trigger)
                        .is_some_and(|key| key.down && key.modifiers == chord.required_modifiers())
            })
        })
    }

    fn capture_name() -> Vec<u16> {
        "Local\\ShroudForge.KeybindCapture"
            .encode_utf16()
            .chain(Some(0))
            .collect()
    }

    pub(super) fn set_capture_active(active: bool, device: InputDevice) -> Result<(), String> {
        if active {
            initialize(None)?;
        }
        let mut capture = CAPTURE
            .lock()
            .map_err(|_| "Keybind capture is unavailable")?;
        if !active {
            capture.armed = false;
            capture.candidate = None;
            capture.pending = None;
            capture.ending = true;
            capture.release_if_idle(KEYS.read().is_ok_and(|keys| keys.is_empty()));
            return Ok(());
        }
        let guard = if let Some(event) = capture.guard {
            event
        } else {
            let name = capture_name();
            // The owner alone retains this signaled event. Closing its handle
            // also removes a crashed owner's marker, without a global reset.
            let event = unsafe { CreateEventW(ptr::null(), 1, 1, name.as_ptr()) };
            if event.is_null() {
                return Err(format!(
                    "Could not create keybind capture marker: {}",
                    std::io::Error::last_os_error()
                ));
            }
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                unsafe { CloseHandle(event) };
                return Err("A shortcut is already being recorded in another window".into());
            }
            event as usize
        };
        *capture = Capture {
            armed: true,
            armed_at: unsafe { sf_keyboard_now_ns() },
            device,
            candidate: None,
            pending: None,
            guard: Some(guard),
            ending: false,
            release_after: None,
        };
        Ok(())
    }

    pub(super) fn is_capture_active() -> bool {
        let name = capture_name();
        let handle = unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()) };
        if handle.is_null() {
            return false;
        }
        let active = unsafe { WaitForSingleObject(handle, 0) } == WAIT_OBJECT_0;
        unsafe {
            CloseHandle(handle);
        }
        active
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn ending_capture() -> Capture {
            Capture {
                armed: false,
                armed_at: 0,
                device: InputDevice::Keyboard,
                candidate: None,
                pending: None,
                guard: None,
                ending: true,
                release_after: None,
            }
        }

        #[test]
        fn protection_waits_for_the_held_inputs_then_ignores_new_presses() {
            let mut capture = ending_capture();
            capture.release_if_idle(false);
            assert!(capture.ending && capture.release_after.is_none(), "the captured key is still held");

            capture.release_if_idle(true);
            let release_after = capture.release_after.expect("the delay starts at release");

            capture.release_if_idle(false);
            assert_eq!(capture.release_after, Some(release_after), "a new press keeps the delay");

            capture.release_after = Some(Instant::now() - Duration::from_millis(1));
            capture.release_if_idle(false);
            assert!(!capture.ending, "the protection ends even while a new key is held");
        }
    }
}
