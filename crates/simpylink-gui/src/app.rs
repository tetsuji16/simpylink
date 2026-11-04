//! Top-level app: palette (left), canvas (center), inspector (right), run/results (bottom).
use crate::api_client::ApiClient;
use crate::canvas::{self, CanvasState};
use crate::model::{Block, BlockDef, Model, Position};
use eframe::egui;
use std::collections::HashMap;

pub struct SimpylinkApp {
    model: Model,
    canvas: CanvasState,
    palette: Vec<BlockDef>,
    api: ApiClient,
    status: String,
    results: Option<HashMap<String, f64>>,
}

impl Default for SimpylinkApp {
    fn default() -> Self {
        let api = ApiClient::default();
        let (palette, status) = match api.fetch_blocks() {
            Ok(p) => (p, "backend connected".into()),
            Err(e) => (Vec::new(), e),
        };
        // Seed a runnable example on first launch.
        let mut model = Model::default();
        model.name = "sum_example".into();
        model.blocks = vec![
            Block {
                id: "c1".into(),
                block_type: "Constant".into(),
                position: Position { x: 40.0, y: 40.0 },
                params: HashMap::from([("value".into(), serde_json::json!(3.0))]),
            },
            Block {
                id: "c2".into(),
                block_type: "Constant".into(),
                position: Position { x: 40.0, y: 140.0 },
                params: HashMap::from([("value".into(), serde_json::json!(4.0))]),
            },
            Block {
                id: "s".into(),
                block_type: "Sum".into(),
                position: Position { x: 240.0, y: 90.0 },
                params: HashMap::from([("inputs".into(), serde_json::json!("++"))]),
            },
            Block {
                id: "o".into(),
                block_type: "Outport".into(),
                position: Position { x: 420.0, y: 90.0 },
                params: HashMap::from([("port".into(), serde_json::json!(1))]),
            },
        ];
        model.connections = vec![
            crate::model::Connection {
                from: crate::model::PortRef {
                    block: "c1".into(),
                    port: "out".into(),
                },
                to: crate::model::PortRef {
                    block: "s".into(),
                    port: "in1".into(),
                },
            },
            crate::model::Connection {
                from: crate::model::PortRef {
                    block: "c2".into(),
                    port: "out".into(),
                },
                to: crate::model::PortRef {
                    block: "s".into(),
                    port: "in2".into(),
                },
            },
            crate::model::Connection {
                from: crate::model::PortRef {
                    block: "s".into(),
                    port: "out".into(),
                },
                to: crate::model::PortRef {
                    block: "o".into(),
                    port: "in".into(),
                },
            },
        ];
        Self {
            model,
            canvas: CanvasState::default(),
            palette,
            api,
            status,
            results: None,
        }
    }
}

impl eframe::App for SimpylinkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("palette").show(ctx, |ui| {
            ui.heading("Library (Math Operations subset)");
            ui.label("Click to add a block to the canvas.");
            for def in self.palette.clone() {
                let label = format!("{} — {}", def.label, def.description);
                if ui.button(label).clicked() {
                    let id = canvas::next_id(&mut self.canvas, "blk");
                    let x = 80.0 + (self.model.blocks.len() as f32 % 5.0) * 40.0;
                    let y = 60.0 + (self.model.blocks.len() as f32 % 5.0) * 40.0;
                    let params = def
                        .params
                        .iter()
                        .map(|p| (p.name.clone(), p.default.clone()))
                        .collect();
                    self.model.blocks.push(Block {
                        id,
                        block_type: def.block_type.clone(),
                        position: Position { x, y },
                        params,
                    });
                }
            }
            ui.separator();
            ui.label("Double-click canvas = add Constant.");
        });

        egui::SidePanel::right("inspector").show(ctx, |ui| {
            ui.heading("Inspector");
            let selected = self.canvas.selected.clone();
            if let Some(id) = selected {
                if let Some(block) = self.model.blocks.iter_mut().find(|b| b.id == id) {
                    ui.label(format!("Block: {} ({})", block.id, block.block_type));
                    let keys: Vec<String> = block.params.keys().cloned().collect();
                    for key in keys {
                        let val = block
                            .params
                            .get(&key)
                            .cloned()
                            .unwrap_or(serde_json::json!(0.0));
                        match &val {
                            serde_json::Value::Number(n) => {
                                let mut v = n.as_f64().unwrap_or(0.0);
                                if ui
                                    .add(egui::DragValue::new(&mut v).prefix(format!("{key}: ")))
                                    .changed()
                                {
                                    block.params.insert(key.clone(), serde_json::json!(v));
                                }
                            }
                            serde_json::Value::String(s) => {
                                let mut v = s.clone();
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut v)
                                            .hint_text(format!("{key}")),
                                    )
                                    .changed()
                                {
                                    block.params.insert(key.clone(), serde_json::json!(v));
                                }
                            }
                            _ => {
                                ui.label(format!("{key}: {val}"));
                            }
                        }
                    }
                    if ui.button("Delete block").clicked() {
                        let bid = block.id.clone();
                        self.model.blocks.retain(|b| b.id != bid);
                        self.model
                            .connections
                            .retain(|c| c.from.block != bid && c.to.block != bid);
                        self.canvas.selected = None;
                    }
                }
            } else {
                ui.label("Select a block on the canvas.");
            }
        });

        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("▶ Run (simulate)").clicked() {
                    match self.api.simulate(&self.model) {
                        Ok(r) => {
                            self.status = format!("ok — outputs: {:?}", r.outputs);
                            self.results = Some(r.outputs);
                        }
                        Err(e) => {
                            self.status = format!("error: {e}");
                        }
                    }
                }
                if ui.button("Refresh library").clicked() {
                    match self.api.fetch_blocks() {
                        Ok(p) => {
                            self.palette = p;
                            self.status = "backend connected".into();
                        }
                        Err(e) => {
                            self.status = e;
                        }
                    }
                }
                if ui.button("Clear").clicked() {
                    self.model.blocks.clear();
                    self.model.connections.clear();
                    self.results = None;
                }
            });
            ui.label(&self.status);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            canvas::draw_canvas(ui, &mut self.model, &mut self.canvas, &self.palette);
        });
    }
}
