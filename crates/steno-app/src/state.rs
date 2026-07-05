//! Persistent settings and on-disk pack discovery.
//!
//! Config lives in the platform config dir (`directories` crate):
//!   `<config>/anysteno/settings.json`
//!   `<config>/anysteno/packs/<code>/{pack.toml,layout.toml,dict.json,user.json}`
//!
//! On first run the embedded official packs are written into that folder so the
//! user can read and customise them; user edits are then loaded from disk.

use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use steno_core::pack::Pack;

/// Where translated text goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Show output only inside anysteno's own practice window.
    InApp,
    /// Inject output into whatever application is focused.
    SystemWide,
}

/// User-persisted settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub pack_code: String,
    pub mode: Mode,
    pub enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pack_code: "en-beginner".to_string(),
            mode: Mode::InApp,
            enabled: true,
        }
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
