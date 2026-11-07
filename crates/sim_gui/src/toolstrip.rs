//! Simulink-style toolstrip: dark-blue tab bar (SIMULATION, DEBUG, MODELING,
//! FORMAT, APPS), sections of large/small icon buttons with captions, and a
//! quick-access toolbar on the right of the tab bar.
//!
//! The toolstrip only reports [`StripAction`]s; `app` performs them.
use crate::theme;
use crate::ui_icons::{paint_ui_icon, UiIcon};
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

/// Height of the tab bar.
pub const TAB_H: f32 = 28.0;
/// Height of the toolstrip body (buttons + captions).
pub const BODY_H: f32 = 94.0;
const CAPTION_H: f32 = 18.0;
const NOT_YET: &str = "Not available in Simpylink yet";

/// Toolstrip tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripTab {
    Simulation,
    Debug,
    Modeling,
    Format,
    Apps,
}

impl StripTab {
    /// All tabs in display order.
    pub const ALL: [StripTab; 5] = [
        StripTab::Simulation,
        StripTab::Debug,
        StripTab::Modeling,
        StripTab::Format,
        StripTab::Apps,
    ];
    /// Tab caption.
    pub fn label(self) -> &'static str {
        match self {
            StripTab::Simulation => "SIMULATION",
            StripTab::Debug => "DEBUG",
            StripTab::Modeling => "MODELING",
            StripTab::Format => "FORMAT",
            StripTab::Apps => "APPS",
        }
    }
}

/// Commands issued by the toolstrip.
#[derive(Debug, Clone, PartialEq)]
pub enum StripAction {
    New,
    Open,
    Save,
    SaveAs,
    LibraryBrowser,
    ModelSettings,
    Run,
    Stop,
    SetStopTime(String),
    DataInspector,
    UpdateModel,
    Diagnostics,
    CreateSubsystem,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    SelectAll,
    ZoomIn,
    ZoomOut,
    Zoom100,
    FitView,
    PythonCode,
    ExportPython,
    ExportSlx,
    Quit,
    /// Scope window: Settings… (block parameters).
    ScopeSettings,
    /// Scope window: zoom/pan mode buttons.
    ScopeZoomIn,
    ScopeZoomX,
    ScopeZoomY,
    ScopePan,
    /// Scope window: Scale X & Y axes limits to the data.
    ScopeAutoscale,
    /// Scope window: show/hide the legend.
    ScopeLegend,
}

/// Read-only information the toolstrip needs to enable/label buttons.
#[derive(Debug, Clone, Default)]
pub struct StripInfo {
    pub running: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub stop_time: f64,
    pub revision: u64,
    pub show_library: bool,
    pub show_diagnostics: bool,
    pub has_results: bool,
    pub has_selection: bool,
}

/// Persistent toolstrip state (selected tab, stop-time text being edited).
#[derive(Debug, Clone)]
pub struct Toolstrip {
    pub tab: StripTab,
    stop_text: String,
    stop_rev: u64,
}

impl Default for Toolstrip {
    fn default() -> Self {
        Toolstrip {
            tab: StripTab::Simulation,
            stop_text: String::new(),
            stop_rev: u64::MAX,
        }
    }
}

/// Allocates a full-width toolstrip (tab bar + body), paints the tabs and
/// returns (whole rect, body rect, clicked tab index).
pub(crate) fn strip_frame(
    ui: &mut egui::Ui,
    salt: &str,
    labels: &[&str],
    selected: usize,
) -> (Rect, Rect, Option<usize>) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), TAB_H + BODY_H),
        Sense::hover(),
    );
    let painter = ui.painter().clone();
    let tab_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TAB_H));
    painter.rect_filled(tab_rect, CornerRadius::ZERO, theme::TAB_BAR);
    let body = Rect::from_min_max(Pos2::new(rect.left(), tab_rect.bottom()), rect.max);
    painter.rect_filled(body, CornerRadius::ZERO, theme::STRIP_BG);
    painter.line_segment(
        [body.left_bottom(), body.right_bottom()],
        Stroke::new(1.0, theme::BORDER),
    );
    let mut x = rect.left();
    let tab_font = FontId::proportional(13.0);
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let g = painter.layout_no_wrap(label.to_string(), tab_font.clone(), Color32::WHITE);
        let r = Rect::from_min_size(
            Pos2::new(x, tab_rect.top() + 3.0),
            Vec2::new(g.size().x + 32.0, TAB_H - 3.0),
        );
        let resp = ui.interact(r, ui.id().with(("tab", salt, i)), Sense::click());
        let is_sel = i == selected;
        if is_sel {
            painter.rect_filled(
                r,
                CornerRadius {
                    nw: 2,
                    ne: 2,
                    sw: 0,
                    se: 0,
                },
                theme::STRIP_BG,
            );
        } else if resp.hovered() {
            painter.rect_filled(r, CornerRadius::ZERO, theme::TAB_HOVER);
        }
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            *label,
            tab_font.clone(),
            if is_sel { theme::TEXT } else { Color32::WHITE },
        );
        if resp.clicked() {
            clicked = Some(i);
        }
        x = r.right() + 2.0;
    }
    (rect, body, clicked)
}

/// One button description.
pub(crate) struct Btn {
    id: &'static str,
    icon: UiIcon,
    label: &'static str,
    enabled: bool,
    tip: &'static str,
    action: Option<StripAction>,
    checked: bool,
}

pub(crate) fn btn(
    id: &'static str,
    icon: UiIcon,
    label: &'static str,
    enabled: bool,
    tip: &'static str,
    action: StripAction,
) -> Btn {
    Btn {
        id,
        icon,
        label,
        enabled,
        tip,
        action: Some(action),
        checked: false,
    }
}

impl Btn {
    /// Marks the button as a pressed toggle.
    pub(crate) fn checked(mut self, on: bool) -> Btn {
        self.checked = on;
        self
    }
}

pub(crate) fn todo_btn(id: &'static str, icon: UiIcon, label: &'static str) -> Btn {
    Btn {
        id,
        icon,
        label,
        enabled: false,
        tip: NOT_YET,
        action: None,
        checked: false,
    }
}

pub(crate) struct Builder<'a> {
    ui: &'a mut egui::Ui,
    x: f32,
    top: f32,
    bottom: f32,
    actions: Vec<StripAction>,
}

impl<'a> Builder<'a> {
    /// Builder for the toolstrip body `body`.
    pub(crate) fn new(ui: &'a mut egui::Ui, body: Rect) -> Builder<'a> {
        Builder {
            ui,
            x: body.left() + 2.0,
            top: body.top(),
            bottom: body.bottom(),
            actions: vec![],
        }
    }

    /// Actions collected so far.
    pub(crate) fn take_actions(&mut self) -> Vec<StripAction> {
        std::mem::take(&mut self.actions)
    }

    fn painter(&self) -> egui::Painter {
        self.ui.painter().clone()
    }

    fn button_frame(&mut self, rect: Rect, b: &Btn) -> bool {
        let sense = if b.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let resp = self.ui.interact(rect, self.ui.id().with(b.id), sense);
        let painter = self.painter();
        if b.checked {
            painter.rect(
                rect,
                CornerRadius::same(2),
                theme::BUTTON_PRESSED,
                Stroke::new(1.0, Color32::from_rgb(0x7F, 0xB2, 0xDE)),
                StrokeKind::Inside,
            );
        } else if b.enabled && resp.is_pointer_button_down_on() {
            painter.rect_filled(rect, CornerRadius::same(2), theme::BUTTON_PRESSED);
        } else if b.enabled && resp.hovered() {
            painter.rect(
                rect,
                CornerRadius::same(2),
                theme::BUTTON_HOVER,
                Stroke::new(1.0, Color32::from_rgb(0xA9, 0xCA, 0xE8)),
                StrokeKind::Inside,
            );
        }
        let resp = resp.on_hover_text(b.tip);
        b.enabled && resp.clicked()
    }

    /// Large button: 32 px icon with a (possibly two-line) label below.
    pub(crate) fn big(&mut self, b: Btn) {
        let font = FontId::proportional(12.0);
        let painter = self.painter();
        let galley = painter.layout_no_wrap(b.label.to_string(), font.clone(), theme::TEXT);
        let w = (galley.size().x + 12.0).max(46.0);
        let rect = Rect::from_min_size(
            Pos2::new(self.x, self.top + 3.0),
            Vec2::new(w, self.bottom - CAPTION_H - self.top - 5.0),
        );
        let clicked = self.button_frame(rect, &b);
        let icon = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + 20.0),
            Vec2::splat(32.0),
        );
        paint_ui_icon(&painter, icon, b.icon, b.enabled);
        let col = if b.enabled {
            theme::TEXT
        } else {
            theme::DISABLED
        };
        for (i, line) in b.label.lines().enumerate() {
            painter.text(
                Pos2::new(rect.center().x, rect.top() + 40.0 + i as f32 * 14.0),
                Align2::CENTER_TOP,
                line,
                font.clone(),
                col,
            );
        }
        if clicked {
            if let Some(a) = b.action {
                self.actions.push(a);
            }
        }
        self.x += w + 2.0;
    }

    /// Column of up to three small buttons (16 px icon + label).
    pub(crate) fn smalls(&mut self, items: Vec<Btn>) {
        let font = FontId::proportional(12.0);
        let painter = self.painter();
        let w = items
            .iter()
            .map(|b| {
                painter
                    .layout_no_wrap(b.label.to_string(), font.clone(), theme::TEXT)
                    .size()
                    .x
            })
            .fold(0.0, f32::max)
            + 30.0;
        let row_h = 23.0;
        for (i, b) in items.into_iter().enumerate() {
            let rect = Rect::from_min_size(
                Pos2::new(self.x, self.top + 5.0 + i as f32 * row_h),
                Vec2::new(w, row_h - 1.0),
            );
            let clicked = self.button_frame(rect, &b);
            let icon = Rect::from_center_size(
                Pos2::new(rect.left() + 12.0, rect.center().y),
                Vec2::splat(16.0),
            );
            paint_ui_icon(&painter, icon, b.icon, b.enabled);
            let col = if b.enabled {
                theme::TEXT
            } else {
                theme::DISABLED
            };
            painter.text(
                Pos2::new(rect.left() + 25.0, rect.center().y),
                Align2::LEFT_CENTER,
                b.label,
                font.clone(),
                col,
            );
            if clicked {
                if let Some(a) = b.action {
                    self.actions.push(a);
                }
            }
        }
        self.x += w + 2.0;
    }

    /// Wraps the buttons added by `f` in a captioned section.
    pub(crate) fn section(&mut self, caption: &str, f: impl FnOnce(&mut Self)) {
        self.x += 6.0;
        let start = self.x;
        f(self);
        self.x += 6.0;
        let painter = self.painter();
        let cap_w = painter
            .layout_no_wrap(
                caption.to_string(),
                FontId::proportional(11.0),
                theme::CAPTION,
            )
            .size()
            .x;
        self.x = self.x.max(start + cap_w + 10.0);
        painter.text(
            Pos2::new((start + self.x) / 2.0, self.bottom - CAPTION_H / 2.0 - 1.0),
            Align2::CENTER_CENTER,
            caption,
            FontId::proportional(11.0),
            theme::CAPTION,
        );
        painter.line_segment(
            [
                Pos2::new(self.x, self.top + 6.0),
                Pos2::new(self.x, self.bottom - 4.0),
            ],
            Stroke::new(1.0, theme::BORDER),
        );
    }
}

impl Toolstrip {
    /// Draws the toolstrip into `ui` (full width) and returns the clicked actions.
    pub fn show(&mut self, ui: &mut egui::Ui, info: &StripInfo) -> Vec<StripAction> {
        if self.stop_rev != info.revision {
            self.stop_text = format_number(info.stop_time);
            self.stop_rev = info.revision;
        }
        let labels: Vec<&str> = StripTab::ALL.iter().map(|t| t.label()).collect();
        let sel = StripTab::ALL
            .iter()
            .position(|t| *t == self.tab)
            .unwrap_or(0);
        let (rect, body, clicked) = strip_frame(ui, "main", &labels, sel);
        if let Some(i) = clicked {
            if let Some(t) = StripTab::ALL.get(i) {
                self.tab = *t;
            }
        }
        let painter = ui.painter().clone();
        let tab_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TAB_H));
        let mut actions = vec![];

        // Quick access toolbar (right side of the tab bar).
        let qa: [(UiIcon, bool, &str, StripAction); 3] = [
            (UiIcon::Save, true, "Save (Ctrl+S)", StripAction::Save),
            (
                UiIcon::Undo,
                info.can_undo,
                "Undo (Ctrl+Z)",
                StripAction::Undo,
            ),
            (
                UiIcon::Redo,
                info.can_redo,
                "Redo (Ctrl+Y)",
                StripAction::Redo,
            ),
        ];
        let mut qx = rect.right() - 8.0 - 26.0 * qa.len() as f32 - 30.0;
        for (icon, enabled, tip, act) in qa {
            let r = Rect::from_min_size(Pos2::new(qx, tab_rect.top() + 2.0), Vec2::splat(24.0));
            let resp = ui.interact(
                r,
                ui.id().with(("qa", tip)),
                if enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            if enabled && resp.hovered() {
                painter.rect_filled(r, CornerRadius::same(2), theme::TAB_HOVER);
            }
            painter.rect_filled(
                r.shrink(3.0),
                CornerRadius::same(2),
                Color32::from_rgb(0xF0, 0xF4, 0xF8),
            );
            paint_ui_icon(&painter, r.shrink(5.0), icon, enabled);
            if resp.on_hover_text(tip).clicked() && enabled {
                actions.push(act);
            }
            qx += 26.0;
        }
        let hr = Rect::from_min_size(Pos2::new(qx + 6.0, tab_rect.top() + 2.0), Vec2::splat(24.0));
        paint_ui_icon(&painter, hr.shrink(3.0), UiIcon::Help, true);
        ui.interact(hr, ui.id().with("qa-help"), Sense::hover())
            .on_hover_text("Simpylink — open-source block diagram simulator");

        // Body.
        let mut b = Builder {
            ui,
            x: body.left() + 2.0,
            top: body.top(),
            bottom: body.bottom(),
            actions,
        };
        match self.tab {
            StripTab::Simulation => {
                file_section(&mut b);
                b.section("LIBRARY", |b| {
                    let mut lb = btn(
                        "lib",
                        UiIcon::LibraryBrowser,
                        "Library\nBrowser",
                        true,
                        "Library Browser (Ctrl+Shift+L)",
                        StripAction::LibraryBrowser,
                    );
                    lb.checked = info.show_library;
                    b.big(lb);
                });
                b.section("PREPARE", |b| {
                    b.big(todo_btn("log", UiIcon::LogSignals, "Log\nSignals"));
                    b.big(btn(
                        "settings",
                        UiIcon::ModelSettings,
                        "Model\nSettings",
                        true,
                        "Model Settings (Ctrl+E)",
                        StripAction::ModelSettings,
                    ));
                });
                self.simulate_section(&mut b, info);
                b.section("REVIEW RESULTS", |b| {
                    b.big(btn(
                        "inspector",
                        UiIcon::DataInspector,
                        "Data\nInspector",
                        info.has_results,
                        "Open the Scope windows of this model with the latest results",
                        StripAction::DataInspector,
                    ));
                });
            }
            StripTab::Debug => {
                b.section("DIAGNOSTICS", |b| {
                    let mut d = btn(
                        "diag",
                        UiIcon::Diagnostics,
                        "Diagnostics",
                        true,
                        "Show or hide the Diagnostic Viewer",
                        StripAction::Diagnostics,
                    );
                    d.checked = info.show_diagnostics;
                    b.big(d);
                });
                b.section("PREPARE", |b| {
                    b.big(btn(
                        "update",
                        UiIcon::UpdateModel,
                        "Update\nModel",
                        true,
                        "Update Model (Ctrl+D)",
                        StripAction::UpdateModel,
                    ));
                });
                self.simulate_section(&mut b, info);
            }
            StripTab::Modeling => {
                b.section("EVALUATE & MANAGE", |b| {
                    b.big(btn(
                        "upd2",
                        UiIcon::UpdateModel,
                        "Update\nModel",
                        true,
                        "Update Model (Ctrl+D)",
                        StripAction::UpdateModel,
                    ));
                });
                b.section("SETUP", |b| {
                    b.big(btn(
                        "settings2",
                        UiIcon::ModelSettings,
                        "Model\nSettings",
                        true,
                        "Model Settings (Ctrl+E)",
                        StripAction::ModelSettings,
                    ));
                });
                b.section("COMPONENT", |b| {
                    b.big(btn(
                        "subsys",
                        UiIcon::Subsystem,
                        "Create\nSubsystem",
                        info.has_selection,
                        "Create Subsystem from Selection (Ctrl+G)",
                        StripAction::CreateSubsystem,
                    ));
                });
                b.section("EDIT", |b| {
                    b.smalls(vec![
                        btn(
                            "cut",
                            UiIcon::Cut,
                            "Cut",
                            info.has_selection,
                            "Ctrl+X",
                            StripAction::Cut,
                        ),
                        btn(
                            "copy",
                            UiIcon::Copy,
                            "Copy",
                            info.has_selection,
                            "Ctrl+C",
                            StripAction::Copy,
                        ),
                        btn(
                            "paste",
                            UiIcon::Paste,
                            "Paste",
                            true,
                            "Ctrl+V",
                            StripAction::Paste,
                        ),
                    ]);
                    b.smalls(vec![
                        btn(
                            "del",
                            UiIcon::Delete,
                            "Delete",
                            info.has_selection,
                            "Delete",
                            StripAction::Delete,
                        ),
                        btn(
                            "selall",
                            UiIcon::SelectAll,
                            "Select All",
                            true,
                            "Ctrl+A",
                            StripAction::SelectAll,
                        ),
                    ]);
                });
                self.simulate_section(&mut b, info);
            }
            StripTab::Format => {
                b.section("ZOOM", |b| {
                    b.big(btn(
                        "fit",
                        UiIcon::FitView,
                        "Fit to\nView",
                        true,
                        "Fit to View (Space)",
                        StripAction::FitView,
                    ));
                    b.smalls(vec![
                        btn(
                            "zin",
                            UiIcon::ZoomIn,
                            "Zoom In",
                            true,
                            "Ctrl++",
                            StripAction::ZoomIn,
                        ),
                        btn(
                            "zout",
                            UiIcon::ZoomOut,
                            "Zoom Out",
                            true,
                            "Ctrl+-",
                            StripAction::ZoomOut,
                        ),
                        btn(
                            "z100",
                            UiIcon::Zoom100,
                            "Zoom 100%",
                            true,
                            "Ctrl+0",
                            StripAction::Zoom100,
                        ),
                    ]);
                });
                b.section("FONT & PARAGRAPH", |b| {
                    b.big(todo_btn("font", UiIcon::Annotation, "Font\nStyle"));
                });
            }
            StripTab::Apps => {
                b.section("CODE GENERATION", |b| {
                    b.big(btn(
                        "pycode",
                        UiIcon::PythonCode,
                        "Python\nCode",
                        true,
                        "Show the Python code generated from this model",
                        StripAction::PythonCode,
                    ));
                    b.big(btn(
                        "pyexport",
                        UiIcon::ExportPython,
                        "Export\nPython",
                        true,
                        "Export the generated Python script…",
                        StripAction::ExportPython,
                    ));
                });
                b.section("SHARE", |b| {
                    b.big(btn(
                        "slx",
                        UiIcon::ExportSlx,
                        "Export\nSLX",
                        true,
                        "Export as SLX (experimental, not verified with Simulink R2026b)",
                        StripAction::ExportSlx,
                    ));
                });
            }
        }
        b.actions
    }

    fn simulate_section(&mut self, b: &mut Builder, info: &StripInfo) {
        b.section("SIMULATE", |b| {
            // Stop Time / mode / Fast Restart column.
            let font = FontId::proportional(12.0);
            let painter = b.painter();
            let x0 = b.x;
            let col_w = 168.0;
            let top = b.top;
            let row = |i: usize| top + 6.0 + i as f32 * 24.0;
            painter.text(
                Pos2::new(x0 + 2.0, row(0) + 10.0),
                Align2::LEFT_CENTER,
                "Stop Time",
                font.clone(),
                theme::TEXT,
            );
            let edit_rect =
                Rect::from_min_size(Pos2::new(x0 + 66.0, row(0)), Vec2::new(col_w - 68.0, 21.0));
            let r = b.ui.put(
                edit_rect,
                egui::TextEdit::singleline(&mut self.stop_text)
                    .font(font.clone())
                    .vertical_align(egui::Align::Center),
            );
            if r.lost_focus() {
                b.actions
                    .push(StripAction::SetStopTime(self.stop_text.trim().to_string()));
            }
            // Simulation mode (only Normal is supported).
            let mode =
                Rect::from_min_size(Pos2::new(x0 + 2.0, row(1)), Vec2::new(col_w - 4.0, 21.0));
            painter.rect(
                mode,
                CornerRadius::same(2),
                Color32::WHITE,
                Stroke::new(1.0, Color32::from_rgb(0xBC, 0xBC, 0xBC)),
                StrokeKind::Inside,
            );
            painter.text(
                Pos2::new(mode.left() + 6.0, mode.center().y),
                Align2::LEFT_CENTER,
                "Normal",
                font.clone(),
                theme::TEXT,
            );
            let c = Pos2::new(mode.right() - 10.0, mode.center().y);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(-4.0, -2.0),
                    c + Vec2::new(4.0, -2.0),
                    c + Vec2::new(0.0, 3.0),
                ],
                theme::CAPTION,
                Stroke::NONE,
            ));
            b.ui.interact(mode, b.ui.id().with("simmode"), Sense::hover())
                .on_hover_text("Simulation mode: only Normal is supported");
            let fr = Rect::from_min_size(Pos2::new(x0, row(2)), Vec2::new(col_w, 22.0));
            paint_ui_icon(
                &painter,
                Rect::from_center_size(
                    Pos2::new(fr.left() + 12.0, fr.center().y),
                    Vec2::splat(16.0),
                ),
                UiIcon::FastRestart,
                false,
            );
            painter.text(
                Pos2::new(fr.left() + 25.0, fr.center().y),
                Align2::LEFT_CENTER,
                "Fast Restart",
                font,
                theme::DISABLED,
            );
            b.ui.interact(fr, b.ui.id().with("fastrestart"), Sense::hover())
                .on_hover_text(NOT_YET);
            b.x += col_w + 4.0;
            b.big(todo_btn("stepback", UiIcon::StepBack, "Step\nBack"));
            b.big(btn(
                "run",
                UiIcon::Run,
                "Run",
                !info.running,
                "Run (Ctrl+T / F5)",
                StripAction::Run,
            ));
            b.big(todo_btn("stepfwd", UiIcon::StepForward, "Step\nForward"));
            b.big(btn(
                "stop",
                UiIcon::Stop,
                "Stop",
                info.running,
                "Stop (Ctrl+Shift+T)",
                StripAction::Stop,
            ));
        });
    }
}

fn file_section(b: &mut Builder) {
    b.section("FILE", |b| {
        b.big(btn(
            "new",
            UiIcon::New,
            "New",
            true,
            "New model (Ctrl+N)",
            StripAction::New,
        ));
        b.smalls(vec![
            btn(
                "open",
                UiIcon::Open,
                "Open",
                true,
                "Open… (Ctrl+O)",
                StripAction::Open,
            ),
            btn(
                "save",
                UiIcon::Save,
                "Save",
                true,
                "Save (Ctrl+S)",
                StripAction::Save,
            ),
            btn(
                "saveas",
                UiIcon::Save,
                "Save As",
                true,
                "Save As…",
                StripAction::SaveAs,
            ),
        ]);
    });
}

/// Formats a stop time the way Simulink shows it ("10", "0.5", "inf").
pub fn format_number(v: f64) -> String {
    if v.is_infinite() {
        if v > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        }
    } else if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_format_like_simulink() {
        assert_eq!(format_number(10.0), "10");
        assert_eq!(format_number(0.5), "0.5");
        assert_eq!(format_number(f64::INFINITY), "inf");
    }

    #[test]
    fn tabs_have_simulink_order() {
        let l: Vec<_> = StripTab::ALL.iter().map(|t| t.label()).collect();
        assert_eq!(l, ["SIMULATION", "DEBUG", "MODELING", "FORMAT", "APPS"]);
    }
}
