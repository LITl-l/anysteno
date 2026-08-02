//! Font setup.
//!
//! egui's bundled fonts cover Latin but not kana, so Japanese used to render as
//! tofu boxes unless the user happened to have a CJK font installed — the
//! shipped `ja-beginner` pack was unusable out of the box on a bare system.
//!
//! anysteno therefore **embeds** a kana subset of Noto Sans CJK JP (~36 KB, see
//! `assets/LICENSE-NotoSansJP.txt`). Everything the shipped packs can produce is
//! guaranteed to render with no system font at all.
//!
//! A system CJK font, when present, is still registered *ahead* of the subset:
//! it carries full kanji, which a user-written Japanese pack may need and the
//! subset deliberately omits.

use std::path::{Path, PathBuf};

use egui::{FontData, FontDefinitions, FontFamily};

/// Kana + CJK punctuation subset, compiled into the binary.
const EMBEDDED_KANA: &[u8] = include_bytes!("../assets/NotoSansJP-kana-subset.otf");

/// Candidate system font files (checked in order), for kanji coverage beyond
/// the embedded subset.
const CANDIDATES: &[&str] = &[
    // Noto CJK (Linux / Nix)
    "/run/current-system/sw/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/run/current-system/sw/share/fonts/truetype/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJKjp-Regular.otf",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    // macOS
    "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/Library/Fonts/Arial Unicode.ttf",
    // Windows
    "C:\\Windows\\Fonts\\YuGothM.ttc",
    "C:\\Windows\\Fonts\\msgothic.ttc",
    "C:\\Windows\\Fonts\\meiryo.ttc",
];

/// Directories to shallow-scan for anything looking like a CJK font, as a
/// fallback when no known path matches (e.g. Nix store profiles).
const SCAN_DIRS: &[&str] = &[
    "/run/current-system/sw/share/fonts",
    "/etc/fonts",
    "/usr/share/fonts",
];

fn find_system_font() -> Option<PathBuf> {
    for c in CANDIDATES {
        let p = Path::new(c);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    for dir in SCAN_DIRS {
        if let Some(p) = scan_dir(Path::new(dir), 0) {
            return Some(p);
        }
    }
    None
}

/// Recursively (shallow, max depth 3) look for a file whose name hints CJK.
fn scan_dir(dir: &Path, depth: usize) -> Option<PathBuf> {
    if depth > 3 {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = scan_dir(&path, depth + 1) {
                return Some(found);
            }
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let lower = name.to_ascii_lowercase();
            let is_font =
                lower.ends_with(".ttc") || lower.ends_with(".otf") || lower.ends_with(".ttf");
            let is_cjk = lower.contains("cjk")
                || lower.contains("gothic")
                || lower.contains("mincho")
                || lower.contains("hiragino")
                || lower.contains("meiryo");
            if is_font && is_cjk {
                return Some(path);
            }
        }
    }
    None
}

/// Install fonts. Returns a status string for the UI, empty when there is
/// nothing the user needs to know.
pub fn install(ctx: &egui::Context) -> String {
    let mut fonts = FontDefinitions::default();
    let mut stack: Vec<String> = Vec::new();

    // Full-coverage system font first, if we can find and read one.
    let system = find_system_font().and_then(|path| match std::fs::read(&path) {
        Ok(bytes) => Some((path, bytes)),
        Err(_) => None,
    });
    if let Some((_, bytes)) = &system {
        fonts
            .font_data
            .insert("cjk-system".to_owned(), FontData::from_owned(bytes.clone()));
        stack.push("cjk-system".to_owned());
    }

    // Then the embedded subset, which always exists.
    fonts.font_data.insert(
        "cjk-kana".to_owned(),
        FontData::from_static(EMBEDDED_KANA),
    );
    stack.push("cjk-kana".to_owned());

    // Appended, so egui's Latin fonts keep priority for ASCII.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .extend(stack.iter().cloned());
    }
    ctx.set_fonts(fonts);

    // Kana always render now, so say nothing in the common case. Only mention
    // the gap that remains: kanji outside the shipped packs.
    match system {
        Some(_) => String::new(),
        None => "Kana are built in. Install Noto Sans CJK for kanji beyond the shipped pack."
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every character the shipped packs can put on screen must be *drawable*
    /// with no system font.
    ///
    /// This goes through `ab_glyph` — the rasterizer egui uses — and demands an
    /// outline, not merely a character-map entry. An earlier version of this
    /// test only checked the cmap and passed against a CFF2 subset that egui
    /// could not rasterize at all: every kana rendered as blank space.
    #[test]
    fn embedded_font_can_draw_every_shipped_character() {
        use ab_glyph::Font;
        let face = ab_glyph::FontRef::try_from_slice(EMBEDDED_KANA).expect("subset parses");

        let mut needed: Vec<char> = Vec::new();
        for pack in steno_core::defaults::EMBEDDED {
            let loaded = pack.load().expect("shipped pack parses");
            needed.extend(loaded.name.chars());
            needed.extend(loaded.description.chars());
            for (_, text) in loaded.dict.entries() {
                needed.extend(text.chars());
            }
        }

        let missing: Vec<char> = needed
            .into_iter()
            // egui's bundled fonts already cover Latin and common punctuation.
            .filter(|c| !c.is_ascii())
            .filter(|c| {
                let id = face.glyph_id(*c);
                // Glyph 0 is .notdef; an outline of None means nothing is drawn.
                id.0 == 0 || face.outline(id).is_none()
            })
            .collect();

        assert!(
            missing.is_empty(),
            "embedded font cannot draw glyphs the shipped packs need: {missing:?}"
        );
    }

    /// A guard on binary size: the subset exists so Japanese works out of the
    /// box, not so anysteno ships a whole CJK font.
    #[test]
    fn embedded_font_stays_small() {
        assert!(
            EMBEDDED_KANA.len() < 128 * 1024,
            "embedded font grew to {} bytes",
            EMBEDDED_KANA.len()
        );
    }
}
