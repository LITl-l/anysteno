//! Dictionary lookup and multi-stroke translation.
//!
//! A dictionary maps a stroke *key* to output text. A key may span several
//! strokes joined by `/` (e.g. `"KAT/-S"` → `"cats"`). The [`Translator`]
//! buffers strokes and resolves them with **greedy longest match**: it keeps
//! waiting while the buffered strokes could still be the prefix of a longer
//! entry, and otherwise emits the best match (or a "no match" marker).

use std::collections::{HashMap, HashSet};

/// An immutable stroke→text dictionary plus the derived prefix index used for
/// multi-stroke lookahead.
#[derive(Debug, Clone, Default)]
pub struct Dictionary {
    entries: HashMap<String, String>,
    /// Every proper multi-stroke prefix, each ending in `/`
    /// (e.g. entry `"A/B/C"` contributes `"A/"` and `"A/B/"`).
    prefixes: HashSet<String>,
    /// Longest key length in strokes (>= 1, or 0 for an empty dictionary).
    max_len: usize,
}

impl Dictionary {
    /// Build a dictionary from a raw stroke→text map.
    pub fn from_map(entries: HashMap<String, String>) -> Self {
        let mut prefixes = HashSet::new();
        let mut max_len = 0;
        for key in entries.keys() {
            let segs: Vec<&str> = key.split('/').collect();
            max_len = max_len.max(segs.len());
            let mut acc = String::new();
            for (i, seg) in segs.iter().enumerate() {
                if i > 0 {
                    acc.push('/');
                }
                acc.push_str(seg);
                if i < segs.len() - 1 {
                    prefixes.insert(format!("{acc}/"));
                }
            }
        }
        Self { entries, prefixes, max_len }
    }

    /// Overlay `other`'s entries on top of a clone of `self` (user dictionary
    /// wins over the shipped one). Returns the merged dictionary.
    pub fn overlaid_with(&self, other: &HashMap<String, String>) -> Dictionary {
        let mut merged = self.entries.clone();
        for (k, v) in other {
            merged.insert(k.clone(), v.clone());
        }
        Dictionary::from_map(merged)
    }

    pub fn lookup(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// True if `key_with_slash` (which must end in `/`) is the prefix of some
    /// longer multi-stroke entry.
    pub fn is_prefix(&self, key_with_slash: &str) -> bool {
        self.prefixes.contains(key_with_slash)
    }

    pub fn max_len(&self) -> usize {
        self.max_len
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The result of resolving one or more strokes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Translation {
    /// A matched entry: the strokes consumed and the text they produce.
    Text { strokes: Vec<String>, text: String },
    /// A stroke with no dictionary entry — surfaced (never silently dropped)
    /// so beginners see exactly what they chorded.
    NoMatch { stroke: String },
}

/// Buffers strokes and resolves them to [`Translation`]s with greedy longest
/// match and prefix lookahead. Holds mutable state (the pending buffer), so it
/// lives for the duration of a typing session.
#[derive(Debug, Default)]
pub struct Translator {
    buffer: Vec<String>,
}

impl Translator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one completed stroke. Returns any translations that became final;
    /// may return empty if the stroke is being held pending a longer match.
    pub fn push(&mut self, dict: &Dictionary, stroke: String) -> Vec<Translation> {
        self.buffer.push(stroke);
        self.resolve(dict, true)
    }

    /// Force-resolve everything pending (e.g. on space/enter or idle timeout),
    /// giving up on any further lookahead.
    pub fn flush(&mut self, dict: &Dictionary) -> Vec<Translation> {
        self.resolve(dict, false)
    }

    /// Number of strokes currently buffered awaiting disambiguation.
    pub fn pending(&self) -> &[String] {
        &self.buffer
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Core resolution loop. When `allow_wait` is true, stops and keeps the
    /// buffer if it could still extend into a longer entry.
    fn resolve(&mut self, dict: &Dictionary, allow_wait: bool) -> Vec<Translation> {
        let mut out = Vec::new();
        while !self.buffer.is_empty() {
            let joined = self.buffer.join("/");

            // Could more strokes still turn this into a longer entry?
            let can_extend = allow_wait
                && self.buffer.len() < dict.max_len()
                && dict.is_prefix(&format!("{joined}/"));
            if can_extend {
                break; // wait for the next stroke
            }

            if let Some(text) = dict.lookup(&joined) {
                out.push(Translation::Text {
                    strokes: std::mem::take(&mut self.buffer),
                    text: text.to_string(),
                });
                break;
            }

            // The whole buffer doesn't match and can't extend: commit the oldest
            // stroke on its own, then re-evaluate the remainder.
            let first = self.buffer.remove(0);
            match dict.lookup(&first) {
                Some(text) => out.push(Translation::Text {
                    strokes: vec![first],
                    text: text.to_string(),
                }),
                None => out.push(Translation::NoMatch { stroke: first }),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict(pairs: &[(&str, &str)]) -> Dictionary {
        Dictionary::from_map(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    fn text(strokes: &[&str], t: &str) -> Translation {
        Translation::Text {
            strokes: strokes.iter().map(|s| s.to_string()).collect(),
            text: t.to_string(),
        }
    }

    #[test]
    fn single_stroke_matches_immediately() {
        let d = dict(&[("KAT", "cat")]);
        let mut tr = Translator::new();
        assert_eq!(tr.push(&d, "KAT".into()), vec![text(&["KAT"], "cat")]);
    }

    #[test]
    fn unknown_stroke_reports_no_match() {
        let d = dict(&[("KAT", "cat")]);
        let mut tr = Translator::new();
        assert_eq!(
            tr.push(&d, "ZZ".into()),
            vec![Translation::NoMatch { stroke: "ZZ".into() }]
        );
    }

    #[test]
    fn multi_stroke_entry_resolves_when_completed() {
        let d = dict(&[("KAT", "cat"), ("KAT/-S", "cats")]);
        let mut tr = Translator::new();
        // First stroke is a prefix of "KAT/-S" -> held pending, nothing emitted.
        assert_eq!(tr.push(&d, "KAT".into()), vec![]);
        assert_eq!(tr.pending(), &["KAT".to_string()]);
        // Second stroke completes the longer entry.
        assert_eq!(
            tr.push(&d, "-S".into()),
            vec![text(&["KAT", "-S"], "cats")]
        );
    }

    #[test]
    fn prefix_then_non_extending_stroke_commits_shorter_match() {
        let d = dict(&[("KAT", "cat"), ("KAT/-S", "cats"), ("HAT", "hat")]);
        let mut tr = Translator::new();
        assert_eq!(tr.push(&d, "KAT".into()), vec![]); // pending (could be cats)
        // Next stroke can't extend KAT/ -> commit "cat", then resolve "HAT".
        assert_eq!(
            tr.push(&d, "HAT".into()),
            vec![text(&["KAT"], "cat"), text(&["HAT"], "hat")]
        );
    }

    #[test]
    fn flush_forces_pending_to_resolve() {
        let d = dict(&[("KAT", "cat"), ("KAT/-S", "cats")]);
        let mut tr = Translator::new();
        assert_eq!(tr.push(&d, "KAT".into()), vec![]); // pending
        assert_eq!(tr.flush(&d), vec![text(&["KAT"], "cat")]);
        assert!(tr.pending().is_empty());
    }

    #[test]
    fn flush_of_unmatched_prefix_reports_no_match() {
        // "KA" is a prefix but not itself an entry; flushing must surface it.
        let d = dict(&[("KA/KO", "kangaroo")]);
        let mut tr = Translator::new();
        assert_eq!(tr.push(&d, "KA".into()), vec![]); // pending
        assert_eq!(
            tr.flush(&d),
            vec![Translation::NoMatch { stroke: "KA".into() }]
        );
    }

    #[test]
    fn user_overlay_overrides_shipped_entry() {
        let base = dict(&[("KAT", "cat")]);
        let mut user = HashMap::new();
        user.insert("KAT".to_string(), "kitty".to_string());
        let merged = base.overlaid_with(&user);
        assert_eq!(merged.lookup("KAT"), Some("kitty"));
    }
}
