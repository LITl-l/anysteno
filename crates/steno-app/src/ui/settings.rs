//! The Settings screen, which is also where anything that went wrong is
//! reported. Pack loading is fallible and used to fail silently; a user who
//! mistypes their `user.json` needs to be told, not left wondering why their
//! new word doesn't work.

use eframe::egui;
use steno_core::Pack;

use super::theme::{font, Theme, ThemeMode, SPACE_MD, SPACE_SM};
use super::{card, chip, heading, keyboard, muted};
use crate::state::{Paths, Settings};

#[derive(Default)]
pub struct SettingsAction {
    /// Index into `packs` of a newly chosen pack.
    pub select_pack: Option<usize>,
    pub settings_changed: bool,
    pub reload_packs: bool,
    pub reset_progress: bool,
}

pub fn show(
    ui: &mut egui::Ui,
    theme: &Theme,
    packs: &[Pack],
    active: &Pack,
    warnings: &[String],
    paths: &Paths,
    settings: &mut Settings,
) -> SettingsAction {
    let mut action = SettingsAction::default();

    egui::ScrollArea::vertical().show(ui, |ui| {
        heading(ui, theme, "Settings");
        ui.add_space(SPACE_MD);

        // --- language -----------------------------------------------------
        card(ui, theme, |ui| {
            ui.label(egui::RichText::new("Language").font(font::body()).strong());
            ui.add_space(SPACE_SM);
            egui::ComboBox::from_id_salt("pack")
                .selected_text(&active.name)
                .width(260.0)
                .show_ui(ui, |ui| {
                    for (i, pack) in packs.iter().enumerate() {
                        if ui
                            .selectable_label(pack.code == settings.pack_code, &pack.name)
                            .clicked()
                            && pack.code != settings.pack_code
                        {
                            action.select_pack = Some(i);
                        }
                    }
                });
            if !active.description.is_empty() {
                ui.add_space(SPACE_SM);
                muted(ui, theme, &active.description);
            }
            ui.add_space(SPACE_SM);
            muted(
                ui,
                theme,
                &format!(
                    "{} entries · {} steno keys · undo stroke “{}”",
                    active.dict.len(),
                    active.layout.order().len(),
                    active.undo_stroke
                ),
            );

            // A pack may bind a key this app has no picture of.
            let missing = keyboard::unrendered_keys(&active.layout);
            if !missing.is_empty() {
                ui.add_space(SPACE_SM);
                let names: Vec<&str> = missing.iter().map(|k| k.id.as_str()).collect();
                ui.label(
                    egui::RichText::new(format!(
                        "Not shown on the keyboard diagram: {}. They still chord — the diagram \
                         only draws the main letter block.",
                        names.join(", ")
                    ))
                    .font(font::small())
                    .color(theme.mid),
                );
            }
        });

        ui.add_space(SPACE_MD);

        // --- learning -----------------------------------------------------
        card(ui, theme, |ui| {
            ui.label(egui::RichText::new("Learning").font(font::body()).strong());
            ui.add_space(SPACE_SM);
            if ui
                .checkbox(&mut settings.show_hints, "Show the stroke hint before I miss")
                .changed()
            {
                action.settings_changed = true;
            }
            muted(
                ui,
                theme,
                "Off means the keyboard only lights up after a wrong answer.",
            );
            ui.add_space(SPACE_SM);
            ui.horizontal(|ui| {
                muted(
                    ui,
                    theme,
                    &format!("{} lessons finished in this pack.", settings.lessons_done(&active.code)),
                );
                if ui.button("Reset progress").clicked() {
                    action.reset_progress = true;
                }
            });
        });

        ui.add_space(SPACE_MD);

        // --- appearance ---------------------------------------------------
        card(ui, theme, |ui| {
            ui.label(egui::RichText::new("Appearance").font(font::body()).strong());
            ui.add_space(SPACE_SM);
            ui.horizontal(|ui| {
                for mode in [ThemeMode::Dark, ThemeMode::Light] {
                    if ui
                        .selectable_label(settings.theme == mode, mode.label())
                        .clicked()
                        && settings.theme != mode
                    {
                        settings.theme = mode;
                        action.settings_changed = true;
                    }
                }
            });
        });

        ui.add_space(SPACE_MD);

        // --- engine -------------------------------------------------------
        card(ui, theme, |ui| {
            ui.label(egui::RichText::new("Engine").font(font::body()).strong());
            ui.add_space(SPACE_SM);
            if ui
                .checkbox(&mut settings.enabled, "Translate chords")
                .changed()
            {
                action.settings_changed = true;
            }
            muted(
                ui,
                theme,
                "Turn off to type normally without anysteno interpreting your keys.",
            );
        });

        ui.add_space(SPACE_MD);

        // --- files and problems -------------------------------------------
        card(ui, theme, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Packs on disk").font(font::body()).strong());
                if warnings.is_empty() {
                    chip(ui, theme, "all loaded", theme.ok);
                } else {
                    chip(ui, theme, &format!("{} problem(s)", warnings.len()), theme.err);
                }
            });
            ui.add_space(SPACE_SM);
            ui.label(
                egui::RichText::new(paths.packs_dir.display().to_string())
                    .font(font::mono())
                    .color(theme.muted),
            );
            ui.add_space(SPACE_SM);
            muted(
                ui,
                theme,
                "Add a folder here to add a language. Drop a user.json beside any dict.json to \
                 override or add words.",
            );

            for warning in warnings {
                ui.add_space(SPACE_SM);
                ui.label(
                    egui::RichText::new(warning)
                        .font(font::small())
                        .color(theme.err),
                );
            }

            ui.add_space(SPACE_SM);
            if ui.button("Reload packs").clicked() {
                action.reload_packs = true;
            }
        });

        ui.add_space(SPACE_MD);
        muted(
            ui,
            theme,
            &format!("anysteno {}", env!("CARGO_PKG_VERSION")),
        );
    });

    action
}
