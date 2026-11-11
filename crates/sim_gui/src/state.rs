//! Application state (egui independent).
use crate::canvas::controller::{CanvasAction, CanvasController};
use crate::dialogs::block_params::ParamDialogState;
use crate::dialogs::model_settings::SettingsDialogState;
use crate::dialogs::quick_insert::QuickInsertState;
use crate::dialogs::{DialogButton, DialogOutcome};
use crate::sim::{SimSession, SimUpdate, StartError};
use sim_core::clipboard::{
    copy_selection, paste_payload, payload_origin, payload_to_text, ClipboardPayload,
};
use sim_core::diag::{Diagnostic, Severity};
use sim_core::edit::{Command, History};
use sim_core::geom::Point;
use sim_core::ids::BlockId;
use sim_core::library::{fill_default_params, new_block, spec};
use sim_core::model::{Model, System};
use sim_core::path::SystemPath;
use sim_runner::{RunStatus, SimResults};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// An open model.
pub struct Document {
    pub model: Model,
    pub file: Option<PathBuf>,
    pub history: History,
    pub path: SystemPath,
    /// SLX data kept for export (M5).
    pub slx: Option<sim_slx::PreservedData>,
}

impl Document {
    /// New untitled model.
    pub fn new_untitled() -> Self {
        Document {
            model: Model::new("untitled"),
            file: None,
            history: History::new(200),
            path: SystemPath::root(),
            slx: None,
        }
    }
}

/// Action awaiting the "Save changes?" answer.
#[derive(Debug, Clone, PartialEq)]
pub enum PendingAction {
    New,
    Open(PathBuf),
    Quit,
}

/// Result of save().
#[derive(Debug, Clone, PartialEq)]
pub enum SaveResult {
    Saved,
    NeedsPath,
}

/// Generated code viewer.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeViewState {
    pub source: String,
    pub file_name: String,
    pub diagnostics: Vec<Diagnostic>,
    pub revision: u64,
}

/// All application state.
pub struct AppState {
    pub doc: Document,
    pub canvas: CanvasController,
    pub show_library: bool,
    pub show_bottom: bool,
    pub status: String,
    pub clipboard: Option<ClipboardPayload>,
    pub pending_system_copy: Option<String>,
    pub param_dialog: Option<ParamDialogState>,
    pub settings_dialog: Option<SettingsDialogState>,
    pub quick_insert: Option<QuickInsertState>,
    pub confirm: Option<PendingAction>,
    pub sim: SimSession,
    pub diagnostics: Vec<Diagnostic>,
    pub results: Option<SimResults>,
    pub run_log: Vec<String>,
    pub open_scopes: BTreeSet<BlockId>,
    pub scope_reset: bool,
    pub code_view: Option<CodeViewState>,
    pub revision: u64,
    pub library_filter: String,
    pub python_error: Option<String>,
    pub import_report: Option<Vec<String>>,
    pub quit_confirmed: bool,
    /// Fit the view (max 100%) once the canvas size is known (set after opening a file).
    pub fit_pending: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

fn find_block_path(sys: &System, sid: BlockId, path: &SystemPath) -> Option<SystemPath> {
    if sys.block(sid).is_some() {
        return Some(path.clone());
    }
    for b in &sys.blocks {
        if let Some(c) = &b.system {
            if let Some(p) = find_block_path(c, sid, &path.child(b.sid)) {
                return Some(p);
            }
        }
    }
    None
}

impl AppState {
    /// Fresh state.
    pub fn new() -> Self {
        AppState {
            doc: Document::new_untitled(),
            canvas: CanvasController::new(),
            show_library: true,
            show_bottom: false,
            status: "Ready".into(),
            clipboard: None,
            pending_system_copy: None,
            param_dialog: None,
            settings_dialog: None,
            quick_insert: None,
            confirm: None,
            sim: SimSession::new(),
            diagnostics: vec![],
            results: None,
            run_log: vec![],
            open_scopes: BTreeSet::new(),
            scope_reset: false,
            code_view: None,
            revision: 0,
            library_filter: String::new(),
            python_error: None,
            import_report: None,
            quit_confirmed: false,
            fit_pending: false,
        }
    }

    /// The system being edited.
    pub fn current_system(&self) -> &System {
        self.doc
            .model
            .system(&self.doc.path)
            .unwrap_or(&self.doc.model.root)
    }

    fn after_change(&mut self) {
        self.revision += 1;
        if self.doc.model.system(&self.doc.path).is_none() {
            self.doc.path = SystemPath::root();
        }
        let sys = self
            .doc
            .model
            .system(&self.doc.path)
            .cloned()
            .unwrap_or_default();
        self.canvas.retain_existing(&sys);
        if let Some(d) = &self.param_dialog {
            if self
                .doc
                .model
                .system(&d.path)
                .and_then(|s| s.block(d.block))
                .is_none()
            {
                self.param_dialog = None;
            }
        }
        let root_ids: BTreeSet<BlockId> = all_sids(&self.doc.model.root);
        self.open_scopes.retain(|s| root_ids.contains(s));
    }

    /// Executes a command through the history.
    pub fn execute(&mut self, cmd: Command) -> bool {
        match self.doc.history.execute(&mut self.doc.model, cmd) {
            Ok(()) => {
                self.after_change();
                true
            }
            Err(e) => {
                self.status = format!("Edit failed: {e}");
                false
            }
        }
    }

    /// Undo.
    pub fn undo(&mut self) {
        match self.doc.history.undo(&mut self.doc.model) {
            Ok(true) => self.after_change(),
            Ok(false) => self.status = "Nothing to undo".into(),
            Err(e) => self.status = format!("Undo failed: {e}"),
        }
    }

    /// Redo.
    pub fn redo(&mut self) {
        match self.doc.history.redo(&mut self.doc.model) {
            Ok(true) => self.after_change(),
            Ok(false) => self.status = "Nothing to redo".into(),
            Err(e) => self.status = format!("Redo failed: {e}"),
        }
    }

    /// Handles a list of canvas actions.
    pub fn handle_actions(&mut self, actions: Vec<CanvasAction>) {
        for a in actions {
            self.handle_action(a);
        }
    }

    /// Handles one canvas action.
    pub fn handle_action(&mut self, action: CanvasAction) {
        match action {
            CanvasAction::Execute(cmd) => {
                self.execute(cmd);
            }
            CanvasAction::Undo => self.undo(),
            CanvasAction::Redo => self.redo(),
            CanvasAction::Status(s) => self.status = s,
            CanvasAction::FitToView => {}
            CanvasAction::OpenBlockDialog(sid) => self.open_block(sid),
            CanvasAction::Copy => self.copy(),
            CanvasAction::Cut => {
                self.copy();
                let blocks: Vec<BlockId> = self.canvas.selection.blocks.iter().copied().collect();
                if !blocks.is_empty() {
                    self.execute(Command::DeleteItems {
                        path: self.doc.path.clone(),
                        blocks,
                        lines: vec![],
                    });
                }
            }
            CanvasAction::Paste { at } => self.paste(at),
            CanvasAction::DuplicateBlocks { blocks, dx, dy } => {
                let p = copy_selection(self.current_system(), &blocks.into_iter().collect());
                self.paste_payload(&p, (dx, dy));
            }
            CanvasAction::QuickInsert { at } => self.quick_insert = Some(QuickInsertState::new(at)),
            CanvasAction::OpenSubsystem(sid) => self.open_subsystem(sid),
            CanvasAction::GoToParent => self.go_to_parent(),
            CanvasAction::CreateSubsystem => self.create_subsystem(),
        }
    }

    /// Opens the dialog / scope / subsystem for a block.
    pub fn open_block(&mut self, sid: BlockId) {
        let Some(b) = self.current_system().block(sid) else {
            return;
        };
        let (is_sub, block_type, name) = (b.system.is_some(), b.block_type.clone(), b.name.clone());
        if is_sub {
            self.open_subsystem(sid);
        } else if block_type == "Scope" {
            self.open_scopes.insert(sid);
        } else {
            self.param_dialog = ParamDialogState::open(&self.doc.model, &self.doc.path, sid);
            if self.param_dialog.is_none() {
                self.status = format!("No parameters for '{name}'");
            }
        }
    }

    /// Opens the parameter dialog regardless of block kind.
    pub fn open_parameters(&mut self, sid: BlockId) {
        if let Some(p) = find_block_path(&self.doc.model.root, sid, &SystemPath::root()) {
            self.param_dialog = ParamDialogState::open(&self.doc.model, &p, sid);
        }
    }

    /// Dialog button pressed.
    pub fn param_dialog_press(&mut self, b: DialogButton) {
        let Some(d) = self.param_dialog.as_mut() else {
            return;
        };
        match d.press(b) {
            DialogOutcome::KeepOpen => {}
            DialogOutcome::Close => self.param_dialog = None,
            DialogOutcome::CloseAndExecute(c) => {
                self.param_dialog = None;
                self.execute(c);
            }
            DialogOutcome::Execute(c) => {
                self.execute(c);
            }
        }
    }

    /// Settings dialog button pressed.
    pub fn settings_dialog_press(&mut self, b: DialogButton) {
        let Some(d) = self.settings_dialog.as_mut() else {
            return;
        };
        match d.press(b) {
            DialogOutcome::KeepOpen => {}
            DialogOutcome::Close => self.settings_dialog = None,
            DialogOutcome::CloseAndExecute(c) => {
                self.settings_dialog = None;
                self.execute(c);
            }
            DialogOutcome::Execute(c) => {
                self.execute(c);
            }
        }
    }

    /// Adds a block centered at `center` (model coordinates).
    pub fn add_block_at(&mut self, block_type: &str, center: Point) -> Option<BlockId> {
        let Some(s) = spec(block_type) else {
            self.status = format!("unknown block type '{block_type}'");
            return None;
        };
        let (w, h) = s.default_size;
        let tl = Point::new((center.x - w / 2.0).round(), (center.y - h / 2.0).round());
        let sid = BlockId(self.doc.model.next_sid);
        let mut b = match new_block(self.current_system(), block_type, sid, tl) {
            Ok(b) => b,
            Err(e) => {
                self.status = e;
                return None;
            }
        };
        if block_type == "SubSystem" {
            b.system = Some(default_subsystem(&self.doc.model, sid));
        }
        if self.execute(Command::AddBlock {
            path: self.doc.path.clone(),
            block: b,
        }) {
            self.canvas.selection.select_only_block(sid);
            Some(sid)
        } else {
            None
        }
    }

    /// Window title.
    pub fn window_title(&self) -> String {
        let dirty = if self.doc.history.is_dirty() { "*" } else { "" };
        let mut sub = String::new();
        if !self.doc.path.is_root() {
            sub = format!(" / {}", self.breadcrumbs().join(" / "));
        }
        format!("{dirty}{}{sub} - Simpylink", self.doc.model.name)
    }

    /// Names of subsystems from the root to the current system.
    pub fn breadcrumbs(&self) -> Vec<String> {
        let mut sys = &self.doc.model.root;
        let mut out = vec![];
        for sid in &self.doc.path.0 {
            let Some(b) = sys.block(*sid) else { break };
            out.push(b.name.clone());
            let Some(c) = &b.system else { break };
            sys = c;
        }
        out
    }

    // ---------------------------------------------------------- clipboard
    fn copy(&mut self) {
        if self.canvas.selection.blocks.is_empty() {
            return;
        }
        let p = copy_selection(self.current_system(), &self.canvas.selection.blocks);
        self.pending_system_copy = Some(payload_to_text(&p));
        self.status = format!("Copied {} block(s)", p.blocks.len());
        self.clipboard = Some(p);
    }

    fn paste(&mut self, at: Point) {
        let Some(p) = self.clipboard.clone() else {
            self.status = "Clipboard is empty".into();
            return;
        };
        let o = payload_origin(&p).unwrap_or_default();
        self.paste_payload(&p, ((at.x - o.x).round(), (at.y - o.y).round()));
    }

    /// Pastes a payload with an offset.
    pub fn paste_payload(&mut self, p: &ClipboardPayload, offset: (f64, f64)) {
        match paste_payload(&self.doc.model, &self.doc.path, p, offset) {
            Ok((cmd, ids)) => {
                if self.execute(cmd) {
                    self.canvas.selection.blocks = ids.into_iter().collect();
                    self.canvas.selection.lines.clear();
                }
            }
            Err(e) => self.status = format!("Paste failed: {e}"),
        }
    }

    // ---------------------------------------------------------- subsystems
    /// Enter a subsystem.
    pub fn open_subsystem(&mut self, sid: BlockId) {
        let child = self.doc.path.child(sid);
        if self.doc.model.system(&child).is_some() {
            self.doc.path = child;
            self.canvas.clear_selection();
            self.canvas.reset_interaction();
            let sys = self.current_system().clone();
            self.canvas.fit_to_view_limited(&sys, 1.5);
        }
    }

    /// Go to the parent system.
    pub fn go_to_parent(&mut self) {
        if let Some(last) = self.doc.path.0.pop() {
            self.canvas.selection.select_only_block(last);
            self.canvas.reset_interaction();
        }
    }

    /// Ctrl+G.
    pub fn create_subsystem(&mut self) {
        let sel = self.canvas.selection.blocks.clone();
        match sim_core::subsystem::create_subsystem_command(&self.doc.model, &self.doc.path, &sel) {
            Ok((cmd, sid)) => {
                if self.execute(cmd) {
                    self.canvas.selection.select_only_block(sid);
                }
            }
            Err(e) => self.status = e,
        }
    }

    // ---------------------------------------------------------- files
    /// New untitled document.
    pub fn new_document(&mut self) {
        self.doc = Document::new_untitled();
        self.reset_view_state();
        self.status = "New model".into();
    }

    fn reset_view_state(&mut self) {
        // A run of the previous document must not deliver results to this one.
        self.sim.abandon();
        let viewport = self.canvas.viewport;
        self.canvas = CanvasController::new();
        self.canvas.viewport = viewport;
        self.results = None;
        self.diagnostics.clear();
        self.open_scopes.clear();
        self.param_dialog = None;
        self.code_view = None;
        self.revision += 1;
    }

    /// Opens a .json (or .slx) file.
    pub fn open_path(&mut self, path: &Path) -> Result<(), String> {
        let is_slx = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("slx"))
            .unwrap_or(false);
        let (mut model, slx, report) = if is_slx {
            let imp = sim_slx::read_slx(path).map_err(|e| {
                let msg = format!("Could not open {}: {e}", path.display());
                self.status = msg.clone();
                msg
            })?;
            (imp.model, Some(imp.preserved), Some(imp.report.lines()))
        } else {
            let m = sim_core::json::load_from_path(path).map_err(|e| {
                let msg = format!("Could not open {}: {e}", path.display());
                self.status = msg.clone();
                msg
            })?;
            (m, None, None)
        };
        fill_all_defaults(&mut model.root);
        self.doc = Document {
            model,
            file: if is_slx {
                None
            } else {
                Some(path.to_path_buf())
            },
            history: History::new(200),
            path: SystemPath::root(),
            slx,
        };
        self.reset_view_state();
        let sys = self.doc.model.root.clone();
        self.canvas.fit_to_view_limited(&sys, 1.0);
        self.fit_pending = true;
        self.import_report = report;
        self.status = format!("Opened {}", path.display());
        Ok(())
    }

    /// Save to the current file.
    pub fn save(&mut self) -> Result<SaveResult, String> {
        match self.doc.file.clone() {
            None => Ok(SaveResult::NeedsPath),
            Some(p) => self.save_as(&p).map(|_| SaveResult::Saved),
        }
    }

    /// Save to a new path (adds .json if no extension).
    pub fn save_as(&mut self, path: &Path) -> Result<(), String> {
        let mut p = path.to_path_buf();
        if p.extension().is_none() {
            p.set_extension("json");
        }
        sim_core::json::save_to_path(&self.doc.model, &p).map_err(|e| {
            let m = format!("Could not save {}: {e}", p.display());
            self.status = m.clone();
            m
        })?;
        self.doc.file = Some(p.clone());
        self.doc.history.mark_saved();
        self.status = format!("Saved {}", p.display());
        Ok(())
    }

    /// Export as SLX (never overwrites).
    pub fn export_slx(&mut self, path: &Path) -> Result<(), String> {
        let mut p = path.to_path_buf();
        if p.extension().is_none() {
            p.set_extension("slx");
        }
        sim_slx::write_slx(&self.doc.model, self.doc.slx.as_ref(), &p).map_err(|e| {
            let m = format!("Could not export {}: {e}", p.display());
            self.status = m.clone();
            m
        })?;
        self.status = format!(
            "Exported {} (SLX export is experimental and not verified in Simulink)",
            p.display()
        );
        Ok(())
    }

    /// Requests an action that may discard changes.
    pub fn request(&mut self, action: PendingAction) -> bool {
        if self.doc.history.is_dirty() {
            self.confirm = Some(action);
            false
        } else {
            self.perform(action);
            true
        }
    }

    /// Performs a pending action without asking.
    pub fn perform(&mut self, action: PendingAction) {
        self.confirm = None;
        match action {
            PendingAction::New => self.new_document(),
            PendingAction::Open(p) => {
                let _shown_in_status = self.open_path(&p);
            }
            PendingAction::Quit => {
                self.sim.abandon();
                self.quit_confirmed = true;
            }
        }
    }

    // ---------------------------------------------------------- simulation
    /// Ctrl+D.
    pub fn update_diagram(&mut self) {
        let (d, _) = SimSession::update_diagram(&self.doc.model);
        let (e, w) = count(&d);
        self.diagnostics = d;
        self.status = format!("Update: {e} errors, {w} warnings");
    }

    /// Ctrl+T / F5.
    pub fn start_run(&mut self) {
        if self.sim.is_running() {
            return;
        }
        match self.sim.start(&self.doc.model) {
            Ok(d) => {
                self.diagnostics = d;
                self.run_log.clear();
                self.status = "Running…".into();
            }
            Err(StartError::Compile(d)) => {
                let (e, w) = count(&d);
                self.diagnostics = d;
                self.status = format!("Cannot run: {e} errors, {w} warnings (see Diagnostics)");
                self.show_bottom = true;
            }
            Err(StartError::Python(m)) => {
                self.status = "Python environment problem".into();
                self.python_error = Some(m);
            }
            Err(StartError::Io(m)) => self.status = format!("Cannot write the script: {m}"),
            Err(StartError::AlreadyRunning) => {}
        }
    }

    /// Opens the Scope windows of every Scope block (Data Inspector button).
    pub fn open_all_scopes(&mut self) {
        self.open_scopes = scope_sids(&self.doc.model.root);
        self.scope_reset = true;
    }

    /// Ctrl+Shift+T.
    pub fn stop_run(&mut self) {
        self.sim.stop();
    }

    /// Called every frame.
    pub fn poll_sim(&mut self) {
        match self.sim.poll() {
            SimUpdate::None => {}
            SimUpdate::Log(l) => self.run_log.extend(l),
            SimUpdate::Done {
                results,
                summary,
                log,
            } => {
                self.run_log.extend(log);
                self.status = match &summary.status {
                    RunStatus::Succeeded if summary.error.is_none() => {
                        format!(
                            "Simulation finished in {:.2} s ({} samples)",
                            summary.elapsed.as_secs_f64(),
                            summary.samples
                        )
                    }
                    RunStatus::Stopped => "Stopped".into(),
                    _ => format!(
                        "Simulation failed: {}",
                        summary.error.clone().unwrap_or_default()
                    ),
                };
                if summary.error.is_some() {
                    self.show_bottom = true;
                }
                if results.is_some() {
                    self.results = results;
                    self.scope_reset = true;
                    if self.open_scopes.is_empty() {
                        self.open_scopes = scope_sids(&self.doc.model.root);
                    }
                }
            }
        }
    }

    /// Generated code view refresh.
    pub fn refresh_code_view(&mut self) {
        let (diagnostics, code) = SimSession::update_diagram(&self.doc.model);
        let (source, file_name) = code.map(|c| (c.source, c.file_name)).unwrap_or_default();
        self.code_view = Some(CodeViewState {
            source,
            file_name,
            diagnostics,
            revision: self.revision,
        });
    }

    /// File → Export Python…
    pub fn export_python(&mut self, path: &Path) -> Result<(), String> {
        let (d, code) = SimSession::update_diagram(&self.doc.model);
        let Some(code) = code else {
            return Err(d
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join("\n"));
        };
        let mut p = path.to_path_buf();
        if p.extension().is_none() {
            p.set_extension("py");
        }
        std::fs::write(&p, code.source)
            .map_err(|e| format!("Could not write {}: {e}", p.display()))?;
        self.status = format!("Exported {}", p.display());
        Ok(())
    }
}

fn count(d: &[Diagnostic]) -> (usize, usize) {
    (
        d.iter().filter(|x| x.severity == Severity::Error).count(),
        d.iter().filter(|x| x.severity == Severity::Warning).count(),
    )
}

fn all_sids(sys: &System) -> BTreeSet<BlockId> {
    let mut s = BTreeSet::new();
    for b in &sys.blocks {
        s.insert(b.sid);
        if let Some(c) = &b.system {
            s.extend(all_sids(c));
        }
    }
    s
}

fn scope_sids(sys: &System) -> BTreeSet<BlockId> {
    let mut s = BTreeSet::new();
    for b in &sys.blocks {
        if b.block_type == "Scope" {
            s.insert(b.sid);
        }
        if let Some(c) = &b.system {
            s.extend(scope_sids(c));
        }
    }
    s
}

fn fill_all_defaults(sys: &mut System) {
    for b in &mut sys.blocks {
        fill_default_params(b);
        if let Some(c) = b.system.as_mut() {
            fill_all_defaults(c);
        }
    }
}

/// Default content of a new Subsystem: In1 → Out1.
fn default_subsystem(model: &Model, sid: BlockId) -> System {
    let mut s = System::default();
    let in_sid = BlockId(sid.0 + 1);
    let out_sid = BlockId(sid.0 + 2);
    if let (Ok(i), Ok(o)) = (
        new_block(&s, "Inport", in_sid, Point::new(40.0, 50.0)),
        new_block(
            &System::default(),
            "Outport",
            out_sid,
            Point::new(160.0, 50.0),
        ),
    ) {
        s.insert_block_sorted(i);
        s.insert_block_sorted(o);
        s.lines.push(sim_core::model::Line {
            id: sim_core::ids::LineId(model.next_line_id),
            src: sim_core::model::PortRef {
                block: in_sid,
                port: 1,
            },
            points: vec![],
            dst: Some(sim_core::model::PortRef {
                block: out_sid,
                port: 1,
            }),
            branches: vec![],
            name: String::new(),
        });
    }
    s
}
