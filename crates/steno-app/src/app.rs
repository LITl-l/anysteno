//! The egui application: input routing, output routing, and screen dispatch.
//!
//! Two input paths feed the same `steno_core::Engine`:
//!   * **In-app** reads egui's own key events (window focused).
//!   * **System-wide** reads the global capture channel and injects text.
//!
//! Everything the engine produces is normalised into [`StenoEvent`]s, which the
//! active screen consumes. Screens therefore never touch the engine, the
//! capture thread or the injector — they see a list of things that happened.

use crossbeam_channel::Receiver;
use eframe::egui;
use steno_core::{Curriculum, Engine, KeyEvent, Pack, ReverseIndex, Translation};

use crate::platform::capture;
use crate::platform::inject::Injector;
use crate::platform::keymap::egui_key_name;
use crate::state::{load_packs, Mode, Paths, Screen, Settings};
use crate::ui::learn::LearnState;
use crate::ui::lookup::LookupState;
use crate::ui::practice::PracticeState;
use crate::ui::theme::{font, Theme, ThemeMode, SPACE_MD, SPACE_SM};
use crate::ui::{learn, lookup, onboarding, practice, settings as settings_ui};

/// Something the engine produced, in the order it happened.
#[derive(Debug, Clone)]
pub enum StenoEvent {
    /// A chord was completed and rendered to this stroke.
    Stroke(String),
    /// A stroke (or run of strokes) translated to this text.
    Text(String),
    /// A stroke with no dictionary entry.
    NoMatch(String),
    /// The pack's undo chord.
    Undo,
}

/// Everything read from disk before the window opens, so the saved window size
/// can be applied to the viewport.
pub struct Boot {
    pub paths: Paths,
    pub settings: Settings,
    pub packs: Vec<Pack>,
    pub warnings: Vec<String>,
}

/// Load config and packs.
pub fn boot() -> Boot {
    let paths = Paths::resolve();
    paths.seed_default_packs();
    let settings = paths.load_settings();
    let (packs, warnings) = load_packs(&paths.packs_dir);
    Boot {
        paths,
        settings,
        packs,
        warnings,
    }
}

pub struct StenoApp {
    paths: Paths,
    settings: Settings,
    engine: Engine,
    packs: Vec<Pack>,
    /// Pack-loading problems, surfaced in Settings rather than discarded.
    warnings: Vec<String>,

    /// Derived from the active pack; rebuilt when the pack changes.
    curriculum: Curriculum,
    index: ReverseIndex,

    capture_rx: Option<Receiver<KeyEvent>>,
    injector: Option<Injector>,

    /// Text written on the Practice screen, kept in chunks so undo removes
    /// exactly what was written.
    output: Vec<String>,

    learn: LearnState,
    practice: PracticeState,
    lookup: LookupState,

    theme: Theme,
    applied_theme: Option<ThemeMode>,
    fonts_ready: bool,
    font_status: String,
    status: String,
    /// Latest window size, written back to settings when eframe saves.
    window_size: Option<[f32; 2]>,
    /// Something changed that must reach disk.
    dirty: bool,
}

impl StenoApp {
    pub fn new(boot: Boot) -> Self {
        let Boot {
            paths,
            settings,
            packs,
            warnings,
        } = boot;

        let active = packs
            .iter()
            .find(|p| p.code == settings.pack_code)
            .or_else(|| packs.first())
            .cloned()
            .expect("at least one pack is always present");
        let curriculum = Curriculum::derive(&active);
        let index = ReverseIndex::build(&active.dict);
        let mut engine = Engine::new(active);
        engine.set_enabled(settings.enabled);
        let theme = Theme::new(settings.theme);

        Self {
            paths,
            settings,
            engine,
            packs,
            warnings,
            curriculum,
            index,
            capture_rx: None,
            injector: None,
            output: Vec::new(),
            learn: LearnState::default(),
            practice: PracticeState::default(),
            lookup: LookupState::default(),
            theme,
            applied_theme: None,
            fonts_ready: false,
            font_status: String::new(),
            status: String::new(),
            window_size: None,
            dirty: false,
        }
    }

    fn persist(&mut self) {
        if let Some(size) = self.window_size {
            self.settings.window = Some(size);
        }
        self.paths.save_settings(&self.settings);
        self.dirty = false;
    }

    fn set_pack(&mut self, index: usize) {
        let Some(pack) = self.packs.get(index).cloned() else {
            return;
        };
        self.settings.pack_code = pack.code.clone();
        self.curriculum = Curriculum::derive(&pack);
        self.index = ReverseIndex::build(&pack.dict);
        self.engine.set_pack(pack);
        // A drill from the previous language would be nonsense here.
        self.learn = LearnState::default();
        self.practice.clear();
        self.output.clear();
        self.dirty = true;
    }

    fn reload_packs(&mut self) {
        let (packs, warnings) = load_packs(&self.paths.packs_dir);
        self.packs = packs;
        self.warnings = warnings;
        let index = self
            .packs
            .iter()
            .position(|p| p.code == self.settings.pack_code)
            .unwrap_or(0);
        self.set_pack(index);
        self.status = format!("reloaded {} pack(s)", self.packs.len());
    }

    // --- output routing -------------------------------------------------

    /// Spacing rule: no space before single characters (fingerspelling and kana
    /// never take one). Otherwise a leading space, except for the very first
    /// word written into anysteno's own buffer.
    ///
    /// System-wide output always leads with a space, following the same
    /// space-before convention as other steno software. The cursor could be
    /// anywhere in someone else's document, so "is this the first word?" is a
    /// question this app cannot answer — and the old code answered it with
    /// anysteno's own buffer, which runs the first injected word into whatever
    /// text was already there.
    fn spaced(&self, text: &str) -> String {
        let single = text.chars().count() <= 1;
        let at_start = self.settings.mode == Mode::InApp && self.output.is_empty();
        if single || at_start {
            text.to_string()
        } else {
            format!(" {text}")
        }
    }

    fn emit(&mut self, text: &str) {
        let chunk = self.spaced(text);
        if self.settings.mode == Mode::SystemWide {
            if let Some(injector) = self.injector.as_mut() {
                if let Err(e) = injector.text(&chunk) {
                    self.status = e.to_string();
                }
            }
        }
        self.output.push(chunk);
    }

    fn undo(&mut self) {
        let Some(chunk) = self.output.pop() else {
            return;
        };
        if self.settings.mode == Mode::SystemWide {
            if let Some(injector) = self.injector.as_mut() {
                let _ = injector.backspace(chunk.chars().count());
            }
        }
    }

    // --- input ----------------------------------------------------------

    /// Collect this frame's engine output.
    fn pump(
        &mut self,
        ctx: &egui::Context,
        wants_keys: bool,
        system_wide: bool,
    ) -> Vec<StenoEvent> {
        let mut events = Vec::new();

        if system_wide {
            if self.capture_rx.is_none() {
                self.capture_rx = Some(capture::spawn());
                match Injector::new() {
                    Ok(injector) => self.injector = Some(injector),
                    Err(e) => self.status = e.to_string(),
                }
            }
            let mut batch = Vec::new();
            if let Some(rx) = &self.capture_rx {
                while let Ok(ev) = rx.try_recv() {
                    batch.push(ev);
                }
            }
            for ev in batch {
                let out = self.engine.on_key(&ev);
                collect(out, &mut events);
            }
            // Keep polling even without GUI interaction.
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
            return events;
        }

        // Not capturing globally: drain anything the thread queued so the
        // channel can't grow without bound, and drop it.
        if let Some(rx) = &self.capture_rx {
            while rx.try_recv().is_ok() {}
        }

        if !wants_keys {
            return events;
        }

        for ev in ctx.input(|i| i.events.clone()) {
            let egui::Event::Key {
                key,
                physical_key,
                pressed,
                repeat,
                ..
            } = ev
            else {
                continue;
            };
            if key == egui::Key::Space && pressed {
                for t in self.engine.flush() {
                    push_translation(t, &mut events);
                }
                continue;
            }
            if repeat {
                continue;
            }
            let physical = physical_key.unwrap_or(key);
            let Some(name) = egui_key_name(physical) else {
                continue;
            };
            let out = self.engine.on_key(&if pressed {
                KeyEvent::down(name)
            } else {
                KeyEvent::up(name)
            });
            collect(out, &mut events);
        }
        events
    }

    // --- chrome ---------------------------------------------------------

    fn nav(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("nav")
            .exact_width(150.0)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(self.theme.bg)
                    .inner_margin(egui::Margin::same(SPACE_SM)),
            )
            .show(ctx, |ui| {
                ui.add_space(SPACE_SM);
                ui.label(
                    egui::RichText::new("anysteno")
                        .font(font::heading())
                        .color(self.theme.text),
                );
                ui.label(
                    egui::RichText::new(&self.engine.pack().name)
                        .font(font::small())
                        .color(self.theme.muted),
                );
                ui.add_space(SPACE_MD);

                for screen in Screen::ALL {
                    let selected = self.settings.screen == screen;
                    let response = ui.add_sized(
                        [ui.available_width(), 30.0],
                        egui::SelectableLabel::new(selected, screen.label()),
                    );
                    if response.clicked() && !selected {
                        self.settings.screen = screen;
                        self.dirty = true;
                    }
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(SPACE_SM);
                    if !self.font_status.is_empty() {
                        ui.label(
                            egui::RichText::new(&self.font_status)
                                .font(font::small())
                                .color(self.theme.muted),
                        );
                    }
                    if !self.warnings.is_empty() {
                        ui.label(
                            egui::RichText::new(format!("{} pack issue(s)", self.warnings.len()))
                                .font(font::small())
                                .color(self.theme.err),
                        );
                    }
                    if !self.settings.enabled {
                        ui.label(
                            egui::RichText::new("translation off")
                                .font(font::small())
                                .color(self.theme.mid),
                        );
                    }
                });
            });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        let text = if self.status.is_empty() {
            "ready".to_string()
        } else {
            self.status.clone()
        };
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::none()
                    .fill(self.theme.bg)
                    .inner_margin(egui::Margin::symmetric(SPACE_MD, 6.0)),
            )
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(text)
                        .font(font::small())
                        .color(self.theme.muted),
                );
            });
    }
}

/// Turn one engine output into events.
fn collect(out: steno_core::EngineOutput, events: &mut Vec<StenoEvent>) {
    if let Some(stroke) = out.stroke {
        events.push(StenoEvent::Stroke(stroke));
    }
    if out.undo {
        events.push(StenoEvent::Undo);
    }
    for t in out.translations {
        push_translation(t, events);
    }
}

fn push_translation(t: Translation, events: &mut Vec<StenoEvent>) {
    match t {
        Translation::Text { text, .. } => events.push(StenoEvent::Text(text)),
        Translation::NoMatch { stroke } => events.push(StenoEvent::NoMatch(stroke)),
    }
}

impl eframe::App for StenoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.fonts_ready {
            self.font_status = crate::fonts::install(ctx);
            self.fonts_ready = true;
        }
        if self.applied_theme != Some(self.settings.theme) {
            self.theme = Theme::new(self.settings.theme);
            self.theme.apply(ctx);
            self.applied_theme = Some(self.settings.theme);
        }
        self.window_size = ctx.input(|i| i.viewport().inner_rect.map(|r| [r.width(), r.height()]));

        // A focused text field owns the keyboard; chording into a search box
        // would make it unusable.
        let typing_in_widget = ctx.memory(|m| m.focused().is_some());
        let onboarding = !self.settings.onboarded;
        let wants_keys = !typing_in_widget && (onboarding || self.settings.screen.captures_keys());
        let system_wide = !onboarding
            && self.settings.screen == Screen::Practice
            && self.settings.mode == Mode::SystemWide;

        let events = self.pump(ctx, wants_keys, system_wide);

        // Escape leaves whatever is in progress. It is never a steno key, so it
        // is safe to bind while chording.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.learn.is_drilling() {
            self.learn = LearnState::default();
        }

        // Feed the active screen.
        if !onboarding {
            match self.settings.screen {
                Screen::Learn => {
                    let pack_code = self.engine.pack().code.clone();
                    if learn::handle_events(
                        &mut self.learn,
                        &events,
                        &pack_code,
                        &mut self.settings,
                    ) {
                        self.dirty = true;
                    }
                }
                Screen::Practice => {
                    for event in &events {
                        match event {
                            StenoEvent::Text(text) => {
                                let text = text.clone();
                                self.emit(&text);
                            }
                            StenoEvent::Undo => self.undo(),
                            _ => {}
                        }
                    }
                    practice::handle_events(&mut self.practice, &events);
                }
                Screen::Dictionary | Screen::Settings => {}
            }
        }

        self.nav(ctx);
        self.status_bar(ctx);

        let held = self.engine.held_steno_ids();
        let pack = self.engine.pack().clone();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(self.theme.panel)
                    .inner_margin(egui::Margin::same(SPACE_MD)),
            )
            .show(ctx, |ui| {
                if onboarding {
                    if onboarding::show(ui, &self.theme, &pack, &self.curriculum, &held) {
                        self.settings.onboarded = true;
                        self.settings.screen = Screen::Learn;
                        self.dirty = true;
                    }
                    return;
                }

                match self.settings.screen {
                    Screen::Learn => {
                        if learn::show(
                            ui,
                            &mut self.learn,
                            &self.theme,
                            &pack,
                            &self.curriculum,
                            &held,
                            &mut self.settings,
                        ) {
                            self.dirty = true;
                        }
                    }
                    Screen::Practice => {
                        let pending: Vec<String> = self.engine.pending().to_vec();
                        let output = self.output.concat();
                        let action = practice::show(
                            ui,
                            &mut self.practice,
                            &self.theme,
                            &pack,
                            &held,
                            &pending,
                            &output,
                            &mut self.settings,
                        );
                        if action.clear {
                            self.output.clear();
                            self.practice.clear();
                        }
                        if action.undo {
                            self.undo();
                        }
                        if action.settings_changed {
                            self.dirty = true;
                        }
                    }
                    Screen::Dictionary => {
                        lookup::show(ui, &mut self.lookup, &self.theme, &pack, &self.index);
                    }
                    Screen::Settings => {
                        let action = settings_ui::show(
                            ui,
                            &self.theme,
                            &self.packs,
                            &pack,
                            &self.warnings,
                            &self.paths,
                            &mut self.settings,
                        );
                        if let Some(i) = action.select_pack {
                            self.set_pack(i);
                        }
                        if action.reload_packs {
                            self.reload_packs();
                        }
                        if action.reset_progress {
                            self.settings.progress.remove(&pack.code);
                            self.learn = LearnState::default();
                            self.dirty = true;
                        }
                        if action.settings_changed {
                            self.engine.set_enabled(self.settings.enabled);
                            self.dirty = true;
                        }
                    }
                }
            });

        // While keys are held the highlight must track them without waiting for
        // a mouse event.
        if !held.is_empty() || !events.is_empty() {
            ctx.request_repaint();
        }
        if self.dirty {
            self.persist();
        }
    }

    /// eframe calls this periodically and on exit — the natural place to record
    /// the window size, rather than writing settings on every resize frame.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.persist();
    }
}
