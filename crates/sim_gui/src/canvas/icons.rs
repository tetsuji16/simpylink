//! Block icons (DES-05 §6 / M1-08 §F.3). Drawn inside the block rectangle in screen space.
use egui::{Align2, Color32, FontId, Pos2, Rect, Shape, Stroke};
use sim_core::model::Block;

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

/// Paints the icon of `b` into the screen rectangle `r`.
pub fn paint_icon(painter: &egui::Painter, r: Rect, b: &Block, zoom: f32) {
    let ink = Color32::BLACK;
    let stroke = Stroke::new(zoom.clamp(0.6, 2.0), ink);
    let font = FontId::proportional((10.0 * zoom).clamp(6.0, 40.0));
    let at = |fx: f32, fy: f32| Pos2::new(r.left() + r.width() * fx, r.top() + r.height() * fy);
    let text = |s: &str| {
        painter.text(r.center(), Align2::CENTER_CENTER, s, font.clone(), ink);
    };
    match b.block_type.as_str() {
        "Constant" => text(&short(param(b, "Value"), 8)),
        "Gain" => {
            painter.add(Shape::closed_line(
                vec![at(0.1, 0.1), at(0.1, 0.9), at(0.9, 0.5)],
                stroke,
            ));
            painter.text(
                at(0.4, 0.5),
                Align2::CENTER_CENTER,
                short(param(b, "Gain"), 6),
                font,
                ink,
            );
        }
        "Sum" => {
            let signs: String = param(b, "Inputs")
                .chars()
                .filter(|c| *c == '+' || *c == '-')
                .collect();
            let signs = if signs.is_empty() {
                "Σ".to_string()
            } else {
                signs
            };
            if param(b, "IconShape") == "round" {
                painter.circle_stroke(r.center(), r.width().min(r.height()) * 0.45, stroke);
            }
            text(&signs);
        }
        "Step" => {
            painter.add(Shape::line(
                vec![at(0.2, 0.75), at(0.5, 0.75), at(0.5, 0.25), at(0.8, 0.25)],
                stroke,
            ));
        }
        "Scope" => {
            let inner = r.shrink(r.width().min(r.height()) * 0.12);
            painter.rect_stroke(inner, 0.0, stroke, egui::StrokeKind::Inside);
            let ip = |fx: f32, fy: f32| {
                Pos2::new(
                    inner.left() + inner.width() * fx,
                    inner.top() + inner.height() * fy,
                )
            };
            painter.add(Shape::line(
                vec![ip(0.1, 0.7), ip(0.4, 0.3), ip(0.6, 0.6), ip(0.9, 0.3)],
                stroke,
            ));
        }
        "Integrator" => text("1/s"),
        "TransferFcn" => text(&format!(
            "{}\n{}",
            short(param(b, "Numerator"), 10),
            short(param(b, "Denominator"), 10)
        )),
        "StateSpace" => text("x' = Ax+Bu\ny = Cx+Du"),
        "UnitDelay" => text("1/z"),
        "DiscreteIntegrator" => text("KTs/(z-1)"),
        "PIDController" => text("PID(s)"),
        "Sin" => {
            let pts: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let f = i as f32 / 20.0;
                    at(
                        0.15 + 0.7 * f,
                        0.5 - 0.3 * (f * std::f32::consts::TAU).sin(),
                    )
                })
                .collect();
            painter.add(Shape::line(pts, stroke));
        }
        "Clock" => {
            painter.circle_stroke(r.center(), r.width().min(r.height()) * 0.35, stroke);
            painter.add(Shape::line(vec![at(0.5, 0.5), at(0.5, 0.3)], stroke));
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
