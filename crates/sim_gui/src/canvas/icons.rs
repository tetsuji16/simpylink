//! Block shapes and icons drawn in screen space, in the style of the Simulink
//! editor: white blocks with thin black outlines, triangular Gain, rounded
//! In/Out ports, round Sum with signs next to the ports, fraction-style
//! transfer functions (`1 / s+1`), and port chevrons only on unconnected ports.
use crate::theme::diagram_family;
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};
use sim_core::model::Block;

/// Ink colour of blocks and lines.
pub const INK: Color32 = Color32::BLACK;

fn short(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
}

fn param<'a>(b: &'a Block, k: &str) -> &'a str {
    b.params.get(k).map(String::as_str).unwrap_or("")
}

/// Formats a number compactly ("1", "0.5", "1e-05").
pub fn fmt_num(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e9 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.4}");
        let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        if s.is_empty() || s == "0" || s == "-0" {
            format!("{v:e}")
        } else {
            s
        }
    }
}

fn superscript(n: usize) -> String {
    const D: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    n.to_string()
        .chars()
        .map(|c| c.to_digit(10).map(|d| D[d as usize]).unwrap_or(c))
        .collect()
}

/// Parses a numeric row vector literal such as `[1 2 1]`, `[1, 0.5]` or `3`.
pub fn parse_vector(src: &str) -> Option<Vec<f64>> {
    let t = src.trim();
    let inner = t
        .strip_prefix('[')
        .and_then(|x| x.strip_suffix(']'))
        .unwrap_or(t);
    let v: Option<Vec<f64>> = inner
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|x| !x.is_empty())
        .map(|x| x.parse::<f64>().ok())
        .collect();
    v.filter(|v| !v.is_empty())
}

/// Polynomial in `var` from descending coefficients: `[1 2 1]` → `s²+2s+1`.
/// Returns the source text unchanged when it is not a numeric vector.
pub fn polynomial(src: &str, var: &str) -> String {
    let Some(c) = parse_vector(src) else {
        return src.trim().to_string();
    };
    let n = c.len() - 1;
    let mut out = String::new();
    for (i, &k) in c.iter().enumerate() {
        let pow = n - i;
        if k == 0.0 {
            continue;
        }
        let neg = k < 0.0;
        let mag = k.abs();
        if out.is_empty() {
            if neg {
                out.push('-');
            }
        } else {
            out.push(if neg { '-' } else { '+' });
        }
        let coef = if mag == 1.0 && pow > 0 {
            String::new()
        } else {
            fmt_num(mag)
        };
        out.push_str(&coef);
        if pow >= 1 {
            out.push_str(var);
        }
        if pow >= 2 {
            out.push_str(&superscript(pow));
        }
    }
    if out.is_empty() {
        "0".into()
    } else {
        out
    }
}

/// Text that fits: shrinks the font until `text` fits into `max_w`.
fn fitted_font(painter: &egui::Painter, text: &str, size: f32, max_w: f32) -> FontId {
    let mut s = size;
    loop {
        let f = FontId::new(s, diagram_family());
        let w = painter
            .layout_no_wrap(text.to_string(), f.clone(), INK)
            .size()
            .x;
        if w <= max_w || s <= 5.0 {
            return f;
        }
        s *= 0.9;
    }
}

/// Draws `num / den` as a stacked fraction centred at `c`.
fn fraction(painter: &egui::Painter, r: Rect, num: &str, den: &str, size: f32, stroke: Stroke) {
    let longest = if num.chars().count() > den.chars().count() {
        num
    } else {
        den
    };
    let font = fitted_font(painter, longest, size, r.width() * 0.86);
    let gn = painter.layout_no_wrap(num.to_string(), font.clone(), INK);
    let gd = painter.layout_no_wrap(den.to_string(), font.clone(), INK);
    let w = gn.size().x.max(gd.size().x) + font.size * 0.3;
    let c = r.center();
    painter.line_segment(
        [Pos2::new(c.x - w / 2.0, c.y), Pos2::new(c.x + w / 2.0, c.y)],
        stroke,
    );
    let gap = font.size * 0.12;
    painter.text(
        Pos2::new(c.x, c.y - gap),
        Align2::CENTER_BOTTOM,
        num,
        font.clone(),
        INK,
    );
    painter.text(
        Pos2::new(c.x, c.y + gap),
        Align2::CENTER_TOP,
        den,
        font,
        INK,
    );
}

/// Outline shape of a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockShape {
    Rect,
    Triangle,
    Rounded,
    Circle,
}

/// Shape used for `b`.
pub fn block_shape(b: &Block) -> BlockShape {
    match b.block_type.as_str() {
        "Gain" => BlockShape::Triangle,
        "Inport" | "Outport" => BlockShape::Rounded,
        "Sum" if param(b, "IconShape") == "round" => BlockShape::Circle,
        _ => BlockShape::Rect,
    }
}

/// Paints the block outline (with fill) and returns nothing; the icon is drawn by [`paint_icon`].
pub fn paint_frame(painter: &egui::Painter, r: Rect, b: &Block, fill: Color32, stroke: Stroke) {
    match block_shape(b) {
        BlockShape::Rect => {
            painter.rect(r, CornerRadius::ZERO, fill, stroke, StrokeKind::Inside);
        }
        BlockShape::Rounded => {
            let rad = (r.height() / 2.0).min(r.width() / 2.0).min(255.0) as u8;
            painter.rect(r, CornerRadius::same(rad), fill, stroke, StrokeKind::Inside);
        }
        BlockShape::Circle => {
            let rad = r.width().min(r.height()) / 2.0 - stroke.width / 2.0;
            painter.circle(r.center(), rad, fill, stroke);
        }
        BlockShape::Triangle => {
            let h = stroke.width / 2.0;
            let pts = triangle_points(r, b, h);
            painter.add(Shape::convex_polygon(pts.to_vec(), fill, stroke));
        }
    }
}

fn triangle_points(r: Rect, b: &Block, inset: f32) -> [Pos2; 3] {
    use sim_core::model::Orientation as O;
    let r = r.shrink(inset);
    match b.orientation {
        O::Right => [r.left_top(), r.left_bottom(), r.right_center()],
        O::Left => [r.right_top(), r.right_bottom(), r.left_center()],
        O::Down => [r.left_top(), r.right_top(), r.center_bottom()],
        O::Up => [r.left_bottom(), r.right_bottom(), r.center_top()],
    }
}

/// Paints the icon of `b` into the screen rectangle `r`. `input_ports` are the
/// screen positions of its input ports (used for Sum signs).
pub fn paint_icon(painter: &egui::Painter, r: Rect, b: &Block, zoom: f32, input_ports: &[Pos2]) {
    let stroke = Stroke::new(zoom.clamp(0.5, 2.0), INK);
    let size = (11.0 * zoom).clamp(4.0, 60.0);
    let at = |fx: f32, fy: f32| Pos2::new(r.left() + r.width() * fx, r.top() + r.height() * fy);
    let text = |s: &str| {
        let font = fitted_font(painter, s, size, r.width() * 0.88);
        painter.text(r.center(), Align2::CENTER_CENTER, s, font, INK);
    };
    match b.block_type.as_str() {
        "Constant" => {
            let v = param(b, "Value");
            text(&if v.chars().count() > 12 {
                "-C-".to_string()
            } else {
                v.to_string()
            });
        }
        "Gain" => {
            let g = param(b, "Gain");
            let s = if g.chars().count() > 8 {
                "-K-".to_string()
            } else {
                g.to_string()
            };
            let tr = Rect::from_min_max(r.min, at(0.72, 1.0));
            let font = fitted_font(painter, &s, size, tr.width() * 0.9);
            painter.text(
                Pos2::new(r.left() + r.width() * 0.36, r.center().y),
                Align2::CENTER_CENTER,
                s,
                font,
                INK,
            );
        }
        "Sum" => {
            let signs: Vec<char> = param(b, "Inputs")
                .chars()
                .filter(|c| *c == '+' || *c == '-')
                .collect();
            let round = param(b, "IconShape") == "round";
            if !round {
                text("Σ");
            }
            let n = input_ports.len().max(1) as f32;
            let sz = if round {
                (size * 0.95).min(r.height() * 0.5).max(4.0)
            } else {
                (r.height() / n * 0.75).clamp(4.0, size * 1.2)
            };
            let font = FontId::new(sz, diagram_family());
            for (i, p) in input_ports.iter().enumerate() {
                let Some(c) = signs.get(i) else { continue };
                let glyph = if *c == '-' { "−" } else { "+" };
                let d = r.center() - *p;
                let len = d.length().max(1e-3);
                let off = if round {
                    (r.width().min(r.height()) * 0.28).min(len * 0.6)
                } else {
                    sz * 0.55 + 1.0
                };
                painter.text(
                    *p + d / len * off,
                    Align2::CENTER_CENTER,
                    glyph,
                    font.clone(),
                    INK,
                );
            }
            if signs.is_empty() && round {
                text("Σ");
            }
        }
        "Step" => {
            painter.add(Shape::line(
                vec![at(0.18, 0.72), at(0.5, 0.72), at(0.5, 0.28), at(0.82, 0.28)],
                stroke,
            ));
        }
        "Scope" => {
            let inner = Rect::from_min_max(at(0.18, 0.2), at(0.82, 0.72));
            painter.rect(
                inner,
                CornerRadius::same(1),
                Color32::from_rgb(0xE9, 0xF1, 0xF8),
                stroke,
                StrokeKind::Inside,
            );
            let ip = |fx: f32, fy: f32| {
                Pos2::new(
                    inner.left() + inner.width() * fx,
                    inner.top() + inner.height() * fy,
                )
            };
            painter.add(Shape::line(
                vec![ip(0.08, 0.75), ip(0.35, 0.3), ip(0.6, 0.62), ip(0.92, 0.25)],
                Stroke::new(stroke.width, Color32::from_rgb(0x00, 0x72, 0xBD)),
            ));
            painter.line_segment([at(0.42, 0.72), at(0.42, 0.82)], stroke);
            painter.line_segment([at(0.58, 0.72), at(0.58, 0.82)], stroke);
            painter.line_segment([at(0.3, 0.82), at(0.7, 0.82)], stroke);
        }
        "Integrator" => fraction(painter, r, "1", "s", size, stroke),
        "TransferFcn" => fraction(
            painter,
            r,
            &polynomial(param(b, "Numerator"), "s"),
            &polynomial(param(b, "Denominator"), "s"),
            size,
            stroke,
        ),
        "StateSpace" => {
            let font = fitted_font(painter, "x' = Ax+Bu", size, r.width() * 0.88);
            painter.text(
                at(0.5, 0.5),
                Align2::CENTER_CENTER,
                "x' = Ax+Bu\n y = Cx+Du",
                font,
                INK,
            );
        }
        "UnitDelay" => fraction(painter, r, "1", "z", size, stroke),
        "DiscreteIntegrator" => fraction(painter, r, "K Ts", "z-1", size, stroke),
        "PIDController" => text("PID(s)"),
        "Sin" => {
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let f = i as f32 / 24.0;
                    at(
                        0.15 + 0.7 * f,
                        0.5 - 0.28 * (f * std::f32::consts::TAU).sin(),
                    )
                })
                .collect();
            painter.add(Shape::line(pts, stroke));
        }
        "Clock" => {
            let c = r.center();
            let rad = r.width().min(r.height()) * 0.34;
            painter.circle_stroke(c, rad, stroke);
            for k in 0..12 {
                let a = k as f32 * std::f32::consts::TAU / 12.0;
                let d = Vec2::new(a.cos(), a.sin());
                painter.line_segment([c + d * rad * 0.82, c + d * rad], stroke);
            }
            painter.line_segment([c, c + Vec2::new(0.0, -rad * 0.7)], stroke);
            painter.line_segment([c, c + Vec2::new(rad * 0.5, 0.0)], stroke);
        }
        "Product" => {
            let font = FontId::new((size * 1.5).min(r.height() * 0.8), diagram_family());
            painter.text(r.center(), Align2::CENTER_CENTER, "×", font, INK);
        }
        "Abs" => text("|u|"),
        "Bias" => text(&format!("u+{}", short(param(b, "Bias"), 6))),
        "Saturate" => {
            painter.line_segment(
                [at(0.15, 0.5), at(0.85, 0.5)],
                Stroke::new(stroke.width * 0.6, Color32::GRAY),
            );
            painter.line_segment(
                [at(0.5, 0.15), at(0.5, 0.85)],
                Stroke::new(stroke.width * 0.6, Color32::GRAY),
            );
            painter.add(Shape::line(
                vec![
                    at(0.15, 0.72),
                    at(0.35, 0.72),
                    at(0.65, 0.28),
                    at(0.85, 0.28),
                ],
                stroke,
            ));
        }
        "Switch" => {
            let ports = input_ports;
            if ports.len() == 3 {
                let pivot = at(0.62, 0.5);
                let tip_in = |p: Pos2| Pos2::new(r.left() + r.width() * 0.3, p.y);
                painter.line_segment([ports[0], tip_in(ports[0])], stroke);
                painter.line_segment([ports[2], tip_in(ports[2])], stroke);
                painter.line_segment([tip_in(ports[0]), pivot], stroke);
                painter.circle_filled(tip_in(ports[2]), stroke.width * 1.5, INK);
                painter.line_segment([pivot, at(1.0, 0.5)], stroke);
                painter.line_segment(
                    [ports[1], at(0.45, 0.5)],
                    Stroke::new(stroke.width, Color32::GRAY),
                );
            } else {
                text("Switch");
            }
        }
        "Inport" | "Outport" => text(param(b, "Port")),
        "SubSystem" => {}
        other => {
            let name = sim_core::library::spec(other)
                .map(|s| s.display_name.to_string())
                .unwrap_or_else(|| format!("Unsupported:\n{other}"));
            text(&short(&name, 18));
        }
    }
}

/// Port labels drawn inside a Subsystem block (names of its Inport/Outport blocks).
pub fn subsystem_port_labels(b: &Block) -> (Vec<String>, Vec<String>) {
    let Some(sys) = &b.system else {
        return (vec![], vec![]);
    };
    let mut ins: Vec<(i64, String)> = vec![];
    let mut outs: Vec<(i64, String)> = vec![];
    for c in &sys.blocks {
        let n = c
            .params
            .get("Port")
            .and_then(|p| p.trim().parse::<i64>().ok())
            .unwrap_or(i64::MAX);
        match c.block_type.as_str() {
            "Inport" => ins.push((n, c.name.clone())),
            "Outport" => outs.push((n, c.name.clone())),
            _ => {}
        }
    }
    ins.sort();
    outs.sort();
    (
        ins.into_iter().map(|x| x.1).collect(),
        outs.into_iter().map(|x| x.1).collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polynomials_render_like_simulink_icons() {
        assert_eq!(polynomial("[1]", "s"), "1");
        assert_eq!(polynomial("[1 1]", "s"), "s+1");
        assert_eq!(polynomial("[1 2 1]", "s"), "s²+2s+1");
        assert_eq!(polynomial("[2, 0, -0.5]", "s"), "2s²-0.5");
        assert_eq!(polynomial("[-1 3]", "s"), "-s+3");
        assert_eq!(polynomial("[0 0]", "s"), "0");
        assert_eq!(polynomial("num", "s"), "num");
        assert_eq!(polynomial("[1 0 0 0 0 0 0 0 0 0 0]", "s"), "s¹⁰");
    }

    #[test]
    fn vectors_parse() {
        assert_eq!(parse_vector("3"), Some(vec![3.0]));
        assert_eq!(parse_vector("[1, 2  3]"), Some(vec![1.0, 2.0, 3.0]));
        assert_eq!(parse_vector("[a 1]"), None);
        assert_eq!(parse_vector("[]"), None);
    }

    #[test]
    fn numbers_are_compact() {
        assert_eq!(fmt_num(2.0), "2");
        assert_eq!(fmt_num(0.25), "0.25");
        assert_eq!(fmt_num(1e-6), "1e-6");
    }
}
