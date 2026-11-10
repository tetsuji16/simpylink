//! Model data structures (JSON format v1, see DES-02).
use crate::geom::{Point, Rect};
use crate::ids::{BlockId, LineId};
use crate::path::SystemPath;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Format marker written to every model file.
pub const MODEL_FORMAT: &str = "simpylink-model";
/// Oldest model format version written; files that use no v2 feature are still written as v1.
pub const MODEL_FORMAT_VERSION: u32 = 1;
/// Newest model format version this build reads and writes (adds `variables`,
/// fixed-step solvers and `SimConfig::fixed_step`, see #94/#99).
pub const MODEL_FORMAT_VERSION_MAX: u32 = 2;

/// A complete model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub format: String,
    pub format_version: u32,
    pub name: String,
    pub config: SimConfig,
    pub next_sid: u32,
    pub next_line_id: u32,
    pub root: System,
    /// Model variables: name → expression (format v2, #94). Empty in v1 files.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, String>,
}

/// Simulation settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimConfig {
    pub start_time: f64,
    pub stop_time: f64,
    pub solver: SolverKind,
    pub rel_tol: f64,
    pub abs_tol: f64,
    pub max_step: Option<f64>,
    pub output_step: f64,
    /// Fixed step size for fixed-step solvers; `None` = auto (#99). Format v2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_step: Option<f64>,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            start_time: 0.0,
            stop_time: 10.0,
            solver: SolverKind::Auto,
            rel_tol: 1e-3,
            abs_tol: 1e-6,
            max_step: None,
            output_step: 0.01,
            fixed_step: None,
        }
    }
}

/// Solver selection. Variable-step solvers map to SciPy methods; fixed-step solvers
/// (format v2, #99) are integrated by the generated runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolverKind {
    Auto,
    Ode45,
    Ode23,
    Ode15s,
    /// Fixed-step, no continuous states allowed (Simulink "discrete (no continuous states)").
    FixedDiscrete,
    /// Fixed-step Euler.
    Ode1,
    /// Fixed-step Heun.
    Ode2,
    /// Fixed-step Bogacki-Shampine.
    Ode3,
    /// Fixed-step classic Runge-Kutta.
    Ode4,
    /// Fixed-step Dormand-Prince.
    Ode5,
}

impl SolverKind {
    /// All solvers in UI order.
    pub const ALL: [SolverKind; 10] = [
        SolverKind::Auto,
        SolverKind::Ode45,
        SolverKind::Ode23,
        SolverKind::Ode15s,
        SolverKind::FixedDiscrete,
        SolverKind::Ode1,
        SolverKind::Ode2,
        SolverKind::Ode3,
        SolverKind::Ode4,
        SolverKind::Ode5,
    ];
    /// Display label.
    pub fn label(&self) -> &'static str {
        match self {
            SolverKind::Auto => "auto (RK45)",
            SolverKind::Ode45 => "ode45 (RK45)",
            SolverKind::Ode23 => "ode23 (RK23)",
            SolverKind::Ode15s => "ode15s (BDF)",
            SolverKind::FixedDiscrete => "discrete (fixed-step, no continuous states)",
            SolverKind::Ode1 => "ode1 (Euler, fixed-step)",
            SolverKind::Ode2 => "ode2 (Heun, fixed-step)",
            SolverKind::Ode3 => "ode3 (Bogacki-Shampine, fixed-step)",
            SolverKind::Ode4 => "ode4 (Runge-Kutta, fixed-step)",
            SolverKind::Ode5 => "ode5 (Dormand-Prince, fixed-step)",
        }
    }
    /// Simulink `SolverName` value for this solver.
    pub fn simulink_name(&self) -> &'static str {
        match self {
            SolverKind::Auto => "VariableStepAuto",
            SolverKind::Ode45 => "ode45",
            SolverKind::Ode23 => "ode23",
            SolverKind::Ode15s => "ode15s",
            SolverKind::FixedDiscrete => "FixedStepDiscrete",
            SolverKind::Ode1 => "ode1",
            SolverKind::Ode2 => "ode2",
            SolverKind::Ode3 => "ode3",
            SolverKind::Ode4 => "ode4",
            SolverKind::Ode5 => "ode5",
        }
    }
    /// True for fixed-step solvers.
    pub fn is_fixed_step(&self) -> bool {
        !matches!(
            self,
            SolverKind::Auto | SolverKind::Ode45 | SolverKind::Ode23 | SolverKind::Ode15s
        )
    }
}

/// A (sub)system: blocks and lines. Invariants: blocks sorted by sid, lines by id.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct System {
    pub blocks: Vec<Block>,
    pub lines: Vec<Line>,
}

/// A block instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub sid: BlockId,
    pub block_type: String,
    pub name: String,
    pub position: Rect,
    #[serde(default)]
    pub orientation: Orientation,
    pub params: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<System>,
}

/// Block orientation (only Right is drawn in v0.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Right,
    Left,
    Up,
    Down,
}

impl Orientation {
    /// Ctrl+R: clockwise rotation.
    pub fn rotated_cw(self) -> Self {
        match self {
            Orientation::Right => Orientation::Down,
            Orientation::Down => Orientation::Left,
            Orientation::Left => Orientation::Up,
            Orientation::Up => Orientation::Right,
        }
    }
    /// Ctrl+Shift+R: counter-clockwise rotation.
    pub fn rotated_ccw(self) -> Self {
        self.rotated_cw().rotated_cw().rotated_cw()
    }
    /// Ctrl+I: flip.
    pub fn flipped(self) -> Self {
        match self {
            Orientation::Right => Orientation::Left,
            Orientation::Left => Orientation::Right,
            Orientation::Up => Orientation::Down,
            Orientation::Down => Orientation::Up,
        }
    }
}

/// Port reference; port numbers start at 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PortRef {
    pub block: BlockId,
    pub port: u32,
}

/// Branch tap position on the parent polyline.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TapPoint {
    pub segment: u32,
    pub t: f64,
}

/// A signal line from one output port.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub id: LineId,
    pub src: PortRef,
    #[serde(default)]
    pub points: Vec<Point>,
    #[serde(default)]
    pub dst: Option<PortRef>,
    #[serde(default)]
    pub branches: Vec<Branch>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
}

/// A branch of a line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub tap: TapPoint,
    #[serde(default)]
    pub points: Vec<Point>,
    #[serde(default)]
    pub dst: Option<PortRef>,
    #[serde(default)]
    pub branches: Vec<Branch>,
}

impl Model {
    /// Lowest format version that can represent this model (1 unless a v2 feature is used).
    pub fn required_format_version(&self) -> u32 {
        if !self.variables.is_empty()
            || self.config.solver.is_fixed_step()
            || self.config.fixed_step.is_some()
        {
            2
        } else {
            MODEL_FORMAT_VERSION
        }
    }
    /// Empty model with default configuration.
    pub fn new(name: &str) -> Model {
        Model {
            format: MODEL_FORMAT.to_string(),
            format_version: MODEL_FORMAT_VERSION,
            name: name.to_string(),
            config: SimConfig::default(),
            next_sid: 1,
            next_line_id: 1,
            root: System::default(),
            variables: BTreeMap::new(),
        }
    }
    /// Allocates a new SID.
    pub fn alloc_sid(&mut self) -> BlockId {
        let id = BlockId(self.next_sid);
        self.next_sid += 1;
        id
    }
    /// Allocates a new line id.
    pub fn alloc_line_id(&mut self) -> LineId {
        let id = LineId(self.next_line_id);
        self.next_line_id += 1;
        id
    }
    /// System at `path`.
    pub fn system(&self, path: &SystemPath) -> Option<&System> {
        let mut sys = &self.root;
        for sid in &path.0 {
            sys = sys.block(*sid)?.system.as_ref()?;
        }
        Some(sys)
    }
    /// Mutable system at `path`.
    pub fn system_mut(&mut self, path: &SystemPath) -> Option<&mut System> {
        let mut sys = &mut self.root;
        for sid in &path.0 {
            sys = sys.block_mut(*sid)?.system.as_mut()?;
        }
        Some(sys)
    }
    /// Largest SID in all levels (0 if none).
    pub fn max_sid(&self) -> u32 {
        fn rec(s: &System) -> u32 {
            s.blocks
                .iter()
                .map(|b| b.sid.0.max(b.system.as_ref().map(rec).unwrap_or(0)))
                .max()
                .unwrap_or(0)
        }
        rec(&self.root)
    }
    /// Largest line id in all levels (0 if none).
    pub fn max_line_id(&self) -> u32 {
        fn rec(s: &System) -> u32 {
            let l = s.lines.iter().map(|l| l.id.0).max().unwrap_or(0);
            let b = s
                .blocks
                .iter()
                .filter_map(|b| b.system.as_ref().map(rec))
                .max()
                .unwrap_or(0);
            l.max(b)
        }
        rec(&self.root)
    }
}

impl System {
    /// Block by sid.
    pub fn block(&self, sid: BlockId) -> Option<&Block> {
        self.blocks.iter().find(|b| b.sid == sid)
    }
    /// Mutable block by sid.
    pub fn block_mut(&mut self, sid: BlockId) -> Option<&mut Block> {
        self.blocks.iter_mut().find(|b| b.sid == sid)
    }
    /// Line by id.
    pub fn line(&self, id: LineId) -> Option<&Line> {
        self.lines.iter().find(|l| l.id == id)
    }
    /// Mutable line by id.
    pub fn line_mut(&mut self, id: LineId) -> Option<&mut Line> {
        self.lines.iter_mut().find(|l| l.id == id)
    }
    /// Inserts keeping sid order.
    pub fn insert_block_sorted(&mut self, b: Block) {
        let idx = self.blocks.partition_point(|x| x.sid <= b.sid);
        self.blocks.insert(idx, b);
    }
    /// Inserts keeping id order.
    pub fn insert_line_sorted(&mut self, l: Line) {
        let idx = self.lines.partition_point(|x| x.id <= l.id);
        self.lines.insert(idx, l);
    }
    /// Block by exact name.
    pub fn block_by_name(&self, name: &str) -> Option<&Block> {
        self.blocks.iter().find(|b| b.name == name)
    }
}

impl Line {
    /// All destinations (depth first, declaration order).
    pub fn destinations(&self) -> Vec<PortRef> {
        fn rec(bs: &[Branch], out: &mut Vec<PortRef>) {
            for b in bs {
                if let Some(d) = b.dst {
                    out.push(d);
                }
                rec(&b.branches, out);
            }
        }
        let mut out = vec![];
        if let Some(d) = self.dst {
            out.push(d);
        }
        rec(&self.branches, &mut out);
        out
    }
}
