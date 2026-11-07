//! Vector icons for the toolstrip, explorer bar and canvas palette.
//!
//! All icons are original drawings on a 32×32 design grid, scaled to the
//! target rectangle; they are inspired by, not copied from, Simulink's icons.
use crate::theme;
use egui::{Color32, CornerRadius, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

/// Icon identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiIcon {
    New,
    Open,
    Save,
    Print,
    LibraryBrowser,
    ModelSettings,
    LogSignals,
    StepBack,
    Run,
    StepForward,
    Stop,
    DataInspector,
    UpdateModel,
    Diagnostics,
    Subsystem,
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
    Back,
    Forward,
    Up,
    Search,
    Help,
    Annotation,
    Image,
    Area,
    Camera,
    HidePanel,
    Model,
    FastRestart,
    ZoomX,
    ZoomY,
    Pan,
    Legend,
    Style,
    Cursors,
    Statistics,
    PeakFinder,
    Bilevel,
}

/// Grey, lightened version of a colour for disabled icons (white stays white).
pub fn disabled_tone(c: Color32) -> Color32 {
    let l = 0.3 * c.r() as f32 + 0.59 * c.g() as f32 + 0.11 * c.b() as f32;
    let v = (175.0 + l * 80.0 / 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(v, v, v, c.a())
}

/// Paints `icon` into `r` (any size; drawn on a 32×32 grid). `enabled = false` greys it out.
pub fn paint_ui_icon(painter: &egui::Painter, r: Rect, icon: UiIcon, enabled: bool) {
    let s = r.width().min(r.height()) / 32.0;
    let o = r.center() - Vec2::splat(16.0 * s);
    let p = |x: f32, y: f32| Pos2::new(o.x + x * s, o.y + y * s);
    let rr = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(p(x0, y0), p(x1, y1));
    let g = |c: Color32| if enabled { c } else { disabled_tone(c) };
    let ink = g(Color32::from_rgb(0x44, 0x44, 0x44));
    let blue = g(theme::ICON_BLUE);
    let orange = g(theme::ICON_ORANGE);
    let w = (1.6 * s).max(1.0);
    let st = |c: Color32| Stroke::new(w, c);
    let fill_rect = |x0, y0, x1, y1, f: Color32, c: Color32| {
        painter.rect(
            rr(x0, y0, x1, y1),
            CornerRadius::same((1.5 * s) as u8),
            f,
            st(c),
            StrokeKind::Inside,
        );
    };
    let poly = |pts: Vec<Pos2>, f: Color32, c: Stroke| {
        painter.add(Shape::convex_polygon(pts, f, c));
    };
    let line = |pts: Vec<Pos2>, c: Color32| {
        painter.add(Shape::line(pts, st(c)));
    };
    let paper = g(Color32::WHITE);
    let doc = |c: Color32| {
        poly(
            vec![
                p(7.0, 3.0),
                p(20.0, 3.0),
                p(25.0, 8.0),
                p(25.0, 29.0),
                p(7.0, 29.0),
            ],
            paper,
            st(c),
        );
        line(vec![p(20.0, 3.0), p(20.0, 8.0), p(25.0, 8.0)], c);
    };
    match icon {
        UiIcon::New => {
            doc(ink);
            painter.line_segment([p(11.0, 18.0), p(21.0, 18.0)], Stroke::new(3.0 * s, orange));
            painter.line_segment([p(16.0, 13.0), p(16.0, 23.0)], Stroke::new(3.0 * s, orange));
        }
        UiIcon::Open => {
            poly(
                vec![
                    p(3.0, 8.0),
                    p(12.0, 8.0),
                    p(14.0, 11.0),
                    p(27.0, 11.0),
                    p(27.0, 26.0),
                    p(3.0, 26.0),
                ],
                g(Color32::from_rgb(0xF2, 0xC5, 0x5C)),
                st(g(Color32::from_rgb(0xB8, 0x86, 0x1B))),
            );
            poly(
                vec![p(6.0, 15.0), p(30.0, 15.0), p(27.0, 26.0), p(3.0, 26.0)],
                g(Color32::from_rgb(0xFA, 0xD9, 0x7E)),
                st(g(Color32::from_rgb(0xB8, 0x86, 0x1B))),
            );
        }
        UiIcon::Save => {
            poly(
                vec![
                    p(5.0, 5.0),
                    p(23.0, 5.0),
                    p(27.0, 9.0),
                    p(27.0, 27.0),
                    p(5.0, 27.0),
                ],
                blue,
                Stroke::NONE,
            );
            fill_rect(
                10.0,
                5.0,
                22.0,
                12.0,
                g(Color32::from_rgb(0xD8, 0xDE, 0xE6)),
                g(Color32::from_rgb(0xD8, 0xDE, 0xE6)),
            );
            fill_rect(18.0, 6.0, 20.5, 11.0, blue, blue);
            fill_rect(9.0, 17.0, 23.0, 27.0, paper, paper);
        }
        UiIcon::Print => {
            fill_rect(9.0, 4.0, 23.0, 12.0, paper, ink);
            fill_rect(
                4.0,
                11.0,
                28.0,
                23.0,
                g(Color32::from_rgb(0x9A, 0x9A, 0x9A)),
                ink,
            );
            fill_rect(9.0, 18.0, 23.0, 28.0, paper, ink);
            line(vec![p(12.0, 22.0), p(20.0, 22.0)], ink);
            line(vec![p(12.0, 25.0), p(20.0, 25.0)], ink);
        }
        UiIcon::LibraryBrowser => {
            fill_rect(
                4.0,
                4.0,
                15.0,
                15.0,
                g(Color32::from_rgb(0xD9, 0x53, 0x19)),
                ink,
            );
            fill_rect(17.0, 4.0, 28.0, 15.0, blue, ink);
            fill_rect(4.0, 17.0, 15.0, 28.0, orange, ink);
            fill_rect(
                17.0,
                17.0,
                28.0,
                28.0,
                g(Color32::from_rgb(0x77, 0xAC, 0x30)),
                ink,
            );
        }
        UiIcon::ModelSettings => {
            let c = p(16.0, 16.0);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin());
                painter.line_segment(
                    [c + d * 8.0 * s, c + d * 13.0 * s],
                    Stroke::new(4.0 * s, ink),
                );
            }
            painter.circle(c, 9.5 * s, ink, Stroke::NONE);
            painter.circle(c, 4.0 * s, paper, Stroke::NONE);
        }
        UiIcon::LogSignals => {
            let c = p(16.0, 24.0);
            for (i, rad) in [5.0f32, 10.0, 15.0].iter().enumerate() {
                let pts: Vec<Pos2> = (0..=12)
                    .map(|k| {
                        let a = std::f32::consts::PI * (1.25 + 0.5 * k as f32 / 12.0);
                        c + Vec2::new(a.cos(), a.sin()) * *rad * s
                    })
                    .collect();
                painter.add(Shape::line(
                    pts,
                    Stroke::new(w * (1.0 + i as f32 * 0.2), blue),
                ));
            }
            painter.circle_filled(c, 2.5 * s, blue);
        }
        UiIcon::Run => {
            painter.circle(
                p(16.0, 16.0),
                13.0 * s,
                g(theme::RUN_GREEN),
                Stroke::new(w, g(Color32::from_rgb(0x1E, 0x6E, 0x2E))),
            );
            poly(
                vec![p(12.5, 9.5), p(23.0, 16.0), p(12.5, 22.5)],
                paper,
                Stroke::NONE,
            );
        }
        UiIcon::Stop => {
            let red = if enabled {
                Color32::from_rgb(0xC8, 0x32, 0x32)
            } else {
                Color32::from_rgb(0xC8, 0xC8, 0xC8)
            };
            painter.circle(
                p(16.0, 16.0),
                13.0 * s,
                red,
                Stroke::new(w, red.gamma_multiply(0.8)),
            );
            painter.rect_filled(rr(11.0, 11.0, 21.0, 21.0), CornerRadius::ZERO, paper);
        }
        UiIcon::StepForward | UiIcon::StepBack => {
            let c = g(Color32::from_rgb(0x2E, 0x8B, 0x57));
            let (bar, tri) = if icon == UiIcon::StepForward {
                (
                    rr(6.0, 6.0, 10.0, 26.0),
                    vec![p(13.0, 6.0), p(27.0, 16.0), p(13.0, 26.0)],
                )
            } else {
                (
                    rr(22.0, 6.0, 26.0, 26.0),
                    vec![p(19.0, 6.0), p(5.0, 16.0), p(19.0, 26.0)],
                )
            };
            painter.rect(
                bar,
                CornerRadius::ZERO,
                c.gamma_multiply(0.25),
                st(c),
                StrokeKind::Inside,
            );
            poly(tri, c.gamma_multiply(0.25), st(c));
        }
        UiIcon::FastRestart => {
            line(
                vec![p(5.0, 20.0), p(10.0, 12.0), p(15.0, 20.0), p(20.0, 12.0)],
                blue,
            );
            poly(
                vec![p(20.0, 9.0), p(27.0, 16.0), p(20.0, 23.0)],
                blue,
                Stroke::NONE,
            );
        }
        UiIcon::DataInspector => {
            fill_rect(4.0, 5.0, 28.0, 27.0, paper, ink);
            line(
                vec![
                    p(7.0, 22.0),
                    p(12.0, 12.0),
                    p(17.0, 19.0),
                    p(21.0, 9.0),
                    p(25.0, 15.0),
                ],
                blue,
            );
            line(
                vec![p(7.0, 19.0), p(12.0, 20.0), p(17.0, 14.0), p(25.0, 21.0)],
                orange,
            );
        }
        UiIcon::UpdateModel => {
            let c = p(16.0, 16.0);
            for half in [0.0f32, std::f32::consts::PI] {
                let pts: Vec<Pos2> = (0..=10)
                    .map(|k| {
                        let a = half + 0.3 + 2.3 * k as f32 / 10.0;
                        c + Vec2::new(a.cos(), a.sin()) * 10.0 * s
                    })
                    .collect();
                let end = pts[pts.len() - 1];
                painter.add(Shape::line(pts, Stroke::new(2.5 * s, blue)));
                let a = half + 2.6;
                let t = Vec2::new(-a.sin(), a.cos());
                let n = Vec2::new(a.cos(), a.sin());
                poly(
                    vec![
                        end + t * 4.0 * s,
                        end + n * 4.0 * s - t * 2.0 * s,
                        end - n * 4.0 * s - t * 2.0 * s,
                    ],
                    blue,
                    Stroke::NONE,
                );
            }
        }
        UiIcon::Diagnostics => {
            poly(
                vec![p(16.0, 4.0), p(29.0, 27.0), p(3.0, 27.0)],
                g(Color32::from_rgb(0xF5, 0xC2, 0x1B)),
                st(g(Color32::from_rgb(0xA0, 0x7A, 0x00))),
            );
            painter.line_segment([p(16.0, 11.0), p(16.0, 20.0)], Stroke::new(2.5 * s, ink));
            painter.circle_filled(p(16.0, 23.5), 1.5 * s, ink);
        }
        UiIcon::Subsystem | UiIcon::Model => {
            fill_rect(8.0, 7.0, 24.0, 25.0, paper, ink);
            poly(
                vec![p(4.0, 13.0), p(8.0, 16.0), p(4.0, 19.0)],
                ink,
                Stroke::NONE,
            );
            poly(
                vec![p(24.0, 13.0), p(28.0, 16.0), p(24.0, 19.0)],
                ink,
                Stroke::NONE,
            );
            if icon == UiIcon::Subsystem {
                fill_rect(
                    11.0,
                    11.0,
                    21.0,
                    21.0,
                    g(Color32::from_rgb(0xDD, 0xE8, 0xF3)),
                    blue,
                );
            }
        }
        UiIcon::Undo | UiIcon::Redo => {
            let flip = if icon == UiIcon::Undo { 1.0 } else { -1.0 };
            let q = |x: f32, y: f32| p(16.0 + (x - 16.0) * flip, y);
            let pts: Vec<Pos2> = (0..=12)
                .map(|k| {
                    let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / 12.0;
                    let c = q(16.0, 18.0);
                    c + Vec2::new(a.cos() * 9.0 * s * flip, a.sin() * 8.0 * s)
                })
                .collect();
            painter.add(Shape::line(pts, Stroke::new(2.5 * s, blue)));
            painter.line_segment([q(16.0, 26.0), q(10.0, 26.0)], Stroke::new(2.5 * s, blue));
            poly(
                vec![q(16.0, 4.0), q(16.0, 16.0), q(8.0, 10.0)],
                blue,
                Stroke::NONE,
            );
        }
        UiIcon::Cut => {
            painter.circle_stroke(p(10.0, 24.0), 4.0 * s, st(ink));
            painter.circle_stroke(p(22.0, 24.0), 4.0 * s, st(ink));
            line(vec![p(12.5, 21.0), p(22.0, 4.0)], ink);
            line(vec![p(19.5, 21.0), p(10.0, 4.0)], ink);
        }
        UiIcon::Copy => {
            fill_rect(5.0, 4.0, 19.0, 22.0, paper, ink);
            fill_rect(13.0, 10.0, 27.0, 28.0, paper, ink);
        }
        UiIcon::Paste => {
            fill_rect(
                5.0,
                6.0,
                23.0,
                28.0,
                g(Color32::from_rgb(0xC9, 0x9A, 0x5B)),
                ink,
            );
            fill_rect(10.0, 3.0, 18.0, 8.0, paper, ink);
            fill_rect(14.0, 12.0, 27.0, 29.0, paper, ink);
        }
        UiIcon::Delete => {
            let red = g(Color32::from_rgb(0xC8, 0x32, 0x32));
            painter.line_segment([p(8.0, 8.0), p(24.0, 24.0)], Stroke::new(3.5 * s, red));
            painter.line_segment([p(24.0, 8.0), p(8.0, 24.0)], Stroke::new(3.5 * s, red));
        }
        UiIcon::SelectAll => {
            for r2 in [rr(5.0, 5.0, 27.0, 27.0)] {
                for sh in Shape::dashed_line(
                    &[
                        r2.left_top(),
                        r2.right_top(),
                        r2.right_bottom(),
                        r2.left_bottom(),
                        r2.left_top(),
                    ],
                    st(ink),
                    3.0 * s,
                    2.0 * s,
                ) {
                    painter.add(sh);
                }
            }
            fill_rect(
                10.0,
                10.0,
                22.0,
                22.0,
                g(Color32::from_rgb(0xDD, 0xE8, 0xF3)),
                blue,
            );
        }
        UiIcon::ZoomIn | UiIcon::ZoomOut | UiIcon::Search => {
            painter.circle(p(13.0, 13.0), 8.5 * s, paper, Stroke::new(2.2 * s, ink));
            painter.line_segment([p(19.0, 19.0), p(27.0, 27.0)], Stroke::new(3.5 * s, ink));
            if icon != UiIcon::Search {
                painter.line_segment([p(9.0, 13.0), p(17.0, 13.0)], Stroke::new(2.0 * s, blue));
                if icon == UiIcon::ZoomIn {
                    painter.line_segment([p(13.0, 9.0), p(13.0, 17.0)], Stroke::new(2.0 * s, blue));
                }
            }
        }
        UiIcon::Zoom100 => {
            fill_rect(4.0, 7.0, 28.0, 25.0, paper, ink);
            painter.text(
                p(16.0, 16.0),
                egui::Align2::CENTER_CENTER,
                "1:1",
                egui::FontId::proportional(10.0 * s),
                blue,
            );
        }
        UiIcon::FitView => {
            for (a, b, c) in [
                ((4.0, 11.0), (4.0, 4.0), (11.0, 4.0)),
                ((21.0, 4.0), (28.0, 4.0), (28.0, 11.0)),
                ((28.0, 21.0), (28.0, 28.0), (21.0, 28.0)),
                ((11.0, 28.0), (4.0, 28.0), (4.0, 21.0)),
            ] {
                painter.add(Shape::line(
                    vec![p(a.0, a.1), p(b.0, b.1), p(c.0, c.1)],
                    Stroke::new(2.2 * s, ink),
                ));
            }
            fill_rect(
                10.0,
                11.0,
                22.0,
                21.0,
                g(Color32::from_rgb(0xDD, 0xE8, 0xF3)),
                blue,
            );
        }
        UiIcon::PythonCode | UiIcon::ExportPython => {
            doc(ink);
            fill_rect(10.0, 12.0, 16.0, 18.0, blue, blue);
            fill_rect(
                16.0,
                18.0,
                22.0,
                24.0,
                g(Color32::from_rgb(0xF2, 0xC8, 0x2F)),
                g(Color32::from_rgb(0xF2, 0xC8, 0x2F)),
            );
            if icon == UiIcon::ExportPython {
                painter.line_segment([p(20.0, 26.0), p(30.0, 26.0)], Stroke::new(2.5 * s, orange));
                poly(
                    vec![p(27.0, 22.0), p(31.0, 26.0), p(27.0, 30.0)],
                    orange,
                    Stroke::NONE,
                );
            }
        }
        UiIcon::ExportSlx => {
            doc(ink);
            painter.text(
                p(16.0, 19.0),
                egui::Align2::CENTER_CENTER,
                "slx",
                egui::FontId::proportional(8.0 * s),
                blue,
            );
            painter.line_segment([p(20.0, 26.0), p(30.0, 26.0)], Stroke::new(2.5 * s, orange));
            poly(
                vec![p(27.0, 22.0), p(31.0, 26.0), p(27.0, 30.0)],
                orange,
                Stroke::NONE,
            );
        }
        UiIcon::Back | UiIcon::Forward | UiIcon::Up => {
            let c = g(Color32::from_rgb(0x8A, 0x8A, 0x8A));
            let pts = match icon {
                UiIcon::Back => vec![
                    p(4.0, 16.0),
                    p(15.0, 6.0),
                    p(15.0, 11.0),
                    p(28.0, 11.0),
                    p(28.0, 21.0),
                    p(15.0, 21.0),
                    p(15.0, 26.0),
                ],
                UiIcon::Forward => vec![
                    p(28.0, 16.0),
                    p(17.0, 6.0),
                    p(17.0, 11.0),
                    p(4.0, 11.0),
                    p(4.0, 21.0),
                    p(17.0, 21.0),
                    p(17.0, 26.0),
                ],
                _ => vec![
                    p(16.0, 4.0),
                    p(26.0, 15.0),
                    p(21.0, 15.0),
                    p(21.0, 28.0),
                    p(11.0, 28.0),
                    p(11.0, 15.0),
                    p(6.0, 15.0),
                ],
            };
            painter.add(Shape::closed_line(pts.clone(), Stroke::new(1.2 * s, c)));
            // Fill as two convex parts (arrow head + shaft).
            match icon {
                UiIcon::Back => {
                    poly(
                        vec![pts[0], pts[1], pts[6]],
                        c.gamma_multiply(0.35),
                        Stroke::NONE,
                    );
                    painter.rect_filled(
                        Rect::from_min_max(pts[2], pts[4]),
                        CornerRadius::ZERO,
                        c.gamma_multiply(0.35),
                    );
                }
                UiIcon::Forward => {
                    poly(
                        vec![pts[0], pts[6], pts[1]],
                        c.gamma_multiply(0.35),
                        Stroke::NONE,
                    );
                    painter.rect_filled(
                        Rect::from_two_pos(pts[2], pts[4]),
                        CornerRadius::ZERO,
                        c.gamma_multiply(0.35),
                    );
                }
                _ => {
                    poly(
                        vec![pts[0], pts[1], pts[6]],
                        c.gamma_multiply(0.35),
                        Stroke::NONE,
                    );
                    painter.rect_filled(
                        Rect::from_two_pos(pts[3], pts[5]),
                        CornerRadius::ZERO,
                        c.gamma_multiply(0.35),
                    );
                }
            }
        }
        UiIcon::ZoomX | UiIcon::ZoomY => {
            painter.circle(p(14.0, 14.0), 8.5 * s, paper, Stroke::new(2.2 * s, ink));
            painter.line_segment([p(20.0, 20.0), p(28.0, 28.0)], Stroke::new(3.5 * s, ink));
            let (a, b) = if icon == UiIcon::ZoomX {
                (p(8.5, 14.0), p(19.5, 14.0))
            } else {
                (p(14.0, 8.5), p(14.0, 19.5))
            };
            painter.line_segment([a, b], Stroke::new(1.8 * s, blue));
            for (tip, back) in [(a, b), (b, a)] {
                let d = (tip - back).normalized();
                let n = Vec2::new(-d.y, d.x);
                poly(
                    vec![
                        tip,
                        tip - d * 3.0 * s + n * 2.2 * s,
                        tip - d * 3.0 * s - n * 2.2 * s,
                    ],
                    blue,
                    Stroke::NONE,
                );
            }
        }
        UiIcon::Pan => {
            let c = p(16.0, 16.0);
            for d in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
                let tip = c + d * 13.0 * s;
                painter.line_segment([c, tip], Stroke::new(2.0 * s, ink));
                let n = Vec2::new(-d.y, d.x);
                poly(
                    vec![
                        tip,
                        tip - d * 5.0 * s + n * 3.5 * s,
                        tip - d * 5.0 * s - n * 3.5 * s,
                    ],
                    ink,
                    Stroke::NONE,
                );
            }
        }
        UiIcon::Legend => {
            fill_rect(4.0, 6.0, 28.0, 26.0, paper, ink);
            for (i, c) in [blue, orange, g(Color32::from_rgb(0x77, 0xAC, 0x30))]
                .iter()
                .enumerate()
            {
                let y = 11.0 + i as f32 * 5.0;
                painter.line_segment([p(8.0, y), p(14.0, y)], Stroke::new(2.0 * s, *c));
                painter.line_segment([p(16.0, y), p(24.0, y)], Stroke::new(1.0 * s, ink));
            }
        }
        UiIcon::Style => {
            fill_rect(
                4.0,
                6.0,
                28.0,
                26.0,
                g(Color32::from_rgb(0x20, 0x20, 0x20)),
                ink,
            );
            line(
                vec![p(7.0, 21.0), p(13.0, 12.0), p(19.0, 18.0), p(25.0, 10.0)],
                g(Color32::from_rgb(0xF0, 0xE0, 0x20)),
            );
        }
        UiIcon::Cursors => {
            fill_rect(4.0, 6.0, 28.0, 26.0, paper, ink);
            line(
                vec![p(6.0, 22.0), p(12.0, 13.0), p(18.0, 19.0), p(26.0, 10.0)],
                blue,
            );
            painter.line_segment([p(12.0, 6.0), p(12.0, 26.0)], Stroke::new(1.2 * s, orange));
            painter.circle_filled(p(12.0, 13.0), 2.0 * s, orange);
        }
        UiIcon::Statistics => {
            fill_rect(4.0, 6.0, 28.0, 26.0, paper, ink);
            painter.text(
                p(16.0, 16.0),
                egui::Align2::CENTER_CENTER,
                "μσ",
                egui::FontId::proportional(11.0 * s),
                blue,
            );
        }
        UiIcon::PeakFinder => {
            fill_rect(4.0, 6.0, 28.0, 26.0, paper, ink);
            line(
                vec![
                    p(6.0, 23.0),
                    p(11.0, 18.0),
                    p(15.0, 10.0),
                    p(19.0, 18.0),
                    p(26.0, 21.0),
                ],
                blue,
            );
            poly(
                vec![p(15.0, 8.0), p(12.5, 4.0), p(17.5, 4.0)],
                orange,
                Stroke::NONE,
            );
        }
        UiIcon::Bilevel => {
            fill_rect(4.0, 6.0, 28.0, 26.0, paper, ink);
            line(
                vec![
                    p(6.0, 22.0),
                    p(11.0, 22.0),
                    p(13.0, 11.0),
                    p(20.0, 11.0),
                    p(22.0, 22.0),
                    p(26.0, 22.0),
                ],
                blue,
            );
        }
        UiIcon::Help => {
            painter.circle(p(16.0, 16.0), 12.0 * s, blue, Stroke::NONE);
            painter.text(
                p(16.0, 16.5),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(17.0 * s),
                paper,
            );
        }
        UiIcon::Annotation => {
            fill_rect(4.0, 6.0, 28.0, 26.0, paper, ink);
            painter.text(
                p(16.0, 16.0),
                egui::Align2::CENTER_CENTER,
                "A≡",
                egui::FontId::proportional(12.0 * s),
                ink,
            );
        }
        UiIcon::Image => {
            fill_rect(4.0, 6.0, 28.0, 26.0, paper, ink);
            poly(
                vec![p(7.0, 23.0), p(14.0, 13.0), p(19.0, 23.0)],
                blue,
                Stroke::NONE,
            );
            poly(
                vec![p(15.0, 23.0), p(20.0, 16.0), p(25.0, 23.0)],
                g(Color32::from_rgb(0x77, 0xAC, 0x30)),
                Stroke::NONE,
            );
            painter.circle_filled(p(21.0, 11.0), 2.0 * s, orange);
        }
        UiIcon::Area => {
            fill_rect(
                4.0,
                6.0,
                28.0,
                26.0,
                g(Color32::from_rgb(0xEE, 0xEE, 0xEE)),
                ink,
            );
            fill_rect(
                4.0,
                6.0,
                28.0,
                11.0,
                g(Color32::from_rgb(0xCC, 0xCC, 0xCC)),
                ink,
            );
        }
        UiIcon::Camera => {
            fill_rect(
                4.0,
                10.0,
                28.0,
                26.0,
                g(Color32::from_rgb(0x88, 0x88, 0x88)),
                ink,
            );
            fill_rect(
                11.0,
                6.0,
                21.0,
                10.0,
                g(Color32::from_rgb(0x88, 0x88, 0x88)),
                ink,
            );
            painter.circle(p(16.0, 18.0), 5.0 * s, paper, st(ink));
        }
        UiIcon::HidePanel => {
            fill_rect(4.0, 6.0, 28.0, 26.0, paper, ink);
            fill_rect(
                4.0,
                6.0,
                12.0,
                26.0,
                g(Color32::from_rgb(0xDD, 0xE8, 0xF3)),
                ink,
            );
        }
    }
}
