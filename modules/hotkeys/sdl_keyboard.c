/* SDL owns platform key translation and its international scancode catalogue.
 * This small C boundary keeps SDL event layouts and constants out of Rust. */
#include "SDL_internal.h"
#include "events/scancodes_windows.h"
#include <windows.h>

typedef struct {
    int scancode;
    unsigned char down;
    unsigned char repeat;
    unsigned char modifiers;
    unsigned char kind;
    Uint64 timestamp;
} SF_KeyboardEvent;

static SDL_Window *layout_window;
static HKL current_layout;
static char initialization_error[512];
static SDL_MouseButtonFlags mouse_state;
static SDL_MouseButtonFlags pending_mouse_changes;
static SDL_MouseButtonFlags pending_mouse_state;
static unsigned char pending_mouse_modifiers;
static Uint64 pending_mouse_timestamp;

static unsigned char modifier_groups(SDL_Keymod modifiers)
{
    return ((modifiers & SDL_KMOD_CTRL) ? 1 : 0)
        | ((modifiers & SDL_KMOD_ALT) ? 2 : 0)
        | ((modifiers & SDL_KMOD_SHIFT) ? 4 : 0)
        | ((modifiers & SDL_KMOD_GUI) ? 8 : 0);
}

int sf_keyboard_modifier(int scancode)
{
    switch (scancode) {
    case SDL_SCANCODE_LCTRL: case SDL_SCANCODE_RCTRL: return 1;
    case SDL_SCANCODE_LALT: case SDL_SCANCODE_RALT: return 2;
    case SDL_SCANCODE_LSHIFT: case SDL_SCANCODE_RSHIFT: return 4;
    case SDL_SCANCODE_LGUI: case SDL_SCANCODE_RGUI: return 8;
    default: return 0;
    }
}

int sf_keyboard_valid_scancode(int scancode)
{
    return scancode > SDL_SCANCODE_UNKNOWN && scancode < SDL_SCANCODE_COUNT;
}

int sf_keyboard_legacy_virtual_key_scancode(int virtual_key)
{
    if (virtual_key == VK_PAUSE) return SDL_SCANCODE_PAUSE;
    if (virtual_key == VK_SNAPSHOT) return SDL_SCANCODE_PRINTSCREEN;

    UINT scan_code = MapVirtualKeyExW(
        (UINT)virtual_key,
        MAPVK_VK_TO_VSC_EX,
        GetKeyboardLayout(0));
    if (scan_code == 0) return SDL_SCANCODE_UNKNOWN;
    if ((scan_code & 0xff00) == 0xe000) {
        scan_code = (scan_code & 0xff) | 0x80;
    } else if ((scan_code & 0xff00) != 0) {
        return SDL_SCANCODE_UNKNOWN;
    }
    return scan_code < SDL_arraysize(windows_scancode_table)
        ? windows_scancode_table[scan_code]
        : SDL_SCANCODE_UNKNOWN;
}

/* A Windows scan code (low 7 bits, 0x80 for the E0 prefix, as Raw Input and
 * key messages report it) as the SDL scancode bindings use. */
int sf_keyboard_windows_scancode(int index)
{
    return index >= 0 && index < (int)SDL_arraysize(windows_scancode_table)
        ? windows_scancode_table[index]
        : SDL_SCANCODE_UNKNOWN;
}

int sf_keyboard_scancode_count(void) { return SDL_SCANCODE_COUNT; }
const char *sf_keyboard_error(void) { return initialization_error; }

static int initialization_failed(void)
{
    SDL_strlcpy(initialization_error, SDL_GetError(), sizeof(initialization_error));
    if (layout_window) SDL_DestroyWindow(layout_window);
    layout_window = NULL;
    SDL_Quit();
    return 0;
}

int sf_keyboard_initialize(void)
{
    SDL_SetMainReady();
    initialization_error[0] = '\0';
    SDL_SetHintWithPriority(SDL_HINT_WINDOWS_GAMEINPUT, "0", SDL_HINT_OVERRIDE);
    SDL_SetHintWithPriority(SDL_HINT_WINDOWS_RAW_KEYBOARD, "1", SDL_HINT_OVERRIDE);
    SDL_SetHintWithPriority(SDL_HINT_WINDOWS_RAW_KEYBOARD_INPUTSINK, "1", SDL_HINT_OVERRIDE);
    SDL_SetHintWithPriority(SDL_HINT_WINDOWS_RAW_KEYBOARD_EXCLUDE_HOTKEYS, "0", SDL_HINT_OVERRIDE);
    SDL_SetHintWithPriority(SDL_HINT_MOUSE_AUTO_CAPTURE, "0", SDL_HINT_OVERRIDE);
    SDL_SetHintWithPriority(SDL_HINT_VIDEO_ALLOW_SCREENSAVER, "1", SDL_HINT_OVERRIDE);
    if (!SDL_Init(SDL_INIT_VIDEO)) return initialization_failed();
    layout_window = SDL_CreateWindow("ShroudForge keyboard input", 1, 1, SDL_WINDOW_HIDDEN);
    if (!layout_window) {
        return initialization_failed();
    }
    /* SDL hint callbacks do not propagate registration failures through Init.
     * Verify the result without registering a second input receiver. */
    UINT count = 0;
    bool registered = false;
    if (GetRegisteredRawInputDevices(NULL, &count, sizeof(RAWINPUTDEVICE)) == 0 && count) {
        RAWINPUTDEVICE *devices = SDL_calloc(count, sizeof(RAWINPUTDEVICE));
        if (devices) {
            UINT received = GetRegisteredRawInputDevices(devices, &count, sizeof(RAWINPUTDEVICE));
            if (received != (UINT)-1) {
                for (UINT i = 0; i < received; ++i) {
                    DWORD process = 0;
                    GetWindowThreadProcessId(devices[i].hwndTarget, &process);
                    if (devices[i].usUsagePage == 1 && devices[i].usUsage == 6
                        && process == GetCurrentProcessId()
                        && (devices[i].dwFlags & RIDEV_INPUTSINK)
                        && !(devices[i].dwFlags & (RIDEV_NOHOTKEYS | RIDEV_NOLEGACY))) {
                        registered = true;
                    }
                }
            }
            SDL_free(devices);
        }
    }
    if (!registered) {
        SDL_SetError("SDL did not register observing background keyboard input");
        return initialization_failed();
    }
    current_layout = GetKeyboardLayout(0);
    mouse_state = SDL_GetGlobalMouseState(NULL, NULL);
    pending_mouse_changes = 0;
    return 1;
}

/* Follow the foreground thread's layout in our own SDL thread. The game's
 * thread and keyboard settings are never changed. SDL rebuilds its keymap. */
int sf_keyboard_sync_layout(void)
{
    DWORD thread = GetWindowThreadProcessId(GetForegroundWindow(), NULL);
    HKL layout = thread ? GetKeyboardLayout(thread) : current_layout;
    if (!layout || layout == current_layout) return 0;
    HWND window = (HWND)SDL_GetPointerProperty(SDL_GetWindowProperties(layout_window),
        SDL_PROP_WINDOW_WIN32_HWND_POINTER, NULL);
    if (!window || !ActivateKeyboardLayout(layout, 0)) return 0;
    SendMessageW(window, WM_INPUTLANGCHANGE, 0, (LPARAM)layout);
    current_layout = layout;
    return 1;
}

const char *sf_keyboard_label(int scancode)
{
    SDL_Keycode key = SDL_GetKeyFromScancode((SDL_Scancode)scancode, SDL_KMOD_NONE, false);
    const char *name = SDL_GetKeyName(key);
    return name && name[0] ? name : SDL_GetScancodeName((SDL_Scancode)scancode);
}

const char *sf_keyboard_mouse_label(int button)
{
    switch (button) {
    case SDL_BUTTON_LEFT: return "Mouse Left";
    case SDL_BUTTON_MIDDLE: return "Mouse Middle";
    case SDL_BUTTON_RIGHT: return "Mouse Right";
    case SDL_BUTTON_X1: return "Mouse X1";
    case SDL_BUTTON_X2: return "Mouse X2";
    default: return "";
    }
}

Uint64 sf_keyboard_now_ns(void) { return SDL_GetTicksNS(); }

/* SDL's Windows raw mouse backend requires relative mode and an SDL-focused
 * window. Its global state API observes buttons without capture or changing
 * another application's input registration. Changes between samples can be
 * missed, so keep the existing SDL loop's wait bounded to five milliseconds. */
static int poll_mouse_buttons(SF_KeyboardEvent *output)
{
    if (!pending_mouse_changes) {
        SDL_MouseButtonFlags state = SDL_GetGlobalMouseState(NULL, NULL);
        pending_mouse_changes = state ^ mouse_state;
        mouse_state = state;
        if (!pending_mouse_changes) return 0;
        pending_mouse_state = state;
        pending_mouse_modifiers = modifier_groups(SDL_GetModState());
        pending_mouse_timestamp = SDL_GetTicksNS();
    }
    for (int button = 1; button <= 32; ++button) {
        SDL_MouseButtonFlags mask = SDL_BUTTON_MASK(button);
        if (pending_mouse_changes & mask) {
            pending_mouse_changes &= ~mask;
            output->scancode = button;
            output->down = (pending_mouse_state & mask) != 0;
            output->repeat = 0;
            output->modifiers = pending_mouse_modifiers;
            output->kind = 1;
            output->timestamp = pending_mouse_timestamp;
            return 1;
        }
    }
    return 0;
}

int sf_keyboard_poll(SF_KeyboardEvent *output, int timeout)
{
    SDL_Event event;
    /* Pump and drain queued keyboard transitions before sampling mouse
     * buttons so a new mouse chord sees SDL's current modifier state. */
    if (SDL_PollEvent(&event)) goto deliver_event;
    if (poll_mouse_buttons(output)) return 1;
    if (timeout < 0 || timeout > 5) timeout = 5;
    if (!SDL_WaitEventTimeout(&event, timeout)) return poll_mouse_buttons(output);
deliver_event:
    if (event.type == SDL_EVENT_KEYMAP_CHANGED) return 2;
    if (event.type != SDL_EVENT_KEY_DOWN && event.type != SDL_EVENT_KEY_UP) {
        return poll_mouse_buttons(output);
    }
    output->scancode = event.key.scancode;
    output->down = event.key.down;
    output->repeat = event.key.repeat;
    output->modifiers = modifier_groups(event.key.mod);
    output->kind = 0;
    output->timestamp = event.key.timestamp;
    return 1;
}

void sf_keyboard_quit(void)
{
    SDL_DestroyWindow(layout_window);
    layout_window = NULL;
    SDL_Quit();
}
