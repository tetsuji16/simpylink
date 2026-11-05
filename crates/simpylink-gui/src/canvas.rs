//! Canvas: blocks + wires with pan/zoom, per-port hit testing, wire selection.
//! Mutations push undo snapshots via `History`.
use crate::model::{Block, Connection, Model, PortRef};
use eframe::egui;
use std::collections::{HashMap, HashSet};

pub const BLOCK_W: f32 = 120.0;
pub const BLOCK_H: f32 = 56.0;

#[derive(Default, Clone)]
pub struct History {
    snapshots: Vec<String>,
    index: usize,
    max: usize,
}

impl History {
    pub fn new() -> Self {
        Self {
            snapshots: vec![],
            index: 0,
            max: 100,
        }
    }

    pub fn push(&mut self, model: &Model) {
        let snap = serde_json::to_string(model).unwrap_or_default();
        self.snapshots.truncate(self.index + 1);
        self.snapshots.push(snap);
        if self.snapshots.len() > self.max {
            self.snapshots.remove(0);
        }
        self.index = self.snapshots.len().saturating_sub(1);
    }

    pub fn undo(&mut self) -> Option<Model> {
        if self.index == 0 {
            return None;
        }
        self.index -= 1;
        serde_json::from_str(&self.snapshots[self.index]).ok()
    }

    pub fn redo(&mut self) -> Option<Model> {
        if self.index + 1 >= self.snapshots.len() {
            return None;
        }
        self.index += 1;
        serde_json::from_str(&self.snapshots[self.index]).ok()
    }

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        self.index > 0
    }

    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        self.index + 1 < self.snapshots.len()
    }
}

pub struct CanvasState {
    pub selected: Option<String>,
    pub selected_wire: Option<usize>,
    pub dragging: Option<String>,
    pub pending_from: Option<PortRef>,
    pub counter: u32,
    pub pan: egui::Vec2,
    pub zoom: f32,
    pub pending_dialog: Option<String>,
    // Multi-select
    pub multi_selected: HashSet<String>,
    pub drag_select_anchor: Option<egui::Pos2>,
    pub drag_selecting: bool,
    // Context menu
    pub context_menu_pos: Option<egui::Pos2>,
    pub context_menu_target: Option<ContextTarget>,
}

#[derive(Clone, Debug)]
pub enum ContextTarget {
    Block(String),
    Wire(usize),
    Canvas(egui::Pos2),
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            selected: None,
            selected_wire: None,
            dragging: None,
            pending_from: None,
            counter: 0,
            pan: egui::Vec2::ZERO,
            zoom: 1.0,
            pending_dialog: None,
            multi_selected: HashSet::new(),
            drag_select_anchor: None,
            drag_selecting: false,
            context_menu_pos: None,
            context_menu_target: None,
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

fn distance_to_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ap = p - a;
    let ab = b - a;
    let ab2 = ab.x * ab.x + ab.y * ab.y;
    if ab2 == 0.0 {
        return p.distance(a);
    }
    let t = ((ap.x * ab.x + ap.y * ab.y) / ab2).clamp(0.0, 1.0);
    let closest = egui::pos2(a.x + t * ab.x, a.y + t * ab.y);
    p.distance(closest)
}

fn port_pos(
    b: &Block,
    port: &str,
    is_output: bool,
    inputs: &[String],
    outputs: &[String],
) -> (egui::Pos2, f32) {
    let r = block_rect(b);
    if is_output {
        let n = outputs.len().max(1) as f32;
        let idx = outputs.iter().position(|p| p == port).unwrap_or(0) as f32;
        let y = r.min.y + BLOCK_H * (idx + 1.0) / (n + 1.0);
        (egui::pos2(r.max.x, y), 5.0)
    } else {
        let n = inputs.len().max(1) as f32;
        let idx = inputs.iter().position(|p| p == port).unwrap_or(0) as f32;
        let y = r.min.y + BLOCK_H * (idx + 1.0) / (n + 1.0);
        (egui::pos2(r.min.x, y), 5.0)
    }
}

pub fn draw_canvas(
    ui: &mut egui::Ui,
    model: &mut Model,
    state: &mut CanvasState,
    palette: &[crate::model::BlockDef],
    history: &mut History,
) {
    let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
    let origin = resp.rect.min;
    let ptr = resp.interact_pointer_pos();

    // --- zoom (Ctrl+wheel) ---
    if let Some(_pos) = ptr {
        let scroll = ui.input(|i| i.zoom_delta());
        if (scroll - 1.0).abs() > f32::EPSILON {
            state.zoom = (state.zoom * scroll).clamp(0.3, 3.0);
        }
    }

    // --- pan (middle-drag, or space+drag) ---
    if resp.dragged_by(egui::PointerButton::Middle)
        || (ui.input(|i| i.key_down(egui::Key::Space))
            && resp.dragged_by(egui::PointerButton::Primary))
    {
        state.pan += resp.drag_delta();
    }

    // drag blocks (scale delta by zoom)
    if resp.dragged_by(egui::PointerButton::Primary) && state.dragging.is_some() {
        let delta = resp.drag_delta() / state.zoom;
        if let Some(id) = state.dragging.clone() {
            if let Some(b) = model.blocks.iter_mut().find(|b| b.id == id) {
                b.position.x += delta.x;
                b.position.y += delta.y;
            }
        }
    }
    if resp.drag_stopped() && state.dragging.is_some() {
        state.dragging = None;
        history.push(model);
    }

    let def_of = |t: &str| palette.iter().find(|d| d.block_type == t);
    let by_id: HashMap<String, Block> = model
        .blocks
        .iter()
        .map(|b| (b.id.clone(), b.clone()))
        .collect();

    // --- wires (orthogonal/Manhattan routing) ---
    for (idx, c) in model.connections.iter().enumerate() {
        if let (Some(sb), Some(tb)) = (by_id.get(&c.from.block), by_id.get(&c.to.block)) {
            let sd = def_of(&sb.block_type);
            let td = def_of(&tb.block_type);
            let (a_model, _) = port_pos(
                sb,
                &c.from.port,
                true,
                &[],
                &sd.map(|d| d.outputs.clone()).unwrap_or_default(),
            );
            let (b_model, _) = port_pos(
                tb,
                &c.to.port,
                false,
                &td.map(|d| d.inputs.clone()).unwrap_or_default(),
                &[],
            );
            let a = origin + state.pan + a_model.to_vec2() * state.zoom;
            let bpos = origin + state.pan + b_model.to_vec2() * state.zoom;

            // Manhattan routing: horizontal then vertical with a corner
            let mid_x = (a.x + bpos.x) / 2.0;
            let corner1 = egui::pos2(mid_x, a.y);
            let corner2 = egui::pos2(mid_x, bpos.y);

            let selected = state.selected_wire == Some(idx);
            let color = if selected {
                egui::Color32::GOLD
            } else {
                egui::Color32::GRAY
            };
            let stroke = egui::Stroke::new(1.5_f32, color);
            painter.line_segment([a, corner1], stroke);
            painter.line_segment([corner1, corner2], stroke);
            painter.line_segment([corner2, bpos], stroke);

            // wire hit test: distance from pointer to any segment
            if let Some(pos) = ptr {
                if resp.clicked() {
                    let d1 = distance_to_segment(pos, a, corner1);
                    let d2 = distance_to_segment(pos, corner1, corner2);
                    let d3 = distance_to_segment(pos, corner2, bpos);
                    if d1.min(d2).min(d3) < 6.0 {
                        state.selected_wire = Some(idx);
                        state.selected = None;
                        state.multi_selected.clear();
                    }
                }
            }
        }
    }

    // live pending wire (Manhattan)
    if let (Some(from), Some(pos)) = (state.pending_from.clone(), ptr) {
        if let Some(sb) = by_id.get(&from.block) {
            let sd = def_of(&sb.block_type);
            let (a_model, _) = port_pos(
                sb,
                &from.port,
                true,
                &[],
                &sd.map(|d| d.outputs.clone()).unwrap_or_default(),
            );
            let a = origin + state.pan + a_model.to_vec2() * state.zoom;
            let mid_x = (a.x + pos.x) / 2.0;
            let corner1 = egui::pos2(mid_x, a.y);
            let corner2 = egui::pos2(mid_x, pos.y);
            let stroke = egui::Stroke::new(1.5_f32, egui::Color32::LIGHT_BLUE);
            painter.line_segment([a, corner1], stroke);
            painter.line_segment([corner1, corner2], stroke);
            painter.line_segment([corner2, pos], stroke);
        }
    }

    // --- blocks ---
    let blocks_snapshot = model.blocks.clone();
    for b in &blocks_snapshot {
        let rect_model = block_rect(b);
        let rect = egui::Rect::from_min_size(
            origin + state.pan + rect_model.min.to_vec2() * state.zoom,
            rect_model.size() * state.zoom,
        );
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

        let def = def_of(&b.block_type);
        let inputs = def
            .map(|d| d.inputs.clone())
            .unwrap_or_else(|| vec!["in".into()]);
        let outputs = def
            .map(|d| d.outputs.clone())
            .unwrap_or_else(|| vec!["out".into()]);

        // port dots
        for port in &inputs {
            let (m, _) = port_pos(b, port, false, &inputs, &outputs);
            let s = origin + state.pan + m.to_vec2() * state.zoom;
            let target = state.pending_from.is_some();
            painter.add(egui::Shape::circle_filled(
                s,
                5.0 * state.zoom.sqrt(),
                if target {
                    egui::Color32::LIGHT_GREEN
                } else {
                    egui::Color32::WHITE
                },
            ));
        }
        for port in &outputs {
            let (m, _) = port_pos(b, port, true, &inputs, &outputs);
            let s = origin + state.pan + m.to_vec2() * state.zoom;
            painter.add(egui::Shape::circle_filled(
                s,
                5.0 * state.zoom.sqrt(),
                egui::Color32::WHITE,
            ));
        }

        if let Some(pos) = ptr {
            // hover/select/drag
            if rect.contains(pos) {
                // Right-click: context menu
                if resp.secondary_clicked() {
                    state.context_menu_pos = Some(pos);
                    state.context_menu_target = Some(ContextTarget::Block(b.id.clone()));
                }

                if resp.clicked() {
                    // near output port -> start wire
                    let near_out = outputs.iter().any(|port| {
                        let (m, _) = port_pos(b, port, true, &inputs, &outputs);
                        let s = origin + state.pan + m.to_vec2() * state.zoom;
                        pos.distance(s) < 10.0
                    });
                    let near_in = inputs.iter().find(|port| {
                        let (m, _) = port_pos(b, port, false, &inputs, &outputs);
                        let s = origin + state.pan + m.to_vec2() * state.zoom;
                        pos.distance(s) < 10.0
                    });
                    if let Some(port) = near_in.filter(|_| state.pending_from.is_some()) {
                        if let Some(from) = state.pending_from.take() {
                            if from.block != b.id
                                && !model
                                    .connections
                                    .iter()
                                    .any(|c| c.to.block == b.id && c.to.port == *port)
                            {
                                history.push(model);
                                model.connections.push(Connection {
                                    from,
                                    to: PortRef {
                                        block: b.id.clone(),
                                        port: (*port).clone(),
                                    },
                                });
                            }
                        }
                    } else if near_out {
                        let port = outputs[0].clone();
                        state.pending_from = Some(PortRef {
                            block: b.id.clone(),
                            port,
                        });
                    } else {
                        // Multi-select: Ctrl+Click toggles, Shift+Click extends, plain click selects only this
                        let ctrl = ui.input(|i| i.modifiers.ctrl);
                        let shift = ui.input(|i| i.modifiers.shift);
                        if ctrl {
                            if state.multi_selected.contains(&b.id) {
                                state.multi_selected.remove(&b.id);
                            } else {
                                state.multi_selected.insert(b.id.clone());
                            }
                            state.selected = Some(b.id.clone());
                        } else if shift && state.selected.is_some() {
                            let last = state.selected.clone().unwrap();
                            let mut adding = false;
                            for bb in &blocks_snapshot {
                                if bb.id == last || bb.id == b.id {
                                    adding = !adding;
                                }
                                if adding || bb.id == b.id {
                                    state.multi_selected.insert(bb.id.clone());
                                }
                            }
                        } else {
                            state.multi_selected.clear();
                            state.multi_selected.insert(b.id.clone());
                            state.selected = Some(b.id.clone());
                        }
                        state.selected_wire = None;
                        state.dragging = Some(b.id.clone());
                    }
                }
                if resp.double_clicked() {
                    state.pending_dialog = Some(b.id.clone());
                }
            }
        }
    }

    if state.pending_from.is_some() {
        ui.label("Click an input port (left edge) to finish the wire. Esc cancels.");
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            state.pending_from = None;
        }
    }

    // double-click empty canvas = add Constant; drag on empty = marquee select
    if resp.double_clicked() {
        if let Some(p) = ptr {
            let on_block = blocks_snapshot.iter().any(|b| {
                let rect_model = block_rect(b);
                let r = egui::Rect::from_min_size(
                    origin + state.pan + rect_model.min.to_vec2() * state.zoom,
                    rect_model.size() * state.zoom,
                );
                r.contains(p)
            });
            if !on_block {
                let pos = (p - origin - state.pan) / state.zoom;
                history.push(model);
                let new_id = next_id(state, "blk");
                model.blocks.push(Block {
                    id: new_id,
                    block_type: "Constant".into(),
                    position: crate::model::Position {
                        x: pos.x - BLOCK_W / 2.0,
                        y: pos.y - BLOCK_H / 2.0,
                    },
                    params: HashMap::from([("value".to_string(), serde_json::json!(1.0))]),
                });
            }
        }
    }

    // Drag-select (marquee) on empty canvas
    if resp.dragged_by(egui::PointerButton::Primary) && state.dragging.is_none() && state.pending_from.is_none() {
        if !state.drag_selecting {
            state.drag_select_anchor = ptr;
            state.drag_selecting = true;
        }
    }
    if resp.drag_stopped() && state.drag_selecting {
        state.drag_selecting = false;
        if let (Some(anchor), Some(current)) = (state.drag_select_anchor, ptr) {
            let min = egui::pos2(anchor.x.min(current.x), anchor.y.min(current.y));
            let max = egui::pos2(anchor.x.max(current.x), anchor.y.max(current.y));
            let select_rect = egui::Rect::from_min_max(min, max);
            state.multi_selected.clear();
            for b in &blocks_snapshot {
                let rect_model = block_rect(b);
                let r = egui::Rect::from_min_size(
                    origin + state.pan + rect_model.min.to_vec2() * state.zoom,
                    rect_model.size() * state.zoom,
                );
                if select_rect.intersects(r) {
                    state.multi_selected.insert(b.id.clone());
                }
            }
            if let Some(first) = state.multi_selected.iter().next().cloned() {
                state.selected = Some(first);
            }
        }
        state.drag_select_anchor = None;
    }

    // Draw drag-select rectangle
    if state.drag_selecting {
        if let (Some(anchor), Some(current)) = (state.drag_select_anchor, ptr) {
            painter.rect_stroke(
                egui::Rect::from_two_pos(anchor, current),
                0.0,
                egui::Stroke::new(1.0_f32, egui::Color32::LIGHT_BLUE),
                egui::StrokeKind::Inside,
            );
        }
    }

    // Delete selected wire or blocks
    if ui.input(|i| i.key_pressed(egui::Key::Delete)) {
        if let Some(idx) = state.selected_wire.take() {
            if idx < model.connections.len() {
                let _ = model.connections.remove(idx);
                history.push(model);
            }
        } else if !state.multi_selected.is_empty() {
            let ids: Vec<String> = state.multi_selected.drain().collect();
            history.push(model);
            model.blocks.retain(|b| !ids.contains(&b.id));
            model.connections.retain(|c| !ids.contains(&c.from.block) && !ids.contains(&c.to.block));
            state.selected = None;
            state.selected_wire = None;
        }
    }
}
