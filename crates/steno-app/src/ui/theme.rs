//! Design tokens and the egui visuals derived from them.
//!
//! Colours live here rather than at their point of use so the whole app can be
//! recoloured in one place, and so the keyboard view and the drill screen agree
//! on what "a left-hand key" looks like. That agreement is the point: bank
//! colour is a learning cue, not decoration.

use eframe::egui;
use serde::{Deserialize, Serialize};
use steno_core::Bank;

/// Which palette to draw with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Dark => "Dark",
            ThemeMode::Light => "Light",
        }
    }
}

/// Spacing scale, in points. Using a scale instead of ad-hoc numbers keeps
/// rhythm consistent across screens.
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 14.0;
pub const SPACE_LG: f32 = 22.0;

pub const RADIUS: f32 = 6.0;

/// The resolved palette for one mode.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: egui::Color32,
    pub panel: egui::Color32,
    pub surface: egui::Color32,
    pub raised: egui::Color32,
    pub border: egui::Color32,
    pub text: egui::Color32,
    pub muted: egui::Color32,
    pub accent: egui::Color32,
    /// Left-hand consonants.
    pub left: egui::Color32,
    /// Vowels and the star.
    pub mid: egui::Color32,
    /// Right-hand consonants.
    pub right: egui::Color32,
    pub ok: egui::Color32,
    pub err: egui::Color32,
    pub is_dark: bool,
}

const fn rgb(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

impl Theme {
    pub fn new(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Theme {
                bg: rgb(0x14, 0x16, 0x1a),
                panel: rgb(0x1a, 0x1d, 0x23),
                surface: rgb(0x23, 0x27, 0x2f),
                raised: rgb(0x2d, 0x32, 0x3c),
                border: rgb(0x33, 0x39, 0x44),
                text: rgb(0xe6, 0xe9, 0xef),
                muted: rgb(0x93, 0x9d, 0xad),
                accent: rgb(0x6e, 0xa8, 0xfe),
                left: rgb(0x6e, 0xa8, 0xfe),
                mid: rgb(0xf5, 0xb1, 0x3d),
                right: rgb(0x4e, 0xcd, 0xc4),
                ok: rgb(0x5b, 0xe0, 0x8f),
                err: rgb(0xf8, 0x71, 0x71),
                is_dark: true,
            },
            ThemeMode::Light => Theme {
                bg: rgb(0xf6, 0xf7, 0xfa),
                panel: rgb(0xff, 0xff, 0xff),
                surface: rgb(0xec, 0xef, 0xf4),
                raised: rgb(0xde, 0xe3, 0xeb),
                border: rgb(0xd2, 0xd8, 0xe1),
                text: rgb(0x1a, 0x1d, 0x23),
                muted: rgb(0x5c, 0x64, 0x72),
                accent: rgb(0x25, 0x63, 0xeb),
                left: rgb(0x25, 0x63, 0xeb),
                mid: rgb(0xb4, 0x53, 0x09),
                right: rgb(0x0d, 0x94, 0x88),
                ok: rgb(0x16, 0xa3, 0x4a),
                err: rgb(0xdc, 0x26, 0x26),
                is_dark: false,
            },
        }
    }

    /// The colour that identifies a steno key's bank.
    pub fn bank(&self, bank: Bank) -> egui::Color32 {
        match bank {
            Bank::Left => self.left,
            Bank::Mid => self.mid,
            Bank::Right => self.right,
        }
    }

    /// Install this palette as the app-wide egui style.
    pub fn apply(&self, ctx: &egui::Context) {
        let mut visuals = if self.is_dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        visuals.panel_fill = self.panel;
        visuals.window_fill = self.panel;
        visuals.extreme_bg_color = self.bg;
        visuals.faint_bg_color = self.surface;
        visuals.override_text_color = Some(self.text);
        visuals.hyperlink_color = self.accent;
        visuals.selection.bg_fill = self.accent.linear_multiply(0.35);
        visuals.selection.stroke = egui::Stroke::new(1.0, self.accent);
        visuals.window_stroke = egui::Stroke::new(1.0, self.border);
        // Flat surfaces: shadows read as noise at this density of controls.
        visuals.window_shadow = egui::epaint::Shadow::NONE;
        visuals.popup_shadow = egui::epaint::Shadow::NONE;

        let rounding = egui::Rounding::same(RADIUS);
        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.rounding = rounding;
            widget.fg_stroke.color = self.text;
        }
        visuals.widgets.noninteractive.bg_fill = self.surface;
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, self.border);
        visuals.widgets.noninteractive.fg_stroke.color = self.muted;
        visuals.widgets.inactive.bg_fill = self.surface;
        visuals.widgets.inactive.weak_bg_fill = self.surface;
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, self.border);
        visuals.widgets.hovered.bg_fill = self.raised;
        visuals.widgets.hovered.weak_bg_fill = self.raised;
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, self.accent);
        visuals.widgets.active.bg_fill = self.raised;
        visuals.widgets.active.weak_bg_fill = self.raised;
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, self.accent);

        ctx.set_visuals(visuals);

        ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(SPACE_SM, SPACE_SM);
            style.spacing.button_padding = egui::vec2(10.0, 6.0);
            style.spacing.menu_margin = egui::Margin::same(SPACE_SM);
            style.spacing.interact_size.y = 26.0;
        });
    }
}

/// Linear interpolation between two colours; `t` of 0 keeps `a`.
pub fn blend(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    egui::Color32::from_rgb(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
    )
}

/// Text styles used across screens, so headings stay consistent.
pub mod font {
    use eframe::egui::{FontFamily, FontId};

    pub fn display() -> FontId {
        FontId::new(34.0, FontFamily::Proportional)
    }
    pub fn heading() -> FontId {
        FontId::new(19.0, FontFamily::Proportional)
    }
    pub fn body() -> FontId {
        FontId::new(14.0, FontFamily::Proportional)
    }
    pub fn small() -> FontId {
        FontId::new(12.0, FontFamily::Proportional)
    }
    pub fn mono_lg() -> FontId {
        FontId::new(20.0, FontFamily::Monospace)
    }
    pub fn mono() -> FontId {
        FontId::new(14.0, FontFamily::Monospace)
    }
}
