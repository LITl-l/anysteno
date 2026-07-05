//! Mapping from physical keyboard keys to steno keys.
//!
//! Physical keys are identified by platform-neutral names (e.g. `"KeyS"`,
//! `"SemiColon"`) so the core never depends on any OS input library — the
//! platform layer is responsible for producing these names.

use std::collections::HashMap;

use crate::stroke::StenoKey;

/// The steno key geometry for one language pack: the ordered key list (used to
/// render strokes) plus the physical→steno mapping (which real keys chord).
#[derive(Debug, Clone)]
pub struct Layout {
    /// Steno keys in canonical stroke order.
    order: Vec<StenoKey>,
    /// Physical key name -> steno key id. Several physical keys may map to the
    /// same steno key (e.g. two keys both acting as `S-`).
    map: HashMap<String, String>,
}

impl Layout {
    pub fn new(order: Vec<StenoKey>, map: HashMap<String, String>) -> Self {
        Self { order, map }
    }

    /// The ordered steno key list, for stroke rendering and GUI diagrams.
    pub fn order(&self) -> &[StenoKey] {
        &self.order
    }

    /// The steno key id bound to a physical key, if that key chords.
    pub fn steno_for(&self, physical: &str) -> Option<&str> {
        self.map.get(physical).map(String::as_str)
    }

    /// Whether a physical key participates in chording at all.
    pub fn is_steno_key(&self, physical: &str) -> bool {
        self.map.contains_key(physical)
    }

    /// All physical keys bound to a given steno key id (for GUI hints).
    pub fn physical_keys_for(&self, steno_id: &str) -> Vec<&str> {
        self.map
            .iter()
            .filter(|(_, v)| v.as_str() == steno_id)
            .map(|(k, _)| k.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::Bank;

    fn layout() -> Layout {
        let order = vec![
            StenoKey::new("S-", "S", Bank::Left),
            StenoKey::new("A", "A", Bank::Mid),
        ];
        let mut map = HashMap::new();
        map.insert("KeyQ".to_string(), "S-".to_string());
        map.insert("KeyA".to_string(), "S-".to_string()); // two keys -> S-
        map.insert("KeyC".to_string(), "A".to_string());
        Layout::new(order, map)
    }

    #[test]
    fn maps_physical_to_steno() {
        let l = layout();
        assert_eq!(l.steno_for("KeyQ"), Some("S-"));
        assert_eq!(l.steno_for("KeyA"), Some("S-"));
        assert_eq!(l.steno_for("KeyC"), Some("A"));
        assert_eq!(l.steno_for("KeyZ"), None);
    }

    #[test]
    fn recognizes_steno_keys() {
        let l = layout();
        assert!(l.is_steno_key("KeyQ"));
        assert!(!l.is_steno_key("Space"));
    }

    #[test]
    fn reverse_lookup_finds_all_bindings() {
        let l = layout();
        let mut keys = l.physical_keys_for("S-");
        keys.sort();
        assert_eq!(keys, vec!["KeyA", "KeyQ"]);
    }
}
