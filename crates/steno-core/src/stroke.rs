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

/// The inverse of [`render_stroke`]: recover the steno key ids that make up a
/// canonical stroke string. Returns `None` if the stroke does not spell out
/// cleanly in this layout (an unknown letter, or letters out of steno order).
///
/// This is unambiguous *only* because strokes are written in canonical order:
/// walking `order` with a cursor into the string is what distinguishes the
/// left-bank `S-` from the right-bank `-S`, which share the letter `S`. An
/// explicit hyphen forces the remainder to be read from the right bank.
pub fn parse_stroke(order: &[StenoKey], stroke: &str) -> Option<Vec<String>> {
    let mut ids = Vec::new();
    let mut rest = stroke;
    let mut past_hyphen = false;

    for key in order {
        if rest.is_empty() {
            break;
        }
        if !past_hyphen {
            if let Some(after) = rest.strip_prefix('-') {
                past_hyphen = true;
                rest = after;
            }
        }
        // After a hyphen only right-bank keys may still match.
        if past_hyphen && key.bank != Bank::Right {
            continue;
        }
        if let Some(after) = rest.strip_prefix(key.letter.as_str()) {
            ids.push(key.id.clone());
            rest = after;
        }
    }

    rest.is_empty().then_some(ids)
}

/// Parse a whole dictionary key (`"KAT/-S"`) into per-stroke key ids. Returns
/// `None` if any constituent stroke fails to parse.
pub fn parse_stroke_sequence(order: &[StenoKey], key: &str) -> Option<Vec<Vec<String>>> {
    key.split('/').map(|s| parse_stroke(order, s)).collect()
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

    fn parsed(stroke: &str) -> Option<Vec<String>> {
        parse_stroke(&order(), stroke)
    }

    #[test]
    fn parses_a_stroke_with_a_vowel() {
        assert_eq!(parsed("KAT").as_deref(), Some(&["K-".into(), "A".into(), "-T".into()][..]));
    }

    /// The hyphen is what tells us `T` belongs to the right bank here.
    #[test]
    fn hyphen_sends_the_remainder_to_the_right_bank() {
        assert_eq!(parsed("S-T").as_deref(), Some(&["S-".into(), "-T".into()][..]));
    }

    #[test]
    fn parses_a_right_bank_only_stroke() {
        assert_eq!(parsed("-PT").as_deref(), Some(&["-P".into(), "-T".into()][..]));
    }

    #[test]
    fn parses_a_left_bank_only_stroke() {
        assert_eq!(parsed("STH").as_deref(), Some(&["S-".into(), "T-".into(), "H-".into()][..]));
    }

    /// Without the hyphen, the same letter resolves to whichever bank comes
    /// first in steno order — `TAT` is `T-` then `-T`, never `-T` twice.
    #[test]
    fn repeated_letter_resolves_by_position() {
        assert_eq!(parsed("TAT").as_deref(), Some(&["T-".into(), "A".into(), "-T".into()][..]));
    }

    #[test]
    fn unknown_letter_fails_to_parse() {
        assert_eq!(parsed("KXT"), None);
    }

    /// Letters that appear out of steno order can't be chorded as written.
    #[test]
    fn out_of_order_letters_fail_to_parse() {
        assert_eq!(parsed("AK"), None);
    }

    #[test]
    fn parses_a_multi_stroke_key() {
        assert_eq!(
            parse_stroke_sequence(&order(), "KAT/-T"),
            Some(vec![
                vec!["K-".to_string(), "A".to_string(), "-T".to_string()],
                vec!["-T".to_string()],
            ])
        );
    }

    #[test]
    fn a_bad_stroke_fails_the_whole_sequence() {
        assert_eq!(parse_stroke_sequence(&order(), "KAT/XX"), None);
    }

    /// Every stroke the renderer can emit must parse back to the keys it came
    /// from — the two functions are each other's inverse.
    #[test]
    fn render_and_parse_round_trip() {
        for ids in [
            vec!["K-", "A", "-T"],
            vec!["S-", "-T"],
            vec!["-P", "-T"],
            vec!["S-", "T-", "H-"],
            vec!["A"],
            vec!["T-", "A", "-T"],
        ] {
            let rendered = render_stroke(&order(), &set(&ids));
            assert_eq!(parsed(&rendered).as_deref(), Some(&ids.iter().map(|s| s.to_string()).collect::<Vec<_>>()[..]), "round trip failed for {rendered}");
        }
    }
}
