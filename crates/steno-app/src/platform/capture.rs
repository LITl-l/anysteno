//! Global keyboard capture for system-wide mode.
//!
//! `rdev::listen` blocks, so it runs on its own thread and forwards each key
//! event to the GUI over a channel. Capture is started lazily (only when the
//! user first switches to system-wide mode) so in-app-only users never trigger
//! an OS permission prompt.
//!
//! Honest limitation: `listen` observes keys but does not *suppress* them, so
//! in system-wide mode the raw letters still reach the focused app in addition
//! to the injected translation. True suppression needs per-OS grab APIs
//! (planned). See the README.

use std::thread;

use crossbeam_channel::{unbounded, Receiver};
use rdev::EventType;
use steno_core::KeyEvent;

use super::keymap::rdev_key_name;

/// Start the global capture thread and return the receiving end of the key
/// event channel. The thread runs for the remainder of the process.
pub fn spawn() -> Receiver<KeyEvent> {
    let (tx, rx) = unbounded();
    thread::spawn(move || {
        let callback = move |event: rdev::Event| match event.event_type {
            EventType::KeyPress(key) => {
                let _ = tx.send(KeyEvent::down(rdev_key_name(key)));
            }
            EventType::KeyRelease(key) => {
                let _ = tx.send(KeyEvent::up(rdev_key_name(key)));
            }
            _ => {}
        };
        if let Err(err) = rdev::listen(callback) {
            eprintln!("anysteno: global capture unavailable: {err:?}");
        }
    });
    rx
}
