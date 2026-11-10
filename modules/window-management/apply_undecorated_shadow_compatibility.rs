use tao::platform::windows::WindowExtWindows;
use tao::window::Window;

/// Disable Tao shadow insets for an undecorated window under Wine or Proton.
pub fn disable_undecorated_shadow_under_wine(window: &Window) {
    if super::running_under_wine() {
        window.set_undecorated_shadow(false);
    }
}
