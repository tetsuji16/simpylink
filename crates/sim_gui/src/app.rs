//! Main window (egui/eframe 0.36): menu bar, toolbar, library browser, canvas,
//! diagnostics panel, status bar, dialogs and Scope windows.
use crate::canvas::controller::{CanvasEvent, CanvasKey};
use crate::canvas::view::{Modifiers, ScreenPos};
use crate::canvas::widget::CanvasWidget;
use crate::dialogs::model_settings::SettingsDialogState;
use crate::panels;
use crate::scope::{scope_series, show_scope_window, ScopeAction, ScopeInfo, ScopeView};
use crate::state::{AppState, PendingAction, SaveResult};
use crate::theme;
use crate::toolstrip::{StripAction, StripInfo, Toolstrip};
use crate::ui_icons::{paint_ui_icon, UiIcon};
use egui::{Key, Modifiers as EM};
use sim_core::edit::Command;
use sim_core::geom::Point;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The eframe application.
pub struct SimpylinkApp {
    pub state: AppState,
    pub initial_file: Option<PathBuf>,
    bottom_tab: usize,
    toolstrip: Toolstrip,
    library_sel: Option<&'static str>,
    hover: Option<ScreenPos>,
    canvas_origin: egui::Pos2,
    last_title: String,
    scope_views: BTreeMap<sim_core::ids::BlockId, ScopeView>,
}

impl SimpylinkApp {
    /// New app; `initial_file` is opened on the first frame.
    pub fn new(initial_file: Option<PathBuf>) -> Self {
        SimpylinkApp {
            state: AppState::new(),
            initial_file,
            bottom_tab: 0,
            toolstrip: Toolstrip::default(),
            library_sel: None,
            hover: None,
            canvas_origin: egui::Pos2::ZERO,
            last_title: String::new(),
            scope_views: BTreeMap::new(),
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

    fn strip_info(&self) -> StripInfo {
        let sel = &self.state.canvas.selection;
        StripInfo {
            running: self.state.sim.is_running(),
            can_undo: self.state.doc.history.can_undo(),
            can_redo: self.state.doc.history.can_redo(),
            stop_time: self.state.doc.model.config.stop_time,
            revision: self.state.revision,
            show_library: self.state.show_library,
            show_diagnostics: self.state.show_bottom,
            has_results: self.state.results.is_some(),
            has_selection: !sel.blocks.is_empty() || !sel.lines.is_empty(),
        }
    }

    fn perform_strip(&mut self, a: StripAction, ctx: &egui::Context) {
        match a {
            StripAction::New => {
                self.state.request(PendingAction::New);
            }
            StripAction::Open => self.open_dialog(),
            StripAction::Save => {
                self.save();
            }
            StripAction::SaveAs => {
                self.save_as_dialog();
            }
            StripAction::LibraryBrowser => self.state.show_library = !self.state.show_library,
            StripAction::ModelSettings => {
                self.state.settings_dialog =
                    Some(SettingsDialogState::open(&self.state.doc.model.config));
            }
            StripAction::Run => self.state.start_run(),
            StripAction::Stop => self.state.stop_run(),
            StripAction::SetStopTime(text) => {
                let parsed = match text.to_ascii_lowercase().as_str() {
                    "inf" => Ok(f64::INFINITY),
                    t => t.parse::<f64>(),
                };
                match parsed {
                    Ok(v) if !v.is_nan() => {
                        let mut cfg = self.state.doc.model.config.clone();
                        cfg.stop_time = v;
                        if cfg != self.state.doc.model.config {
                            self.state.execute(Command::SetConfig { config: cfg });
                        }
                    }
                    _ => {
                        self.state.status = format!("Invalid stop time '{text}'");
                        // Force the field to show the current value again.
                        self.state.revision = self.state.revision.wrapping_add(1);
                    }
                }
            }
            StripAction::DataInspector => self.state.open_all_scopes(),
            StripAction::UpdateModel => {
                self.state.update_diagram();
                self.state.show_bottom = true;
            }
            StripAction::Diagnostics => self.state.show_bottom = !self.state.show_bottom,
            StripAction::CreateSubsystem => self.state.create_subsystem(),
            StripAction::Undo => self.state.undo(),
            StripAction::Redo => self.state.redo(),
            StripAction::Cut => self.key_to_canvas(CanvasKey::X, true),
            StripAction::Copy => self.key_to_canvas(CanvasKey::C, true),
            StripAction::Paste => self.key_to_canvas(CanvasKey::V, true),
            StripAction::SelectAll => self.key_to_canvas(CanvasKey::A, true),
            StripAction::Delete => self.key_to_canvas(CanvasKey::Delete, false),
            StripAction::ZoomIn => self.key_to_canvas(CanvasKey::Plus, true),
            StripAction::ZoomOut => self.key_to_canvas(CanvasKey::Minus, true),
            StripAction::Zoom100 => self.key_to_canvas(CanvasKey::Zero, true),
            StripAction::FitView => {
                let sys = self.state.current_system().clone();
                self.state.canvas.fit_to_view(&sys);
            }
            StripAction::PythonCode => self.state.refresh_code_view(),
            StripAction::ExportPython => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("Python", &["py"])
                    .set_file_name(format!("{}.py", self.state.doc.model.name))
                    .save_file()
                {
                    if let Err(e) = self.state.export_python(&p) {
                        self.state.status = e;
                    }
                }
            }
            StripAction::ExportSlx => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("Simulink model", &["slx"])
                    .set_file_name(format!("{}.slx", self.state.doc.model.name))
                    .save_file()
                {
                    if let Err(e) = self.state.export_slx(&p) {
                        self.state.status = e;
                    }
                }
            }
            StripAction::ScopeSettings
            | StripAction::ScopeZoomIn
            | StripAction::ScopeZoomX
            | StripAction::ScopeZoomY
            | StripAction::ScopePan
            | StripAction::ScopeAutoscale
            | StripAction::ScopeLegend => {}
            StripAction::Quit => {
                if self.state.request(PendingAction::Quit) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    /// Explorer bar: back/forward/up arrows, the model tab and the address (breadcrumb) bar.
    fn explorer_bar(&mut self, ui: &mut egui::Ui) {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 60.0), egui::Sense::hover());
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, theme::BAR_BG);
        let in_sub = !self.state.doc.path.is_root();
        // Row 1: navigation arrows + model tab.
        let mut x = rect.left() + 6.0;
        for (icon, enabled, tip, id) in [
            (UiIcon::Back, in_sub, "Back", "nav-back"),
            (UiIcon::Forward, false, "Forward", "nav-fwd"),
            (UiIcon::Up, in_sub, "Navigate up to parent (Esc)", "nav-up"),
        ] {
            let r =
                egui::Rect::from_min_size(egui::pos2(x, rect.top() + 4.0), egui::vec2(24.0, 24.0));
            let resp = ui.interact(
                r,
                ui.id().with(id),
                if enabled {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                },
            );
            if enabled && resp.hovered() {
                painter.rect_filled(r, 2.0, theme::BUTTON_HOVER);
            }
            paint_ui_icon(&painter, r.shrink(3.0), icon, enabled);
            if resp.on_hover_text(tip).clicked() && enabled {
                self.state.go_to_parent();
            }
            x += 26.0;
        }
        x += 8.0;
        let dirty = if self.state.doc.history.is_dirty() {
            "*"
        } else {
            ""
        };
        let name = format!("{}{}", self.state.doc.model.name, dirty);
        let font = egui::FontId::proportional(12.5);
        let g = painter.layout_no_wrap(name.clone(), font.clone(), theme::TEXT);
        let tab = egui::Rect::from_min_size(
            egui::pos2(x, rect.top() + 5.0),
            egui::vec2(g.size().x + 36.0, 27.0),
        );
        painter.rect(
            tab,
            egui::CornerRadius {
                nw: 3,
                ne: 3,
                sw: 0,
                se: 0,
            },
            egui::Color32::WHITE,
            egui::Stroke::new(1.0, theme::BORDER),
            egui::StrokeKind::Inside,
        );
        painter.line_segment(
            [
                egui::pos2(tab.left() + 1.0, tab.top() + 1.0),
                egui::pos2(tab.right() - 1.0, tab.top() + 1.0),
            ],
            egui::Stroke::new(2.0, theme::ACCENT),
        );
        paint_ui_icon(
            &painter,
            egui::Rect::from_center_size(
                egui::pos2(tab.left() + 13.0, tab.center().y),
                egui::vec2(16.0, 16.0),
            ),
            UiIcon::Model,
            true,
        );
        painter.text(
            egui::pos2(tab.left() + 25.0, tab.center().y),
            egui::Align2::LEFT_CENTER,
            &name,
            font.clone(),
            theme::TEXT,
        );
        let row2_top = tab.bottom();
        painter.line_segment(
            [
                egui::pos2(rect.left(), row2_top),
                egui::pos2(tab.left(), row2_top),
            ],
            egui::Stroke::new(1.0, theme::BORDER),
        );
        painter.line_segment(
            [
                egui::pos2(tab.right(), row2_top),
                egui::pos2(rect.right(), row2_top),
            ],
            egui::Stroke::new(1.0, theme::BORDER),
        );
        // Row 2: address bar with breadcrumbs.
        let addr = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 6.0, row2_top + 3.0),
            egui::pos2(rect.right() - 6.0, rect.bottom() - 3.0),
        );
        painter.rect(
            addr,
            2.0,
            egui::Color32::WHITE,
            egui::Stroke::new(1.0, theme::BORDER),
            egui::StrokeKind::Inside,
        );
        let mut segs = vec![self.state.doc.model.name.clone()];
        segs.extend(self.state.breadcrumbs());
        let mut bx = addr.left() + 6.0;
        paint_ui_icon(
            &painter,
            egui::Rect::from_center_size(
                egui::pos2(bx + 8.0, addr.center().y),
                egui::vec2(16.0, 16.0),
            ),
            UiIcon::Model,
            true,
        );
        bx += 20.0;
        let depth = segs.len();
        for (i, seg) in segs.iter().enumerate() {
            let g = painter.layout_no_wrap(seg.clone(), font.clone(), theme::TEXT);
            let r = egui::Rect::from_min_size(
                egui::pos2(bx, addr.top() + 2.0),
                egui::vec2(g.size().x + 8.0, addr.height() - 4.0),
            );
            let last = i + 1 == depth;
            let resp = ui.interact(r, ui.id().with(("crumb", i)), egui::Sense::click());
            if !last && resp.hovered() {
                painter.rect_filled(r, 2.0, theme::BUTTON_HOVER);
            }
            painter.text(
                egui::pos2(r.left() + 4.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                seg,
                font.clone(),
                if last { theme::TEXT } else { theme::ACCENT },
            );
            if !last && resp.clicked() {
                for _ in 0..(depth - 1 - i) {
                    self.state.go_to_parent();
                }
            }
            bx = r.right() + 2.0;
            if !last {
                let c = egui::pos2(bx + 4.0, addr.center().y);
                painter.add(egui::Shape::line(
                    vec![
                        c + egui::vec2(-2.0, -4.0),
                        c + egui::vec2(2.0, 0.0),
                        c + egui::vec2(-2.0, 4.0),
                    ],
                    egui::Stroke::new(1.2, theme::CAPTION),
                ));
                bx += 12.0;
            }
        }
    }

    /// Vertical palette on the left edge of the canvas.
    fn palette(&mut self, ui: &mut egui::Ui) {
        let full = ui.max_rect();
        let painter = ui.painter().clone();
        painter.rect_filled(full, 0.0, theme::BAR_BG);
        painter.line_segment(
            [full.right_top(), full.right_bottom()],
            egui::Stroke::new(1.0, theme::BORDER),
        );
        let items: [(UiIcon, Option<StripAction>, &str); 7] = [
            (
                UiIcon::HidePanel,
                Some(StripAction::LibraryBrowser),
                "Hide/Show Library Browser",
            ),
            (
                UiIcon::ZoomIn,
                Some(StripAction::ZoomIn),
                "Zoom In (Ctrl++)",
            ),
            (
                UiIcon::FitView,
                Some(StripAction::FitView),
                "Fit to View (Space)",
            ),
            (
                UiIcon::Zoom100,
                Some(StripAction::Zoom100),
                "Zoom 100% (Ctrl+0)",
            ),
            (
                UiIcon::Annotation,
                None,
                "Annotation — not available in Simpylink yet",
            ),
            (
                UiIcon::Image,
                None,
                "Image — not available in Simpylink yet",
            ),
            (UiIcon::Area, None, "Area — not available in Simpylink yet"),
        ];
        let mut y = full.top() + 6.0;
        let mut clicked = None;
        for (i, (icon, act, tip)) in items.into_iter().enumerate() {
            if i == 1 || i == 4 {
                painter.line_segment(
                    [
                        egui::pos2(full.left() + 6.0, y),
                        egui::pos2(full.right() - 6.0, y),
                    ],
                    egui::Stroke::new(1.0, theme::BORDER),
                );
                y += 6.0;
            }
            let r = egui::Rect::from_min_size(
                egui::pos2(full.center().x - 13.0, y),
                egui::vec2(26.0, 26.0),
            );
            let enabled = act.is_some();
            let resp = ui.interact(
                r,
                ui.id().with(("pal", i)),
                if enabled {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                },
            );
            if enabled && resp.hovered() {
                painter.rect_filled(r, 2.0, theme::BUTTON_HOVER);
            }
            paint_ui_icon(&painter, r.shrink(4.0), icon, enabled);
            if resp.on_hover_text(tip).clicked() {
                clicked = act;
            }
            y += 30.0;
        }
        let r = egui::Rect::from_min_size(
            egui::pos2(full.center().x - 13.0, full.bottom() - 32.0),
            egui::vec2(26.0, 26.0),
        );
        paint_ui_icon(&painter, r.shrink(4.0), UiIcon::Camera, false);
        ui.interact(r, ui.id().with("pal-cam"), egui::Sense::hover())
            .on_hover_text("Screenshot — not available in Simpylink yet");
        if let Some(a) = clicked {
            let ctx = ui.ctx().clone();
            self.perform_strip(a, &ctx);
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        let painter = ui.painter().clone();
        let font = egui::FontId::proportional(12.0);
        let running = self.state.sim.is_running();
        let left = if running {
            "Running…".to_string()
        } else {
            self.state.status.clone()
        };
        painter.text(
            egui::pos2(rect.left() + 8.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            left,
            font.clone(),
            theme::TEXT,
        );
        let solver = self.state.doc.model.config.solver.simulink_name();
        let zoom = format!("{:.0}%", self.state.canvas.view.zoom * 100.0);
        let solver_x = rect.right() - 150.0;
        let zoom_x = solver_x - 120.0;
        for x in [zoom_x - 40.0, solver_x - 20.0] {
            painter.line_segment(
                [
                    egui::pos2(x, rect.top() + 4.0),
                    egui::pos2(x, rect.bottom() - 4.0),
                ],
                egui::Stroke::new(1.0, theme::BORDER),
            );
        }
        painter.text(
            egui::pos2(zoom_x, rect.center().y),
            egui::Align2::CENTER_CENTER,
            zoom,
            font.clone(),
            theme::TEXT,
        );
        painter.text(
            egui::pos2(solver_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            solver,
            font,
            theme::TEXT,
        );
    }
}

/// Opens the main window and blocks until it is closed.
pub fn run_gui(initial_file: Option<PathBuf>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Simpylink")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([760.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Simpylink",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(SimpylinkApp::new(initial_file)))
        }),
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

        let info = self.strip_info();
        let strip_actions = egui::Panel::top("toolstrip")
            .frame(egui::Frame::NONE)
            .resizable(false)
            .show(ui, |ui| self.toolstrip.show(ui, &info))
            .inner;
        for a in strip_actions {
            self.perform_strip(a, &ctx);
        }
        egui::Panel::top("explorer")
            .frame(egui::Frame::NONE)
            .resizable(false)
            .show(ui, |ui| self.explorer_bar(ui));
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::NONE
                    .fill(theme::STATUS_BG)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER)),
            )
            .exact_size(24.0)
            .resizable(false)
            .show(ui, |ui| self.status_bar(ui));
        if self.state.show_bottom {
            egui::Panel::bottom("bottom")
                .frame(
                    egui::Frame::NONE
                        .fill(egui::Color32::WHITE)
                        .inner_margin(6.0)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER)),
                )
                .resizable(true)
                .default_size(160.0)
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
                .frame(
                    egui::Frame::NONE
                        .fill(egui::Color32::WHITE)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER)),
                )
                .resizable(true)
                .default_size(270.0)
                .min_size(200.0)
                .show(ui, |ui| {
                    if let Some(t) = panels::library_panel(
                        ui,
                        &mut self.state.library_filter,
                        &mut self.library_sel,
                    ) {
                        let c = self.view_center();
                        self.state.add_block_at(&t, c);
                    }
                });
        }
        egui::Panel::left("palette")
            .frame(egui::Frame::NONE)
            .exact_size(36.0)
            .resizable(false)
            .show(ui, |ui| self.palette(ui));
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
                if st.fit_pending {
                    st.fit_pending = false;
                    let sys = st.current_system().clone();
                    st.canvas.fit_to_view_limited(&sys, 1.0);
                }
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
        let info = ScopeInfo {
            running: self.state.sim.is_running(),
            sim_time: self
                .state
                .results
                .as_ref()
                .and_then(|r| r.time.last().copied()),
        };
        self.scope_views
            .retain(|k, _| self.state.open_scopes.contains(k));
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
            let view = self.scope_views.entry(sid).or_default();
            let actions =
                show_scope_window(&ctx, sid, &name, &series, &mut open, reset, view, &info);
            for a in actions {
                match a {
                    ScopeAction::Parameters => self.state.open_parameters(sid),
                    ScopeAction::Run => self.state.start_run(),
                    ScopeAction::Stop => self.state.stop_run(),
                }
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
