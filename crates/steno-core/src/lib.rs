//! # steno-core
//!
//! The platform-neutral brain of anysteno. It has **no I/O and no GUI/OS
//! dependencies**, so it is identical on Linux, Windows and macOS and is fully
//! unit-tested with `cargo test`.
//!
//! Pipeline (data flows top to bottom):
//!
//! ```text
//!   physical key events
//!         │
//!   [chord]      accumulate simultaneous presses -> one chord on full release
//!         │
//!   [layout]     physical key -> steno key id
//!         │
//!   [stroke]     steno keys -> canonical stroke string (e.g. "KAT")
//!         │
//!   [dictionary] stroke(s) -> output text (greedy longest match)
//!         │
//!   [engine]     ties it together, produces translations for the app
//! ```
//!
//! A **language pack** ([`Pack`]) bundles a [`Layout`] + [`Dictionary`] + meta.
//! Adding a language never requires a recompile — drop a pack folder in the
//! user config dir. Two official packs ship embedded (see [`defaults`]).

pub mod chord;
pub mod defaults;
pub mod dictionary;
pub mod engine;
pub mod layout;
pub mod pack;
pub mod stroke;

pub use chord::ChordAccumulator;
pub use dictionary::{Dictionary, Translation, Translator};
pub use engine::{Engine, EngineOutput, KeyEvent};
pub use layout::Layout;
pub use pack::{Pack, PackError};
pub use stroke::{Bank, StenoKey};
