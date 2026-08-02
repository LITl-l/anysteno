//! The live keyboard: the app's main teaching surface.
//!
//! It draws the *physical* QWERTY keyboard the user actually has, with each
//! pack's steno letters overlaid on the keys that chord. Showing the real
//! keyboard rather than an abstract steno diagram is the whole point of
//! "stenography for any keyboard" — the learner needs to map steno onto the
//! hardware in front of them, not onto a machine they don't own.
//!
//! The geometry table below is presentation data and deliberately lives in the
//! app, not the core: `steno-core` must not learn what a keyboard looks like.
//! Because it is keyed by the same platform-neutral names that `layout.toml`
//! uses (`KeyQ`, `SemiColon`), it renders any pack — shipped or user-written —
//! with no per-pack code.

use std::collections::HashSet;

use eframe::egui;
use steno_core::{Layout, StenoKey};

use super::theme::{blend, font, Theme, RADIUS};

/// One row of the physical keyboard. `offset` is the row's stagger, in key
/// widths, which is what makes the drawing read as a keyboard at a glance.
struct Row {
    offset: f32,
    keys: &'static [&'static str],
}

const ROWS: &[Row] = &[
    Row {
        offset: 0.0,
        keys: &[
            "KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyY", "KeyU", "KeyI", "KeyO", "KeyP",
            "LeftBracket", "RightBracket",
        ],
    },
    Row {
        offset: 0.35,
        keys: &[
            "KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH", "KeyJ", "KeyK", "KeyL", "SemiColon",
            "Quote",
        ],
    },
    Row {
        offset: 0.85,
        keys: &[
            "KeyZ", "KeyX", "KeyC", "KeyV", "KeyB", "KeyN", "KeyM", "Comma", "Dot", "Slash",
        ],
    },
];

/// Widest row, in key units — the drawing's aspect ratio.
const COLUMNS: f32 = 12.0;

/// What a key is showing right now. Ordered by visual priority: a key that is
/// both hinted and held reads as held.
#[derive(Clone, Copy, PartialEq, Eq)]
enum KeyState {
    /// Not part of this pack's chording at all.
    Inert,
    /// Chords, but idle.
    Resting,
    /// Part of the chord the user is being asked to press.
    Hinted,
    /// Currently held down.
    Held,
}

/// Draw the keyboard. `held` and `target` are steno key *ids*; `target` is the
/// hint chord and may be empty.
pub fn show(
    ui: &mut egui::Ui,
    theme: &Theme,
    layout: &Layout,
    held: &[String],
    target: &[String],
) -> egui::Response {
    let gap = 4.0;
    let unit = ((ui.available_width() - gap) / COLUMNS).clamp(24.0, 56.0);
    let size = egui::vec2(unit * COLUMNS, unit * ROWS.len() as f32 + gap);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let painter = ui.painter();
    let held: HashSet<&str> = held.iter().map(String::as_str).collect();
    let target: HashSet<&str> = target.iter().map(String::as_str).collect();

    for (r, row) in ROWS.iter().enumerate() {
        for (c, physical) in row.keys.iter().enumerate() {
            let top_left = rect.min
                + egui::vec2(
                    (row.offset + c as f32) * unit,
                    r as f32 * unit,
                );
            let key_rect =
                egui::Rect::from_min_size(top_left, egui::vec2(unit - gap, unit - gap));

            let steno = layout
                .steno_for(physical)
                .and_then(|id| layout.order().iter().find(|k| k.id == id));
            let state = match steno {
                None => KeyState::Inert,
                Some(k) if held.contains(k.id.as_str()) => KeyState::Held,
                Some(k) if target.contains(k.id.as_str()) => KeyState::Hinted,
                Some(_) => KeyState::Resting,
            };
            draw_key(painter, theme, key_rect, physical, steno, state, unit);
        }
    }

    response
}

fn draw_key(
    painter: &egui::Painter,
    theme: &Theme,
    rect: egui::Rect,
    physical: &str,
    steno: Option<&StenoKey>,
    state: KeyState,
    unit: f32,
) {
    let rounding = egui::Rounding::same(RADIUS);
    let bank = steno.map(|k| theme.bank(k.bank));

    let (fill, border, width) = match (state, bank) {
        (KeyState::Inert, _) | (_, None) => (
            blend(theme.surface, theme.bg, 0.55),
            blend(theme.border, theme.bg, 0.45),
            1.0,
        ),
        (KeyState::Resting, Some(c)) => (blend(c, theme.bg, 0.88), blend(c, theme.bg, 0.55), 1.0),
        (KeyState::Hinted, Some(c)) => (blend(c, theme.bg, 0.68), c, 2.0),
        (KeyState::Held, Some(c)) => (c, c, 2.0),
    };

    painter.rect_filled(rect, rounding, fill);
    painter.rect_stroke(rect, rounding, egui::Stroke::new(width, border));

    // The physical letter sits small in the corner: it's what the user hunts
    // for, but the steno letter is what they should be learning.
    let physical_colour = match state {
        KeyState::Held => contrast_on(theme, bank.unwrap_or(theme.accent)),
        KeyState::Inert => blend(theme.muted, theme.bg, 0.45),
        _ => theme.muted,
    };
    painter.text(
        rect.left_top() + egui::vec2(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        physical_label(physical),
        egui::FontId::new((unit * 0.24).clamp(8.0, 12.0), egui::FontFamily::Monospace),
        physical_colour,
    );

    let Some(key) = steno else { return };
    let steno_colour = match state {
        KeyState::Held => contrast_on(theme, bank.unwrap_or(theme.accent)),
        KeyState::Hinted => theme.text,
        _ => bank.unwrap_or(theme.text),
    };
    painter.text(
        rect.center() + egui::vec2(0.0, 3.0),
        egui::Align2::CENTER_CENTER,
        &key.letter,
        egui::FontId::new((unit * 0.42).clamp(12.0, 22.0), egui::FontFamily::Proportional),
        steno_colour,
    );
}

/// Pick a legible foreground for a filled key.
fn contrast_on(theme: &Theme, fill: egui::Color32) -> egui::Color32 {
    // Rec. 601 luma is good enough to decide black-or-white on flat fills.
    let luma =
        0.299 * fill.r() as f32 + 0.587 * fill.g() as f32 + 0.114 * fill.b() as f32;
    if luma > 140.0 {
        egui::Color32::from_rgb(0x10, 0x12, 0x16)
    } else {
        theme.text
    }
}

/// The glyph printed on a physical key.
fn physical_label(name: &str) -> &str {
    match name {
        "SemiColon" => ";",
        "Quote" => "'",
        "LeftBracket" => "[",
        "RightBracket" => "]",
        "Comma" => ",",
        "Dot" => ".",
        "Slash" => "/",
        "BackSlash" => "\\",
        "Minus" => "-",
        "Equal" => "=",
        // "KeyQ" -> "Q"
        other => other.strip_prefix("Key").unwrap_or(other),
    }
}

/// Steno keys this pack defines that the drawing has nowhere to put — a pack
/// may bind a key the geometry doesn't include. Surfaced rather than silently
/// omitted, so a user debugging their own layout can see it.
pub fn unrendered_keys(layout: &Layout) -> Vec<&StenoKey> {
    let drawn: HashSet<&str> = ROWS.iter().flat_map(|r| r.keys.iter().copied()).collect();
    layout
        .order()
        .iter()
        .filter(|key| {
            !layout
                .physical_keys_for(&key.id)
                .iter()
                .any(|p| drawn.contains(p))
        })
        .collect()
}

/// A colour key for the three banks, so the keyboard's colouring is decodable.
pub fn legend(ui: &mut egui::Ui, theme: &Theme) {
    ui.horizontal(|ui| {
        for (colour, label) in [
            (theme.left, "left hand"),
            (theme.mid, "vowels"),
            (theme.right, "right hand"),
        ] {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, egui::Rounding::same(2.0), colour);
            ui.label(egui::RichText::new(label).font(font::small()).color(theme.muted));
            ui.add_space(6.0);
        }
    });
}

/// Render a stroke with each letter in its bank's colour, e.g. `KAT` as a blue
/// K, an amber A and a teal T — the same colours the keyboard uses, so the
/// hint and the keys agree at a glance.
///
/// This produces one `LayoutJob` rather than a row of chip widgets on purpose:
/// a row of widgets always fills the available width, so it cannot be centred
/// under a centred prompt. A single label can.
pub fn stroke_label(ui: &mut egui::Ui, theme: &Theme, layout: &Layout, stroke: &str) {
    let font = font::mono_lg();
    let Some(ids) = steno_core::parse_stroke(layout.order(), stroke) else {
        // An unparseable stroke still has to be readable; show it verbatim.
        ui.label(egui::RichText::new(stroke).font(font).color(theme.muted));
        return;
    };

    let mut job = egui::text::LayoutJob::default();
    for (i, id) in ids.iter().enumerate() {
        let Some(key) = layout.order().iter().find(|k| &k.id == id) else {
            continue;
        };
        job.append(
            &key.letter,
            if i == 0 { 0.0 } else { 6.0 },
            egui::TextFormat {
                font_id: font.clone(),
                color: theme.bank(key.bank),
                ..Default::default()
            },
        );
    }
    ui.label(job);
}
