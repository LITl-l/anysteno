//! The dictionary read backwards: text → the strokes that produce it.
//!
//! The forward dictionary answers "what did I just chord?". Learners need the
//! opposite question — "how do I chord *cat*?" — for lesson hints and for the
//! dictionary browser. A dictionary may spell the same text several ways, so
//! the index also has to pick a *best* one.

use std::collections::HashMap;

use crate::dictionary::Dictionary;

/// One dictionary entry, keeping the strokes split rather than joined so the
/// UI can render each stroke on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The strokes to chord, in order (`["KAT", "-S"]`).
    pub strokes: Vec<String>,
    /// The text they produce.
    pub text: String,
}

impl Entry {
    /// The dictionary key form (`"KAT/-S"`).
    pub fn key(&self) -> String {
        self.strokes.join("/")
    }

    /// Ordering used to choose between spellings of the same text: fewest
    /// strokes first, then fewest characters, then alphabetical. The final
    /// tiebreak exists so the choice is deterministic across runs — without it
    /// the winner would depend on `HashMap` iteration order.
    fn is_better_than(&self, other: &Entry) -> bool {
        let key = |e: &Entry| {
            let joined = e.key();
            (e.strokes.len(), joined.chars().count(), joined)
        };
        key(self) < key(other)
    }
}

/// How well an entry matched a search query. Lower sorts first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    TextExact,
    StrokeExact,
    TextPrefix,
    StrokePrefix,
    TextContains,
    StrokeContains,
}

/// An inverted view of a [`Dictionary`], built once per pack.
///
/// Entries are stored once; `best` and `best_order` index into `all` rather
/// than cloning, so a large user dictionary doesn't cost a second copy.
#[derive(Debug, Clone, Default)]
pub struct ReverseIndex {
    /// Every entry, sorted by text then key — a stable order for browsing.
    all: Vec<Entry>,
    /// Output text → index in `all` of its best spelling.
    best: HashMap<String, usize>,
    /// Indices of the best spellings, in text order.
    best_order: Vec<usize>,
}

impl ReverseIndex {
    pub fn build(dict: &Dictionary) -> Self {
        let mut all: Vec<Entry> = dict
            .entries()
            .map(|(key, text)| Entry {
                strokes: key.split('/').map(str::to_string).collect(),
                text: text.to_string(),
            })
            .collect();
        all.sort_by(|a, b| a.text.cmp(&b.text).then_with(|| a.key().cmp(&b.key())));

        let mut best: HashMap<String, usize> = HashMap::new();
        for (i, entry) in all.iter().enumerate() {
            match best.get(&entry.text) {
                Some(&current) if !entry.is_better_than(&all[current]) => {}
                _ => {
                    best.insert(entry.text.clone(), i);
                }
            }
        }

        // `all` is text-sorted, so collecting the winners in index order also
        // yields them in text order.
        let mut best_order: Vec<usize> = best.values().copied().collect();
        best_order.sort_unstable();

        Self { all, best, best_order }
    }

    /// The best strokes for some text, or `None` if the dictionary can't
    /// produce it.
    pub fn strokes_for(&self, text: &str) -> Option<&[String]> {
        self.entry_for(text).map(|e| e.strokes.as_slice())
    }

    /// The best entry for some text.
    pub fn entry_for(&self, text: &str) -> Option<&Entry> {
        self.best.get(text).map(|&i| &self.all[i])
    }

    /// Every entry, sorted by text.
    pub fn entries(&self) -> &[Entry] {
        &self.all
    }

    /// One entry per distinct output text — the best spelling of each — in text
    /// order. This is the candidate pool for lesson generation: drilling two
    /// spellings of the same word would set a target the drill can't tell
    /// apart.
    pub fn best_entries(&self) -> impl Iterator<Item = &Entry> {
        self.best_order.iter().map(|&i| &self.all[i])
    }

    pub fn len(&self) -> usize {
        self.all.len()
    }

    pub fn is_empty(&self) -> bool {
        self.all.is_empty()
    }

    /// Search both sides of the dictionary at once — a query matches if it
    /// appears in the text *or* in the strokes, so "cat" and "KAT" both find
    /// the same entry. Results are ordered exact → prefix → substring, then by
    /// the entry ordering. An empty query returns the first `limit` entries.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&Entry> {
        let query = query.trim();
        if query.is_empty() {
            return self.all.iter().take(limit).collect();
        }
        let text_q = query.to_lowercase();
        let stroke_q = query.to_uppercase();

        let mut hits: Vec<(Rank, &Entry)> = self
            .all
            .iter()
            .filter_map(|e| rank(e, &text_q, &stroke_q).map(|r| (r, e)))
            .collect();

        // `all` is already sorted, and sort_by is stable, so entries of equal
        // rank keep their alphabetical order.
        hits.sort_by_key(|(rank, _)| *rank);
        hits.into_iter().take(limit).map(|(_, e)| e).collect()
    }
}

fn rank(entry: &Entry, text_q: &str, stroke_q: &str) -> Option<Rank> {
    let text = entry.text.to_lowercase();
    let key = entry.key();

    if text == text_q {
        Some(Rank::TextExact)
    } else if key == *stroke_q {
        Some(Rank::StrokeExact)
    } else if text.starts_with(text_q) {
        Some(Rank::TextPrefix)
    } else if key.starts_with(stroke_q) {
        Some(Rank::StrokePrefix)
    } else if text.contains(text_q) {
        Some(Rank::TextContains)
    } else if key.contains(stroke_q) {
        Some(Rank::StrokeContains)
    } else {
        None
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

    #[test]
    fn finds_the_strokes_for_a_word() {
        let idx = ReverseIndex::build(&dict(&[("KAT", "cat"), ("HAT", "hat")]));
        assert_eq!(idx.strokes_for("cat"), Some(&["KAT".to_string()][..]));
        assert_eq!(idx.strokes_for("nothing"), None);
    }

    #[test]
    fn splits_multi_stroke_entries() {
        let idx = ReverseIndex::build(&dict(&[("KAT/-S", "cats")]));
        assert_eq!(
            idx.strokes_for("cats"),
            Some(&["KAT".to_string(), "-S".to_string()][..])
        );
    }

    #[test]
    fn prefers_the_fewest_strokes() {
        let idx = ReverseIndex::build(&dict(&[("K/A/T", "cat"), ("KAT", "cat")]));
        assert_eq!(idx.strokes_for("cat"), Some(&["KAT".to_string()][..]));
    }

    #[test]
    fn prefers_the_shorter_stroke_when_counts_tie() {
        let idx = ReverseIndex::build(&dict(&[("KWAT", "cat"), ("KAT", "cat")]));
        assert_eq!(idx.strokes_for("cat"), Some(&["KAT".to_string()][..]));
    }

    /// Equal-cost spellings must resolve the same way every run, regardless of
    /// the dictionary's hash order.
    #[test]
    fn ties_break_deterministically() {
        let entries = [("KAT", "cat"), ("SAT", "cat"), ("PAT", "cat")];
        let first = ReverseIndex::build(&dict(&entries));
        let reversed: Vec<_> = entries.iter().rev().copied().collect();
        let second = ReverseIndex::build(&dict(&reversed));
        assert_eq!(first.strokes_for("cat"), second.strokes_for("cat"));
        assert_eq!(first.strokes_for("cat"), Some(&["KAT".to_string()][..]));
    }

    #[test]
    fn search_matches_text_and_strokes() {
        let idx = ReverseIndex::build(&dict(&[("KAT", "cat"), ("HAT", "hat")]));
        assert_eq!(idx.search("cat", 10).len(), 1);
        assert_eq!(idx.search("KAT", 10)[0].text, "cat");
    }

    #[test]
    fn search_ranks_exact_before_substring() {
        let idx = ReverseIndex::build(&dict(&[("KAT", "cat"), ("KAT/-S", "cats")]));
        let hits = idx.search("cat", 10);
        assert_eq!(hits[0].text, "cat");
        assert_eq!(hits[1].text, "cats");
    }

    #[test]
    fn search_is_case_insensitive_on_text() {
        let idx = ReverseIndex::build(&dict(&[("KAT", "cat")]));
        assert_eq!(idx.search("CAT", 10)[0].text, "cat");
    }

    #[test]
    fn empty_query_browses_alphabetically() {
        let idx = ReverseIndex::build(&dict(&[("HAT", "hat"), ("KAT", "cat")]));
        let hits = idx.search("", 10);
        assert_eq!(hits[0].text, "cat");
        assert_eq!(hits[1].text, "hat");
    }

    #[test]
    fn best_entries_keeps_one_spelling_per_text() {
        // `en-beginner` really does spell "r" two ways; only one may be drilled.
        let idx = ReverseIndex::build(&dict(&[("R", "r"), ("-R", "r"), ("KAT", "cat")]));
        let best: Vec<_> = idx.best_entries().map(|e| e.key()).collect();
        assert_eq!(best, vec!["KAT", "R"]);
    }

    #[test]
    fn search_respects_the_limit() {
        let idx = ReverseIndex::build(&dict(&[("KAT", "cat"), ("HAT", "hat"), ("SAT", "sat")]));
        assert_eq!(idx.search("", 2).len(), 2);
    }
}
