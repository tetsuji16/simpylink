//! Scope window data and view.
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

/// Shows a Scope window. Returns true if the user pressed "Parameters…".
pub fn show_scope_window(
    ctx: &egui::Context,
    sid: BlockId,
    title: &str,
    series: &[ScopeSeries],
    open: &mut bool,
    reset_view: bool,
) -> bool {
    let mut params = false;
    egui::Window::new(title)
        .id(egui::Id::new(("scope", sid.0)))
        .open(open)
        .default_size([480.0, 320.0])
        .show(ctx, |ui| {
            let mut reset = reset_view;
            ui.horizontal(|ui| {
                if ui.button("Autoscale").clicked() {
                    reset = true;
                }
                if ui.button("Parameters…").clicked() {
                    params = true;
                }
            });
            if series.is_empty() {
                ui.label("No simulation data. Press Run (Ctrl+T).");
                return;
            }
            egui_plot::Plot::new(("scope_plot", sid.0))
                .legend(egui_plot::Legend::default())
                .show(ui, |p| {
                    for s in series {
                        p.line(egui_plot::Line::new(
                            s.label.clone(),
                            egui_plot::PlotPoints::from(s.points.clone()),
                        ));
                    }
                    if reset {
                        if let Some((x0, x1, y0, y1)) = autoscale_bounds(series) {
                            p.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                                [x0, y0],
                                [x1, y1],
                            ));
                        }
                    }
                });
        });
    params
}
