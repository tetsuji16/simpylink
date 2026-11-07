//! Scope window: data extraction (egui independent) and a Simulink-style view.
//!
//! The view follows the layout of the Simulink Scope: its own window with a
//! toolstrip (SCOPE / MEASUREMENTS), a dark figure with black axes, yellow
//! first signal, light-grey ticks and grid, and a status bar
//! ("Ready" … "Sample based  T=…"). Colours are approximations chosen by eye.
use crate::theme;
use crate::toolstrip::{btn, strip_frame, todo_btn, Builder, StripAction};
use crate::ui_icons::UiIcon;
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};
use sim_core::ids::BlockId;
use sim_runner::SimResults;

/// One plotted series.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeSeries {
    pub label: String,
    pub points: Vec<[f64; 2]>,
}

/// Series of a Scope block.
pub fn scope_series(results: &SimResults, sid: BlockId) -> Vec<ScopeSeries> {
    let mut chs: Vec<_> = results
        .channels
        .iter()
        .filter(|c| c.block_sid == sid.0)
        .collect();
    chs.sort_by_key(|c| c.port);
    chs.iter()
        .map(|c| ScopeSeries {
            label: c.name.clone(),
            points: results
                .time
                .iter()
                .zip(&c.values)
                .filter(|(_, v)| v.is_finite())
                .map(|(t, v)| [*t, *v])
                .collect(),
        })
        .collect()
}

/// (xmin, xmax, ymin, ymax) with 5 % y margin.
pub fn autoscale_bounds(series: &[ScopeSeries]) -> Option<(f64, f64, f64, f64)> {
    let pts: Vec<[f64; 2]> = series
        .iter()
        .flat_map(|s| s.points.iter().copied())
        .collect();
    if pts.is_empty() {
        return None;
    }
    let (mut x0, mut x1, mut y0, mut y1) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    for [x, y] in pts {
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    if x0 == x1 {
        x0 -= 0.5;
        x1 += 0.5;
    }
    if y0 == y1 {
        return Some((x0, x1, y0 - 1.0, y1 + 1.0));
    }
    let m = (y1 - y0) * 0.05;
    Some((x0, x1, y0 - m, y1 + m))
}

/// Line colours in the order Simulink's Scope assigns them to signals.
pub const SCOPE_COLORS: [Color32; 7] = [
    Color32::from_rgb(0xFF, 0xFF, 0x11),
    Color32::from_rgb(0x13, 0x9F, 0xFF),
    Color32::from_rgb(0xFF, 0x69, 0x29),
    Color32::from_rgb(0x64, 0xD4, 0x13),
    Color32::from_rgb(0xB7, 0x46, 0xFF),
    Color32::from_rgb(0x0F, 0xFF, 0xFF),
    Color32::from_rgb(0xFF, 0x13, 0xA6),
];
/// Figure (window) background.
pub const FIGURE_BG: Color32 = Color32::from_rgb(0x28, 0x28, 0x28);
/// Axes background.
pub const AXES_BG: Color32 = Color32::BLACK;
/// Tick labels and axes box.
pub const AXES_FG: Color32 = Color32::from_rgb(0xAF, 0xAF, 0xAF);
/// Grid lines.
pub const GRID: Color32 = Color32::from_rgb(0x50, 0x50, 0x50);

/// Colour of the `i`-th signal.
pub fn signal_color(i: usize) -> Color32 {
    SCOPE_COLORS[i % SCOPE_COLORS.len()]
}

/// "Nice" grid step (1, 2 or 5 × 10^k) giving about `target` divisions of `span`.
pub fn nice_step(span: f64, target: f64) -> f64 {
    if !(span.is_finite() && span > 0.0 && target > 0.0) {
        return 1.0;
    }
    let raw = span / target;
    let mag = 10f64.powf(raw.log10().floor());
    let f = raw / mag;
    let m = if f <= 1.0 {
        1.0
    } else if f <= 2.0 {
        2.0
    } else if f <= 5.0 {
        5.0
    } else {
        10.0
    };
    m * mag
}

/// Single-level grid like Simulink's Scope (about `target` divisions).
fn scope_grid(input: egui_plot::GridInput, target: f64) -> Vec<egui_plot::GridMark> {
    let (lo, hi) = input.bounds;
    let step = nice_step(hi - lo, target);
    let first = (lo / step).ceil() as i64;
    let last = (hi / step).floor() as i64;
    if last - first > 1000 {
        return vec![];
    }
    (first..=last)
        .map(|k| egui_plot::GridMark {
            value: k as f64 * step,
            step_size: step,
        })
        .collect()
}

/// Mouse mode of the axes (Zoom & Pan section).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScopeMode {
    /// Drag a box to zoom both axes; click to zoom in ×2.
    ZoomIn,
    /// Drag/click to zoom the time axis only.
    ZoomX,
    /// Drag/click to zoom the y axis only.
    ZoomY,
    /// Drag to pan (default).
    #[default]
    Pan,
}

/// Commands from a Scope window to the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeAction {
    /// "Settings" — open the Scope block parameters.
    Parameters,
    Run,
    Stop,
}

/// Information the Scope window shows but does not own.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScopeInfo {
    pub running: bool,
    /// Simulation time of the last sample, if any results exist.
    pub sim_time: Option<f64>,
}

/// Per-window view state.
#[derive(Debug, Clone, Default)]
pub struct ScopeView {
    pub tab: usize,
    pub mode: ScopeMode,
    pub legend: bool,
    pending: Option<[f64; 4]>,
    press: Option<Pos2>,
    autoscale: bool,
}

/// New bounds (x0, x1, y0, y1) after a zoom gesture from `a` to `b` in plot
/// coordinates, given the current bounds. A tiny drag counts as a click,
/// which zooms ×2 about `b` on the axes the mode allows.
pub fn zoom_bounds(mode: ScopeMode, a: [f64; 2], b: [f64; 2], cur: [f64; 4]) -> Option<[f64; 4]> {
    let [x0, x1, y0, y1] = cur;
    let (w, h) = (x1 - x0, y1 - y0);
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
    let click = (a[0] - b[0]).abs() < w * 0.01 && (a[1] - b[1]).abs() < h * 0.01;
    let (zx, zy) = match mode {
        ScopeMode::ZoomIn => (true, true),
        ScopeMode::ZoomX => (true, false),
        ScopeMode::ZoomY => (false, true),
        ScopeMode::Pan => return None,
    };
    let (nx0, nx1) = if !zx {
        (x0, x1)
    } else if click {
        (b[0] - w / 4.0, b[0] + w / 4.0)
    } else {
        (a[0].min(b[0]), a[0].max(b[0]))
    };
    let (ny0, ny1) = if !zy {
        (y0, y1)
    } else if click {
        (b[1] - h / 4.0, b[1] + h / 4.0)
    } else {
        (a[1].min(b[1]), a[1].max(b[1]))
    };
    if nx1 - nx0 <= 0.0 || ny1 - ny0 <= 0.0 {
        return None;
    }
    Some([nx0, nx1, ny0, ny1])
}

fn scope_toolstrip(ui: &mut egui::Ui, view: &mut ScopeView, info: &ScopeInfo) -> Vec<ScopeAction> {
    let (_, body, clicked) = strip_frame(ui, "scope", &["SCOPE", "MEASUREMENTS"], view.tab);
    if let Some(i) = clicked {
        view.tab = i;
    }
    let mut b = Builder::new(ui, body);
    if view.tab == 0 {
        b.section("CONFIGURATION", |b| {
            b.big(btn(
                "sc-settings",
                UiIcon::ModelSettings,
                "Settings",
                true,
                "Scope block parameters",
                StripAction::ScopeSettings,
            ));
            b.big(todo_btn("sc-style", UiIcon::Style, "Style"));
        });
        b.section("SIMULATE", |b| {
            b.big(btn(
                "sc-run",
                UiIcon::Run,
                "Run",
                !info.running,
                "Run (Ctrl+T)",
                StripAction::Run,
            ));
            b.big(btn(
                "sc-stop",
                UiIcon::Stop,
                "Stop",
                info.running,
                "Stop",
                StripAction::Stop,
            ));
        });
        let m = view.mode;
        b.section("ZOOM & PAN", |b| {
            b.smalls(vec![
                btn(
                    "sc-zin",
                    UiIcon::ZoomIn,
                    "Zoom In",
                    true,
                    "Drag a box or click to zoom",
                    StripAction::ScopeZoomIn,
                )
                .checked(m == ScopeMode::ZoomIn),
                btn(
                    "sc-zx",
                    UiIcon::ZoomX,
                    "Zoom X",
                    true,
                    "Zoom the time axis",
                    StripAction::ScopeZoomX,
                )
                .checked(m == ScopeMode::ZoomX),
                btn(
                    "sc-zy",
                    UiIcon::ZoomY,
                    "Zoom Y",
                    true,
                    "Zoom the y axis",
                    StripAction::ScopeZoomY,
                )
                .checked(m == ScopeMode::ZoomY),
            ]);
            b.smalls(vec![btn(
                "sc-pan",
                UiIcon::Pan,
                "Pan",
                true,
                "Drag to pan",
                StripAction::ScopePan,
            )
            .checked(m == ScopeMode::Pan)]);
            b.big(btn(
                "sc-auto",
                UiIcon::FitView,
                "Scale X & Y\nAxes Limits",
                true,
                "Scale X & Y axes limits to the data (autoscale)",
                StripAction::ScopeAutoscale,
            ));
        });
        let legend = view.legend;
        b.section("DISPLAY", |b| {
            b.big(
                btn(
                    "sc-legend",
                    UiIcon::Legend,
                    "Legend",
                    true,
                    "Show or hide the legend",
                    StripAction::ScopeLegend,
                )
                .checked(legend),
            );
        });
    } else {
        b.section("MEASUREMENTS", |b| {
            b.big(todo_btn("sc-cur", UiIcon::Cursors, "Data\nCursors"));
            b.big(todo_btn(
                "sc-stat",
                UiIcon::Statistics,
                "Signal\nStatistics",
            ));
            b.big(todo_btn("sc-peak", UiIcon::PeakFinder, "Peak\nFinder"));
            b.big(todo_btn("sc-bil", UiIcon::Bilevel, "Bilevel\nMeasurements"));
        });
    }
    let mut out = vec![];
    for a in b.take_actions() {
        match a {
            StripAction::ScopeSettings => out.push(ScopeAction::Parameters),
            StripAction::Run => out.push(ScopeAction::Run),
            StripAction::Stop => out.push(ScopeAction::Stop),
            StripAction::ScopeZoomIn => view.mode = ScopeMode::ZoomIn,
            StripAction::ScopeZoomX => view.mode = ScopeMode::ZoomX,
            StripAction::ScopeZoomY => view.mode = ScopeMode::ZoomY,
            StripAction::ScopePan => view.mode = ScopeMode::Pan,
            StripAction::ScopeAutoscale => view.autoscale = true,
            StripAction::ScopeLegend => view.legend = !view.legend,
            _ => {}
        }
    }
    out
}

fn status_bar(ui: &mut egui::Ui, info: &ScopeInfo) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    let font = FontId::proportional(12.0);
    let left = if info.running { "Running" } else { "Ready" };
    painter.text(
        Pos2::new(rect.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        left,
        font.clone(),
        theme::TEXT,
    );
    let t = info
        .sim_time
        .map(|t| format!("T={t:.3}"))
        .unwrap_or_else(|| "T=0.000".into());
    painter.text(
        Pos2::new(rect.right() - 10.0, rect.center().y),
        Align2::RIGHT_CENTER,
        t,
        font.clone(),
        theme::TEXT,
    );
    painter.text(
        Pos2::new(rect.right() - 110.0, rect.center().y),
        Align2::RIGHT_CENTER,
        "Sample based",
        font,
        theme::TEXT,
    );
}

fn paint_legend(painter: &egui::Painter, frame: Rect, series: &[ScopeSeries]) {
    if series.is_empty() {
        return;
    }
    let font = FontId::proportional(12.0);
    let w = series
        .iter()
        .map(|s| {
            painter
                .layout_no_wrap(s.label.clone(), font.clone(), AXES_FG)
                .size()
                .x
        })
        .fold(0.0, f32::max)
        + 44.0;
    let h = series.len() as f32 * 18.0 + 8.0;
    let r = Rect::from_min_size(
        Pos2::new(frame.right() - w - 8.0, frame.top() + 8.0),
        Vec2::new(w, h),
    );
    painter.rect(
        r,
        CornerRadius::ZERO,
        Color32::from_rgb(0x1E, 0x1E, 0x1E),
        Stroke::new(1.0, Color32::from_rgb(0x70, 0x70, 0x70)),
        StrokeKind::Inside,
    );
    for (i, s) in series.iter().enumerate() {
        let y = r.top() + 13.0 + i as f32 * 18.0;
        painter.line_segment(
            [Pos2::new(r.left() + 6.0, y), Pos2::new(r.left() + 30.0, y)],
            Stroke::new(1.5, signal_color(i)),
        );
        painter.text(
            Pos2::new(r.left() + 36.0, y),
            Align2::LEFT_CENTER,
            &s.label,
            font.clone(),
            AXES_FG,
        );
    }
}

fn axes(
    ui: &mut egui::Ui,
    sid: BlockId,
    series: &[ScopeSeries],
    view: &mut ScopeView,
    reset: bool,
) {
    // Dark plot style for this scope only.
    let v = ui.visuals_mut();
    v.extreme_bg_color = AXES_BG;
    v.override_text_color = Some(AXES_FG);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, AXES_FG);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, AXES_FG);
    v.window_fill = Color32::from_rgb(0x1E, 0x1E, 0x1E);
    v.window_stroke = Stroke::new(1.0, Color32::from_rgb(0x70, 0x70, 0x70));
    if reset || view.autoscale {
        view.autoscale = false;
        view.pending = autoscale_bounds(series).map(|(x0, x1, y0, y1)| [x0, x1, y0, y1]);
    }
    let pan = view.mode == ScopeMode::Pan;
    let pending = view.pending.take();
    let resp = egui_plot::Plot::new(("scope_plot", sid.0))
        .show_background(true)
        .grid_color(GRID)
        .grid_spacing(egui::Rangef::new(4.0, 8.0))
        .x_grid_spacer(|i| scope_grid(i, 10.0))
        .y_grid_spacer(|i| scope_grid(i, 8.0))
        .custom_x_axes(vec![egui_plot::AxisHints::new_x()
            .label_spacing(egui::Rangef::new(28.0, 36.0))
            .tick_label_color(AXES_FG)])
        .custom_y_axes(vec![egui_plot::AxisHints::new_y().tick_label_color(AXES_FG)])
        .show_x(false)
        .show_y(false)
        .allow_drag(pan)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .show(ui, |p| {
            if let Some([x0, x1, y0, y1]) = pending {
                p.set_plot_bounds(egui_plot::PlotBounds::from_min_max([x0, y0], [x1, y1]));
            }
            for (i, s) in series.iter().enumerate() {
                p.line(
                    egui_plot::Line::new(
                        s.label.clone(),
                        egui_plot::PlotPoints::from(s.points.clone()),
                    )
                    .color(signal_color(i))
                    .width(1.5),
                );
            }
        });
    let frame = *resp.transform.frame();
    let painter = ui.painter_at(frame);
    let r = &resp.response;
    let (origin, latest) = ui.input(|i| (i.pointer.press_origin(), i.pointer.latest_pos()));
    if !pan {
        if r.drag_started() {
            view.press = origin.or(latest);
        }
        let gesture = if r.clicked() {
            latest.map(|l| (l, l))
        } else if r.drag_stopped() {
            match (view.press.take(), latest) {
                (Some(o), Some(l)) => Some((o, l)),
                _ => None,
            }
        } else {
            None
        };
        if let Some((o, l)) = gesture {
            let a = resp.transform.value_from_position(o);
            let b = resp.transform.value_from_position(l);
            let bb = resp.transform.bounds();
            let cur = [bb.min()[0], bb.max()[0], bb.min()[1], bb.max()[1]];
            view.pending = zoom_bounds(view.mode, [a.x, a.y], [b.x, b.y], cur);
            ui.ctx().request_repaint();
        } else if let (true, Some(o), Some(l)) = (r.dragged(), view.press, latest) {
            // Rubber band while zooming.
            let band = match view.mode {
                ScopeMode::ZoomX => {
                    Rect::from_two_pos(Pos2::new(o.x, frame.top()), Pos2::new(l.x, frame.bottom()))
                }
                ScopeMode::ZoomY => {
                    Rect::from_two_pos(Pos2::new(frame.left(), o.y), Pos2::new(frame.right(), l.y))
                }
                _ => Rect::from_two_pos(o, l),
            };
            painter.rect(
                band,
                CornerRadius::ZERO,
                Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 20),
                Stroke::new(1.0, Color32::WHITE),
                StrokeKind::Inside,
            );
        }
    }
    painter.rect_stroke(
        frame,
        CornerRadius::ZERO,
        Stroke::new(1.0, AXES_FG),
        StrokeKind::Inside,
    );
    if view.legend {
        paint_legend(&painter, frame, series);
    }
}

/// Shows the Scope window of block `sid` (a separate native window when the
/// platform supports it). `reset_view` autoscales to the data.
#[allow(clippy::too_many_arguments)]
pub fn show_scope_window(
    ctx: &egui::Context,
    sid: BlockId,
    title: &str,
    series: &[ScopeSeries],
    open: &mut bool,
    reset_view: bool,
    view: &mut ScopeView,
    info: &ScopeInfo,
) -> Vec<ScopeAction> {
    let vid = egui::ViewportId::from_hash_of(("scope", sid.0));
    let builder = egui::ViewportBuilder::default()
        .with_title(title)
        .with_inner_size([620.0, 460.0])
        .with_min_inner_size([360.0, 280.0]);
    ctx.show_viewport_immediate(vid, builder, |ui, _class| {
        if ui.ctx().input(|i| i.viewport().close_requested()) {
            *open = false;
        }
        let actions = egui::Panel::top(egui::Id::new(("scope_strip", sid.0)))
            .frame(egui::Frame::NONE)
            .resizable(false)
            .show(ui, |ui| scope_toolstrip(ui, view, info))
            .inner;
        egui::Panel::bottom(egui::Id::new(("scope_status", sid.0)))
            .frame(
                egui::Frame::NONE
                    .fill(theme::STATUS_BG)
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .exact_size(22.0)
            .resizable(false)
            .show(ui, |ui| status_bar(ui, info));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(FIGURE_BG)
                    .inner_margin(egui::Margin {
                        left: 6,
                        right: 14,
                        top: 12,
                        bottom: 6,
                    }),
            )
            .show(ui, |ui| {
                if series.is_empty() {
                    let r = ui.max_rect();
                    ui.painter().rect_filled(r, CornerRadius::ZERO, AXES_BG);
                    ui.painter().text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        "No data. Press Run.",
                        FontId::proportional(13.0),
                        AXES_FG,
                    );
                } else {
                    axes(ui, sid, series, view, reset_view);
                }
            });
        actions
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_signal_is_yellow_and_colours_cycle() {
        assert_eq!(signal_color(0), Color32::from_rgb(0xFF, 0xFF, 0x11));
        assert_eq!(signal_color(7), signal_color(0));
    }

    #[test]
    fn nice_steps() {
        assert_eq!(nice_step(2.0, 10.0), 0.2);
        assert_eq!(nice_step(10.0, 10.0), 1.0);
        assert_eq!(nice_step(0.9, 8.0), 0.2);
        assert_eq!(nice_step(350.0, 10.0), 50.0);
    }

    #[test]
    fn zoom_box_and_click() {
        let cur = [0.0, 10.0, -1.0, 1.0];
        assert_eq!(
            zoom_bounds(ScopeMode::ZoomIn, [2.0, 0.5], [4.0, -0.5], cur),
            Some([2.0, 4.0, -0.5, 0.5])
        );
        assert_eq!(
            zoom_bounds(ScopeMode::ZoomX, [2.0, 0.5], [4.0, -0.5], cur),
            Some([2.0, 4.0, -1.0, 1.0])
        );
        assert_eq!(
            zoom_bounds(ScopeMode::ZoomY, [2.0, 0.5], [4.0, -0.5], cur),
            Some([0.0, 10.0, -0.5, 0.5])
        );
        // Click zooms ×2 about the point.
        assert_eq!(
            zoom_bounds(ScopeMode::ZoomIn, [5.0, 0.0], [5.0, 0.0], cur),
            Some([2.5, 7.5, -0.5, 0.5])
        );
        assert_eq!(
            zoom_bounds(ScopeMode::Pan, [1.0, 0.0], [2.0, 0.5], cur),
            None
        );
    }
}
