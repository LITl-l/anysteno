//! Chord accumulation — the mechanism that turns an ordinary keyboard into a
//! steno machine.
//!
//! In stenography you press several keys *at the same time* and the whole group
//! counts as one "stroke". The trick is deciding when the chord is finished: it
//! is complete the moment the **last** held key is released. Until then, any key
//! that goes down joins the chord (the chord only ever grows), which is what
//! lets sloppy, slightly-staggered presses still register as one stroke.
//!
//! Feed this only keys that belong to the layout — the engine filters the rest.

use std::collections::BTreeSet;

/// Accumulates simultaneous key presses into a single chord.
#[derive(Debug, Default)]
pub struct ChordAccumulator {
    /// Keys currently physically held down.
    held: BTreeSet<String>,
    /// Union of every key touched since the chord began (grows only).
    chord: BTreeSet<String>,
    /// True once at least one key has been pressed for the current chord.
    active: bool,
}

impl ChordAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a key press. Chords never complete on press, so this returns
    /// nothing — it just grows the current chord.
    pub fn press(&mut self, key: &str) {
        self.held.insert(key.to_string());
        self.chord.insert(key.to_string());
        self.active = true;
    }

    /// Register a key release. Returns the completed chord's keys when the last
    /// held key is lifted, otherwise `None`.
    pub fn release(&mut self, key: &str) -> Option<BTreeSet<String>> {
        self.held.remove(key);
        if self.active && self.held.is_empty() {
            self.active = false;
            return Some(std::mem::take(&mut self.chord));
        }
        None
    }

    /// Keys currently held (for a live GUI display of the in-progress chord).
    pub fn held(&self) -> &BTreeSet<String> {
        &self.held
    }

    /// The chord built up so far (held plus already-released-this-chord keys).
    pub fn building(&self) -> &BTreeSet<String> {
        &self.chord
    }

    /// Whether a chord is currently in progress.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Abandon any in-progress chord (e.g. when disabling or switching packs).
    pub fn reset(&mut self) {
        self.held.clear();
        self.chord.clear();
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn simultaneous_chord_completes_on_full_release() {
        let mut c = ChordAccumulator::new();
        c.press("K-");
        c.press("A");
        c.press("-T");
        assert_eq!(c.release("K-"), None);
        assert_eq!(c.release("A"), None);
        assert_eq!(c.release("-T"), Some(set(&["K-", "A", "-T"])));
    }

    #[test]
    fn staggered_presses_still_form_one_chord() {
        // Press K, release K, but P is still held -> chord keeps growing.
        let mut c = ChordAccumulator::new();
        c.press("K-");
        c.press("P-");
        assert_eq!(c.release("K-"), None); // P- still held
        c.press("A"); // joins the same chord even though K- was released
        assert_eq!(c.release("P-"), None);
        assert_eq!(c.release("A"), Some(set(&["K-", "P-", "A"])));
    }

    #[test]
    fn next_chord_starts_fresh() {
        let mut c = ChordAccumulator::new();
        c.press("S-");
        assert_eq!(c.release("S-"), Some(set(&["S-"])));
        c.press("T-");
        assert_eq!(c.release("T-"), Some(set(&["T-"])));
    }

    #[test]
    fn duplicate_press_is_idempotent() {
        let mut c = ChordAccumulator::new();
        c.press("S-");
        c.press("S-"); // auto-repeat, say
        assert_eq!(c.release("S-"), Some(set(&["S-"])));
    }

    #[test]
    fn reset_discards_in_progress_chord() {
        let mut c = ChordAccumulator::new();
        c.press("S-");
        c.press("T-");
        c.reset();
        assert!(!c.is_active());
        assert!(c.held().is_empty());
        // A release after reset produces nothing.
        assert_eq!(c.release("S-"), None);
    }
}
