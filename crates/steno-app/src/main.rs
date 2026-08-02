//! anysteno — cross-platform stenography for any keyboard, any language.

mod app;
mod fonts;
mod platform;
mod state;
mod ui;

/// Size used the first time the app runs, before there is a saved one.
const DEFAULT_SIZE: [f32; 2] = [980.0, 760.0];

fn main() -> eframe::Result<()> {
    // Config is read before the window opens so the saved size can be applied
    // to the viewport rather than resizing visibly after the first frame.
    let boot = app::boot();
    let size = boot.settings.window.unwrap_or(DEFAULT_SIZE);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size([720.0, 560.0])
            .with_title("anysteno"),
        ..Default::default()
    };

    eframe::run_native(
        "anysteno",
        options,
        Box::new(|_cc| Ok(Box::new(app::StenoApp::new(boot)))),
    )
}
