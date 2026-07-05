//! Language packs — the unit of extensibility.
//!
//! A pack is a folder of plain files loaded at runtime (no recompile to add a
//! language):
//!
//! ```text
//!   <pack>/
//!     pack.toml     # meta: code, name, lang, undo stroke
//!     layout.toml   # steno key order + physical->steno map
//!     dict.json     # { "STROKE": "text", ... }
//!     user.json     # optional overlay, wins over dict.json
//! ```
//!
//! Loading is fallible and never silent: a malformed pack yields a
//! [`PackError`] describing what failed, so the app can keep the previous pack
//! and show the reason.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

use crate::dictionary::Dictionary;
use crate::layout::Layout;
use crate::stroke::StenoKey;

/// Everything needed to translate one language.
#[derive(Debug, Clone)]
pub struct Pack {
    pub code: String,
    pub name: String,
    pub lang: String,
    pub description: String,
    /// The stroke (e.g. `"*"`) that means "undo the last output". Empty = none.
    pub undo_stroke: String,
    pub layout: Layout,
    pub dict: Dictionary,
}

#[derive(Debug, Error)]
pub enum PackError {
    #[error("failed to read {file}: {source}")]
    Io {
        file: String,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid pack.toml: {0}")]
    Meta(String),
    #[error("invalid layout.toml: {0}")]
    Layout(String),
    #[error("invalid dict.json: {0}")]
    Dict(String),
}

#[derive(Deserialize)]
struct MetaFile {
    code: String,
    name: String,
    lang: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_undo")]
    undo_stroke: String,
}

fn default_undo() -> String {
    "*".to_string()
}

#[derive(Deserialize)]
struct LayoutFile {
    key: Vec<StenoKey>,
    map: HashMap<String, String>,
}

impl Pack {
    /// Build a pack directly from file contents. `user_json` is an optional
    /// overlay merged on top of `dict_json`. This is the pure, I/O-free entry
    /// point used by embedded defaults and by tests.
    pub fn from_strs(
        pack_toml: &str,
        layout_toml: &str,
        dict_json: &str,
        user_json: Option<&str>,
    ) -> Result<Pack, PackError> {
        let meta: MetaFile =
            toml::from_str(pack_toml).map_err(|e| PackError::Meta(e.to_string()))?;
        let layout_file: LayoutFile =
            toml::from_str(layout_toml).map_err(|e| PackError::Layout(e.to_string()))?;

        let base: HashMap<String, String> =
            serde_json::from_str(dict_json).map_err(|e| PackError::Dict(e.to_string()))?;
        let dict = Dictionary::from_map(base);
        let dict = match user_json {
            Some(u) if !u.trim().is_empty() => {
                let overlay: HashMap<String, String> =
                    serde_json::from_str(u).map_err(|e| PackError::Dict(e.to_string()))?;
                dict.overlaid_with(&overlay)
            }
            _ => dict,
        };

        Ok(Pack {
            code: meta.code,
            name: meta.name,
            lang: meta.lang,
            description: meta.description,
            undo_stroke: meta.undo_stroke,
            layout: Layout::new(layout_file.key, layout_file.map),
            dict,
        })
    }

    /// Load a pack from a directory on disk. Reads `pack.toml`, `layout.toml`,
    /// `dict.json`, and an optional `user.json`.
    pub fn load_dir(dir: &Path) -> Result<Pack, PackError> {
        let read = |name: &str| -> Result<String, PackError> {
            std::fs::read_to_string(dir.join(name)).map_err(|e| PackError::Io {
                file: name.to_string(),
                source: e,
            })
        };
        let pack_toml = read("pack.toml")?;
        let layout_toml = read("layout.toml")?;
        let dict_json = read("dict.json")?;
        let user_json = std::fs::read_to_string(dir.join("user.json")).ok();
        Pack::from_strs(&pack_toml, &layout_toml, &dict_json, user_json.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACK: &str = r#"
        code = "t"
        name = "Test"
        lang = "xx"
        undo_stroke = "*"
    "#;

    const LAYOUT: &str = r#"
        key = [
          { id = "S-", letter = "S", bank = "left" },
          { id = "A",  letter = "A", bank = "mid" },
          { id = "-T", letter = "T", bank = "right" },
        ]
        [map]
        KeyA = "S-"
        KeyC = "A"
        KeyP = "-T"
    "#;

    #[test]
    fn parses_a_valid_pack() {
        let p = Pack::from_strs(PACK, LAYOUT, r#"{"SAT":"sat"}"#, None).unwrap();
        assert_eq!(p.code, "t");
        assert_eq!(p.undo_stroke, "*");
        assert_eq!(p.layout.order().len(), 3);
        assert_eq!(p.layout.steno_for("KeyC"), Some("A"));
        assert_eq!(p.dict.lookup("SAT"), Some("sat"));
    }

    #[test]
    fn user_overlay_wins() {
        let p =
            Pack::from_strs(PACK, LAYOUT, r#"{"SAT":"sat"}"#, Some(r#"{"SAT":"seat"}"#)).unwrap();
        assert_eq!(p.dict.lookup("SAT"), Some("seat"));
    }

    #[test]
    fn bad_json_is_reported_not_panicked() {
        let err = Pack::from_strs(PACK, LAYOUT, "{ not json", None).unwrap_err();
        assert!(matches!(err, PackError::Dict(_)));
    }

    #[test]
    fn missing_meta_field_is_reported() {
        let err = Pack::from_strs("name = \"x\"", LAYOUT, "{}", None).unwrap_err();
        assert!(matches!(err, PackError::Meta(_)));
    }
}
