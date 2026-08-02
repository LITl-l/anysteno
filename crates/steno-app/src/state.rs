//! Persistent settings and on-disk pack discovery.
//!
//! Config lives in the platform config dir (`directories` crate):
//!   `<config>/anysteno/settings.json`
//!   `<config>/anysteno/packs/<code>/{pack.toml,layout.toml,dict.json,user.json}`
//!
//! On first run the embedded official packs are written into that folder so the
//! user can read and customise them; user edits are then loaded from disk.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use steno_core::pack::Pack;

use crate::ui::theme::ThemeMode;

/// Where translated text goes while practising.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Show output only inside anysteno's own practice window.
    InApp,
    /// Inject output into whatever application is focused.
    SystemWide,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::InApp => "Type here",
            Mode::SystemWide => "Type into other apps",
        }
    }
}

/// Which screen the app is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Screen {
    #[default]
    Learn,
    Practice,
    Dictionary,
    Settings,
}

impl Screen {
    pub const ALL: [Screen; 4] = [
        Screen::Learn,
        Screen::Practice,
        Screen::Dictionary,
        Screen::Settings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Screen::Learn => "Learn",
            Screen::Practice => "Practice",
            Screen::Dictionary => "Dictionary",
            Screen::Settings => "Settings",
        }
    }

    /// Screens where chording drives the engine rather than the UI. On the
    /// others the keyboard belongs to the widgets (searching, typing settings).
    pub fn captures_keys(self) -> bool {
        matches!(self, Screen::Learn | Screen::Practice)
    }
}

/// User-persisted settings.
///
/// Every field added after the first release carries `#[serde(default)]`: an
/// older `settings.json` is missing them, and without the default the whole
/// file would fail to parse and silently reset the user's pack choice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub pack_code: String,
    pub mode: Mode,
    pub enabled: bool,
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default)]
    pub screen: Screen,
    /// Whether the first-run walkthrough has been dismissed.
    #[serde(default)]
    pub onboarded: bool,
    /// Show the stroke hint before the first mistake, not just after one.
    #[serde(default = "yes")]
    pub show_hints: bool,
    /// Pack code → number of lessons finished.
    #[serde(default)]
    pub progress: BTreeMap<String, usize>,
    /// Last window size, so the app reopens the size the user left it.
    #[serde(default)]
    pub window: Option<[f32; 2]>,
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pack_code: "en-beginner".to_string(),
            mode: Mode::InApp,
            enabled: true,
            theme: ThemeMode::default(),
            screen: Screen::default(),
            onboarded: false,
            show_hints: true,
            progress: BTreeMap::new(),
            window: None,
        }
    }
}

impl Settings {
    /// Lessons finished in the given pack.
    pub fn lessons_done(&self, pack_code: &str) -> usize {
        self.progress.get(pack_code).copied().unwrap_or(0)
    }

    /// Record a finished lesson. Progress only moves forward, so replaying an
    /// earlier lesson can't take back what the user already unlocked.
    pub fn mark_lesson_done(&mut self, pack_code: &str, lesson_index: usize) {
        let done = self.progress.entry(pack_code.to_string()).or_insert(0);
        *done = (*done).max(lesson_index + 1);
    }
}

/// Resolved config-directory paths.
pub struct Paths {
    pub root: PathBuf,
    pub settings_file: PathBuf,
    pub packs_dir: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        let root = ProjectDirs::from("dev", "anysteno", "anysteno")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".anysteno"));
        Self {
            settings_file: root.join("settings.json"),
            packs_dir: root.join("packs"),
            root,
        }
    }

    pub fn load_settings(&self) -> Settings {
        fs::read_to_string(&self.settings_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &Settings) {
        let _ = fs::create_dir_all(&self.root);
        if let Ok(json) = serde_json::to_string_pretty(settings) {
            let _ = fs::write(&self.settings_file, json);
        }
    }

    /// Write the embedded official packs to disk on first run (never clobbering
    /// existing user edits).
    pub fn seed_default_packs(&self) {
        for embedded in steno_core::defaults::EMBEDDED {
            let dir = self.packs_dir.join(embedded.code);
            if dir.exists() {
                continue;
            }
            if fs::create_dir_all(&dir).is_err() {
                continue;
            }
            let _ = fs::write(dir.join("pack.toml"), embedded.pack_toml);
            let _ = fs::write(dir.join("layout.toml"), embedded.layout_toml);
            let _ = fs::write(dir.join("dict.json"), embedded.dict_json);
        }
    }
}

/// Load every available pack: those on disk (user-editable), falling back to the
/// embedded copies for any that fail to load. Returns packs plus any warnings.
pub fn load_packs(packs_dir: &Path) -> (Vec<Pack>, Vec<String>) {
    let mut packs = Vec::new();
    let mut warnings = Vec::new();
    let mut loaded_codes = Vec::new();

    if let Ok(entries) = fs::read_dir(packs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            match Pack::load_dir(&path) {
                Ok(pack) => {
                    loaded_codes.push(pack.code.clone());
                    packs.push(pack);
                }
                Err(e) => warnings.push(format!("{}: {e}", path.display())),
            }
        }
    }

    // Ensure the official packs are always present even if disk load failed.
    for embedded in steno_core::defaults::EMBEDDED {
        if !loaded_codes.iter().any(|c| c == embedded.code) {
            match embedded.load() {
                Ok(pack) => packs.push(pack),
                Err(e) => warnings.push(format!("embedded {}: {e}", embedded.code)),
            }
        }
    }

    packs.sort_by(|a, b| a.code.cmp(&b.code));
    (packs, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A settings file written by the previous release must still load, keeping
    /// the user's pack choice rather than silently resetting to defaults.
    #[test]
    fn settings_from_an_older_version_still_parse() {
        let old = r#"{"pack_code":"ja-beginner","mode":"SystemWide","enabled":false}"#;
        let s: Settings = serde_json::from_str(old).expect("old settings parse");
        assert_eq!(s.pack_code, "ja-beginner");
        assert_eq!(s.mode, Mode::SystemWide);
        assert!(!s.enabled);
        // New fields fall back to their defaults.
        assert!(s.show_hints);
        assert_eq!(s.lessons_done("ja-beginner"), 0);
    }

    #[test]
    fn settings_round_trip() {
        let mut s = Settings::default();
        s.mark_lesson_done("en-beginner", 2);
        s.window = Some([800.0, 600.0]);
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.lessons_done("en-beginner"), 3);
        assert_eq!(back.window, Some([800.0, 600.0]));
    }

    #[test]
    fn progress_only_moves_forward() {
        let mut s = Settings::default();
        s.mark_lesson_done("en-beginner", 4);
        s.mark_lesson_done("en-beginner", 1); // replaying an earlier lesson
        assert_eq!(s.lessons_done("en-beginner"), 5);
    }

    #[test]
    fn progress_is_tracked_per_pack() {
        let mut s = Settings::default();
        s.mark_lesson_done("en-beginner", 0);
        assert_eq!(s.lessons_done("en-beginner"), 1);
        assert_eq!(s.lessons_done("ja-beginner"), 0);
    }
}
