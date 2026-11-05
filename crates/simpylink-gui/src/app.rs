//! Top-level app: palette (left), canvas (center), inspector (right), run/results (bottom).
use crate::api_client::ApiClient;
use crate::canvas::{self, CanvasState, History};
use crate::model::{Block, BlockDef, Connection, Model, Position, SimulateResponse, TimeSimResponse};
use eframe::egui;
use std::collections::HashMap;

pub struct SimpylinkApp {
    model: Model,
    canvas: CanvasState,
    history: History,
    palette: Vec<BlockDef>,
    api: ApiClient,
    status: String,
    results: Option<SimulateResponse>,
    time_results: Option<TimeSimResponse>,
    search: String,
    file_path: Option<String>,
    mode: String,
    dark_theme: bool,
    use_codegen: bool,
    codegen_info: String,
    t_stop: f64,
    dt: f64,
    dialog_block: Option<String>,
    dialog_error: String,
}

impl Default for SimpylinkApp {
    fn default() -> Self {
        let api = ApiClient::default();
        let (palette, status) = match api.fetch_blocks() {
            Ok(p) => (p, "backend connected".into()),
            Err(e) => (Vec::new(), e),
        };
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
        let mut history = History::new();
        history.push(&model);
        Self {
            model,
            canvas: CanvasState::default(),
            history,
            palette,
            api,
            status,
            results: None,
            time_results: None,
            search: String::new(),
            file_path: None,
            mode: "One-shot".into(),
            dark_theme: false,
            use_codegen: true,
            codegen_info: String::new(),
            t_stop: 1.0,
            dt: 0.1,
            dialog_block: None,
            dialog_error: String::new(),
        }
    }
}

impl SimpylinkApp {
    fn add_block(&mut self, def: &BlockDef) {
        let id = canvas::next_id(&mut self.canvas, "blk");
        let x = 80.0 + (self.model.blocks.len() as f32 % 5.0) * 40.0;
        let y = 60.0 + (self.model.blocks.len() as f32 % 5.0) * 40.0;
        let params = def
            .params
            .iter()
            .map(|p| (p.name.clone(), p.default.clone()))
            .collect();
        self.history.push(&self.model);
        self.model.blocks.push(Block {
            id,
            block_type: def.block_type.clone(),
            position: Position { x, y },
            params,
        });
    }

    fn delete_selected(&mut self) {
        if let Some(id) = self.canvas.selected.clone() {
            self.history.push(&self.model);
            self.model.blocks.retain(|b| b.id != id);
            self.model
                .connections
                .retain(|c| c.from.block != id && c.to.block != id);
            self.canvas.selected = None;
        }
    }

    fn save_to(&mut self, path: &std::path::Path) {
        match std::fs::write(path, serde_json::to_string_pretty(&self.model).unwrap()) {
            Ok(()) => {
                self.file_path = Some(path.display().to_string());
                self.status = format!("saved {}", path.display());
            }
            Err(e) => self.status = format!("save failed: {e}"),
        }
    }

    fn open_from(&mut self, path: &std::path::Path) {
            match std::fs::read_to_string(path) {
                Ok(text) => match serde_json::from_str::<Model>(&text) {
                    Ok(m) => {
                        self.history.push(&self.model);
                        self.model = m;
                        self.file_path = Some(path.display().to_string());
                        self.results = None;
                        self.time_results = None;
                        self.status = format!("opened {}", path.display());
                    }
                    Err(e) => self.status = format!("load failed (invalid model): {e}"),
                },
                Err(e) => self.status = format!("open failed: {e}"),
            }
        }

        // --- Clipboard / Multi-select helpers ---

        fn selected_block_ids(&self) -> Vec<String> {
            if !self.canvas.multi_selected.is_empty() {
                self.canvas.multi_selected.iter().cloned().collect()
            } else if let Some(id) = self.canvas.selected.clone() {
                vec![id]
            } else {
                vec![]
            }
        }

        fn copy_selected(&mut self) {
            let ids = self.selected_block_ids();
            if ids.is_empty() { return; }
            let blocks: Vec<Block> = self.model.blocks.iter().filter(|b| ids.contains(&b.id)).cloned().collect();
            let conns: Vec<Connection> = self.model.connections.iter()
                .filter(|c| ids.contains(&c.from.block) && ids.contains(&c.to.block))
                .cloned().collect();
            let data = serde_json::json!({ "blocks": blocks, "connections": conns });
                    if let Ok(text) = serde_json::to_string(&data) {
                        if let Some(mut clipboard) = arboard::Clipboard::new().ok() {
                            let _ = clipboard.set_text(text);
                            self.status = format!("Copied {} block(s)", blocks.len());
                        }
                    }
                }

                fn paste_at_cursor(&mut self) {
                    let text = match arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()) {
                        Some(t) => t,
                        None => return,
                    };
            let data: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => return,
            };
            let Some(blocks_val) = data.get("blocks") else { return };
            let Some(conns_val) = data.get("connections") else { return };
            let blocks: Vec<Block> = match serde_json::from_value(blocks_val.clone()) {
                Ok(b) => b,
                Err(_) => return,
            };
            let conns: Vec<Connection> = match serde_json::from_value(conns_val.clone()) {
                Ok(c) => c,
                Err(_) => return,
            };
            if blocks.is_empty() { return; }

            self.history.push(&self.model);

            // Offset pasted blocks slightly from original positions
            let offset = egui::vec2(20.0, 20.0);
            let mut id_map = HashMap::new();
            for mut b in blocks {
                let old_id = b.id.clone();
                b.id = canvas::next_id(&mut self.canvas, "blk");
                b.position.x += offset.x;
                b.position.y += offset.y;
                id_map.insert(old_id, b.id.clone());
                self.model.blocks.push(b);
            }
            for mut c in conns {
                if let (Some(new_from), Some(new_to)) = (id_map.get(&c.from.block), id_map.get(&c.to.block)) {
                    c.from.block = new_from.clone();
                    c.to.block = new_to.clone();
                    self.model.connections.push(c);
                }
            }
            self.status = format!("Pasted {} block(s)", id_map.len());
        }

        fn duplicate_selected(&mut self) {
            let ids = self.selected_block_ids();
            if ids.is_empty() { return; }
            self.copy_selected();
            self.paste_at_cursor();
            // After paste, the new blocks are selected
            self.status = format!("Duplicated {} block(s)", ids.len());
        }
    }

impl eframe::App for SimpylinkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        {
            let mut style = egui::Style::default();
            if self.dark_theme {
                style.visuals = egui::Visuals::dark();
            } else {
                style.visuals = egui::Visuals::light();
            }
            ctx.set_style(style);
        }
        // keyboard shortcuts
                if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)) {
                    // plain Ctrl+Z = undo (redo via Ctrl+Shift+Z below)
                    if let Some(m) = self.history.undo() {
                        self.model = m;
                    }
                }
                if ctx.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::Z)) {
                    if let Some(m) = self.history.redo() {
                        self.model = m;
                    }
                }
                // Delete for multi-select
                if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
                    if !self.canvas.multi_selected.is_empty() || self.canvas.selected_wire.is_some() {
                        // canvas handles this
                    } else if self.canvas.selected.is_some() {
                        self.delete_selected();
                    }
                }
                // Copy (Ctrl+C) / Paste (Ctrl+V) / Duplicate (Ctrl+D)
                if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::C)) {
                    self.copy_selected();
                }
                if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::V)) {
                    self.paste_at_cursor();
                }
                if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::D)) {
                    self.duplicate_selected();
                }
                // Select all (Ctrl+A)
                if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::A)) {
                    self.canvas.multi_selected = self.model.blocks.iter().map(|b| b.id.clone()).collect();
                    if let Some(first) = self.canvas.multi_selected.iter().next().cloned() {
                        self.canvas.selected = Some(first);
                    }
                }
                // arrow-key nudge for multi-select
                let nudge = if ctx.input(|i| i.modifiers.shift) {
                    10.0
                } else {
                    1.0
                };
                let mut dx = 0.0;
                let mut dy = 0.0;
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    dx -= nudge;
                }
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    dx += nudge;
                }
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    dy -= nudge;
                }
                if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    dy += nudge;
                }
                if (dx, dy) != (0.0, 0.0) {
                    let targets: Vec<String> = if !self.canvas.multi_selected.is_empty() {
                        self.canvas.multi_selected.iter().cloned().collect()
                    } else if let Some(id) = self.canvas.selected.clone() {
                        vec![id]
                    } else {
                        vec![]
                    };
                    for id in targets {
                        if let Some(b) = self.model.blocks.iter_mut().find(|b| b.id == id) {
                            b.position.x += dx;
                            b.position.y += dy;
                        }
                    }
                    if (dx, dy) != (0.0, 0.0) {
                        self.history.push(&self.model);
                    }
                }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.vertical(|ui| {
            ui.label("SIMULATION");
            ui.horizontal(|ui| {
                ui.label("FILE:");
                if ui.button("New").clicked() {
                    self.history.push(&self.model);
                    self.model = Model::default();
                    self.file_path = None;
                    self.results = None;
                    self.time_results = None;
                }
                if ui.button("Open…").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("simpylink", &["json"])
                        .pick_file()
                    {
                        self.open_from(&p);
                    }
                }
                if ui.button("Save").clicked() {
                    if let Some(p) = self.file_path.clone() {
                        self.save_to(&std::path::PathBuf::from(p));
                    } else if let Some(p) = rfd::FileDialog::new()
                        .add_filter("simpylink", &["json"])
                        .save_file()
                    {
                        self.save_to(&p);
                    }
                }
                if ui.button("Save As…").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("simpylink", &["json"])
                        .save_file()
                    {
                        self.save_to(&p);
                    }
                }
                ui.separator();
                if ui.button("⟲ Undo").clicked() {
                    if let Some(m) = self.history.undo() {
                        self.model = m;
                    }
                }
                if ui.button("⟳ Redo").clicked() {
                    if let Some(m) = self.history.redo() {
                        self.model = m;
                    }
                }
                ui.separator();
                ui.label("SIMULATE:");
                ui.add(egui::DragValue::new(&mut self.t_stop).prefix("Stop Time (s): "));
                ui.label("(Run is below)");
                ui.separator();
                ui.label("PUBLISH:");
                {
                    let pub_resp = ui.button("Publish ▾");
                    if pub_resp.clicked() {
                        self.status = "Publish: use Save As for model JSON; Export .py for generated code (no FMU/Protect in this OSS subset)".into();
                    }
                    if pub_resp.on_hover_text("Export generated Python / model JSON").clicked() {
                        // fallthrough to status hint below
                    }
                }
                if ui.button("Export .py").clicked() {
                    // Reuse codegen run to materialize model.py in temp dir and report its path.
                    match self.api.run_codegen(&self.model, "oneshot", self.t_stop, self.dt) {
                        Ok(j) => self.status = format!("exported (codegen run ok): {}", j),
                        Err(e) => self.status = format!("export failed: {e}"),
                    }
                }
                if ui.button("Library").clicked() {
                    // focus left panel: clear search to show all
                    self.search.clear();
                }
                ui.separator();
                ui.label(self.file_path.clone().unwrap_or_else(|| "untitled".into()));
                ui.label(format!("zoom {:.0}%", self.canvas.zoom * 100.0));
                ui.checkbox(&mut self.dark_theme, "Dark");
            });
            });
        });

        egui::SidePanel::left("palette").show(ctx, |ui| {
            ui.heading("Library");
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search blocks…"));
            let q = self.search.to_lowercase();
            let mut cats: std::collections::BTreeMap<String, Vec<BlockDef>> =
                std::collections::BTreeMap::new();
            for def in self.palette.clone() {
                let hit = q.is_empty()
                    || def.label.to_lowercase().contains(&q)
                    || def.block_type.to_lowercase().contains(&q)
                    || def.description.to_lowercase().contains(&q);
                if hit {
                    cats.entry(def.category.clone()).or_default().push(def);
                }
            }
            for (cat, defs) in cats {
                egui::CollapsingHeader::new(if cat.is_empty() { "Other" } else { &cat })
                    .default_open(true)
                    .show(ui, |ui| {
                        for def in defs {
                            let label = format!("{} — {}", def.label, def.description);
                            let resp = ui.button(label);
                            if resp.clicked() {
                                self.add_block(&def);
                            }
                            if resp.double_clicked() {
                                self.add_block(&def);
                                let id = self.model.blocks.last().unwrap().id.clone();
                                self.dialog_block = Some(id);
                            }
                        }
                    });
            }
            if self.palette.is_empty() {
                ui.label(self.status.clone());
                ui.label("Backend unreachable — start: python backend/run.py");
            }
            ui.separator();
            ui.label("Double-click canvas = add Constant. Double-click block = parameters.");
        });

        egui::SidePanel::right("inspector").show(ctx, |ui| {
            ui.heading("Inspector");
            let selected = self.canvas.selected.clone();
            if let Some(id) = selected {
                if self.model.blocks.iter().any(|b| b.id == id) {
                    if ui.button("Open Parameters…").clicked() {
                        self.dialog_block = Some(id.clone());
                    }
                    if ui.button("Delete block").clicked() {
                        self.canvas.selected = Some(id.clone());
                        self.delete_selected();
                    }
                    ui.label(format!("id: {id}"));
                    ui.label("Edit parameters in the dialog or here:");
                    if let Some(block) = self.model.blocks.iter_mut().find(|b| b.id == id) {
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
                                        .add(
                                            egui::DragValue::new(&mut v).prefix(format!("{key}: ")),
                                        )
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
                                                .hint_text(key.clone()),
                                        )
                                        .changed()
                                    {
                                        block.params.insert(key.clone(), serde_json::json!(v));
                                    }
                                }
                                _ => {
                                    ui.label(format!("{key}: {val} (read-only here)"));
                                }
                            }
                        }
                    }
                }
            } else {
                ui.label("Select a block on the canvas.");
            }
        });

        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("▶ Run").clicked() {
                    let is_time = self.mode == "Time-stepped";
                    if self.use_codegen {
                        match self.api.run_codegen(
                            &self.model,
                            if is_time { "time" } else { "oneshot" },
                            self.t_stop,
                            self.dt,
                        ) {
                            Ok(json_text) => {
                                if is_time {
                                    match serde_json::from_str::<TimeSimResponse>(&json_text) {
                                        Ok(r) => {
                                            self.status = format!(
                                                "ok (codegen, time) — {} outputs",
                                                r.outputs.len()
                                            );
                                            self.time_results = Some(r);
                                        }
                                        Err(e) => self.status = format!("codegen parse: {e}"),
                                    }
                                } else {
                                    match serde_json::from_str::<SimulateResponse>(&json_text) {
                                        Ok(r) => {
                                            self.status =
                                                format!("ok (codegen) — outputs: {:?}", r.outputs);
                                            self.results = Some(r);
                                        }
                                        Err(e) => self.status = format!("codegen parse: {e}"),
                                    }
                                }
                                self.codegen_info = format!(
                                    "generated model.py in temp dir; stdout: {}",
                                    json_text
                                );
                            }
                            Err(e) => self.status = format!("codegen error: {e}"),
                        }
                    } else if is_time {
                        match self.api.simulate_time(&self.model, self.t_stop, self.dt) {
                            Ok(r) => {
                                self.status =
                                    format!("ok (http, time) — {} outputs", r.outputs.len());
                                self.time_results = Some(r);
                            }
                            Err(e) => self.status = format!("error: {e}"),
                        }
                    } else {
                        match self.api.simulate(&self.model) {
                            Ok(r) => {
                                self.status = format!("ok (http) — outputs: {:?}", r.outputs);
                                self.results = Some(r);
                            }
                            Err(e) => self.status = format!("error: {e}"),
                        }
                    }
                }
                ui.checkbox(&mut self.use_codegen, "codegen (no HTTP)");
                if ui.button("Refresh library").clicked() {
                    match self.api.fetch_blocks() {
                        Ok(p) => {
                            self.palette = p;
                            self.status = "backend connected".into();
                        }
                        Err(e) => self.status = e,
                    }
                }
                if ui.button("Clear").clicked() {
                    self.history.push(&self.model);
                    self.model.blocks.clear();
                    self.model.connections.clear();
                    self.results = None;
                    self.time_results = None;
                }
                egui::ComboBox::from_label("Mode")
                    .selected_text(&self.mode)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.mode, "One-shot".into(), "One-shot");
                        ui.selectable_value(&mut self.mode, "Time-stepped".into(), "Time-stepped");
                    });
                if self.mode == "Time-stepped" {
                    ui.add(egui::DragValue::new(&mut self.t_stop).prefix("t_stop: "));
                    ui.add(egui::DragValue::new(&mut self.dt).prefix("dt: "));
                }
            });
            ui.label(&self.status);
            // results table
            if let Some(r) = &self.results {
                egui::CollapsingHeader::new("Results (one-shot)")
                    .default_open(true)
                    .show(ui, |ui| {
                        for (k, v) in &r.outputs {
                            ui.label(format!("{k}: {v}"));
                        }
                        if !r.trace.is_empty() {
                            ui.label(format!("trace: {} entries", r.trace.len()));
                        }
                    });
            }
            if let Some(r) = &self.time_results {
                egui::CollapsingHeader::new("Results (time)")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label(format!("{} steps", r.time.len()));
                        for (k, v) in &r.outputs {
                            let first = v.first().cloned().unwrap_or(serde_json::json!(0.0));
                            let last = v.last().cloned().unwrap_or(serde_json::json!(0.0));
                            ui.label(format!("{k}: {first} … {last}"));
                        }
                        // plot first output series (manual draw, no egui_plot dep)
                        if let Some((_, vals)) = r.outputs.iter().next() {
                            let ys: Vec<f64> =
                                vals.iter().map(|y| y.as_f64().unwrap_or(0.0)).collect();
                            let ts: Vec<f64> = r.time.clone();
                            if ys.len() > 1 {
                                let (resp, painter) = ui.allocate_painter(
                                    egui::vec2(ui.available_width(), 120.0),
                                    egui::Sense::hover(),
                                );
                                let rect = resp.rect;
                                let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
                                let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                                let t0 = *ts.first().unwrap_or(&0.0);
                                let t1 = *ts.last().unwrap_or(&1.0);
                                let range_y = (max_y - min_y).max(1e-9);
                                let range_t = (t1 - t0).max(1e-9);
                                for w in ts.iter().zip(ys.iter()).collect::<Vec<_>>().windows(2) {
                                    let (ta, ya) = w[0];
                                    let (tb, yb) = w[1];
                                    let pa = egui::pos2(
                                        rect.min.x + ((*ta - t0) / range_t) as f32 * rect.width(),
                                        rect.max.y
                                            - ((ya - min_y) / range_y) as f32 * rect.height(),
                                    );
                                    let pb = egui::pos2(
                                        rect.min.x + ((*tb - t0) / range_t) as f32 * rect.width(),
                                        rect.max.y
                                            - ((yb - min_y) / range_y) as f32 * rect.height(),
                                    );
                                    painter.line_segment(
                                        [pa, pb],
                                        egui::Stroke::new(1.5_f32, egui::Color32::LIGHT_BLUE),
                                    );
                                }
                            }
                        }
                    });
            }
        });

        // parameter dialog
        if let Some(bid) = self.dialog_block.clone() {
            let def = self
                .palette
                .iter()
                .find(|d| {
                    self.model
                        .blocks
                        .iter()
                        .any(|b| b.id == bid && b.block_type == d.block_type)
                })
                .cloned();
            egui::Window::new("Block Parameters").show(ctx, |ui| {
                let block = self.model.blocks.iter_mut().find(|b| b.id == bid);
                if let Some(block) = block {
                    ui.label(format!("{} ({})", block.id, block.block_type));
                    if let Some(def) = def {
                        for p in def.params.clone() {
                            let current = block
                                .params
                                .get(&p.name)
                                .cloned()
                                .unwrap_or(p.default.clone());
                            match p.param_type.as_str() {
                                "number" => {
                                    let mut v = current.as_f64().unwrap_or(0.0);
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut v)
                                                .prefix(format!("{}: ", p.label)),
                                        )
                                        .changed()
                                    {
                                        block.params.insert(p.name.clone(), serde_json::json!(v));
                                    }
                                }
                                "integer" => {
                                    let mut v = current.as_i64().unwrap_or(0);
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut v)
                                                .prefix(format!("{}: ", p.label)),
                                        )
                                        .changed()
                                    {
                                        block.params.insert(p.name.clone(), serde_json::json!(v));
                                    }
                                }
                                "boolean" => {
                                    let mut v = current.as_bool().unwrap_or(false);
                                    if ui.checkbox(&mut v, &p.label).changed() {
                                        block.params.insert(p.name.clone(), serde_json::json!(v));
                                    }
                                }
                                "array" => {
                                    let mut v = current.to_string();
                                    if ui
                                        .add(
                                            egui::TextEdit::singleline(&mut v)
                                                .hint_text(format!("{} (JSON array)", p.label)),
                                        )
                                        .changed()
                                    {
                                        match serde_json::from_str::<serde_json::Value>(&v) {
                                            Ok(parsed) if parsed.is_array() => {
                                                block.params.insert(p.name.clone(), parsed);
                                                self.dialog_error.clear();
                                            }
                                            _ => {
                                                self.dialog_error =
                                                    "must be a JSON array, e.g. [1, 0, -1]".into()
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    let mut v = current.as_str().unwrap_or("").to_string();
                                    if ui
                                        .add(egui::TextEdit::singleline(&mut v).hint_text(&p.label))
                                        .changed()
                                    {
                                        block.params.insert(p.name.clone(), serde_json::json!(v));
                                    }
                                }
                            }
                        }
                    } else {
                        ui.label("(block def not in palette — editing raw params)");
                        let keys: Vec<String> = block.params.keys().cloned().collect();
                        for key in keys {
                            let val = block
                                .params
                                .get(&key)
                                .cloned()
                                .unwrap_or(serde_json::json!(0.0));
                            ui.label(format!("{key}: {val}"));
                        }
                    }
                    if !self.dialog_error.is_empty() {
                        ui.colored_label(egui::Color32::RED, &self.dialog_error);
                    }
                    if ui.button("Close").clicked() {
                        self.dialog_block = None;
                        self.dialog_error.clear();
                        self.history.push(&self.model);
                    }
                } else {
                    self.dialog_block = None;
                }
            });
                    }

                    // Context menu
                    if let (Some(pos), Some(target)) = (self.canvas.context_menu_pos, self.canvas.context_menu_target.clone()) {
                        egui::Area::new(egui::Id::new("context_menu"))
                            .fixed_pos(pos)
                            .order(egui::Order::Foreground)
                            .show(ctx, |ui| {
                                egui::Frame::menu(ui.style()).show(ui, |ui| {
                                    ui.set_min_width(180.0);
                                    match &target {
                                        canvas::ContextTarget::Block(bid) => {
                                            if ui.button("✂ Cut").clicked() {
                                                self.copy_selected();
                                                self.delete_selected();
                                                self.canvas.context_menu_pos = None;
                                            }
                                            if ui.button("📋 Copy").clicked() {
                                                self.copy_selected();
                                                self.canvas.context_menu_pos = None;
                                            }
                                            if ui.button("📄 Duplicate").clicked() {
                                                self.duplicate_selected();
                                                self.canvas.context_menu_pos = None;
                                            }
                                            ui.separator();
                                            if ui.button("🗑 Delete").clicked() {
                                                self.delete_selected();
                                                self.canvas.context_menu_pos = None;
                                            }
                                            ui.separator();
                                            if ui.button("⚙ Parameters…").clicked() {
                                                self.dialog_block = Some(bid.clone());
                                                self.canvas.context_menu_pos = None;
                                            }
                                        }
                                        canvas::ContextTarget::Wire(idx) => {
                                            if ui.button("🗑 Delete Wire").clicked() {
                                                if *idx < self.model.connections.len() {
                                                    self.history.push(&self.model);
                                                    self.model.connections.remove(*idx);
                                                }
                                                self.canvas.context_menu_pos = None;
                                                self.canvas.selected_wire = None;
                                            }
                                        }
                                        canvas::ContextTarget::Canvas(model_pos) => {
                                            if ui.button("➕ Add Constant").clicked() {
                                                let id = canvas::next_id(&mut self.canvas, "blk");
                                                self.history.push(&self.model);
                                                self.model.blocks.push(Block {
                                                    id,
                                                    block_type: "Constant".into(),
                                                    position: crate::model::Position {
                                                        x: model_pos.x - canvas::BLOCK_W / 2.0,
                                                        y: model_pos.y - canvas::BLOCK_H / 2.0,
                                                    },
                                                    params: HashMap::from([("value".to_string(), serde_json::json!(1.0))]),
                                                });
                                                self.canvas.context_menu_pos = None;
                                            }
                                            if ui.button("📋 Paste").clicked() {
                                                self.paste_at_cursor();
                                                self.canvas.context_menu_pos = None;
                                            }
                                        }
                                    }
                                    // Close on click outside
                                    if ui.input(|i| i.pointer.any_click() && !ui.rect_contains_pointer(ui.max_rect())) {
                                        self.canvas.context_menu_pos = None;
                                    }
                                });
                            });
                    }

                    egui::CentralPanel::default().show(ctx, |ui| {
            canvas::draw_canvas(
                ui,
                &mut self.model,
                &mut self.canvas,
                &self.palette,
                &mut self.history,
            );
            if let Some(id) = self.canvas.pending_dialog.take() {
                self.dialog_block = Some(id);
            }
        });
    }
}
