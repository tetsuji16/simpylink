//! egui views for panels and dialogs. All logic lives in `state` / `dialogs`;
//! these functions only draw and forward button presses.
use crate::canvas::widget::LibraryDrag;
use crate::dialogs::DialogButton;
use crate::state::{AppState, PendingAction};
use egui::{Color32, RichText};
use sim_core::diag::Severity;
use sim_core::library::{registry, ParamKind};
use sim_core::model::SolverKind;

const ERR: Color32 = Color32::from_rgb(0xD0, 0x30, 0x30);
/// Library headings in Simulink order.
pub const LIBRARIES: [&str; 6] = [
    "Sources",
    "Sinks",
    "Math Operations",
    "Continuous",
    "Discrete",
    "Ports & Subsystems",
];

/// Blocks listed under "Commonly Used Blocks" (Simulink's own selection, limited to supported blocks).
pub const COMMONLY_USED: [&str; 12] = [
    "Constant",
    "Gain",
    "Inport",
    "Integrator",
    "Outport",
    "Product",
    "Saturate",
    "Scope",
    "SubSystem",
    "Sum",
    "Switch",
    "UnitDelay",
];
const COMMON: &str = "Commonly Used Blocks";

/// A block with default parameters used to draw library previews.
pub fn preview_block(s: &sim_core::library::BlockSpec) -> sim_core::model::Block {
    let mut b = sim_core::model::Block {
        sid: sim_core::ids::BlockId(0),
        block_type: s.block_type.to_string(),
        name: s.display_name.to_string(),
        position: sim_core::geom::Rect::new(0.0, 0.0, s.default_size.0, s.default_size.1),
        orientation: sim_core::model::Orientation::Right,
        params: Default::default(),
        system: None,
    };
    sim_core::library::fill_default_params(&mut b);
    b
}

/// Paints a library preview of `s` centred in `area`.
fn paint_preview(painter: &egui::Painter, area: egui::Rect, s: &sim_core::library::BlockSpec) {
    use crate::canvas::geometry::{block_ports, chevron_dir};
    use crate::canvas::icons::{paint_frame, paint_icon, INK};
    let b = preview_block(s);
    let (w, h) = s.default_size;
    let k = ((area.width() as f64 - 12.0) / w)
        .min((area.height() as f64 - 4.0) / h)
        .min(1.4) as f32;
    let c = area.center();
    let map = |q: sim_core::geom::Point| {
        egui::pos2(
            c.x + (q.x - w / 2.0) as f32 * k,
            c.y + (q.y - h / 2.0) as f32 * k,
        )
    };
    let r = egui::Rect::from_center_size(c, egui::vec2(w as f32 * k, h as f32 * k));
    let stroke = egui::Stroke::new(1.0, INK);
    paint_frame(painter, r, &b, Color32::WHITE, stroke);
    let ports = block_ports(&b);
    let ins: Vec<egui::Pos2> = ports.inputs.iter().map(|q| map(*q)).collect();
    paint_icon(painter, r, &b, k, &ins);
    for q in &ports.inputs {
        let t = chevron_dir(*q, b.orientation, true).map(map);
        painter.add(egui::Shape::line(vec![t[0], t[2], t[1]], stroke));
    }
    for q in &ports.outputs {
        let t = chevron_dir(*q, b.orientation, false).map(map);
        painter.add(egui::Shape::line(vec![t[0], t[2], t[1]], stroke));
    }
}

/// Library Browser (left panel): search field, library tree and a grid of
/// block icons that can be dragged onto the canvas. Returns a block type to
/// add at the view centre (double-click or context menu).
pub fn library_panel(
    ui: &mut egui::Ui,
    filter: &mut String,
    selected: &mut Option<&'static str>,
) -> Option<String> {
    let mut add = None;
    let sel = selected.unwrap_or(COMMON);
    // Title row.
    egui::Frame::NONE
        .fill(crate::theme::BAR_BG)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Library Browser").strong().size(13.0));
            ui.add(
                egui::TextEdit::singleline(filter)
                    .hint_text("Enter search term")
                    .desired_width(f32::INFINITY),
            );
        });
    let q = filter.trim().to_lowercase();
    // Tree.
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(4, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::CollapsingHeader::new(RichText::new("Simulink").strong())
                .default_open(true)
                .show(ui, |ui| {
                    let mut libs: Vec<&'static str> = vec![COMMON];
                    let mut rest: Vec<&'static str> = LIBRARIES.to_vec();
                    rest.sort_unstable();
                    libs.extend(rest);
                    for lib in libs {
                        let is_sel = q.is_empty() && sel == lib;
                        let r = ui.add(
                            egui::Button::selectable(is_sel, lib)
                                .frame_when_inactive(false)
                                .min_size(egui::vec2(ui.available_width(), 20.0)),
                        );
                        if r.clicked() {
                            *selected = Some(lib);
                            filter.clear();
                        }
                    }
                });
        });
    ui.separator();
    let items: Vec<&'static sim_core::library::BlockSpec> = if !q.is_empty() {
        registry()
            .iter()
            .filter(|s| {
                s.display_name.to_lowercase().contains(&q)
                    || s.block_type.to_lowercase().contains(&q)
            })
            .collect()
    } else if sel == COMMON {
        COMMONLY_USED
            .iter()
            .filter_map(|t| sim_core::library::spec(t))
            .collect()
    } else {
        registry().iter().filter(|s| s.library == sel).collect()
    };
    let heading = if q.is_empty() {
        format!("Simulink/{sel}")
    } else {
        format!("Search results: '{}' ({})", filter.trim(), items.len())
    };
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(RichText::new(heading).small().color(crate::theme::CAPTION));
    });
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let tile = egui::vec2(82.0, 78.0);
            let cols = ((ui.available_width() - 4.0) / tile.x).floor().max(1.0) as usize;
            for row in items.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for s in row {
                        let r = ui.dnd_drag_source(
                            egui::Id::new(("lib", s.block_type)),
                            LibraryDrag(s.block_type.to_string()),
                            |ui| {
                                let (rect, resp) =
                                    ui.allocate_exact_size(tile, egui::Sense::click());
                                let painter = ui.painter();
                                if resp.hovered() {
                                    painter.rect_filled(
                                        rect.shrink(1.0),
                                        2.0,
                                        crate::theme::BUTTON_HOVER,
                                    );
                                }
                                let icon_area = egui::Rect::from_min_size(
                                    rect.min + egui::vec2(6.0, 6.0),
                                    egui::vec2(tile.x - 12.0, 42.0),
                                );
                                paint_preview(painter, icon_area, s);
                                let font = egui::FontId::proportional(11.0);
                                let g = painter.layout(
                                    s.display_name.to_string(),
                                    font,
                                    crate::theme::TEXT,
                                    tile.x - 6.0,
                                );
                                let gw = g.size().x;
                                painter.galley(
                                    egui::pos2(rect.center().x - gw / 2.0, rect.top() + 52.0),
                                    g,
                                    crate::theme::TEXT,
                                );
                                resp
                            },
                        );
                        let resp = r.inner.on_hover_text(s.description);
                        if resp.double_clicked() {
                            add = Some(s.block_type.to_string());
                        }
                        resp.context_menu(|ui| {
                            if ui.button("Add block to model").clicked() {
                                add = Some(s.block_type.to_string());
                                ui.close();
                            }
                        });
                    }
                });
            }
        });
    add
}

/// Bottom panel: Diagnostics / Output tabs. Returns a block to select.
pub fn bottom_panel(
    ui: &mut egui::Ui,
    st: &AppState,
    tab: &mut usize,
) -> Option<sim_core::ids::BlockId> {
    let mut select = None;
    ui.horizontal(|ui| {
        ui.selectable_value(tab, 0, format!("Diagnostics ({})", st.diagnostics.len()));
        ui.selectable_value(tab, 1, "Output");
    });
    ui.separator();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(*tab == 1)
        .show(ui, |ui| {
            if *tab == 0 {
                egui::Grid::new("diag").striped(true).show(ui, |ui| {
                    for d in &st.diagnostics {
                        let (txt, col) = match d.severity {
                            Severity::Error => ("Error", ERR),
                            Severity::Warning => ("Warning", Color32::from_rgb(0xC0, 0x80, 0x00)),
                            Severity::Info => ("Info", Color32::GRAY),
                        };
                        ui.label(RichText::new(txt).color(col));
                        ui.monospace(&d.code);
                        if ui.link(&d.message).clicked() {
                            select = d.location.block;
                        }
                        ui.label(d.location.block.map(|b| b.to_string()).unwrap_or_default());
                        ui.end_row();
                    }
                });
            } else {
                for l in &st.run_log {
                    ui.monospace(l);
                }
            }
        });
    select
}

/// Block Parameters dialog.
pub fn param_dialog(ctx: &egui::Context, st: &mut AppState) {
    let Some(d) = st.param_dialog.as_mut() else {
        return;
    };
    let mut pressed = None;
    let mut open = true;
    egui::Window::new(&d.title)
        .id(egui::Id::new("param_dialog"))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            if !d.description.is_empty() {
                ui.label(RichText::new(&d.description).italics());
                ui.separator();
            }
            let n = d.fields.len();
            for i in 0..n {
                let f = &d.fields[i];
                ui.label(&f.label);
                let mut v = f.value.clone();
                match f.kind {
                    ParamKind::Enum(opts) => {
                        egui::ComboBox::from_id_salt(("enum", i))
                            .selected_text(&v)
                            .show_ui(ui, |ui| {
                                for o in opts {
                                    ui.selectable_value(&mut v, o.to_string(), *o);
                                }
                            });
                    }
                    _ => {
                        ui.text_edit_singleline(&mut v);
                    }
                }
                if v != d.fields[i].value {
                    d.set_value(i, v);
                }
                if let Some(e) = &d.fields[i].error {
                    ui.label(RichText::new(e).color(ERR));
                }
            }
            ui.separator();
            let ok = !d.has_errors();
            ui.horizontal(|ui| {
                if ui.add_enabled(ok, egui::Button::new("OK")).clicked() {
                    pressed = Some(DialogButton::Ok);
                }
                if ui.button("Cancel").clicked() {
                    pressed = Some(DialogButton::Cancel);
                }
                if ui.add_enabled(ok, egui::Button::new("Apply")).clicked() {
                    pressed = Some(DialogButton::Apply);
                }
            });
        });
    if !open {
        pressed = Some(DialogButton::Cancel);
    }
    if let Some(b) = pressed {
        st.param_dialog_press(b);
    }
}

/// Model Settings (Ctrl+E) dialog.
pub fn settings_dialog(ctx: &egui::Context, st: &mut AppState) {
    let Some(d) = st.settings_dialog.as_mut() else {
        return;
    };
    let mut pressed = None;
    egui::Window::new("Model Settings")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("settings").num_columns(2).show(ui, |ui| {
                let row = |ui: &mut egui::Ui, label: &str, key: &'static str, v: &mut String| {
                    ui.label(label);
                    ui.vertical(|ui| {
                        ui.text_edit_singleline(v);
                        if let Some(e) = d.errors.get(key) {
                            ui.label(RichText::new(e).color(ERR));
                        }
                    });
                    ui.end_row();
                };
                let (mut a, mut b, mut c, mut e, mut f, mut g) = (
                    d.start.clone(),
                    d.stop.clone(),
                    d.rel_tol.clone(),
                    d.abs_tol.clone(),
                    d.max_step.clone(),
                    d.output_step.clone(),
                );
                row(ui, "Start time", "start", &mut a);
                row(ui, "Stop time", "stop", &mut b);
                row(ui, "Relative tolerance", "rel_tol", &mut c);
                row(ui, "Absolute tolerance", "abs_tol", &mut e);
                row(ui, "Max step size", "max_step", &mut f);
                row(ui, "Output step", "output_step", &mut g);
                (
                    d.start,
                    d.stop,
                    d.rel_tol,
                    d.abs_tol,
                    d.max_step,
                    d.output_step,
                ) = (a, b, c, e, f, g);
                ui.label("Solver");
                egui::ComboBox::from_id_salt("solver")
                    .selected_text(solver_name(d.solver))
                    .show_ui(ui, |ui| {
                        for s in [
                            SolverKind::Auto,
                            SolverKind::Ode45,
                            SolverKind::Ode23,
                            SolverKind::Ode15s,
                        ] {
                            ui.selectable_value(&mut d.solver, s, solver_name(s));
                        }
                    });
                ui.end_row();
            });
            ui.label(
                RichText::new(
                    "Solvers are mapped to SciPy methods and are not identical to Simulink.",
                )
                .small(),
            );
            ui.horizontal(|ui| {
                for (t, b) in [
                    ("OK", DialogButton::Ok),
                    ("Cancel", DialogButton::Cancel),
                    ("Apply", DialogButton::Apply),
                ] {
                    if ui.button(t).clicked() {
                        pressed = Some(b);
                    }
                }
            });
        });
    if let Some(b) = pressed {
        st.settings_dialog_press(b);
    }
}

fn solver_name(s: SolverKind) -> &'static str {
    match s {
        SolverKind::Auto => "auto (RK45)",
        SolverKind::Ode45 => "ode45 (RK45)",
        SolverKind::Ode23 => "ode23 (RK23)",
        SolverKind::Ode15s => "ode15s (BDF)",
    }
}

/// Quick insert popup (double-click on the canvas).
pub fn quick_insert(ctx: &egui::Context, st: &mut AppState, canvas_origin: egui::Pos2) {
    let Some(q) = st.quick_insert.as_mut() else {
        return;
    };
    let s = st.canvas.view.to_screen(q.at);
    let mut chosen = None;
    let mut close = false;
    egui::Area::new(egui::Id::new("quick_insert"))
        .fixed_pos(canvas_origin + egui::vec2(s.x, s.y))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let r =
                    ui.add(egui::TextEdit::singleline(&mut q.query).hint_text("Type a block name"));
                if !q.focus_requested {
                    r.request_focus();
                    q.focus_requested = true;
                }
                let (up, down, enter, esc) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::ArrowUp),
                        i.key_pressed(egui::Key::ArrowDown),
                        i.key_pressed(egui::Key::Enter),
                        i.key_pressed(egui::Key::Escape),
                    )
                });
                if up {
                    q.move_highlight(-1);
                }
                if down {
                    q.move_highlight(1);
                }
                for (i, m) in q.matches().iter().enumerate() {
                    if ui
                        .selectable_label(i == q.highlighted, m.display_name)
                        .clicked()
                    {
                        chosen = Some(m.block_type);
                    }
                }
                if enter {
                    chosen = q.chosen();
                    if chosen.is_none() {
                        close = false;
                    }
                }
                if esc {
                    close = true;
                }
            });
        });
    if let Some(t) = chosen {
        let at = q.at;
        st.quick_insert = None;
        st.add_block_at(t, at);
    } else if close {
        st.quick_insert = None;
    }
}

/// "Save changes?" confirmation. Returns true if the user picked "Save" (the caller shows the save dialog).
pub fn confirm_dialog(ctx: &egui::Context, st: &mut AppState) -> Option<PendingAction> {
    let pending = st.confirm.clone()?;
    let mut save = None;
    egui::Window::new("Save changes?")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(format!(
                "Save changes to '{}' before closing?",
                st.doc.model.name
            ));
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    save = Some(pending.clone());
                }
                if ui.button("Don't Save").clicked() {
                    st.perform(pending.clone());
                }
                if ui.button("Cancel").clicked() {
                    st.confirm = None;
                }
            });
        });
    save
}

/// Simple message windows (Python environment error, SLX import report, generated code).
pub fn message_windows(ctx: &egui::Context, st: &mut AppState) {
    if let Some(m) = st.python_error.clone() {
        let mut open = true;
        egui::Window::new("Python environment").open(&mut open).show(ctx, |ui| {
            ui.label(m);
            ui.label("Set SIMPYLINK_PYTHON or install the packages: python -m pip install numpy scipy");
        });
        if !open {
            st.python_error = None;
        }
    }
    if let Some(lines) = st.import_report.clone() {
        let mut open = true;
        egui::Window::new("SLX Import Report")
            .open(&mut open)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for l in lines {
                            ui.label(l);
                        }
                    });
            });
        if !open {
            st.import_report = None;
        }
    }
    let stale = st
        .code_view
        .as_ref()
        .map(|c| c.revision != st.revision)
        .unwrap_or(false);
    let mut refresh = false;
    if let Some(cv) = st.code_view.as_ref() {
        let mut open = true;
        egui::Window::new(format!("Generated Python — {}", cv.file_name))
            .open(&mut open)
            .default_size([640.0, 480.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let label = if stale {
                        RichText::new("Model changed — Refresh").strong()
                    } else {
                        RichText::new("Refresh")
                    };
                    if ui.button(label).clicked() {
                        refresh = true;
                    }
                    if ui.button("Copy").clicked() {
                        ctx.copy_text(cv.source.clone());
                    }
                });
                for d in &cv.diagnostics {
                    ui.label(RichText::new(d.to_string()).color(ERR));
                }
                egui::ScrollArea::both().show(ui, |ui| {
                    let mut s = cv.source.as_str();
                    ui.add(
                        egui::TextEdit::multiline(&mut s)
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    );
                });
            });
        if !open {
            st.code_view = None;
        }
    }
    if refresh {
        st.refresh_code_view();
    }
}
