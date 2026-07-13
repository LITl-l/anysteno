//! Best-effort CJK font loading.
//!
//! egui's bundled fonts cover Latin but not kana/kanji, so Japanese would show
//! as tofu boxes. We scan a few common locations for a CJK-capable font and, if
//! found, register it as a fallback. If none is found, English still works and
//! the app notes it.

use std::path::{Path, PathBuf};

use egui::{FontData, FontDefinitions, FontFamily};

/// Candidate font files (checked in order). Covers common Nix/Linux, macOS and
/// Windows locations for a Japanese-capable font.
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

fn find_font() -> Option<PathBuf> {
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
            let is_font = lower.ends_with(".ttc") || lower.ends_with(".otf") || lower.ends_with(".ttf");
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

/// Install a CJK fallback font into egui if one can be found. Returns a status
/// string for the UI.
pub fn install(ctx: &egui::Context) -> String {
    let Some(path) = find_font() else {
        return "No CJK font found — Japanese may show as boxes. Install Noto Sans CJK.".to_string();
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return format!("Found {} but could not read it.", path.display());
    };

    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk".to_owned(), FontData::from_owned(bytes));
    // Add as the last fallback for both proportional and monospace families.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push("cjk".to_owned());
    }
    ctx.set_fonts(fonts);
    format!("CJK font loaded: {}", path.display())
}
