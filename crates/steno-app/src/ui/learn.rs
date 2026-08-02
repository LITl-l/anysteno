//! The Learn screen: pick a lesson, then drill it.
//!
//! The drill is deliberately forgiving. A wrong answer never advances and never
//! ends the run — it reveals the stroke hint on the keyboard instead, so the
//! learner's next move is always visible. Steno's hard part is recalling a
//! chord, and hiding the answer after a miss just stalls the session.

use std::time::Instant;

use eframe::egui;
use steno_core::{Curriculum, Drill, DrillOutcome, Pack, SessionStats};

use super::theme::{font, Theme, SPACE_LG, SPACE_MD, SPACE_SM, SPACE_XS};
use super::{card, chip, heading, keyboard, muted, progress_bar, stat};
use crate::app::StenoEvent;
use crate::state::Settings;

/// Result of the most recent answer, shown until the next one.
enum Feedback {
    Correct,
    Wrong { got: String },
}

#[derive(Default)]
pub struct LearnState {
    drill: Option<Drill>,
    lesson_index: usize,
    stats: SessionStats,
    started: Option<Instant>,
    feedback: Option<Feedback>,
    /// Set when the final item is answered, so the summary can be shown.
    completed: bool,
}

impl LearnState {
    /// Begin `index`, discarding any run in progress.
    fn start(&mut self, curriculum: &Curriculum, index: usize) {
        let Some(lesson) = curriculum.get(index) else {
            return;
        };
        self.drill = Some(Drill::new(lesson));
        self.lesson_index = index;
        self.stats = SessionStats::new();
        self.started = None;
        self.feedback = None;
        self.completed = false;
    }

    fn stop(&mut self) {
        self.drill = None;
        self.feedback = None;
        self.completed = false;
    }

    pub fn is_drilling(&self) -> bool {
        self.drill.is_some() && !self.completed
    }

    /// The chord the learner should be pressing, for the keyboard hint.
    fn target_strokes(&self) -> Vec<String> {
        self.drill
            .as_ref()
            .and_then(|d| d.current())
            .map(|item| item.strokes.clone())
            .unwrap_or_default()
    }

    /// Whether the hint is currently revealed: always on if the user asked for
    /// it, and forced on after a mistake.
    fn hint_visible(&self, settings: &Settings) -> bool {
        settings.show_hints
            || self
                .drill
                .as_ref()
                .is_some_and(|d| d.attempts_on_current() > 0)
    }
}

/// Feed engine output into the drill. Returns `true` if settings changed.
pub fn handle_events(
    state: &mut LearnState,
    events: &[StenoEvent],
    pack_code: &str,
    settings: &mut Settings,
) -> bool {
    let Some(drill) = state.drill.as_mut() else {
        return false;
    };
    let mut settings_changed = false;

    for event in events {
        match event {
            StenoEvent::Stroke(_) => {
                state.started.get_or_insert_with(Instant::now);
                state.stats.record_stroke();
            }
            StenoEvent::Text(text) => {
                let Some(outcome) = drill.submit(text) else {
                    continue;
                };
                match outcome {
                    DrillOutcome::Correct => {
                        state.stats.record_item(text, true);
                        state.feedback = Some(Feedback::Correct);
                    }
                    DrillOutcome::Wrong => {
                        state.stats.record_item(text, false);
                        state.feedback = Some(Feedback::Wrong { got: text.clone() });
                    }
                }
                if drill.is_complete() && !state.completed {
                    state.completed = true;
                    settings.mark_lesson_done(pack_code, state.lesson_index);
                    settings_changed = true;
                }
            }
            // An unmatched chord is a wrong answer the dictionary couldn't even
            // name. Show what was chorded so the learner can see the miss.
            StenoEvent::NoMatch(stroke) => {
                state.stats.record_item("", false);
                state.feedback = Some(Feedback::Wrong {
                    got: format!("{stroke} (no match)"),
                });
            }
            StenoEvent::Undo => state.feedback = None,
        }
    }

    if let Some(started) = state.started {
        state.stats.set_elapsed_ms(started.elapsed().as_millis() as u64);
    }
    settings_changed
}

/// Draw the screen. Returns `true` if settings changed and should be saved.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    state: &mut LearnState,
    theme: &Theme,
    pack: &Pack,
    curriculum: &Curriculum,
    held: &[String],
    settings: &mut Settings,
) -> bool {
    if curriculum.is_empty() {
        return empty_state(ui, theme, pack);
    }
    if state.completed {
        return summary(ui, state, theme, curriculum, settings);
    }
    match state.drill.is_some() {
        true => drill_view(ui, state, theme, pack, curriculum, held, settings),
        false => lesson_list(ui, state, theme, pack, curriculum, settings),
    }
}

// --- lesson picker ------------------------------------------------------

fn lesson_list(
    ui: &mut egui::Ui,
    state: &mut LearnState,
    theme: &Theme,
    pack: &Pack,
    curriculum: &Curriculum,
    settings: &mut Settings,
) -> bool {
    let done = settings.lessons_done(&pack.code);
    let next = done.min(curriculum.len().saturating_sub(1));

    heading(ui, theme, &format!("{} — lessons", pack.name));
    muted(
        ui,
        theme,
        &format!(
            "{} of {} finished. Lessons are generated from this pack's layout and dictionary.",
            done.min(curriculum.len()),
            curriculum.len()
        ),
    );
    ui.add_space(SPACE_SM);
    progress_bar(
        ui,
        theme,
        done as f32 / curriculum.len() as f32,
        theme.accent,
    );
    ui.add_space(SPACE_MD);

    let mut start: Option<usize> = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        for lesson in curriculum.lessons() {
            let is_done = lesson.index < done;
            let is_next = lesson.index == next && !is_done;
            card(ui, theme, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(lesson.title())
                                    .font(font::body())
                                    .strong()
                                    .color(theme.text),
                            );
                            if is_done {
                                chip(ui, theme, "done", theme.ok);
                            }
                            if is_next {
                                chip(ui, theme, "next up", theme.accent);
                            }
                            if lesson.is_review() {
                                chip(ui, theme, "review", theme.mid);
                            }
                        });
                        let subtitle = if lesson.is_review() {
                            format!("{} words to consolidate", lesson.len())
                        } else {
                            format!("new keys {} · {} words", lesson.keys_label(), lesson.len())
                        };
                        muted(ui, theme, &subtitle);
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if is_done { "Practise again" } else { "Start" };
                        if ui.button(label).clicked() {
                            start = Some(lesson.index);
                        }
                    });
                });
            });
            ui.add_space(SPACE_SM);
        }
    });

    if let Some(index) = start {
        state.start(curriculum, index);
    }
    false
}

// --- the drill ----------------------------------------------------------

fn drill_view(
    ui: &mut egui::Ui,
    state: &mut LearnState,
    theme: &Theme,
    pack: &Pack,
    curriculum: &Curriculum,
    held: &[String],
    settings: &mut Settings,
) -> bool {
    let hint_visible = state.hint_visible(settings);
    let targets = state.target_strokes();
    let (position, total, attempts) = state
        .drill
        .as_ref()
        .map(|d| (d.position(), d.total(), d.attempts_on_current()))
        .unwrap_or((0, 0, 0));

    let mut quit = false;
    let mut skip = false;

    ui.horizontal(|ui| {
        let title = curriculum
            .get(state.lesson_index)
            .map(|l| l.title())
            .unwrap_or_default();
        heading(ui, theme, &title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("End lesson").clicked() {
                quit = true;
            }
            if ui.button("Skip word").clicked() {
                skip = true;
            }
        });
    });
    ui.add_space(SPACE_SM);
    progress_bar(
        ui,
        theme,
        position as f32 / total.max(1) as f32,
        theme.ok,
    );
    ui.add_space(SPACE_LG);

    // The prompt.
    let current = state
        .drill
        .as_ref()
        .and_then(|d| d.current())
        .map(|i| i.text.clone())
        .unwrap_or_default();
    ui.vertical_centered(|ui| {
        muted(ui, theme, &format!("word {} of {}", position + 1, total));
        ui.add_space(SPACE_XS);
        ui.label(
            egui::RichText::new(&current)
                .font(font::display())
                .color(theme.text),
        );
        ui.add_space(SPACE_SM);
        if hint_visible {
            for stroke in &targets {
                keyboard::stroke_label(ui, theme, &pack.layout, stroke);
            }
            if targets.len() > 1 {
                muted(ui, theme, "two strokes, one after the other");
            }
        } else {
            muted(ui, theme, "chord it — a hint appears if you miss");
        }
    });

    ui.add_space(SPACE_MD);

    // The keyboard only hints the *first* stroke: highlighting a two-stroke
    // word's keys at once would show a chord that must never be pressed
    // together.
    let target_ids = if hint_visible {
        targets
            .first()
            .and_then(|s| steno_core::parse_stroke(pack.layout.order(), s))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    keyboard::show(ui, theme, &pack.layout, held, &target_ids);
    ui.add_space(SPACE_SM);

    // Feedback, centred under the prompt it refers to. Deliberately worded
    // rather than symbolic: the tick and cross glyphs are not in egui's bundled
    // fonts and render as empty boxes.
    ui.vertical_centered(|ui| {
        match &state.feedback {
            Some(Feedback::Correct) => {
                ui.label(
                    egui::RichText::new("Correct")
                        .font(font::body())
                        .color(theme.ok),
                );
            }
            Some(Feedback::Wrong { got }) => {
                ui.label(
                    egui::RichText::new(format!("Not quite — that gave “{got}”"))
                        .font(font::body())
                        .color(theme.err),
                );
            }
            None => muted(ui, theme, "chord the word above"),
        }
        if attempts > 0 {
            let plural = if attempts == 1 { "try" } else { "tries" };
            muted(ui, theme, &format!("{attempts} {plural} on this word"));
        }
    });

    // The score sits at the bottom of the panel rather than directly under the
    // keyboard, so the drill doesn't float in a half-empty window.
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        ui.horizontal(|ui| {
            stat(ui, theme, &fmt_wpm(&state.stats), "wpm");
            ui.add_space(SPACE_LG);
            stat(ui, theme, &fmt_accuracy(&state.stats), "accuracy");
            ui.add_space(SPACE_LG);
            stat(ui, theme, &format!("{position}/{total}"), "words");
        });
    });

    if skip {
        if let Some(drill) = state.drill.as_mut() {
            drill.skip();
            state.feedback = None;
            if drill.is_complete() && !state.completed {
                state.completed = true;
                settings.mark_lesson_done(&pack.code, state.lesson_index);
                return true;
            }
        }
    }
    if quit {
        state.stop();
    }
    false
}

// --- end of lesson ------------------------------------------------------

fn summary(
    ui: &mut egui::Ui,
    state: &mut LearnState,
    theme: &Theme,
    curriculum: &Curriculum,
    settings: &mut Settings,
) -> bool {
    let mut next: Option<usize> = None;
    let mut repeat = false;
    let mut back = false;

    // Sit the summary a little above centre rather than pinned to the top of an
    // otherwise empty panel.
    ui.add_space(ui.available_height() * 0.18);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new("Lesson complete")
                .font(font::display())
                .color(theme.ok),
        );
        ui.add_space(SPACE_SM);
        muted(
            ui,
            theme,
            &format!(
                "{} of {} lessons finished",
                settings.lessons_done(&settings.pack_code).min(curriculum.len()),
                curriculum.len()
            ),
        );
        ui.add_space(SPACE_LG);

        // Score on the left, next actions on the right. A right-to-left layout
        // anchors the buttons reliably, which centring a free-floating row of
        // them does not — an egui row always claims the full width.
        card(ui, theme, |ui| {
            ui.horizontal(|ui| {
                stat(ui, theme, &fmt_wpm(&state.stats), "wpm");
                ui.add_space(SPACE_LG);
                stat(ui, theme, &fmt_accuracy(&state.stats), "accuracy");
                ui.add_space(SPACE_LG);
                stat(ui, theme, &state.stats.strokes().to_string(), "strokes");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("All lessons").clicked() {
                        back = true;
                    }
                    if ui.button("Repeat").clicked() {
                        repeat = true;
                    }
                    if state.lesson_index + 1 < curriculum.len()
                        && ui.button("Next lesson").clicked()
                    {
                        next = Some(state.lesson_index + 1);
                    }
                });
            });
        });
    });

    if let Some(index) = next {
        state.start(curriculum, index);
    } else if repeat {
        let index = state.lesson_index;
        state.start(curriculum, index);
    } else if back {
        state.stop();
    }
    false
}

fn empty_state(ui: &mut egui::Ui, theme: &Theme, pack: &Pack) -> bool {
    ui.add_space(SPACE_LG);
    ui.vertical_centered(|ui| {
        heading(ui, theme, "No lessons for this pack");
        ui.add_space(SPACE_SM);
        muted(
            ui,
            theme,
            &format!(
                "“{}” has {} dictionary entries that this layout can chord. Add entries to its \
                 dict.json or user.json and restart to generate lessons.",
                pack.name,
                pack.dict.len()
            ),
        );
    });
    false
}

fn fmt_wpm(stats: &SessionStats) -> String {
    stats
        .wpm()
        .map(|w| format!("{w:.0}"))
        .unwrap_or_else(|| "—".to_string())
}

fn fmt_accuracy(stats: &SessionStats) -> String {
    stats
        .accuracy()
        .map(|a| format!("{:.0}%", a * 100.0))
        .unwrap_or_else(|| "—".to_string())
}
