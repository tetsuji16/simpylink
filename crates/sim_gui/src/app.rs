//! Main window (egui/eframe 0.36): menu bar, toolbar, library browser, canvas,
//! diagnostics panel, status bar, dialogs and Scope windows.
use crate::canvas::controller::{CanvasEvent, CanvasKey};
use crate::canvas::view::{Modifiers, ScreenPos};
use crate::canvas::widget::CanvasWidget;
use crate::dialogs::model_settings::SettingsDialogState;
use crate::panels;
use crate::scope::{scope_series, show_scope_window};
use crate::state::{AppState, PendingAction, SaveResult};
use egui::{Key, Modifiers as EM};
use sim_core::edit::Command;
use sim_core::geom::Point;
use std::path::PathBuf;

/// The eframe application.
pub struct SimpylinkApp {
    pub state: AppState,
    pub initial_file: Option<PathBuf>,
    bottom_tab: usize,
    stop_time_text: String,
    stop_time_rev: u64,
    hover: Option<ScreenPos>,
    canvas_origin: egui::Pos2,
    last_title: String,
}

impl SimpylinkApp {
    /// New app; `initial_file` is opened on the first frame.
    pub fn new(initial_file: Option<PathBuf>) -> Self {
        SimpylinkApp {
            state: AppState::new(),
            initial_file,
            bottom_tab: 0,
            stop_time_text: String::new(),
            stop_time_rev: u64::MAX,
            hover: None,
            canvas_origin: egui::Pos2::ZERO,
            last_title: String::new(),
        }
    }

    fn key_to_canvas(&mut self, key: CanvasKey, ctrl: bool) {
        let mods = Modifiers {
            ctrl,
            ..Default::default()
        };
        let a = self.state.canvas.handle(
            CanvasEvent::Key {
                key,
                pressed: true,
                mods,
            },
            &self.state.doc.model,
            &self.state.doc.path,
        );
        self.state.handle_actions(a);
    }

    fn view_center(&self) -> Point {
        let v = self.state.canvas.viewport;
        self.state
            .canvas
            .view
            .to_model(ScreenPos::new(v.x / 2.0, v.y / 2.0))
    }

    fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Simpylink model", &["json"])
            .add_filter("Simulink model (experimental)", &["slx"])
            .pick_file()
        {
            self.state.request(PendingAction::Open(p));
        }
    }

    fn save_as_dialog(&mut self) -> bool {
        let name = format!("{}.json", self.state.doc.model.name);
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Simpylink model", &["json"])
            .set_file_name(name)
            .save_file()
        {
            return match self.state.save_as(&p) {
                Ok(()) => true,
                Err(e) => {
                    self.state.status = e;
                    false
                }
            };
        }
        false
    }

    fn save(&mut self) -> bool {
        match self.state.save() {
            Ok(SaveResult::Saved) => true,
            Ok(SaveResult::NeedsPath) => self.save_as_dialog(),
            Err(e) => {
                self.state.status = e;
                false
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = EM::COMMAND;
        let cs = EM::COMMAND | EM::SHIFT;
        let (n, o, s, t, f5, stop, d, e, l) = ctx.input_mut(|i| {
            (
                i.consume_key(cmd, Key::N),
                i.consume_key(cmd, Key::O),
                i.consume_key(cmd, Key::S),
                i.consume_key(cmd, Key::T),
                i.consume_key(EM::NONE, Key::F5),
                i.consume_key(cs, Key::T),
                i.consume_key(cmd, Key::D),
                i.consume_key(cmd, Key::E),
                i.consume_key(cs, Key::L),
            )
        });
        if n {
            self.state.request(PendingAction::New);
        }
        if o {
            self.open_dialog();
        }
        if s {
            self.save();
        }
        if stop {
            self.state.stop_run();
        } else if t || f5 {
            self.state.start_run();
        }
        if d {
            self.state.update_diagram();
        }
        if e {
            self.state.settings_dialog =
                Some(SettingsDialogState::open(&self.state.doc.model.config));
        }
        if l {
            self.state.show_library = !self.state.show_library;
        }
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New            Ctrl+N").clicked() {
                    self.state.request(PendingAction::New);
                    ui.close();
                }
                if ui.button("Open…          Ctrl+O").clicked() {
                    ui.close();
                    self.open_dialog();
                }
                if ui.button("Save           Ctrl+S").clicked() {
                    ui.close();
                    self.save();
                }
                if ui.button("Save As…").clicked() {
                    ui.close();
                    self.save_as_dialog();
                }
                ui.separator();
                if ui.button("Export Python…").clicked() {
                    ui.close();
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Python", &["py"])
                        .save_file()
                    {
                        if let Err(e) = self.state.export_python(&p) {
                            self.state.status = e;
                        }
                    }
                }
                if ui.button("Export as SLX… (experimental)").clicked() {
                    ui.close();
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Simulink model", &["slx"])
                        .save_file()
                    {
                        if let Err(e) = self.state.export_slx(&p) {
                            self.state.status = e;
                        }
                    }
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.close();
                    if self.state.request(PendingAction::Quit) {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            });
            ui.menu_button("Edit", |ui| {
                let ul = self
                    .state
                    .doc
                    .history
                    .undo_label()
                    .map(|l| format!("Undo {l}"))
                    .unwrap_or_else(|| "Undo".into());
                if ui
                    .add_enabled(
                        self.state.doc.history.can_undo(),
                        egui::Button::new(format!("{ul}   Ctrl+Z")),
                    )
                    .clicked()
                {
                    self.state.undo();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.state.doc.history.can_redo(),
                        egui::Button::new("Redo   Ctrl+Y"),
                    )
                    .clicked()
                {
                    self.state.redo();
                    ui.close();
                }
                ui.separator();
                for (label, key) in [
                    ("Cut   Ctrl+X", CanvasKey::X),
                    ("Copy   Ctrl+C", CanvasKey::C),
                    ("Paste   Ctrl+V", CanvasKey::V),
                    ("Select All   Ctrl+A", CanvasKey::A),
                ] {
                    if ui.button(label).clicked() {
                        self.key_to_canvas(key, true);
                        ui.close();
                    }
                }
                if ui.button("Delete   Del").clicked() {
                    self.key_to_canvas(CanvasKey::Delete, false);
                    ui.close();
                }
                ui.separator();
                if ui.button("Create Subsystem   Ctrl+G").clicked() {
                    self.state.create_subsystem();
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(
                    &mut self.state.show_library,
                    "Library Browser   Ctrl+Shift+L",
                );
                ui.checkbox(&mut self.state.show_bottom, "Diagnostics / Output");
                if ui.button("Fit to View   Space").clicked() {
                    let sys = self.state.current_system().clone();
                    self.state.canvas.fit_to_view(&sys);
                    ui.close();
                }
                for (label, key) in [
                    ("Zoom In   Ctrl++", CanvasKey::Plus),
                    ("Zoom Out   Ctrl+-", CanvasKey::Minus),
                    ("Zoom 100%   Ctrl+0", CanvasKey::Zero),
                ] {
                    if ui.button(label).clicked() {
                        self.key_to_canvas(key, true);
                        ui.close();
                    }
                }
                if ui.button("Generated Python Code").clicked() {
                    self.state.refresh_code_view();
                    ui.close();
                }
            });
            ui.menu_button("Simulation", |ui| {
                let running = self.state.sim.is_running();
                if ui
                    .add_enabled(!running, egui::Button::new("Run   Ctrl+T / F5"))
                    .clicked()
                {
                    self.state.start_run();
                    ui.close();
                }
                if ui
                    .add_enabled(running, egui::Button::new("Stop   Ctrl+Shift+T"))
                    .clicked()
                {
                    self.state.stop_run();
                    ui.close();
                }
                if ui.button("Update Diagram   Ctrl+D").clicked() {
                    self.state.update_diagram();
                    ui.close();
                }
                if ui.button("Model Settings…   Ctrl+E").clicked() {
                    self.state.settings_dialog =
                        Some(SettingsDialogState::open(&self.state.doc.model.config));
                    ui.close();
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        if self.stop_time_rev != self.state.revision {
            self.stop_time_text = format!("{}", self.state.doc.model.config.stop_time);
            self.stop_time_rev = self.state.revision;
        }
        ui.horizontal(|ui| {
            let running = self.state.sim.is_running();
            if ui
                .add_enabled(!running, egui::Button::new("▶ Run"))
                .on_hover_text("Ctrl+T / F5")
                .clicked()
            {
                self.state.start_run();
            }
            if ui
                .add_enabled(running, egui::Button::new("■ Stop"))
                .on_hover_text("Ctrl+Shift+T")
                .clicked()
            {
                self.state.stop_run();
            }
            ui.separator();
            ui.label("Stop time");
            let r =
                ui.add(egui::TextEdit::singleline(&mut self.stop_time_text).desired_width(70.0));
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                match self.stop_time_text.trim().parse::<f64>() {
                    Ok(v) if v.is_finite() => {
                        let mut cfg = self.state.doc.model.config.clone();
                        cfg.stop_time = v;
                        if cfg != self.state.doc.model.config {
                            self.state.execute(Command::SetConfig { config: cfg });
                        }
                    }
                    _ => self.state.status = format!("Invalid stop time '{}'", self.stop_time_text),
                }
            }
            ui.separator();
            if !self.state.doc.path.is_root() {
                if ui.button("⬆ Parent").on_hover_text("Esc").clicked() {
                    self.state.go_to_parent();
                }
                ui.label(format!(
                    "{} / {}",
                    self.state.doc.model.name,
                    self.state.breadcrumbs().join(" / ")
                ));
            }
        });
    }
}

/// Opens the main window and blocks until it is closed.
pub fn run_gui(initial_file: Option<PathBuf>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Simpylink")
            .with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Simpylink",
        options,
        Box::new(move |_cc| Ok(Box::new(SimpylinkApp::new(initial_file)))),
    )
}

impl eframe::App for SimpylinkApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(p) = self.initial_file.take() {
            if let Err(e) = self.state.open_path(&p) {
                self.state.status = e;
            }
        }
        // Close request with unsaved changes.
        if ctx.input(|i| i.viewport().close_requested())
            && !self.state.quit_confirmed
            && self.state.doc.history.is_dirty()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.state.confirm = Some(PendingAction::Quit);
        }
        if self.state.quit_confirmed {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.shortcuts(&ctx);
        self.state.poll_sim();
        if self.state.sim.is_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(t) = self.state.pending_system_copy.take() {
            ctx.copy_text(t);
        }
        let title = self.state.window_title();
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }

        egui::Panel::top("menu").show(ui, |ui| {
            self.menu(ui);
            self.toolbar(ui);
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.state.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}%", self.state.canvas.view.zoom * 100.0));
                });
            });
        });
        if self.state.show_bottom {
            egui::Panel::bottom("bottom")
                .resizable(true)
                .default_size(150.0)
                .show(ui, |ui| {
                    let mut tab = self.bottom_tab;
                    if let Some(b) = panels::bottom_panel(ui, &self.state, &mut tab) {
                        self.state.canvas.selection.select_only_block(b);
                    }
                    self.bottom_tab = tab;
                });
        }
        if self.state.show_library {
            egui::Panel::left("library")
                .resizable(true)
                .default_size(200.0)
                .show(ui, |ui| {
                    if let Some(t) = panels::library_panel(ui, &mut self.state.library_filter) {
                        let c = self.view_center();
                        self.state.add_block_at(&t, c);
                    }
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::WHITE))
            .show(ui, |ui| {
                self.canvas_origin = ui.max_rect().min;
                let st = &mut self.state;
                let actions = CanvasWidget {
                    model: &st.doc.model,
                    path: &st.doc.path,
                    controller: &mut st.canvas,
                    hover_pos: &mut self.hover,
                }
                .show(ui);
                st.handle_actions(actions);
            });

        panels::param_dialog(&ctx, &mut self.state);
        panels::settings_dialog(&ctx, &mut self.state);
        panels::quick_insert(&ctx, &mut self.state, self.canvas_origin);
        panels::message_windows(&ctx, &mut self.state);
        if let Some(pending) = panels::confirm_dialog(&ctx, &mut self.state) {
            if self.save() {
                self.state.perform(pending);
            }
        }
        // Scope windows.
        let reset = std::mem::take(&mut self.state.scope_reset);
        let scopes: Vec<_> = self.state.open_scopes.iter().copied().collect();
        for sid in scopes {
            let name = find_name(&self.state.doc.model.root, sid)
                .unwrap_or_else(|| format!("Scope {sid}"));
            let series = self
                .state
                .results
                .as_ref()
                .map(|r| scope_series(r, sid))
                .unwrap_or_default();
            let mut open = true;
            if show_scope_window(
                &ctx,
                sid,
                &format!("Scope: {name}"),
                &series,
                &mut open,
                reset,
            ) {
                self.state.open_parameters(sid);
            }
            if !open {
                self.state.open_scopes.remove(&sid);
            }
        }
    }
}

fn find_name(sys: &sim_core::model::System, sid: sim_core::ids::BlockId) -> Option<String> {
    for b in &sys.blocks {
        if b.sid == sid {
            return Some(b.name.clone());
        }
        if let Some(c) = &b.system {
            if let Some(n) = find_name(c, sid) {
                return Some(n);
            }
        }
    }
    None
}
