//! Steno keys and canonical stroke rendering.
//!
//! A *stroke* is the set of steno keys pressed together in one chord, written
//! in a fixed order. Steno notation puts the keys in "steno order" and, when a
//! stroke has no middle (vowel/star) key, inserts a hyphen to separate the
//! left bank from the right bank — so `S` + `-T` renders as `S-T`, but
//! `K` + `A` + `-T` renders as `KAT`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Which bank of the steno keyboard a key belongs to. The middle bank
/// (vowels and the star) is what decides whether a hyphen is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bank {
    /// Left-hand consonants (rendered first, no hyphen).
    Left,
    /// Vowels and the star (rendered in the middle).
    Mid,
    /// Right-hand consonants (rendered last; hyphen inserted if no `Mid` key).
    Right,
}

/// Definition of one steno key: its stable `id` (e.g. `"S-"`, `"-F"`), the
/// `letter` shown when rendering a stroke (e.g. `"S"`, `"F"`), and its `bank`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StenoKey {
    pub id: String,
    pub letter: String,
    pub bank: Bank,
}

impl StenoKey {
    pub fn new(id: impl Into<String>, letter: impl Into<String>, bank: Bank) -> Self {
        Self { id: id.into(), letter: letter.into(), bank }
    }
}

/// Render a canonical stroke string from the set of pressed steno key *ids*,
/// using `order` (the pack's ordered key list) for sequencing and hyphen rules.
///
/// Unknown ids in `present` are ignored. Returns an empty string if nothing in
/// `order` is present.
pub fn render_stroke(order: &[StenoKey], present: &BTreeSet<String>) -> String {
    let has_mid = order
        .iter()
        .any(|k| k.bank == Bank::Mid && present.contains(&k.id));

    let mut out = String::new();
    let mut hyphen_done = false;
    for key in order {
        if !present.contains(&key.id) {
            continue;
        }
        // Insert the disambiguating hyphen once, before the first right-bank
        // key, but only when the stroke has no middle key to separate banks.
        if key.bank == Bank::Right && !has_mid && !hyphen_done {
            out.push('-');
            hyphen_done = true;
        }
        out.push_str(&key.letter);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> Vec<StenoKey> {
        vec![
            StenoKey::new("S-", "S", Bank::Left),
            StenoKey::new("T-", "T", Bank::Left),
            StenoKey::new("K-", "K", Bank::Left),
            StenoKey::new("H-", "H", Bank::Left),
            StenoKey::new("A", "A", Bank::Mid),
            StenoKey::new("O", "O", Bank::Mid),
            StenoKey::new("E", "E", Bank::Mid),
            StenoKey::new("-P", "P", Bank::Right),
            StenoKey::new("-T", "T", Bank::Right),
        ]
    }

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn word_with_vowel_has_no_hyphen() {
        assert_eq!(render_stroke(&order(), &set(&["K-", "A", "-T"])), "KAT");
    }

    #[test]
    fn no_vowel_inserts_hyphen_before_right_bank() {
        assert_eq!(render_stroke(&order(), &set(&["S-", "-T"])), "S-T");
    }

    #[test]
    fn only_left_bank_no_hyphen() {
        assert_eq!(render_stroke(&order(), &set(&["S-", "T-", "H-"])), "STH");
    }

    #[test]
    fn only_right_bank_gets_leading_hyphen() {
        assert_eq!(render_stroke(&order(), &set(&["-P", "-T"])), "-PT");
    }

    #[test]
    fn only_vowel() {
        assert_eq!(render_stroke(&order(), &set(&["A"])), "A");
    }

    #[test]
    fn keys_render_in_order_not_press_order() {
        // Present in "wrong" order — output still follows `order`.
        assert_eq!(render_stroke(&order(), &set(&["-T", "A", "T-"])), "TAT");
    }
}
