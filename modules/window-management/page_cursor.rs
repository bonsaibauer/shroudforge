use tao::window::CursorIcon;

/// Map CSS cursor keywords to the matching native Tao cursor.
pub fn page_cursor_icon(name: &str) -> Option<CursorIcon> {
    Some(match name {
        "default" | "auto" => CursorIcon::Default,
        "pointer" => CursorIcon::Hand,
        "text" => CursorIcon::Text,
        "vertical-text" => CursorIcon::VerticalText,
        "not-allowed" => CursorIcon::NotAllowed,
        "no-drop" => CursorIcon::NoDrop,
        "wait" => CursorIcon::Wait,
        "progress" => CursorIcon::Progress,
        "help" => CursorIcon::Help,
        "crosshair" => CursorIcon::Crosshair,
        "move" => CursorIcon::Move,
        "grab" => CursorIcon::Grab,
        "grabbing" => CursorIcon::Grabbing,
        "all-scroll" => CursorIcon::AllScroll,
        "ew-resize" | "e-resize" | "w-resize" => CursorIcon::EwResize,
        "ns-resize" | "n-resize" | "s-resize" => CursorIcon::NsResize,
        "nesw-resize" | "ne-resize" | "sw-resize" => CursorIcon::NeswResize,
        "nwse-resize" | "nw-resize" | "se-resize" => CursorIcon::NwseResize,
        "col-resize" => CursorIcon::ColResize,
        "row-resize" => CursorIcon::RowResize,
        "copy" => CursorIcon::Copy,
        "alias" => CursorIcon::Alias,
        "cell" => CursorIcon::Cell,
        "context-menu" => CursorIcon::ContextMenu,
        "zoom-in" => CursorIcon::ZoomIn,
        "zoom-out" => CursorIcon::ZoomOut,
        _ => return None,
    })
}
