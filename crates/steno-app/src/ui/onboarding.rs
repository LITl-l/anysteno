//! First run. Steno's one counter-intuitive idea is that you press several keys
//! *together* and the word appears when you let go — someone who types normally
//! will hold nothing and see nothing. This screen exists to say that once, with
//! the keyboard right there, before handing over to the first lesson.

use eframe::egui;
use steno_core::{Curriculum, LessonItem, Pack};

use super::theme::{font, Theme, SPACE_MD, SPACE_SM};
use super::{card, heading, keyboard, muted};

/// Pick something to demonstrate with.
///
/// It must be a *chord*, and preferably a real word. The earliest lesson items
/// are single keys, so demonstrating "hold these keys at once" with the first
/// one available would teach nothing; and a fingerspelled letter, while
/// technically a chord, doesn't show why anyone would bother. Each preference
/// falls back to the next, so an unusual pack still gets something to show.
fn example(pack: &Pack, curriculum: &Curriculum) -> Option<LessonItem> {
    let order = pack.layout.order();
    let items = || curriculum.lessons().iter().flat_map(|l| l.items.iter());
    let is_chord = |item: &LessonItem| {
        item.strokes.len() == 1
            && item
                .strokes
                .first()
                .and_then(|s| steno_core::parse_stroke(order, s))
                .is_some_and(|ids| ids.len() >= 2)
    };
    let is_word = |item: &LessonItem| item.text.chars().count() > 1;

    items()
        .find(|item| is_chord(item) && is_word(item))
        .or_else(|| items().find(|item| is_chord(item)))
        .or_else(|| items().next())
        .cloned()
}

/// Draw the walkthrough. Returns `true` when the user dismisses it.
pub fn show(
    ui: &mut egui::Ui,
    theme: &Theme,
    pack: &Pack,
    curriculum: &Curriculum,
    held: &[String],
) -> bool {
    let mut done = false;
    let demo = example(pack, curriculum);

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("Welcome to anysteno")
                    .font(font::display())
                    .color(theme.text),
            );
            muted(
                ui,
                theme,
                "Stenography on the keyboard you already own — no machine required.",
            );
        });

        ui.add_space(SPACE_MD);

        card(ui, theme, |ui| {
            heading(ui, theme, "Press keys together, not one at a time");
            ui.add_space(SPACE_SM);
            ui.label(
                egui::RichText::new(
                    "A whole word is one chord. Hold all of its keys down at once and let go — \
                     the word appears when the last key comes up. Staggered presses still count \
                     as one chord, so you don't have to be precise.",
                )
                .font(font::body())
                .color(theme.text),
            );
        });

        ui.add_space(SPACE_MD);

        if let Some(item) = &demo {
            card(ui, theme, |ui| {
                ui.horizontal(|ui| {
                    heading(ui, theme, "Try it:");
                    ui.label(
                        egui::RichText::new(&item.text)
                            .font(font::heading())
                            .color(theme.accent),
                    );
                    muted(ui, theme, "— hold the outlined keys, then release");
                });
                ui.add_space(SPACE_SM);
                let ids = item
                    .strokes
                    .first()
                    .and_then(|s| steno_core::parse_stroke(pack.layout.order(), s))
                    .unwrap_or_default();
                keyboard::show(ui, theme, &pack.layout, held, &ids);
                ui.add_space(SPACE_SM);
                keyboard::legend(ui, theme);
            });

            ui.add_space(SPACE_MD);
        }

        card(ui, theme, |ui| {
            for line in [
                format!(
                    "The “{}” chord undoes the last word — it's your backspace.",
                    pack.undo_stroke
                ),
                "Learn drills you word by word. Practice is a free page to write on.".to_string(),
            ] {
                ui.label(
                    egui::RichText::new(format!("· {line}"))
                        .font(font::body())
                        .color(theme.text),
                );
            }
        });

        ui.add_space(SPACE_MD);
        ui.vertical_centered(|ui| {
            if ui.button("Start the first lesson").clicked() {
                done = true;
            }
        });
        ui.add_space(SPACE_SM);
    });

    done
}
