//! Simulation IR types.
use sim_core::expr::Matrix;
use sim_core::ids::BlockId;
use sim_core::model::PortRef;

/// Signal identifier (one per output port).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignalId(pub u32);

/// Resolved sample time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SampleTime {
    Constant,
    Continuous,
    Discrete { period: f64, offset: f64 },
}

/// The compiled model.
#[derive(Debug, Clone, PartialEq)]
pub struct SimIr {
    pub model_name: String,
    pub config: IrConfig,
    pub blocks: Vec<IrBlock>,
    pub signals: Vec<IrSignal>,
    pub num_cont_states: usize,
    pub num_disc_states: usize,
    pub discrete_rates: Vec<(f64, f64)>,
    pub breakpoints: Vec<f64>,
    pub logs: Vec<IrLog>,
}

/// Solver configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct IrConfig {
    pub start_time: f64,
    pub stop_time: f64,
    pub output_step: f64,
    pub solver_method: String,
    pub rel_tol: f64,
    pub abs_tol: f64,
    pub max_step: Option<f64>,
}

/// One signal.
#[derive(Debug, Clone, PartialEq)]
pub struct IrSignal {
    pub id: SignalId,
    pub source: PortRef,
    pub var_name: String,
}

/// One non-virtual block in execution order.
#[derive(Debug, Clone, PartialEq)]
pub struct IrBlock {
    pub sid: BlockId,
    pub name: String,
    pub block_type: String,
    pub var_base: String,
    pub kind: IrBlockKind,
    pub inputs: Vec<Option<SignalId>>,
    pub outputs: Vec<SignalId>,
    pub sample_time: SampleTime,
    pub cont_state: Option<std::ops::Range<usize>>,
    pub disc_state: Option<std::ops::Range<usize>>,
    pub rate_index: Option<usize>,
}

/// Block semantics.
#[derive(Debug, Clone, PartialEq)]
pub enum IrBlockKind {
    Constant {
        value: f64,
    },
    Step {
        time: f64,
        before: f64,
        after: f64,
    },
    Gain {
        gain: f64,
    },
    Sum {
        signs: Vec<i8>,
    },
    Product {
        ops: Vec<char>,
    },
    Abs,
    Bias {
        bias: f64,
    },
    Saturate {
        upper: f64,
        lower: f64,
    },
    /// criteria: 0 = u2 >= T, 1 = u2 > T, 2 = u2 != 0
    Switch {
        criteria: u8,
        threshold: f64,
    },
    Scope,
    Outport {
        port: u32,
    },
    Integrator {
        x0: f64,
    },
    TransferFcn {
        a: Matrix,
        b: Vec<f64>,
        c: Vec<f64>,
        d: f64,
    },
    StateSpace {
        a: Matrix,
        b: Vec<f64>,
        c: Vec<f64>,
        d: f64,
        x0: Vec<f64>,
    },
    UnitDelay {
        x0: f64,
    },
    DiscreteIntegrator {
        gain: f64,
        x0: f64,
    },
    Pid {
        p: f64,
        i: f64,
        d: f64,
        n: f64,
    },
    SineWave {
        amplitude: f64,
        bias: f64,
        frequency: f64,
        phase: f64,
    },
    Clock,
}

impl IrBlockKind {
    /// True if inputs are needed to compute outputs at the same time.
    pub fn direct_feedthrough(&self) -> bool {
        match self {
            IrBlockKind::TransferFcn { d, .. } | IrBlockKind::StateSpace { d, .. } => *d != 0.0,
            IrBlockKind::Constant { .. }
            | IrBlockKind::Step { .. }
            | IrBlockKind::Integrator { .. }
            | IrBlockKind::UnitDelay { .. }
            | IrBlockKind::DiscreteIntegrator { .. }
            | IrBlockKind::SineWave { .. }
            | IrBlockKind::Clock => false,
            _ => true,
        }
    }
    /// Number of continuous states.
    pub fn num_cont_states(&self) -> usize {
        match self {
            IrBlockKind::Integrator { .. } => 1,
            IrBlockKind::TransferFcn { a, .. } | IrBlockKind::StateSpace { a, .. } => a.rows,
            IrBlockKind::Pid { .. } => 2,
            _ => 0,
        }
    }
    /// Number of discrete states.
    pub fn num_disc_states(&self) -> usize {
        match self {
            IrBlockKind::UnitDelay { .. } | IrBlockKind::DiscreteIntegrator { .. } => 1,
            _ => 0,
        }
    }
    /// True if the block requires a discrete sample time.
    pub fn needs_discrete(&self) -> bool {
        self.num_disc_states() > 0
    }
}

/// One logged channel.
#[derive(Debug, Clone, PartialEq)]
pub struct IrLog {
    pub name: String,
    pub block_sid: BlockId,
    pub port: u32,
    pub signal: Option<SignalId>,
}
