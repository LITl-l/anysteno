//! The Practice screen: free chording with a live stroke feed.
//!
//! Unlike Learn there is no target — this is where you write whatever you like
//! and watch what the engine makes of it. It is also the only screen that can
//! type into other applications, because injecting a drill's answers into
//! whatever window happens to be behind anysteno would be a nasty surprise.

use std::time::Instant;

use eframe::egui;
use steno_core::{Pack, SessionStats};

use super::theme::{font, Theme, SPACE_LG, SPACE_MD, SPACE_SM};
use super::{card, gap, heading, keyboard, muted, stat};
use crate::app::StenoEvent;
use crate::state::{Mode, Settings};

/// How many recent strokes the feed keeps.
const FEED_LEN: usize = 12;

/// One line of the live feed: what was chorded and what it produced.
pub struct FeedLine {
    pub stroke: String,
    pub result: Result<String, String>,
}

#[derive(Default)]
pub struct PracticeState {
    pub feed: Vec<FeedLine>,
    stats: SessionStats,
    started: Option<Instant>,
    last_stroke: String,
}

/// What the user asked for on this frame.
#[derive(Default)]
pub struct PracticeAction {
    pub clear: bool,
    pub undo: bool,
    pub settings_changed: bool,
}

impl PracticeState {
    pub fn clear(&mut self) {
        self.feed.clear();
        self.stats = SessionStats::new();
        self.started = None;
        self.last_stroke.clear();
    }
}

pub fn handle_events(state: &mut PracticeState, events: &[StenoEvent]) {
    for event in events {
        match event {
            StenoEvent::Stroke(stroke) => {
                state.started.get_or_insert_with(Instant::now);
                state.stats.record_stroke();
                state.last_stroke = stroke.clone();
                state.feed.push(FeedLine {
                    stroke: stroke.clone(),
                    // Filled in by the Text/NoMatch event that follows, if any.
                    result: Ok(String::new()),
                });
                if state.feed.len() > FEED_LEN {
                    state.feed.remove(0);
                }
            }
            StenoEvent::Text(text) => {
                state.stats.record_item(text, true);
                if let Some(last) = state.feed.last_mut() {
                    last.result = Ok(text.clone());
                }
            }
            StenoEvent::NoMatch(stroke) => {
                if let Some(last) = state.feed.last_mut() {
                    last.result = Err(stroke.clone());
                }
            }
            StenoEvent::Undo => {
                if let Some(last) = state.feed.last_mut() {
                    last.result = Ok("(undo)".to_string());
                }
            }
        }
    }
    if let Some(started) = state.started {
        state.stats.set_elapsed_ms(started.elapsed().as_millis() as u64);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    state: &mut PracticeState,
    theme: &Theme,
    pack: &Pack,
    held: &[String],
    pending: &[String],
    output: &str,
    settings: &mut Settings,
) -> PracticeAction {
    let mut action = PracticeAction::default();

    ui.horizontal(|ui| {
        heading(ui, theme, "Practice");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Clear").clicked() {
                action.clear = true;
            }
            if ui.button("Undo").clicked() {
                action.undo = true;
            }
        });
    });

    // Output destination. This is a consequential switch, so it says where the
    // text will land rather than naming an internal mode.
    ui.horizontal(|ui| {
        for mode in [Mode::InApp, Mode::SystemWide] {
            if ui
                .selectable_label(settings.mode == mode, mode.label())
                .clicked()
                && settings.mode != mode
            {
                settings.mode = mode;
                action.settings_changed = true;
            }
        }
    });
    if settings.mode == Mode::SystemWide {
        ui.label(
            egui::RichText::new(
                "Heads up: the letters you press also reach the other app — anysteno can't \
                 suppress them yet.",
            )
            .font(font::small())
            .color(theme.mid),
        );
    }

    ui.add_space(SPACE_MD);

    card(ui, theme, |ui| {
        ui.set_min_height(96.0);
        ui.horizontal_wrapped(|ui| {
            if output.is_empty() {
                muted(ui, theme, "Chord something — what you write appears here.");
            } else {
                ui.label(
                    egui::RichText::new(output)
                        .font(font::mono_lg())
                        .color(theme.text),
                );
            }
        });
    });

    ui.add_space(SPACE_MD);
    keyboard::show(ui, theme, &pack.layout, held, &[]);
    ui.add_space(SPACE_SM);
    keyboard::legend(ui, theme);

    // The feed sits directly under the keyboard: "what I just pressed" and
    // "what it produced" belong next to each other.
    ui.add_space(SPACE_MD);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(200.0);
            muted(ui, theme, "recent strokes");
            ui.add_space(4.0);
            if state.feed.is_empty() {
                muted(ui, theme, "—");
            }
            for line in state.feed.iter().rev().take(6) {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&line.stroke)
                            .font(font::mono())
                            .color(theme.accent),
                    );
                    match &line.result {
                        Ok(text) if text.is_empty() => {
                            muted(ui, theme, "…");
                        }
                        Ok(text) => {
                            ui.label(
                                egui::RichText::new(format!("→ {text}"))
                                    .font(font::mono())
                                    .color(theme.text),
                            );
                        }
                        Err(_) => {
                            ui.label(
                                egui::RichText::new("→ no match")
                                    .font(font::mono())
                                    .color(theme.err),
                            );
                        }
                    }
                });
            }
        });

        ui.add_space(SPACE_LG);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                stat(
                    ui,
                    theme,
                    &state
                        .stats
                        .wpm()
                        .map(|w| format!("{w:.0}"))
                        .unwrap_or_else(|| "—".into()),
                    "wpm",
                );
                ui.add_space(SPACE_LG);
                stat(ui, theme, &state.stats.strokes().to_string(), "strokes");
            });
            gap(ui);
            if !pending.is_empty() {
                muted(
                    ui,
                    theme,
                    &format!("waiting on: {} (press space to settle)", pending.join(" / ")),
                );
            }
        });
    });

    action
}
