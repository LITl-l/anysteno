//! Thin OS-facing shims: everything that touches the keyboard or the display.
//! Kept deliberately small so the testable logic stays in `steno-core`.

pub mod capture;
pub mod inject;
pub mod keymap;
