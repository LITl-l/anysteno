//! The app's presentation layer: one module per screen, plus the shared
//! design tokens and widgets they are built from.

pub mod keyboard;
pub mod learn;
pub mod lookup;
pub mod onboarding;
pub mod practice;
pub mod settings;
pub mod theme;

use eframe::egui;

use theme::{blend, font, Theme, RADIUS, SPACE_MD, SPACE_SM};

/// A block of content on a raised surface.
///
/// `Frame` shrink-wraps its contents, which leaves cards at whatever width
/// their text happens to need — a column of ragged boxes. Claiming the full
/// width up front is what makes a stack of cards line up.
pub fn card<R>(
    ui: &mut egui::Ui,
    theme: &Theme,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Frame::none()
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.border))
        .rounding(egui::Rounding::same(RADIUS))
        .inner_margin(egui::Margin::same(SPACE_MD))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add(ui)
        })
}

/// A screen title.
pub fn heading(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(font::heading())
            .color(theme.text),
    );
}

/// Secondary explanatory text.
pub fn muted(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(font::small())
            .color(theme.muted),
    );
}

/// A labelled number, e.g. "28  wpm".
pub fn stat(ui: &mut egui::Ui, theme: &Theme, value: &str, label: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(
            egui::RichText::new(value)
                .font(font::mono_lg())
                .color(theme.text),
        );
        ui.label(
            egui::RichText::new(label)
                .font(font::small())
                .color(theme.muted),
        );
    });
}

/// A flat progress bar. Hand-drawn so its weight matches the rest of the UI.
pub fn progress_bar(ui: &mut egui::Ui, theme: &Theme, fraction: f32, colour: egui::Color32) {
    let height = 6.0;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let rounding = egui::Rounding::same(height / 2.0);
    ui.painter()
        .rect_filled(rect, rounding, blend(theme.border, theme.bg, 0.3));
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let filled = egui::Rect::from_min_size(
            rect.min,
            egui::vec2((rect.width() * fraction).max(height), height),
        );
        ui.painter().rect_filled(filled, rounding, colour);
    }
}

/// A small coloured tag.
pub fn chip(ui: &mut egui::Ui, theme: &Theme, text: &str, colour: egui::Color32) {
    egui::Frame::none()
        .fill(blend(colour, theme.bg, 0.8))
        .stroke(egui::Stroke::new(1.0, blend(colour, theme.bg, 0.45)))
        .rounding(egui::Rounding::same(4.0))
        .inner_margin(egui::Margin::symmetric(6.0, 2.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).font(font::small()).color(colour));
        });
}

/// Vertical breathing room between sections.
pub fn gap(ui: &mut egui::Ui) {
    ui.add_space(SPACE_SM);
}
