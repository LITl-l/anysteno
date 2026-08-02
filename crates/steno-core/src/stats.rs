//! Typing statistics for a practice or drill session.
//!
//! Deliberately clock-free: elapsed time is supplied by the caller rather than
//! read from `std::time`. That keeps the core free of I/O, and lets tests
//! assert exact rates instead of sleeping.

/// Characters per "word" in the conventional WPM definition.
const CHARS_PER_WORD: f64 = 5.0;

/// Counters for one session, plus the derived rates the UI displays.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionStats {
    strokes: usize,
    correct: usize,
    incorrect: usize,
    chars: usize,
    elapsed_ms: u64,
}

impl SessionStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Count one chord, whatever it produced.
    pub fn record_stroke(&mut self) {
        self.strokes += 1;
    }

    /// Count one drill item or one emitted word. `text` supplies the character
    /// count that WPM is derived from; only correct output counts towards it,
    /// so mistakes can't inflate the rate.
    pub fn record_item(&mut self, text: &str, correct: bool) {
        if correct {
            self.correct += 1;
            self.chars += text.chars().count();
        } else {
            self.incorrect += 1;
        }
    }

    /// Set the total elapsed time for the session so far.
    pub fn set_elapsed_ms(&mut self, ms: u64) {
        self.elapsed_ms = ms;
    }

    pub fn strokes(&self) -> usize {
        self.strokes
    }

    pub fn correct(&self) -> usize {
        self.correct
    }

    pub fn incorrect(&self) -> usize {
        self.incorrect
    }

    pub fn attempts(&self) -> usize {
        self.correct + self.incorrect
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }

    /// Words per minute, or `None` before there is anything to measure. The
    /// `None` is deliberate: showing "0 wpm" at the start of a session reads as
    /// a score rather than an absence of data.
    pub fn wpm(&self) -> Option<f64> {
        if self.elapsed_ms == 0 || self.chars == 0 {
            return None;
        }
        let minutes = self.elapsed_ms as f64 / 60_000.0;
        Some((self.chars as f64 / CHARS_PER_WORD) / minutes)
    }

    /// Fraction of attempts that were correct, or `None` before the first
    /// attempt.
    pub fn accuracy(&self) -> Option<f64> {
        let attempts = self.attempts();
        (attempts > 0).then(|| self.correct as f64 / attempts as f64)
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_session_has_nothing_to_report() {
        let s = SessionStats::new();
        assert_eq!(s.wpm(), None);
        assert_eq!(s.accuracy(), None);
        assert_eq!(s.attempts(), 0);
    }

    /// 25 characters in 30 s = 5 words in half a minute = 10 wpm.
    #[test]
    fn wpm_uses_five_characters_per_word() {
        let mut s = SessionStats::new();
        s.record_item("abcde", true);
        s.record_item("abcde", true);
        s.record_item("abcde", true);
        s.record_item("abcde", true);
        s.record_item("abcde", true);
        s.set_elapsed_ms(30_000);
        assert_eq!(s.wpm(), Some(10.0));
    }

    #[test]
    fn wrong_answers_do_not_count_towards_wpm() {
        let mut s = SessionStats::new();
        s.record_item("abcde", false);
        s.set_elapsed_ms(30_000);
        assert_eq!(s.wpm(), None);
    }

    #[test]
    fn accuracy_is_correct_over_attempts() {
        let mut s = SessionStats::new();
        s.record_item("cat", true);
        s.record_item("cat", true);
        s.record_item("cat", false);
        s.record_item("cat", false);
        assert_eq!(s.accuracy(), Some(0.5));
        assert_eq!(s.attempts(), 4);
    }

    #[test]
    fn wpm_is_absent_until_time_has_passed() {
        let mut s = SessionStats::new();
        s.record_item("cat", true);
        assert_eq!(s.wpm(), None);
        s.set_elapsed_ms(60_000);
        assert_eq!(s.wpm(), Some(0.6));
    }

    /// WPM counts characters, not bytes — kana must not score triple.
    #[test]
    fn multibyte_text_counts_characters() {
        let mut s = SessionStats::new();
        s.record_item("かきくけこ", true);
        s.set_elapsed_ms(60_000);
        assert_eq!(s.wpm(), Some(1.0));
    }

    #[test]
    fn strokes_count_independently_of_items() {
        let mut s = SessionStats::new();
        s.record_stroke();
        s.record_stroke();
        s.record_item("cat", true);
        assert_eq!(s.strokes(), 2);
        assert_eq!(s.correct(), 1);
    }

    #[test]
    fn reset_clears_everything() {
        let mut s = SessionStats::new();
        s.record_stroke();
        s.record_item("cat", true);
        s.set_elapsed_ms(1000);
        s.reset();
        assert_eq!(s, SessionStats::new());
    }
}
