//! Translate OS-specific key identifiers into the platform-neutral names that
//! `steno-core` layouts use (`"KeyA"`, `"SemiColon"`, `"Quote"`, ...).
//!
//! Two sources need mapping:
//!   * `rdev` — global capture for system-wide mode. Its `Key` enum's `Debug`
//!     names already match our convention, so we reuse them directly.
//!   * `egui` — the in-app practice window's own key events.

/// Name for an rdev key. rdev's `Debug` output (`KeyA`, `SemiColon`, ...) is
/// exactly the convention our layouts use.
pub fn rdev_key_name(key: rdev::Key) -> String {
    format!("{key:?}")
}

/// Name for an egui key, or `None` for keys anysteno never chords. Only the
/// keys used by the shipped layouts are mapped; extend as needed.
pub fn egui_key_name(key: egui::Key) -> Option<&'static str> {
    use egui::Key;
    Some(match key {
        Key::A => "KeyA",
        Key::B => "KeyB",
        Key::C => "KeyC",
        Key::D => "KeyD",
        Key::E => "KeyE",
        Key::F => "KeyF",
        Key::G => "KeyG",
        Key::H => "KeyH",
        Key::I => "KeyI",
        Key::J => "KeyJ",
        Key::K => "KeyK",
        Key::L => "KeyL",
        Key::M => "KeyM",
        Key::N => "KeyN",
        Key::O => "KeyO",
        Key::P => "KeyP",
        Key::Q => "KeyQ",
        Key::R => "KeyR",
        Key::S => "KeyS",
        Key::T => "KeyT",
        Key::U => "KeyU",
        Key::V => "KeyV",
        Key::W => "KeyW",
        Key::X => "KeyX",
        Key::Y => "KeyY",
        Key::Z => "KeyZ",
        Key::Semicolon => "SemiColon",
        Key::Quote => "Quote",
        Key::OpenBracket => "LeftBracket",
        Key::CloseBracket => "RightBracket",
        Key::Backslash => "BackSlash",
        Key::Comma => "Comma",
        Key::Period => "Dot",
        Key::Slash => "Slash",
        Key::Minus => "Minus",
        _ => return None,
    })
}
