//! The egui application: input handling, output routing, and the UI.
//!
//! Two input paths feed the same `steno_core::Engine`:
//!   * **In-app mode** reads egui's own key events (window focused).
//!   * **System-wide mode** reads the global capture channel and injects text.

use crossbeam_channel::Receiver;
use eframe::egui;
use steno_core::{Engine, KeyEvent, Translation};

use crate::platform::capture;
use crate::platform::inject::Injector;
use crate::platform::keymap::egui_key_name;
use crate::state::{load_packs, Mode, Paths, Settings};

pub struct StenoApp {
    paths: Paths,
    settings: Settings,
    engine: Engine,
    packs: Vec<steno_core::Pack>,

    /// System-wide capture channel (spawned lazily).
    capture_rx: Option<Receiver<KeyEvent>>,
    injector: Option<Injector>,

    /// Inserted text chunks, for the in-app display and for undo.
    history: Vec<String>,
    /// Live view.
    last_stroke: String,
    status: String,
    font_status: String,
    fonts_ready: bool,
}

impl StenoApp {
    pub fn new(paths: Paths, settings: Settings, packs: Vec<steno_core::Pack>) -> Self {
        let active = packs
            .iter()
            .find(|p| p.code == settings.pack_code)
            .or_else(|| packs.first())
            .cloned()
            .expect("at least one pack is always present");
        let mut engine = Engine::new(active);
        engine.set_enabled(settings.enabled);

        Self {
            paths,
            settings,
            engine,
            packs,
            capture_rx: None,
            injector: None,
            history: Vec::new(),
            last_stroke: String::new(),
            status: String::new(),
            font_status: String::new(),
            fonts_ready: false,
        }
    }

    fn save(&self) {
        self.paths.save_settings(&self.settings);
    }

    // --- output routing -------------------------------------------------

    /// Spacing rule: a leading space before multi-character words, none before
    /// single characters (fingerspelling / kana, which never take spaces).
    fn spaced(&self, text: &str) -> String {
        let single = text.chars().count() <= 1;
        if single || self.history.is_empty() {
            text.to_string()
        } else {
            format!(" {text}")
        }
    }

    fn emit(&mut self, text: &str) {
        let chunk = self.spaced(text);
        if self.settings.mode == Mode::SystemWide {
            if let Some(inj) = self.injector.as_mut() {
                if let Err(e) = inj.text(&chunk) {
                    self.status = e.to_string();
                }
            }
        }
        self.history.push(chunk);
    }

    fn undo(&mut self) {
        if let Some(chunk) = self.history.pop() {
            if self.settings.mode == Mode::SystemWide {
                if let Some(inj) = self.injector.as_mut() {
                    let _ = inj.backspace(chunk.chars().count());
                }
            }
        }
    }

    fn apply_output(&mut self, out: steno_core::EngineOutput) {
        if let Some(s) = out.stroke {
            self.last_stroke = s;
        }
        if out.undo {
            self.undo();
            self.status = "undo".to_string();
        }
        for t in out.translations {
            match t {
                Translation::Text { text, .. } => self.emit(&text),
                Translation::NoMatch { stroke } => {
                    self.status = format!("no match: {stroke}");
                }
            }
        }
    }

    fn feed(&mut self, ev: KeyEvent) {
        let out = self.engine.on_key(&ev);
        self.apply_output(out);
    }

    fn flush_pending(&mut self) {
        let translations = self.engine.flush();
        for t in translations {
            match t {
                Translation::Text { text, .. } => self.emit(&text),
                Translation::NoMatch { stroke } => self.status = format!("no match: {stroke}"),
            }
        }
    }

    // --- input paths ----------------------------------------------------

    fn pump_in_app(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            if let egui::Event::Key {
                key,
                physical_key,
                pressed,
                repeat,
                ..
            } = ev
            {
                if key == egui::Key::Space && pressed {
                    self.flush_pending();
                    continue;
                }
                if repeat {
                    continue;
                }
                let physical = physical_key.unwrap_or(key);
                if let Some(name) = egui_key_name(physical) {
                    self.feed(if pressed {
                        KeyEvent::down(name)
                    } else {
                        KeyEvent::up(name)
                    });
                }
            }
        }
    }

    fn pump_system_wide(&mut self, ctx: &egui::Context) {
        if self.capture_rx.is_none() {
            self.capture_rx = Some(capture::spawn());
            match Injector::new() {
                Ok(inj) => self.injector = Some(inj),
                Err(e) => self.status = e.to_string(),
            }
        }
        // Drain everything currently queued.
        let mut batch = Vec::new();
        if let Some(rx) = &self.capture_rx {
            while let Ok(ev) = rx.try_recv() {
                batch.push(ev);
            }
        }
        for ev in batch {
            self.feed(ev);
        }
        // Keep polling even without GUI interaction.
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }

    // --- UI -------------------------------------------------------------

    fn ui_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                // Language selector.
                let current = self.engine.pack().name.clone();
                egui::ComboBox::from_id_salt("lang")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        let mut chosen: Option<usize> = None;
                        for (i, p) in self.packs.iter().enumerate() {
                            let selected = p.code == self.settings.pack_code;
                            if ui.selectable_label(selected, &p.name).clicked() {
                                chosen = Some(i);
                            }
                        }
                        if let Some(i) = chosen {
                            let pack = self.packs[i].clone();
                            self.settings.pack_code = pack.code.clone();
                            self.engine.set_pack(pack);
                            self.save();
                        }
                    });

                ui.separator();

                // Master on/off.
                let mut enabled = self.settings.enabled;
                let label = if enabled { "● ON" } else { "○ OFF" };
                if ui.toggle_value(&mut enabled, label).changed() {
                    self.settings.enabled = enabled;
                    self.engine.set_enabled(enabled);
                    self.save();
                }

                ui.separator();

                // Output mode.
                let mut mode = self.settings.mode;
                let changed = ui.radio_value(&mut mode, Mode::InApp, "In-app").changed()
                    | ui.radio_value(&mut mode, Mode::SystemWide, "System-wide").changed();
                if changed {
                    self.settings.mode = mode;
                    self.save();
                }
            });
        });
    }

    fn ui_live_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("Live");
        egui::Grid::new("live").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
            ui.label("Holding:");
            let held = self.engine.held_steno_ids().join(" ");
            ui.monospace(if held.is_empty() { "—".into() } else { held });
            ui.end_row();

            ui.label("Last stroke:");
            ui.monospace(if self.last_stroke.is_empty() { "—".into() } else { self.last_stroke.clone() });
            ui.end_row();

            ui.label("Pending:");
            let pending = self.engine.pending().join(" / ");
            ui.monospace(if pending.is_empty() { "—".into() } else { pending });
            ui.end_row();
        });
    }

    fn ui_practice(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Output");
            if ui.button("Clear").clicked() {
                self.history.clear();
            }
            if ui.button("Undo").clicked() {
                self.undo();
            }
        });
        let text: String = self.history.concat();
        egui::ScrollArea::vertical()
            .min_scrolled_height(160.0)
            .show(ui, |ui| {
                let mut display = text;
                ui.add_sized(
                    [ui.available_width(), 160.0],
                    egui::TextEdit::multiline(&mut display)
                        .interactive(false)
                        .hint_text("Chord here — translations appear in this box."),
                );
            });
    }
}

impl eframe::App for StenoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.fonts_ready {
            self.font_status = crate::fonts::install(ctx);
            self.fonts_ready = true;
        }

        // Route input.
        match self.settings.mode {
            Mode::InApp => self.pump_in_app(ctx),
            Mode::SystemWide => self.pump_system_wide(ctx),
        }

        self.ui_top_bar(ctx);

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(if self.status.is_empty() { "ready".to_string() } else { self.status.clone() });
                ui.separator();
                ui.weak(&self.font_status);
            });
            if self.settings.mode == Mode::SystemWide {
                ui.weak("System-wide: raw keys are not suppressed on this version — see README.");
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.ui_live_view(ui);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            self.ui_practice(ui);
            ui.add_space(8.0);
            ui.weak("Tip: press keys together, release to make a stroke. Space flushes a pending word. The star key undoes.");
        });
    }
}

/// Build the app from config on disk.
pub fn build() -> StenoApp {
    let paths = Paths::resolve();
    paths.seed_default_packs();
    let settings = paths.load_settings();
    let (packs, _warnings) = load_packs(&paths.packs_dir);
    StenoApp::new(paths, settings, packs)
}
