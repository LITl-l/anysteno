//! Text injection into the focused application (system-wide mode).
//!
//! Wraps `enigo` so the rest of the app deals only in "type this text" /
//! "delete N characters". Construction can fail (e.g. missing macOS
//! Accessibility permission or no display); callers surface that instead of
//! crashing.

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

pub struct Injector {
    enigo: Enigo,
}

impl Injector {
    pub fn new() -> anyhow::Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow::anyhow!("cannot access keyboard output: {e}"))?;
        Ok(Self { enigo })
    }

    /// Type a string into the focused application.
    pub fn text(&mut self, text: &str) -> anyhow::Result<()> {
        self.enigo
            .text(text)
            .map_err(|e| anyhow::anyhow!("injection failed: {e}"))
    }

    /// Delete `count` characters with backspaces (for undo).
    pub fn backspace(&mut self, count: usize) -> anyhow::Result<()> {
        for _ in 0..count {
            self.enigo
                .key(Key::Backspace, Direction::Click)
                .map_err(|e| anyhow::anyhow!("backspace failed: {e}"))?;
        }
        Ok(())
    }
}
