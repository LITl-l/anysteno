//! The engine — the single object the app talks to.
//!
//! It owns the active [`Pack`], the [`ChordAccumulator`] and the [`Translator`],
//! and converts a stream of raw key events into [`Translation`]s. It is still
//! pure: no threads, no I/O. The platform layer feeds it [`KeyEvent`]s and acts
//! on the [`EngineOutput`] it returns.

use std::collections::BTreeSet;

use crate::chord::ChordAccumulator;
use crate::dictionary::{Translation, Translator};
use crate::pack::Pack;
use crate::stroke::render_stroke;

/// A raw keyboard event from the platform capture layer.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// Platform-neutral physical key name, e.g. `"KeyS"`.
    pub physical: String,
    /// `true` for key-down, `false` for key-up.
    pub pressed: bool,
}

impl KeyEvent {
    pub fn down(physical: impl Into<String>) -> Self {
        Self { physical: physical.into(), pressed: true }
    }
    pub fn up(physical: impl Into<String>) -> Self {
        Self { physical: physical.into(), pressed: false }
    }
}

/// What the engine produced from a single key event.
#[derive(Debug, Clone, Default)]
pub struct EngineOutput {
    /// The completed stroke string, if this event finished a chord.
    pub stroke: Option<String>,
    /// Translations that became final (text, no-match).
    pub translations: Vec<Translation>,
    /// `true` if the completed stroke was the pack's undo command.
    pub undo: bool,
}

impl EngineOutput {
    fn empty() -> Self {
        Self::default()
    }
}

/// The stateful steno engine.
pub struct Engine {
    pack: Pack,
    chord: ChordAccumulator,
    translator: Translator,
    enabled: bool,
}

impl Engine {
    pub fn new(pack: Pack) -> Self {
        Self {
            pack,
            chord: ChordAccumulator::new(),
            translator: Translator::new(),
            enabled: true,
        }
    }

    pub fn pack(&self) -> &Pack {
        &self.pack
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Turn translation on/off. Disabling abandons any in-progress chord and
    /// pending strokes so re-enabling starts clean.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        if !on {
            self.chord.reset();
            self.translator.clear();
        }
    }

    /// Swap the active language pack, resetting transient state.
    pub fn set_pack(&mut self, pack: Pack) {
        self.pack = pack;
        self.chord.reset();
        self.translator.clear();
    }

    /// Feed one raw key event. Non-steno keys and events while disabled are
    /// ignored (the caller passes them through to the OS as normal typing).
    pub fn on_key(&mut self, ev: &KeyEvent) -> EngineOutput {
        if !self.enabled || !self.pack.layout.is_steno_key(&ev.physical) {
            return EngineOutput::empty();
        }

        if ev.pressed {
            self.chord.press(&ev.physical);
            return EngineOutput::empty();
        }

        // key-up: may complete the chord.
        let Some(physical_keys) = self.chord.release(&ev.physical) else {
            return EngineOutput::empty();
        };

        // Map physical keys -> steno key ids, then render the canonical stroke.
        let steno_ids: BTreeSet<String> = physical_keys
            .iter()
            .filter_map(|p| self.pack.layout.steno_for(p).map(str::to_string))
            .collect();
        let stroke = render_stroke(self.pack.layout.order(), &steno_ids);

        if stroke.is_empty() {
            return EngineOutput::empty();
        }

        // The undo command bypasses the dictionary and flushes pending strokes.
        if !self.pack.undo_stroke.is_empty() && stroke == self.pack.undo_stroke {
            self.translator.clear();
            return EngineOutput {
                stroke: Some(stroke),
                translations: Vec::new(),
                undo: true,
            };
        }

        let translations = self.translator.push(&self.pack.dict, stroke.clone());
        EngineOutput { stroke: Some(stroke), translations, undo: false }
    }

    /// Force-resolve any strokes held pending a longer match (call on
    /// space/enter or an idle timeout so buffered words aren't stuck).
    pub fn flush(&mut self) -> Vec<Translation> {
        self.translator.flush(&self.pack.dict)
    }

    /// Strokes currently buffered awaiting disambiguation (for the live view).
    pub fn pending(&self) -> &[String] {
        self.translator.pending()
    }

    /// Steno key ids currently held, in canonical order — for a live GUI view
    /// of the chord being built.
    pub fn held_steno_ids(&self) -> Vec<String> {
        let held: BTreeSet<String> = self
            .chord
            .building()
            .iter()
            .filter_map(|p| self.pack.layout.steno_for(p).map(str::to_string))
            .collect();
        self.pack
            .layout
            .order()
            .iter()
            .filter(|k| held.contains(&k.id))
            .map(|k| k.id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pack() -> Pack {
        let pack = r#"
            code = "t"
            name = "Test"
            lang = "xx"
            undo_stroke = "*"
        "#;
        let layout = r#"
            key = [
              { id = "K-", letter = "K", bank = "left" },
              { id = "H-", letter = "H", bank = "left" },
              { id = "A",  letter = "A", bank = "mid" },
              { id = "*",  letter = "*", bank = "mid" },
              { id = "-T", letter = "T", bank = "right" },
              { id = "-S", letter = "S", bank = "right" },
            ]
            [map]
            KeyK = "K-"
            KeyH = "H-"
            KeyC = "A"
            KeyG = "*"
            KeyP = "-T"
            KeyL = "-S"
        "#;
        let dict = r#"{ "KAT": "cat", "HAT": "hat", "KAT/-S": "cats" }"#;
        Pack::from_strs(pack, layout, dict, None).unwrap()
    }

    /// Chord the keys of a word and return the engine output of the release
    /// that completes the chord.
    fn chord(engine: &mut Engine, keys: &[&str]) -> EngineOutput {
        for k in keys {
            engine.on_key(&KeyEvent::down(*k));
        }
        let mut last = EngineOutput::empty();
        for k in keys {
            last = engine.on_key(&KeyEvent::up(*k));
        }
        last
    }

    #[test]
    fn chords_translate_to_text() {
        let mut e = Engine::new(test_pack());
        // "HAT" is not a prefix of any multi-stroke entry, so it resolves at once.
        let out = chord(&mut e, &["KeyH", "KeyC", "KeyP"]); // H A -T
        assert_eq!(out.stroke.as_deref(), Some("HAT"));
        assert_eq!(
            out.translations,
            vec![Translation::Text {
                strokes: vec!["HAT".into()],
                text: "hat".into()
            }]
        );
    }

    #[test]
    fn unknown_chord_surfaces_no_match() {
        let mut e = Engine::new(test_pack());
        let out = chord(&mut e, &["KeyH", "KeyP"]); // H -T -> "H-T", not in dict
        assert_eq!(out.stroke.as_deref(), Some("H-T"));
        assert_eq!(
            out.translations,
            vec![Translation::NoMatch { stroke: "H-T".into() }]
        );
    }

    #[test]
    fn disabled_engine_passes_through() {
        let mut e = Engine::new(test_pack());
        e.set_enabled(false);
        let out = chord(&mut e, &["KeyK", "KeyC", "KeyP"]);
        assert!(out.stroke.is_none());
        assert!(out.translations.is_empty());
    }

    #[test]
    fn undo_stroke_is_flagged() {
        let mut e = Engine::new(test_pack());
        let out = chord(&mut e, &["KeyG"]); // "*" alone
        assert_eq!(out.stroke.as_deref(), Some("*"));
        assert!(out.undo);
        assert!(out.translations.is_empty());
    }

    #[test]
    fn multi_stroke_waits_then_resolves() {
        let mut e = Engine::new(test_pack());
        // "KAT" is a prefix of "KAT/-S" -> pending, no translation yet.
        let first = chord(&mut e, &["KeyK", "KeyC", "KeyP"]);
        assert_eq!(first.stroke.as_deref(), Some("KAT"));
        assert!(first.translations.is_empty());
        // "-S" completes "cats".
        let second = chord(&mut e, &["KeyL"]);
        assert_eq!(
            second.translations,
            vec![Translation::Text {
                strokes: vec!["KAT".into(), "-S".into()],
                text: "cats".into()
            }]
        );
    }

    #[test]
    fn held_ids_reflect_current_chord_in_order() {
        let mut e = Engine::new(test_pack());
        e.on_key(&KeyEvent::down("KeyP")); // -T
        e.on_key(&KeyEvent::down("KeyK")); // K-
        e.on_key(&KeyEvent::down("KeyC")); // A
        assert_eq!(e.held_steno_ids(), vec!["K-", "A", "-T"]); // canonical order
    }
}
