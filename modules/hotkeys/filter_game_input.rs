//! Keeps reserved input (reserve_game_input) from the game where the game
//! reads it: its import of GetRawInputData, and its window's key and mouse
//! messages. Enshrouded registers mouse and keyboard in Raw Input without
//! RIDEV_NOLEGACY, so Windows sends it both, and both are filtered.
//! ShroudForge reads the keys in its own keyboard process: a key kept from
//! the game still reaches the mods.

/// Installs the filter in the game process (once), and filters the game
/// window again if it was replaced. Cheap when nothing changed: call it as
/// often as needed.
pub fn install_input_filter() -> Result<(), String> {
    #[cfg(windows)]
    return windows::install();
    #[cfg(not(windows))]
    Err("Input reservations are implemented for Windows".into())
}

#[cfg(windows)]
mod windows {
    use crate::reserve_game_input::{KeptPresses, anything_reserved, wheel};
    use std::{
        ffi::c_void,
        mem::size_of,
        sync::{
            Mutex,
            atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Memory::{PAGE_READWRITE, VirtualProtect},
            Threading::GetCurrentProcessId,
        },
        UI::{
            Input::KeyboardAndMouse::GetAsyncKeyState,
            WindowsAndMessaging::{
                CallWindowProcW, EnumWindows, GW_OWNER, GWLP_WNDPROC, GetWindow, GetWindowLongPtrW,
                GetWindowRect, GetWindowThreadProcessId, IsWindow, IsWindowVisible,
                SetWindowLongPtrW, WNDPROC,
            },
        },
    };

    unsafe extern "C" {
        fn sf_keyboard_windows_scancode(index: i32) -> i32;
    }

    type GetRawInputDataFn =
        unsafe extern "system" fn(*mut c_void, u32, *mut c_void, *mut u32, u32) -> u32;

    const RID_INPUT: u32 = 0x1000_0003;
    const RIM_TYPEMOUSE: u32 = 0;
    const RIM_TYPEKEYBOARD: u32 = 1;
    const RI_KEY_BREAK: u16 = 1;
    const RI_KEY_E0: u16 = 2;
    const RI_KEY_E1: u16 = 4;
    const RI_MOUSE_WHEEL: u16 = 0x0400;
    const RI_MOUSE_HWHEEL: u16 = 0x0800;
    const WHEEL_DELTA: i32 = 120;
    /// SDL button, Raw Input down flag, up flag.
    const RAW_BUTTONS: [(u8, u16, u16); 5] = [
        (1, 0x0001, 0x0002),
        (3, 0x0004, 0x0008),
        (2, 0x0010, 0x0020),
        (4, 0x0040, 0x0080),
        (5, 0x0100, 0x0200),
    ];

    const WM_KEYDOWN: u32 = 0x0100;
    const WM_KEYUP: u32 = 0x0101;
    const WM_CHAR: u32 = 0x0102;
    const WM_DEADCHAR: u32 = 0x0103;
    const WM_SYSKEYDOWN: u32 = 0x0104;
    const WM_SYSKEYUP: u32 = 0x0105;
    const WM_SYSCHAR: u32 = 0x0106;
    const WM_LBUTTONDOWN: u32 = 0x0201;
    const WM_LBUTTONUP: u32 = 0x0202;
    const WM_LBUTTONDBLCLK: u32 = 0x0203;
    const WM_RBUTTONDOWN: u32 = 0x0204;
    const WM_RBUTTONUP: u32 = 0x0205;
    const WM_RBUTTONDBLCLK: u32 = 0x0206;
    const WM_MBUTTONDOWN: u32 = 0x0207;
    const WM_MBUTTONUP: u32 = 0x0208;
    const WM_MBUTTONDBLCLK: u32 = 0x0209;
    const WM_MOUSEWHEEL: u32 = 0x020A;
    const WM_XBUTTONDOWN: u32 = 0x020B;
    const WM_XBUTTONUP: u32 = 0x020C;
    const WM_XBUTTONDBLCLK: u32 = 0x020D;
    const WM_MOUSEHWHEEL: u32 = 0x020E;

    #[repr(C)]
    struct RawHeader {
        kind: u32,
        size: u32,
        device: *mut c_void,
        wparam: usize,
    }

    #[repr(C)]
    struct RawKeyboard {
        make_code: u16,
        flags: u16,
        reserved: u16,
        virtual_key: u16,
        message: u32,
        extra: u32,
    }

    #[repr(C)]
    struct RawMouse {
        flags: u16,
        padding: u16,
        button_flags: u16,
        button_data: u16,
        raw_buttons: u32,
        x: i32,
        y: i32,
        extra: u32,
    }

    #[repr(C)]
    struct ImportDescriptor {
        original_first_thunk: u32,
        time_date_stamp: u32,
        forwarder_chain: u32,
        name: u32,
        first_thunk: u32,
    }

    static ORIGINAL_RAW_INPUT: AtomicUsize = AtomicUsize::new(0);
    static ORIGINAL_WINDOW_PROC: AtomicIsize = AtomicIsize::new(0);
    static WINDOW: AtomicIsize = AtomicIsize::new(0);
    static RAW_FILTERED: AtomicBool = AtomicBool::new(false);
    static RAW_TRIED: AtomicBool = AtomicBool::new(false);
    static NEXT_WINDOW_CHECK: Mutex<Option<Instant>> = Mutex::new(None);
    static RAW_KEPT: Mutex<KeptPresses> = Mutex::new(KeptPresses::new());
    static MESSAGE_KEPT: Mutex<KeptPresses> = Mutex::new(KeptPresses::new());
    static RAW_WHEEL_REST: AtomicI32 = AtomicI32::new(0);
    static MESSAGE_WHEEL_REST: AtomicI32 = AtomicI32::new(0);

    pub(super) fn install() -> Result<(), String> {
        if !RAW_TRIED.swap(true, Ordering::AcqRel) {
            match unsafe { replace_raw_input_import() } {
                Ok(()) => {
                    RAW_FILTERED.store(true, Ordering::Release);
                    tracing::info!(target: "shroudforge::hotkeys", "Input reservations: the game's Raw Input is filtered");
                }
                Err(error) => {
                    tracing::warn!(target: "shroudforge::hotkeys", "Input reservations: Raw Input not filtered: {error}");
                }
            }
        }
        let Ok(mut next) = NEXT_WINDOW_CHECK.lock() else {
            return Err("input filter state is unavailable".into());
        };
        let now = Instant::now();
        if next.is_some_and(|next| now < next) {
            return Ok(());
        }
        *next = Some(now + Duration::from_secs(1));
        drop(next);
        unsafe { filter_window() }
    }

    /// Replaces the game executable's import slot of GetRawInputData.
    unsafe fn replace_raw_input_import() -> Result<(), String> {
        let base = unsafe { GetModuleHandleW(std::ptr::null()) } as *const u8;
        if base.is_null() {
            return Err("no game module".into());
        }
        let nt = unsafe { base.add(*(base.add(0x3c) as *const u32) as usize) };
        // PE32+: optional header at 24, import directory at 24 + 112 + 8.
        let imports = unsafe { *(nt.add(24 + 112 + 8) as *const u32) } as usize;
        if imports == 0 {
            return Err("the game has no import table".into());
        }
        let mut entry = unsafe { base.add(imports) } as *const ImportDescriptor;
        while unsafe { (*entry).name } != 0 {
            let name = unsafe { std::ffi::CStr::from_ptr(base.add((*entry).name as usize).cast()) };
            if name.to_bytes().eq_ignore_ascii_case(b"user32.dll") {
                let names_offset = unsafe {
                    if (*entry).original_first_thunk != 0 {
                        (*entry).original_first_thunk
                    } else {
                        (*entry).first_thunk
                    }
                };
                let mut names = unsafe { base.add(names_offset as usize) } as *const u64;
                let mut slot = unsafe { base.add((*entry).first_thunk as usize) } as *mut usize;
                while unsafe { *names } != 0 {
                    let thunk = unsafe { *names };
                    if thunk & (1 << 63) == 0 {
                        let function = unsafe {
                            std::ffi::CStr::from_ptr(base.add(thunk as usize + 2).cast())
                        };
                        if function.to_bytes() == b"GetRawInputData" {
                            let mut protection = 0;
                            if unsafe {
                                VirtualProtect(
                                    slot.cast(),
                                    size_of::<usize>(),
                                    PAGE_READWRITE,
                                    &mut protection,
                                )
                            } == 0
                            {
                                return Err("its import slot cannot be written".into());
                            }
                            ORIGINAL_RAW_INPUT.store(unsafe { *slot }, Ordering::Release);
                            unsafe { *slot = filtered_raw_input as *const () as usize };
                            unsafe {
                                VirtualProtect(
                                    slot.cast(),
                                    size_of::<usize>(),
                                    protection,
                                    &mut protection,
                                )
                            };
                            return Ok(());
                        }
                    }
                    names = unsafe { names.add(1) };
                    slot = unsafe { slot.add(1) };
                }
            }
            entry = unsafe { entry.add(1) };
        }
        Err("the game does not import GetRawInputData".into())
    }

    unsafe extern "system" fn find_game_window(window: HWND, best: LPARAM) -> i32 {
        let mut process = 0;
        unsafe { GetWindowThreadProcessId(window, &mut process) };
        if process != unsafe { GetCurrentProcessId() }
            || unsafe { IsWindowVisible(window) } == 0
            || !unsafe { GetWindow(window, GW_OWNER) }.is_null()
        {
            return 1;
        }
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe { GetWindowRect(window, &mut rect) };
        let area = i64::from(rect.right - rect.left) * i64::from(rect.bottom - rect.top);
        let best = unsafe { &mut *(best as *mut (i64, isize)) };
        if area > best.0 {
            *best = (area, window as isize);
        }
        1
    }

    /// Subclasses the game's main window (its largest visible top-level
    /// window), again if the game replaced it.
    unsafe fn filter_window() -> Result<(), String> {
        let current = WINDOW.load(Ordering::Acquire) as HWND;
        if !current.is_null()
            && unsafe { IsWindow(current) } != 0
            && unsafe { GetWindowLongPtrW(current, GWLP_WNDPROC) }
                == filtered_window_proc as *const () as isize
        {
            return Ok(());
        }
        let mut best: (i64, isize) = (0, 0);
        unsafe { EnumWindows(Some(find_game_window), &mut best as *mut _ as LPARAM) };
        if best.1 == 0 {
            return Err("the game window is not open yet".into());
        }
        let window = best.1 as HWND;
        let previous = unsafe {
            SetWindowLongPtrW(
                window,
                GWLP_WNDPROC,
                filtered_window_proc as *const () as isize,
            )
        };
        if previous == 0 {
            return Err("the game window cannot be filtered".into());
        }
        ORIGINAL_WINDOW_PROC.store(previous, Ordering::Release);
        WINDOW.store(window as isize, Ordering::Release);
        tracing::info!(target: "shroudforge::hotkeys", "Input reservations: the game window's messages are filtered");
        Ok(())
    }

    /// Ctrl 1, Alt 2, Shift 4, Meta 8, as bindings write them.
    fn modifiers() -> u8 {
        let down = |key: i32| unsafe { GetAsyncKeyState(key) } < 0;
        (if down(0x11) { 1 } else { 0 })
            | (if down(0x12) { 2 } else { 0 })
            | (if down(0x10) { 4 } else { 0 })
            | (if down(0x5B) || down(0x5C) { 8 } else { 0 })
    }

    fn sdl_scancode(scan: u16, extended: bool) -> u16 {
        let index = i32::from(scan & 0x7f) | if extended { 0x80 } else { 0 };
        u16::try_from(unsafe { sf_keyboard_windows_scancode(index) }).unwrap_or(0)
    }

    /// Whole wheel steps in `delta`, keeping the rest of a high resolution wheel.
    fn wheel_steps(rest: &AtomicI32, delta: i32) -> i32 {
        let total = rest.load(Ordering::Relaxed) + delta;
        rest.store(total % WHEEL_DELTA, Ordering::Relaxed);
        total / WHEEL_DELTA
    }

    unsafe extern "system" fn filtered_raw_input(
        input: *mut c_void,
        command: u32,
        data: *mut c_void,
        size: *mut u32,
        header_size: u32,
    ) -> u32 {
        let original: GetRawInputDataFn =
            unsafe { std::mem::transmute(ORIGINAL_RAW_INPUT.load(Ordering::Acquire)) };
        let result = unsafe { original(input, command, data, size, header_size) };
        if command != RID_INPUT || data.is_null() || result == u32::MAX || !anything_reserved() {
            return result;
        }
        let read = result as usize;
        let header = unsafe { &*(data as *const RawHeader) };
        let body = unsafe { (data as *mut u8).add(size_of::<RawHeader>()) };
        if header.kind == RIM_TYPEKEYBOARD
            && read >= size_of::<RawHeader>() + size_of::<RawKeyboard>()
        {
            let key = unsafe { &mut *(body as *mut RawKeyboard) };
            if key.flags & RI_KEY_E1 != 0 {
                return result;
            }
            let code = sdl_scancode(key.make_code, key.flags & RI_KEY_E0 != 0);
            let down = key.flags & RI_KEY_BREAK == 0;
            if code != 0
                && RAW_KEPT
                    .lock()
                    .is_ok_and(|mut kept| kept.key(code, down, modifiers()))
            {
                // No key: the game ignores it.
                key.virtual_key = 0xFF;
                key.make_code = 0;
            }
        } else if header.kind == RIM_TYPEMOUSE
            && read >= size_of::<RawHeader>() + size_of::<RawMouse>()
        {
            let mouse = unsafe { &mut *(body as *mut RawMouse) };
            if mouse.button_flags == 0 {
                return result;
            }
            let held = modifiers();
            if let Ok(mut kept) = RAW_KEPT.lock() {
                for (button, down, up) in RAW_BUTTONS {
                    if mouse.button_flags & down != 0 && kept.button(button, true, held) {
                        mouse.button_flags &= !down;
                    }
                    if mouse.button_flags & up != 0 && kept.button(button, false, held) {
                        mouse.button_flags &= !up;
                    }
                }
            }
            if mouse.button_flags & (RI_MOUSE_WHEEL | RI_MOUSE_HWHEEL) != 0 {
                let vertical = mouse.button_flags & RI_MOUSE_WHEEL != 0;
                let delta = i32::from(mouse.button_data as i16);
                let steps = if vertical {
                    wheel_steps(&RAW_WHEEL_REST, delta)
                } else {
                    0
                };
                if wheel(steps, vertical) {
                    mouse.button_flags &= !(RI_MOUSE_WHEEL | RI_MOUSE_HWHEEL);
                    mouse.button_data = 0;
                }
            }
        }
        result
    }

    /// Some(result) when the game must not get this message.
    fn keep_message(message: u32, w: WPARAM, l: LPARAM) -> Option<LRESULT> {
        let scan = || ((l >> 16) & 0xff) as u16;
        let extended = || (l >> 24) & 1 != 0;
        let button = |button: u8, down: bool, result: LRESULT| {
            MESSAGE_KEPT
                .lock()
                .is_ok_and(|mut kept| kept.button(button, down, modifiers()))
                .then_some(result)
        };
        let x_button = || if (w >> 16) & 0xffff == 2 { 5 } else { 4 };
        match message {
            WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP => {
                let code = sdl_scancode(scan(), extended());
                let down = matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN);
                (code != 0
                    && MESSAGE_KEPT
                        .lock()
                        .is_ok_and(|mut kept| kept.key(code, down, modifiers())))
                .then_some(0)
            }
            WM_CHAR | WM_DEADCHAR | WM_SYSCHAR => {
                let code = sdl_scancode(scan(), extended());
                MESSAGE_KEPT
                    .lock()
                    .is_ok_and(|kept| code != 0 && kept.holds_key(code))
                    .then_some(0)
            }
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => button(1, true, 0),
            WM_LBUTTONUP => button(1, false, 0),
            WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => button(3, true, 0),
            WM_RBUTTONUP => button(3, false, 0),
            WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => button(2, true, 0),
            WM_MBUTTONUP => button(2, false, 0),
            WM_XBUTTONDOWN | WM_XBUTTONDBLCLK => button(x_button(), true, 1),
            WM_XBUTTONUP => button(x_button(), false, 1),
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                // Counted from Raw Input when it is filtered; from here otherwise.
                let count = message == WM_MOUSEWHEEL && !RAW_FILTERED.load(Ordering::Acquire);
                let delta = i32::from(((w >> 16) & 0xffff) as u16 as i16);
                let steps = if count {
                    wheel_steps(&MESSAGE_WHEEL_REST, delta)
                } else {
                    0
                };
                wheel(steps, count).then_some(0)
            }
            _ => None,
        }
    }

    unsafe extern "system" fn filtered_window_proc(
        window: HWND,
        message: u32,
        w: WPARAM,
        l: LPARAM,
    ) -> LRESULT {
        if anything_reserved()
            && let Some(result) = keep_message(message, w, l)
        {
            return result;
        }
        let original: WNDPROC =
            unsafe { std::mem::transmute(ORIGINAL_WINDOW_PROC.load(Ordering::Acquire)) };
        unsafe { CallWindowProcW(original, window, message, w, l) }
    }
}
