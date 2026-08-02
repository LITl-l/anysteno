//! Lessons: a progression derived from a pack, and the drill that runs it.
//!
//! Curricula are **generated, not authored**. A language is still just a folder
//! of a layout and a dictionary, so demanding a hand-written lesson plan per
//! language would break the project's central promise. Instead the progression
//! falls out of the two files that already exist:
//!
//! 1. Introduce steno keys a few at a time, following the layout's canonical
//!    order (which is already grouped by hand).
//! 2. A lesson drills the dictionary entries that are *reachable* with the keys
//!    taught so far and that *use* at least one key just introduced.
//!
//! The result is a typing-tutor progression that adapts to whatever dictionary
//! the user supplies, including one they wrote themselves.

use std::collections::HashSet;

use crate::pack::Pack;
use crate::reverse::ReverseIndex;
use crate::stroke::{parse_stroke_sequence, Bank, StenoKey};

/// How many keys a lesson introduces at once.
const CHUNK: usize = 4;
/// A chunk yielding fewer items than this doesn't become a lesson on its own;
/// its keys are carried into the next one. Some banks have no words of their
/// own — Japanese has no consonant-only entries at all — and a lesson with two
/// items isn't worth a screen.
const MIN_ITEMS: usize = 3;
/// Upper bound on drill length, so importing a 100k-entry Plover dictionary
/// can't produce a lesson nobody could finish. Anything over the limit spills
/// into review lessons rather than being dropped.
const MAX_ITEMS: usize = 16;
/// Ceiling on the whole curriculum. A full Plover dictionary would otherwise
/// spill into thousands of review lessons; past this point free practice is the
/// better tool, so the remainder is intentionally left out.
const MAX_LESSONS: usize = 40;

/// One thing to type in a drill, with the strokes that produce it so the UI can
/// show a hint without a second lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonItem {
    pub text: String,
    pub strokes: Vec<String>,
}

/// A single step of the progression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lesson {
    /// Zero-based position in the curriculum.
    pub index: usize,
    /// The steno keys this lesson teaches.
    pub new_keys: Vec<StenoKey>,
    /// What to drill, easiest first.
    pub items: Vec<LessonItem>,
}

impl Lesson {
    pub fn title(&self) -> String {
        format!("Lesson {}", self.index + 1)
    }

    /// A lesson that introduces no new keys: pure practice over vocabulary that
    /// overflowed the item cap of an earlier lesson.
    pub fn is_review(&self) -> bool {
        self.new_keys.is_empty()
    }

    /// The new keys as a compact label, e.g. `"S T K P"`.
    pub fn keys_label(&self) -> String {
        self.new_keys
            .iter()
            .map(|k| k.letter.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// The full progression for a pack.
#[derive(Debug, Clone, Default)]
pub struct Curriculum {
    lessons: Vec<Lesson>,
}

/// A dictionary entry that survived parsing, with the steno keys it needs.
struct Candidate {
    text: String,
    strokes: Vec<String>,
    keys: HashSet<String>,
}

impl Curriculum {
    /// Build the progression for `pack`.
    pub fn derive(pack: &Pack) -> Curriculum {
        let order = pack.layout.order();
        let candidates = candidates(pack);
        let chunks = key_chunks(order, &pack.undo_stroke);

        let mut lessons: Vec<Lesson> = Vec::new();
        // The undo key counts as available from the start: it is never *taught*
        // as lesson material (there is nothing to drill), but a dictionary may
        // legitimately use it inside a longer stroke, and such entries must
        // still be reachable.
        let mut introduced: HashSet<String> = HashSet::new();
        introduced.insert(pack.undo_stroke.clone());
        let mut used_texts: HashSet<String> = HashSet::new();
        // Keys from chunks that were too thin to stand alone, awaiting a lesson.
        let mut pending: Vec<StenoKey> = Vec::new();

        for chunk in chunks {
            for key in &chunk {
                introduced.insert(key.id.clone());
            }
            pending.extend(chunk);

            let items = select(&candidates, &introduced, Some(&pending), &used_texts);
            if items.len() < MIN_ITEMS {
                continue; // carry `pending` into the next chunk
            }
            for item in &items {
                used_texts.insert(item.text.clone());
            }
            lessons.push(Lesson {
                index: lessons.len(),
                new_keys: std::mem::take(&mut pending),
                items,
            });
        }

        // Whatever the final chunks could reach is below MIN_ITEMS by
        // construction (nothing new is introduced after the loop), so fold it
        // into the last lesson rather than ending on a one-word screen.
        if !pending.is_empty() {
            let items = select(&candidates, &introduced, Some(&pending), &used_texts);
            if !items.is_empty() {
                for item in &items {
                    used_texts.insert(item.text.clone());
                }
                if let Some(last) = lessons.last_mut() {
                    last.new_keys.append(&mut pending);
                    last.items.extend(items);
                } else {
                    lessons.push(Lesson {
                        index: 0,
                        new_keys: std::mem::take(&mut pending),
                        items,
                    });
                }
            }
        }

        // Entries that overflowed a lesson's item cap are still untaught, and
        // every key is introduced by now, so no later lesson would ever pick
        // them up. Spill them into review lessons instead of dropping them.
        while lessons.len() < MAX_LESSONS {
            let items = select(&candidates, &introduced, None, &used_texts);
            if items.is_empty() {
                break;
            }
            for item in &items {
                used_texts.insert(item.text.clone());
            }
            lessons.push(Lesson {
                index: lessons.len(),
                new_keys: Vec::new(),
                items,
            });
        }

        Curriculum { lessons }
    }

    pub fn lessons(&self) -> &[Lesson] {
        &self.lessons
    }

    pub fn get(&self, index: usize) -> Option<&Lesson> {
        self.lessons.get(index)
    }

    pub fn len(&self) -> usize {
        self.lessons.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lessons.is_empty()
    }
}

/// One candidate per distinct output text, cheapest spelling first.
///
/// Deduplicating by *text* matters: `en-beginner` spells "r" as both `R` and
/// `-R`, and a drill can only check the text that came out, not which key made
/// it. Offering both would set a target that cannot be marked wrong.
fn candidates(pack: &Pack) -> Vec<Candidate> {
    let order = pack.layout.order();
    let index = ReverseIndex::build(&pack.dict);

    let mut out: Vec<Candidate> = index
        .best_entries()
        .filter(|e| !e.text.is_empty())
        .filter(|e| e.key() != pack.undo_stroke)
        .filter_map(|e| {
            // A stroke that doesn't parse can't be shown as a hint, and is a
            // sign of a typo in a hand-written dictionary. Skip it rather than
            // drill something unreachable.
            let keys: HashSet<String> = parse_stroke_sequence(order, &e.key())?
                .into_iter()
                .flatten()
                .collect();
            (!keys.is_empty()).then(|| Candidate {
                text: e.text.clone(),
                strokes: e.strokes.clone(),
                keys,
            })
        })
        .collect();

    out.sort_by(|a, b| {
        let cost = |c: &Candidate| {
            (
                c.strokes.len(),
                c.strokes.join("/").chars().count(),
                c.text.clone(),
            )
        };
        cost(a).cmp(&cost(b))
    });
    out
}

/// Entries reachable with `introduced` that haven't been drilled yet.
///
/// `new` restricts the result to entries exercising at least one of those keys,
/// which is what makes a lesson practise what it just taught. Passing `None`
/// lifts that restriction, for review lessons where every key is already known.
fn select(
    candidates: &[Candidate],
    introduced: &HashSet<String>,
    new: Option<&[StenoKey]>,
    used_texts: &HashSet<String>,
) -> Vec<LessonItem> {
    let new_ids: Option<HashSet<&str>> =
        new.map(|keys| keys.iter().map(|k| k.id.as_str()).collect());
    candidates
        .iter()
        .filter(|c| !used_texts.contains(&c.text))
        .filter(|c| c.keys.iter().all(|k| introduced.contains(k)))
        .filter(|c| match &new_ids {
            Some(ids) => c.keys.iter().any(|k| ids.contains(k.as_str())),
            None => true,
        })
        .take(MAX_ITEMS)
        .map(|c| LessonItem {
            text: c.text.clone(),
            strokes: c.strokes.clone(),
        })
        .collect()
}

/// Split the layout into the groups of keys successive lessons introduce.
///
/// The middle bank goes second, straight after the first group of left-hand
/// keys. That bank is by definition the one that joins the hands (it is what
/// decides hyphenation), which in practice means the vowels — and almost no
/// dictionary entry is reachable without one. Teaching it early is what makes
/// lesson two produce words instead of isolated letters.
fn key_chunks(order: &[StenoKey], undo_stroke: &str) -> Vec<Vec<StenoKey>> {
    let bank = |b: Bank| -> Vec<StenoKey> {
        order
            .iter()
            .filter(|k| k.bank == b && k.id != undo_stroke)
            .cloned()
            .collect()
    };
    let (left, mid, right) = (bank(Bank::Left), bank(Bank::Mid), bank(Bank::Right));

    let mut chunks: Vec<Vec<StenoKey>> = Vec::new();
    let mut left_groups = left.chunks(CHUNK);
    if let Some(first) = left_groups.next() {
        chunks.push(first.to_vec());
    }
    if !mid.is_empty() {
        chunks.push(mid);
    }
    chunks.extend(left_groups.map(<[StenoKey]>::to_vec));
    chunks.extend(right.chunks(CHUNK).map(<[StenoKey]>::to_vec));
    chunks
}

/// What a submitted answer was worth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrillOutcome {
    Correct,
    Wrong,
}

/// A run through one lesson's items. Holds position and per-item attempts;
/// timing and scoring live in [`crate::stats::SessionStats`].
#[derive(Debug, Clone, Default)]
pub struct Drill {
    items: Vec<LessonItem>,
    at: usize,
    attempts: usize,
}

impl Drill {
    pub fn new(lesson: &Lesson) -> Self {
        Self {
            items: lesson.items.clone(),
            at: 0,
            attempts: 0,
        }
    }

    /// The item being drilled, or `None` once the lesson is finished.
    pub fn current(&self) -> Option<&LessonItem> {
        self.items.get(self.at)
    }

    /// Judge translated text against the current target. Returns `None` when
    /// the drill is already finished and there is nothing to answer.
    pub fn submit(&mut self, text: &str) -> Option<DrillOutcome> {
        let target = self.current()?.text.clone();
        if text.trim() == target {
            self.at += 1;
            self.attempts = 0;
            Some(DrillOutcome::Correct)
        } else {
            self.attempts += 1;
            Some(DrillOutcome::Wrong)
        }
    }

    /// Give up on the current item and move on.
    pub fn skip(&mut self) {
        if self.at < self.items.len() {
            self.at += 1;
            self.attempts = 0;
        }
    }

    /// Wrong answers on the current item. The UI reveals the stroke hint once
    /// this is non-zero, so help arrives exactly when it's needed.
    pub fn attempts_on_current(&self) -> usize {
        self.attempts
    }

    /// How many items are done.
    pub fn position(&self) -> usize {
        self.at
    }

    pub fn total(&self) -> usize {
        self.items.len()
    }

    pub fn is_complete(&self) -> bool {
        self.at >= self.items.len()
    }

    pub fn restart(&mut self) {
        self.at = 0;
        self.attempts = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults;

    fn en() -> Pack {
        defaults::EMBEDDED
            .iter()
            .find(|p| p.code == "en-beginner")
            .expect("english pack ships")
            .load()
            .expect("english pack parses")
    }

    fn ja() -> Pack {
        defaults::EMBEDDED
            .iter()
            .find(|p| p.code == "ja-beginner")
            .expect("japanese pack ships")
            .load()
            .expect("japanese pack parses")
    }

    fn lesson(items: &[&str]) -> Lesson {
        Lesson {
            index: 0,
            new_keys: Vec::new(),
            items: items
                .iter()
                .map(|t| LessonItem {
                    text: t.to_string(),
                    strokes: vec!["X".to_string()],
                })
                .collect(),
        }
    }

    // --- curriculum -----------------------------------------------------

    #[test]
    fn english_gets_a_multi_lesson_curriculum() {
        let c = Curriculum::derive(&en());
        assert!(c.len() >= 4, "expected several lessons, got {}", c.len());
    }

    /// Japanese has no consonant-only entries, so the first chunk of left-hand
    /// keys reaches nothing. It must be merged forward, not shipped empty.
    #[test]
    fn japanese_has_no_empty_lessons() {
        let c = Curriculum::derive(&ja());
        assert!(!c.is_empty());
        for l in c.lessons() {
            assert!(!l.is_empty(), "{} is empty", l.title());
        }
    }

    #[test]
    fn japanese_first_lesson_teaches_vowels_and_produces_kana() {
        let c = Curriculum::derive(&ja());
        let first = c.get(0).expect("at least one lesson");
        assert!(first.new_keys.iter().any(|k| k.bank == Bank::Mid));
        assert!(first.items.iter().any(|i| i.text == "あ"));
    }

    /// Every lesson may only require keys already taught — that is the whole
    /// point of a progression.
    #[test]
    fn lessons_never_need_an_untaught_key() {
        for pack in [en(), ja()] {
            let order = pack.layout.order();
            let c = Curriculum::derive(&pack);
            let mut taught: HashSet<String> = HashSet::new();
            for l in c.lessons() {
                taught.extend(l.new_keys.iter().map(|k| k.id.clone()));
                for item in &l.items {
                    let keys = parse_stroke_sequence(order, &item.strokes.join("/"))
                        .expect("lesson items always parse");
                    for id in keys.into_iter().flatten() {
                        assert!(
                            taught.contains(&id),
                            "{} drills {:?} which needs untaught key {id}",
                            l.title(),
                            item.text
                        );
                    }
                }
            }
        }
    }

    /// Each lesson has to practise what it just taught, or introducing keys
    /// would be pointless.
    #[test]
    fn every_lesson_exercises_at_least_one_new_key() {
        for pack in [en(), ja()] {
            let order = pack.layout.order();
            let c = Curriculum::derive(&pack);
            for l in c.lessons().iter().filter(|l| !l.is_review()) {
                let new: HashSet<&str> = l.new_keys.iter().map(|k| k.id.as_str()).collect();
                let uses_new = l.items.iter().any(|item| {
                    parse_stroke_sequence(order, &item.strokes.join("/"))
                        .into_iter()
                        .flatten()
                        .flatten()
                        .any(|id| new.contains(id.as_str()))
                });
                assert!(uses_new, "{} never uses a new key", l.title());
            }
        }
    }

    /// The item cap truncates a lesson, and every key is taught by the end, so
    /// without review lessons the overflow would be unreachable forever. Both
    /// shipped packs are small enough to be covered completely.
    #[test]
    fn every_word_in_a_shipped_pack_is_taught_somewhere() {
        for pack in [en(), ja()] {
            let c = Curriculum::derive(&pack);
            assert!(c.len() < MAX_LESSONS, "pack hit the curriculum ceiling");
            let drilled: HashSet<&str> = c
                .lessons()
                .iter()
                .flat_map(|l| l.items.iter().map(|i| i.text.as_str()))
                .collect();
            let index = ReverseIndex::build(&pack.dict);
            for entry in index.best_entries() {
                assert!(
                    drilled.contains(entry.text.as_str()),
                    "{} ({}) is never drilled in {}",
                    entry.text,
                    entry.key(),
                    pack.code
                );
            }
        }
    }

    #[test]
    fn overflow_becomes_review_lessons_with_no_new_keys() {
        let c = Curriculum::derive(&ja());
        let reviews: Vec<_> = c.lessons().iter().filter(|l| l.is_review()).collect();
        assert!(!reviews.is_empty(), "kana overflow should spill into review");
        for l in reviews {
            assert!(l.keys_label().is_empty());
            assert!(!l.is_empty());
        }
    }

    #[test]
    fn no_text_is_drilled_twice() {
        for pack in [en(), ja()] {
            let c = Curriculum::derive(&pack);
            let mut seen = HashSet::new();
            for l in c.lessons() {
                for item in &l.items {
                    assert!(seen.insert(item.text.clone()), "{} repeated", item.text);
                }
            }
        }
    }

    #[test]
    fn lessons_are_never_longer_than_the_cap() {
        for pack in [en(), ja()] {
            for l in Curriculum::derive(&pack).lessons() {
                // The final fold-in can push one lesson slightly over.
                assert!(l.len() <= MAX_ITEMS * 2, "{} has {} items", l.title(), l.len());
            }
        }
    }

    #[test]
    fn the_undo_key_is_never_taught_as_a_lesson_key() {
        let pack = en();
        for l in Curriculum::derive(&pack).lessons() {
            assert!(!l.new_keys.iter().any(|k| k.id == pack.undo_stroke));
        }
    }

    #[test]
    fn a_pack_with_an_empty_dictionary_yields_no_lessons() {
        let pack = Pack::from_strs(
            "code = \"t\"\nname = \"T\"\nlang = \"xx\"\n",
            "key = [{ id = \"S-\", letter = \"S\", bank = \"left\" }]\n[map]\nKeyQ = \"S-\"\n",
            "{}",
            None,
        )
        .expect("pack parses");
        let c = Curriculum::derive(&pack);
        assert!(
            c.is_empty(),
            "expected no lessons, got {:?}",
            c.lessons()
        );
    }

    #[test]
    fn items_carry_the_strokes_that_produce_them() {
        let c = Curriculum::derive(&en());
        let item = c
            .lessons()
            .iter()
            .flat_map(|l| &l.items)
            .find(|i| i.text == "cat")
            .expect("'cat' is drilled somewhere");
        assert_eq!(item.strokes, vec!["KAT".to_string()]);
    }

    #[test]
    fn keys_label_lists_the_new_letters() {
        let l = Lesson {
            index: 0,
            new_keys: vec![
                StenoKey::new("S-", "S", Bank::Left),
                StenoKey::new("T-", "T", Bank::Left),
            ],
            items: Vec::new(),
        };
        assert_eq!(l.keys_label(), "S T");
        assert_eq!(l.title(), "Lesson 1");
    }

    // --- drill ----------------------------------------------------------

    #[test]
    fn correct_answer_advances() {
        let mut d = Drill::new(&lesson(&["cat", "hat"]));
        assert_eq!(d.current().map(|i| i.text.as_str()), Some("cat"));
        assert_eq!(d.submit("cat"), Some(DrillOutcome::Correct));
        assert_eq!(d.current().map(|i| i.text.as_str()), Some("hat"));
        assert_eq!(d.position(), 1);
    }

    #[test]
    fn wrong_answer_stays_put_and_counts() {
        let mut d = Drill::new(&lesson(&["cat"]));
        assert_eq!(d.submit("hat"), Some(DrillOutcome::Wrong));
        assert_eq!(d.current().map(|i| i.text.as_str()), Some("cat"));
        assert_eq!(d.attempts_on_current(), 1);
    }

    #[test]
    fn attempts_reset_on_the_next_item() {
        let mut d = Drill::new(&lesson(&["cat", "hat"]));
        d.submit("wrong");
        d.submit("cat");
        assert_eq!(d.attempts_on_current(), 0);
    }

    /// Output arrives from the engine with the app's spacing already applied.
    #[test]
    fn surrounding_whitespace_is_ignored() {
        let mut d = Drill::new(&lesson(&["cat"]));
        assert_eq!(d.submit(" cat"), Some(DrillOutcome::Correct));
    }

    #[test]
    fn finishing_the_last_item_completes_the_drill() {
        let mut d = Drill::new(&lesson(&["cat"]));
        assert!(!d.is_complete());
        assert_eq!(d.submit("cat"), Some(DrillOutcome::Correct));
        assert!(d.is_complete());
        assert_eq!(d.current(), None);
    }

    #[test]
    fn submitting_after_completion_reports_nothing_to_answer() {
        let mut d = Drill::new(&lesson(&["cat"]));
        d.submit("cat");
        assert_eq!(d.submit("cat"), None);
    }

    #[test]
    fn skip_moves_on_without_crediting() {
        let mut d = Drill::new(&lesson(&["cat", "hat"]));
        d.skip();
        assert_eq!(d.current().map(|i| i.text.as_str()), Some("hat"));
    }

    #[test]
    fn skipping_past_the_end_is_harmless() {
        let mut d = Drill::new(&lesson(&["cat"]));
        d.skip();
        d.skip();
        assert!(d.is_complete());
        assert_eq!(d.position(), 1);
    }

    #[test]
    fn restart_returns_to_the_first_item() {
        let mut d = Drill::new(&lesson(&["cat", "hat"]));
        d.submit("cat");
        d.submit("wrong");
        d.restart();
        assert_eq!(d.position(), 0);
        assert_eq!(d.attempts_on_current(), 0);
        assert_eq!(d.current().map(|i| i.text.as_str()), Some("cat"));
    }

    #[test]
    fn an_empty_drill_is_immediately_complete() {
        let mut d = Drill::new(&lesson(&[]));
        assert!(d.is_complete());
        assert_eq!(d.submit("anything"), None);
    }
}
