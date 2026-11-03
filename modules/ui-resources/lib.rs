mod decode_window_icon;
pub use decode_window_icon::rgba;

#[cfg(windows)]
mod embedded_document;
#[cfg(windows)]
pub use embedded_document::with_embedded_document;
