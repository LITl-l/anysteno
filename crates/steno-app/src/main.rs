//! anysteno — cross-platform stenography for any keyboard, any language.

mod app;
mod fonts;
mod platform;
mod state;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([560.0, 480.0])
            .with_min_inner_size([420.0, 360.0])
            .with_title("anysteno"),
        ..Default::default()
    };

    eframe::run_native(
        "anysteno",
        options,
        Box::new(|_cc| Ok(Box::new(app::build()))),
    )
}
