//! Canvas: draw blocks + wires, drag blocks, click-to-select, drag-port-to-connect.
//! Skeleton scope: no undo/redo, no delete-key wiring cleanup (see issues).
use crate::model::{Block, Connection, Model, PortRef};
use eframe::egui;
use std::collections::HashMap;

pub const BLOCK_W: f32 = 120.0;
pub const BLOCK_H: f32 = 56.0;

pub struct CanvasState {
    pub selected: Option<String>,
    pub dragging: Option<String>,
    pub pending_from: Option<PortRef>,
    pub counter: u32,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            selected: None,
            dragging: None,
            pending_from: None,
            counter: 0,
        }
    }
}

pub fn next_id(state: &mut CanvasState, prefix: &str) -> String {
    state.counter += 1;
    format!("{prefix}{}", state.counter)
}

fn block_rect(b: &Block) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(b.position.x, b.position.y),
        egui::vec2(BLOCK_W, BLOCK_H),
    )
}

fn port_pos(b: &Block, port: &str, is_output: bool) -> egui::Pos2 {
    let r = block_rect(b);
    if is_output {
        egui::pos2(r.max.x, r.center().y)
    } else {
        egui::pos2(r.min.x, r.center().y + port_offset(port))
    }
}

fn port_offset(port: &str) -> f32 {
    // in1/in2 stacked vertically; single-port blocks centered
    match port {
        "in1" => -10.0,
        "in2" => 10.0,
        _ => 0.0,
    }
}

pub fn draw_canvas(
    ui: &mut egui::Ui,
    model: &mut Model,
    state: &mut CanvasState,
    palette: &[crate::model::BlockDef],
) {
    let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
    let origin = resp.rect.min.to_vec2();
    let ptr = resp.interact_pointer_pos();

    // --- drag existing blocks ---
    if let Some(pos) = ptr {
        if resp.dragged_by(egui::PointerButton::Primary) {
            if let Some(id) = state.dragging.clone() {
                let delta = resp.drag_delta();
                if let Some(b) = model.blocks.iter_mut().find(|b| b.id == id) {
                    b.position.x += delta.x;
                    b.position.y += delta.y;
                }
            }
        }
    }

    // --- wires ---
    let by_id: HashMap<&str, &Block> = model.blocks.iter().map(|b| (b.id.as_str(), b)).collect();
    for c in &model.connections {
        if let (Some(sb), Some(tb)) = (
            by_id.get(c.from.block.as_str()),
            by_id.get(c.to.block.as_str()),
        ) {
            let a = port_pos(sb, &c.from.port, true) + origin;
            let bpos = port_pos(tb, &c.to.port, false) + origin;
            painter.line_segment([a, bpos], egui::Stroke::new(2.0, egui::Color32::GRAY));
        }
    }

    // --- blocks ---
    for b in model.blocks.clone() {
        let rect = block_rect(&b).translate(origin);
        let color = if state.selected.as_deref() == Some(b.id.as_str()) {
            egui::Color32::LIGHT_BLUE
        } else {
            egui::Color32::DARK_GRAY
        };
        painter.rect_filled(rect, 6.0, color);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &b.block_type,
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
        // output port dot (click = start a wire)
        let out_dot = rect.right_center();
        let out_resp = painter.add(egui::Shape::circle_filled(
            out_dot,
            5.0,
            egui::Color32::WHITE,
        ));
        let _ = out_resp;
        // input port dots are hit-tested below via proximity
        if let Some(pos) = ptr {
            if rect.contains(pos) {
                if resp.clicked() {
                    // distinguish: near right edge = output port, else select
                    if (pos.x - rect.max.x).abs() < 8.0 && (pos.y - rect.center().y).abs() < 8.0 {
                        state.pending_from = Some(PortRef {
                            block: b.id.clone(),
                            port: "out".into(),
                        });
                    } else {
                        state.selected = Some(b.id.clone());
                        state.dragging = Some(b.id.clone());
                    }
                }
            }
            if resp.drag_stopped() {
                state.dragging = None;
            }
            // drop a pending wire on an input port
            if resp.clicked()
                && state.pending_from.is_some()
                && rect.contains(pos)
                && (pos.x - rect.min.x).abs() < 10.0
            {
                if let Some(from) = state.pending_from.take() {
                    // find nearest declared input port name from palette
                    let def = palette.iter().find(|d| d.block_type == b.block_type);
                    let port = def
                        .and_then(|d| d.inputs.first())
                        .cloned()
                        .unwrap_or_else(|| "in".into());
                    if from.block != b.id {
                        model.connections.push(Connection {
                            from,
                            to: PortRef {
                                block: b.id.clone(),
                                port,
                            },
                        });
                    }
                }
            }
        }
    }

    if state.pending_from.is_some() {
        ui.label("Click an input port (left edge of a block) to finish the wire. Esc/right-click cancels.");
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            state.pending_from = None;
        }
    }

    // --- double-click empty canvas = add Constant (skeleton shortcut) ---
    if resp.double_clicked() && ptr.is_some() {
        let pos = ptr.unwrap() - origin;
        model.blocks.push(Block {
            id: next_id(state, "blk"),
            block_type: "Constant".into(),
            position: crate::model::Position {
                x: pos.x - BLOCK_W / 2.0,
                y: pos.y - BLOCK_H / 2.0,
            },
            params: HashMap::from([("value".to_string(), serde_json::json!(1.0))]),
        });
    }
}
