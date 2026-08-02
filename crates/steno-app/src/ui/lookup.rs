//! The Dictionary screen: "how do I write this?"
//!
//! Search runs over both sides of the dictionary at once, so typing `cat` and
//! typing `KAT` find the same entry. Selecting a result paints its chord onto
//! the keyboard, which is the answer the learner actually needs — a stroke
//! string like `TPH` means nothing until you see which keys it is.

use eframe::egui;
use steno_core::{Entry, Pack, ReverseIndex};

use super::theme::{font, Theme, SPACE_MD, SPACE_SM};
use super::{card, heading, keyboard, muted};

/// Results shown at once. The browser is for looking things up, not for
/// scrolling a whole Plover dictionary.
const LIMIT: usize = 60;

#[derive(Default)]
pub struct LookupState {
    query: String,
    selected: Option<Entry>,
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut LookupState,
    theme: &Theme,
    pack: &Pack,
    index: &ReverseIndex,
) {
    heading(ui, theme, "Dictionary");
    muted(
        ui,
        theme,
        &format!(
            "{} entries in “{}”. Search by word or by stroke.",
            index.len(),
            pack.name
        ),
    );
    ui.add_space(SPACE_SM);

    ui.horizontal(|ui| {
        let response = ui.add(
            egui::TextEdit::singleline(&mut state.query)
                .hint_text("a word, or a stroke like KAT")
                .desired_width(240.0),
        );
        if response.changed() {
            state.selected = None;
        }
        if !state.query.is_empty() && ui.button("Clear").clicked() {
            state.query.clear();
            state.selected = None;
        }
    });

    ui.add_space(SPACE_MD);

    // The chord for whatever is selected.
    if let Some(entry) = state.selected.clone() {
        card(ui, theme, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&entry.text)
                        .font(font::heading())
                        .color(theme.text),
                );
                ui.add_space(SPACE_SM);
                for stroke in &entry.strokes {
                    keyboard::stroke_label(ui, theme, &pack.layout, stroke);
                }
            });
            if entry.strokes.len() > 1 {
                muted(ui, theme, "two strokes, chorded one after the other");
            }
            ui.add_space(SPACE_SM);
            // Only the first stroke can be shown as a chord: later strokes are
            // separate presses, and lighting them all up would depict a chord
            // that must never be pressed.
            let ids = entry
                .strokes
                .first()
                .and_then(|s| steno_core::parse_stroke(pack.layout.order(), s))
                .unwrap_or_default();
            keyboard::show(ui, theme, &pack.layout, &[], &ids);
        });
        ui.add_space(SPACE_MD);
    }

    let results = index.search(&state.query, LIMIT);
    if results.is_empty() {
        muted(ui, theme, "Nothing matches that.");
        return;
    }
    if results.len() == LIMIT {
        muted(
            ui,
            theme,
            &format!("Showing the first {LIMIT} matches — narrow the search to see more."),
        );
        ui.add_space(SPACE_SM);
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for entry in results {
            let selected = state
                .selected
                .as_ref()
                .is_some_and(|s| s.text == entry.text && s.strokes == entry.strokes);
            let response = ui.selectable_label(
                selected,
                egui::RichText::new(format!("{:<18} {}", entry.text, entry.key()))
                    .font(font::mono()),
            );
            if response.clicked() {
                state.selected = Some(entry.clone());
            }
        }
    });
}
