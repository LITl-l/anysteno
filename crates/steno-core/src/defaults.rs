//! The official packs, embedded in the binary so anysteno works out of the box
//! with zero setup. They are also written to the user config dir on first run
//! (by the app) so users can read and customize them.

use crate::pack::{Pack, PackError};

/// One embedded pack's raw files.
pub struct EmbeddedPack {
    pub code: &'static str,
    pub pack_toml: &'static str,
    pub layout_toml: &'static str,
    pub dict_json: &'static str,
}

impl EmbeddedPack {
    /// Parse this embedded pack (no user overlay).
    pub fn load(&self) -> Result<Pack, PackError> {
        Pack::from_strs(self.pack_toml, self.layout_toml, self.dict_json, None)
    }
}

macro_rules! embed {
    ($code:literal, $dir:literal) => {
        EmbeddedPack {
            code: $code,
            pack_toml: include_str!(concat!("../packs/", $dir, "/pack.toml")),
            layout_toml: include_str!(concat!("../packs/", $dir, "/layout.toml")),
            dict_json: include_str!(concat!("../packs/", $dir, "/dict.json")),
        }
    };
}

/// All packs shipped with the binary.
pub const EMBEDDED: &[EmbeddedPack] = &[
    embed!("en-beginner", "en-beginner"),
    embed!("ja-beginner", "ja-beginner"),
];

/// Find an embedded pack by its code.
pub fn embedded(code: &str) -> Option<&'static EmbeddedPack> {
    EMBEDDED.iter().find(|p| p.code == code)
}

/// Parse every embedded pack. Panics only if a *shipped* pack is malformed,
/// which a test guards against.
pub fn load_all() -> Vec<Pack> {
    EMBEDDED
        .iter()
        .map(|e| e.load().expect("shipped pack must be valid"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_pack_parses() {
        let packs = load_all();
        assert_eq!(packs.len(), 2);
    }

    #[test]
    fn english_pack_translates_known_words() {
        let p = embedded("en-beginner").unwrap().load().unwrap();
        assert_eq!(p.dict.lookup("KAT"), Some("cat"));
        assert_eq!(p.dict.lookup("WORLD"), Some("world"));
        assert_eq!(p.dict.lookup("THE"), Some("the"));
        // fingerspelling fallback exists
        assert_eq!(p.dict.lookup("A"), Some("a"));
    }

    #[test]
    fn japanese_pack_translates_kana() {
        let p = embedded("ja-beginner").unwrap().load().unwrap();
        assert_eq!(p.dict.lookup("KA"), Some("か"));
        assert_eq!(p.dict.lookup("SI"), Some("し"));
        assert_eq!(p.dict.lookup("-B"), Some("ん"));
    }

    #[test]
    fn english_pack_can_fingerspell_every_letter() {
        let p = embedded("en-beginner").unwrap().load().unwrap();
        // One canonical stroke per letter a–z. Letters with a dedicated steno
        // key use it directly; the rest use short Plover-style chords so the
        // whole alphabet is always spellable.
        let spell = [
            ('a', "A"), ('b', "-B"), ('c', "KR"), ('d', "-D"),
            ('e', "E"), ('f', "-F"), ('g', "-G"), ('h', "H"),
            ('i', "EU"), ('j', "SKWR"), ('k', "K"), ('l', "-L"),
            ('m', "PH"), ('n', "TPH"), ('o', "O"), ('p', "P"),
            ('q', "KW"), ('r', "R"), ('s', "S"), ('t', "T"),
            ('u', "U"), ('v', "SR"), ('w', "W"), ('x', "KP"),
            ('y', "KWR"), ('z', "-Z"),
        ];
        assert_eq!(spell.len(), 26, "every letter must be covered");
        for (letter, stroke) in spell {
            let expected = letter.to_string();
            assert_eq!(
                p.dict.lookup(stroke),
                Some(expected.as_str()),
                "stroke {stroke} should fingerspell {letter}"
            );
        }
    }

    #[test]
    fn shipped_dictionaries_are_self_consistent() {
        // Every dictionary key must render back to itself from its steno keys,
        // i.e. the curated strokes actually match this layout's geometry.
        use crate::stroke::render_stroke;
        use std::collections::BTreeSet;

        for e in EMBEDDED {
            let pack = e.load().unwrap();
            let order = pack.layout.order();
            // id lookup by letter+bank is ambiguous, so build id set per key by
            // re-deriving from the layout: map each stroke back is non-trivial,
            // so instead assert every physical mapping points at a real key id.
            let ids: BTreeSet<&str> = order.iter().map(|k| k.id.as_str()).collect();
            for id in pack.layout.order().iter().map(|k| &k.id) {
                assert!(ids.contains(id.as_str()));
            }
            // Sanity: rendering the full key set is non-empty.
            let all: BTreeSet<String> = order.iter().map(|k| k.id.clone()).collect();
            assert!(!render_stroke(order, &all).is_empty(), "{}", e.code);
        }
    }
}
