//! Global keyboard capture for typing into other applications.
//!
//! Two backends, tried in order:
//!
//! * **grab** (`rdev::grab`) — *consumes* the keys it takes, so the focused
//!   application never sees the raw letters. On Linux this goes through evdev,
//!   which sits below the display server and therefore works on X11 **and**
//!   Wayland; on Windows it is a low-level keyboard hook; on macOS a
//!   `CGEventTap`. It needs permission: read/write on `/dev/input` and
//!   `/dev/uinput` (usually the `input` group) on Linux, Accessibility on macOS.
//! * **listen** (`rdev::listen`) — observe only. Used when grab is unavailable.
//!   The raw letters still reach the focused app alongside the translation, so
//!   the app says so rather than pretending otherwise.
//!
//! Only keys the active pack actually chords are ever consumed. Everything else
//! — Escape, modifiers, Tab, function keys, unbound letters — always passes
//! through, so the keyboard can never be "captured" to the point where the user
//! cannot switch away or quit.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

use crossbeam_channel::{unbounded, Receiver};
use rdev::EventType;
use steno_core::KeyEvent;

use super::keymap::rdev_key_name;

/// Which backend a capture ended up using.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Grab has been started but has neither failed nor proven itself yet.
    Starting,
    /// Keys are consumed; the focused app sees only the translation.
    Suppressing,
    /// Keys are observed; the focused app also receives the raw letters.
    Observing,
}

/// How long to let `rdev::grab` fail before believing it is running.
///
/// Grab either refuses during setup (returning an error almost immediately) or
/// blocks forever. There is no success signal to wait for, so the absence of an
/// error is the only evidence available — but it has to be given time to
/// arrive, or a slow refusal would be read as success and no capture would run
/// at all.
const GRAB_GRACE: std::time::Duration = std::time::Duration::from_millis(1500);

/// Decides which physical keys the capture thread consumes.
///
/// Shared with that thread, and read on every keystroke. Every failure mode
/// resolves to "do not consume": a lock that has been poisoned by a panic
/// elsewhere must not be able to swallow the user's keyboard.
#[derive(Debug, Default)]
pub struct Policy {
    keys: RwLock<HashSet<String>>,
    active: AtomicBool,
}

impl Policy {
    pub fn new() -> Self {
        Self::default()
    }

    /// The physical key names the active pack chords.
    pub fn set_keys(&self, keys: HashSet<String>) {
        if let Ok(mut guard) = self.keys.write() {
            *guard = keys;
        }
    }

    /// Whether suppression is wanted at all right now.
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }

    #[cfg_attr(not(feature = "suppress"), allow(dead_code))]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Should this key be taken from the focused application?
    ///
    /// Only the grab backend consults this, so a build without the `suppress`
    /// feature never calls it — but it stays compiled and tested either way.
    #[cfg_attr(not(feature = "suppress"), allow(dead_code))]
    pub fn consumes(&self, physical: &str) -> bool {
        if !self.is_active() {
            return false;
        }
        // Fail open on a poisoned lock: passing a key through is a harmless
        // annoyance, swallowing one is not.
        match self.keys.read() {
            Ok(keys) => keys.contains(physical),
            Err(_) => false,
        }
    }
}

/// What the UI needs to say about suppression right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Capture has not been started yet (the user hasn't chosen this mode).
    NotStarted,
    /// Raw keys are consumed; only the translation reaches the other app.
    Suppressing,
    /// Raw keys also reach the other app, for this reason.
    Observing(String),
}

/// A running global capture.
pub struct Capture {
    pub events: Receiver<KeyEvent>,
    pub policy: Arc<Policy>,
    backend: Backend,
    /// Why suppression is unavailable, when it is.
    note: Option<String>,
    /// Carries a grab failure back from the capture thread, whenever it lands.
    grab_status: Receiver<String>,
    /// Kept so the observing fallback can be started later.
    sender: crossbeam_channel::Sender<KeyEvent>,
    started: std::time::Instant,
}

/// Start capturing. Returns immediately; call [`Capture::poll`] each frame to
/// let the outcome settle.
pub fn spawn() -> Capture {
    let (tx, rx) = unbounded();
    let policy = Arc::new(Policy::new());
    let (status_tx, status_rx) = unbounded::<String>();

    {
        let tx = tx.clone();
        let policy = Arc::clone(&policy);
        thread::spawn(move || {
            if let Some(err) = grab_loop(tx, policy) {
                let _ = status_tx.send(err);
            }
        });
    }

    Capture {
        events: rx,
        policy,
        backend: Backend::Starting,
        note: None,
        grab_status: status_rx,
        sender: tx,
        started: std::time::Instant::now(),
    }
}

impl Capture {
    /// Resolve the backend. Cheap; intended to be called once per frame.
    pub fn poll(&mut self) {
        if self.backend != Backend::Starting {
            return;
        }
        match self.grab_status.try_recv() {
            Ok(err) => {
                self.backend = Backend::Observing;
                self.note = Some(err);
                self.start_observing();
            }
            Err(_) if self.started.elapsed() >= GRAB_GRACE => {
                // No refusal in the grace window, so grab is blocking on events.
                self.backend = Backend::Suppressing;
            }
            Err(_) => {}
        }
    }

    /// Start the observe-only backend, used when grab refused.
    fn start_observing(&self) {
        let tx = self.sender.clone();
        thread::spawn(move || {
            let callback = move |event: rdev::Event| forward(&tx, &event);
            if let Err(err) = rdev::listen(callback) {
                eprintln!("anysteno: global capture unavailable: {err:?}");
            }
        });
    }

    pub fn status(&self) -> Status {
        match self.backend {
            Backend::Starting => Status::NotStarted,
            Backend::Suppressing => Status::Suppressing,
            Backend::Observing => Status::Observing(
                self.note
                    .clone()
                    .unwrap_or_else(|| "suppression unavailable".to_string()),
            ),
        }
    }

    /// The reason suppression is unavailable, once known.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
}

/// Run `rdev::grab`. Returns `Some(reason)` if it refused; on success it blocks
/// for the life of the process and never returns.
fn grab_loop(tx: crossbeam_channel::Sender<KeyEvent>, policy: Arc<Policy>) -> Option<String> {
    #[cfg(feature = "suppress")]
    {
        let callback = move |event: rdev::Event| -> Option<rdev::Event> {
            let name = match event.event_type {
                EventType::KeyPress(key) | EventType::KeyRelease(key) => rdev_key_name(key),
                // Mouse and scroll events are never ours to take.
                _ => return Some(event),
            };
            forward(&tx, &event);
            if policy.consumes(&name) {
                None
            } else {
                Some(event)
            }
        };
        match rdev::grab(callback) {
            Ok(()) => None,
            Err(err) => Some(describe_grab_error(&format!("{err:?}"))),
        }
    }
    #[cfg(not(feature = "suppress"))]
    {
        let _ = (tx, policy);
        Some("this build has the `suppress` feature turned off".to_string())
    }
}

/// Turn rdev's error into something a user can act on.
#[cfg(feature = "suppress")]
fn describe_grab_error(raw: &str) -> String {
    if cfg!(target_os = "linux") {
        if raw.contains("PermissionDenied") || raw.contains("code: 13") {
            return "no permission to read /dev/input — add your user to the `input` group \
                    and log back in"
                .to_string();
        }
        if raw.contains("NotFound") || raw.contains("code: 2") {
            return "no /dev/input devices are visible to this process".to_string();
        }
    }
    if cfg!(target_os = "macos") {
        return "grant anysteno Accessibility permission in System Settings".to_string();
    }
    raw.to_string()
}

fn forward(tx: &crossbeam_channel::Sender<KeyEvent>, event: &rdev::Event) {
    match event.event_type {
        EventType::KeyPress(key) => {
            let _ = tx.send(KeyEvent::down(rdev_key_name(key)));
        }
        EventType::KeyRelease(key) => {
            let _ = tx.send(KeyEvent::up(rdev_key_name(key)));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_with(keys: &[&str]) -> Policy {
        let p = Policy::new();
        p.set_keys(keys.iter().map(|k| k.to_string()).collect());
        p
    }

    #[test]
    fn nothing_is_consumed_while_inactive() {
        let p = policy_with(&["KeyS"]);
        assert!(!p.consumes("KeyS"));
    }

    #[test]
    fn active_policy_consumes_only_its_own_keys() {
        let p = policy_with(&["KeyS", "KeyT"]);
        p.set_active(true);
        assert!(p.consumes("KeyS"));
        assert!(p.consumes("KeyT"));
        assert!(!p.consumes("KeyZ"));
    }

    /// The escape hatch: keys anysteno does not chord must always reach the
    /// system, or a user could not switch away from a misbehaving capture.
    #[test]
    fn control_keys_always_pass_through() {
        let p = policy_with(&["KeyS"]);
        p.set_active(true);
        for key in ["Escape", "Tab", "ControlLeft", "Alt", "MetaLeft", "F4"] {
            assert!(!p.consumes(key), "{key} must never be consumed");
        }
    }

    #[test]
    fn deactivating_releases_every_key() {
        let p = policy_with(&["KeyS"]);
        p.set_active(true);
        assert!(p.consumes("KeyS"));
        p.set_active(false);
        assert!(!p.consumes("KeyS"));
    }

    #[test]
    fn changing_pack_changes_what_is_consumed() {
        let p = policy_with(&["KeyS"]);
        p.set_active(true);
        assert!(p.consumes("KeyS"));
        p.set_keys(["KeyQ".to_string()].into_iter().collect());
        assert!(!p.consumes("KeyS"));
        assert!(p.consumes("KeyQ"));
    }

    #[test]
    fn an_empty_policy_consumes_nothing() {
        let p = Policy::new();
        p.set_active(true);
        assert!(!p.consumes("KeyS"));
    }

    /// A panic elsewhere in the process poisons the lock. The capture thread
    /// must keep passing keys through rather than swallow them.
    #[test]
    fn a_poisoned_lock_fails_open() {
        let p = Arc::new(policy_with(&["KeyS"]));
        p.set_active(true);
        let poisoner = Arc::clone(&p);
        let _ = thread::spawn(move || {
            let _guard = poisoner.keys.write().unwrap();
            panic!("poison the lock");
        })
        .join();
        assert!(p.keys.is_poisoned());
        assert!(!p.consumes("KeyS"), "must fail open when the lock is poisoned");
    }
}
