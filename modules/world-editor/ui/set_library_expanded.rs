use super::position_editor_window::position_window;
use super::position_editor_window::set_fixed_size;
use super::read_editor_arguments::Arguments;
use crate::panel_layout::DEFAULT_HEIGHT;
use tao::dpi::LogicalSize;

pub(crate) fn set_library_expanded(
    expanded: bool,
    height: u32,
    library_expanded: &mut bool,
    library_expanded_height: &mut u32,
    window: &tao::window::Window,
    arguments: &Arguments,
    panel_width: u32,
    panel_height: u32,
    panel_position_x: i32,
    panel_position_y: u32,
) {
    *library_expanded = expanded;
    *library_expanded_height = height.clamp(DEFAULT_HEIGHT, 720);
    let height = if *library_expanded {
        *library_expanded_height as f64
    } else {
        panel_height as f64
    };
    set_fixed_size(window, panel_width, height);
    window.set_inner_size(LogicalSize::new(panel_width as f64, height));
    position_window(
        window,
        arguments.game_pid,
        panel_width as f64,
        panel_position_x,
        panel_position_y,
    );
}
