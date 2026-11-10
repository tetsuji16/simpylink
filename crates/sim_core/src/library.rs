//! Block library: specs, parameters, port counts and default names.
use crate::expr::{parse_value, Value};
use crate::geom::{Point, Rect};
use crate::ids::BlockId;
use crate::model::{Block, Orientation, System};
use std::collections::BTreeMap;

/// How a parameter string is validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    Expr {
        shape: Shape,
    },
    SampleTime,
    SumSigns,
    ProductOps,
    Enum(&'static [&'static str]),
    PositiveInt,
    /// An arbitrary text value.
    Text,
}

/// Expected value shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Scalar,
    Vector,
    Matrix,
}

/// One parameter definition.
#[derive(Debug, Clone, Copy)]
pub struct ParamSpec {
    pub name: &'static str,
    pub label: &'static str,
    pub default: &'static str,
    pub kind: ParamKind,
}

/// Number of input and output ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortCounts {
    pub inputs: u32,
    pub outputs: u32,
}

/// Reserved input-port count parameter for opaque imported blocks.
pub const OPAQUE_INPUTS: &str = "__opaque_inputs";
/// Reserved output-port count parameter for opaque imported blocks.
pub const OPAQUE_OUTPUTS: &str = "__opaque_outputs";

/// Port count function.
pub type PortsFn = fn(&BTreeMap<String, String>) -> Result<PortCounts, String>;

/// One block type.
pub struct BlockSpec {
    pub block_type: &'static str,
    pub display_name: &'static str,
    pub library: &'static str,
    pub default_size: (f64, f64),
    pub params: &'static [ParamSpec],
    pub ports: PortsFn,
    pub base_name: &'static str,
    pub description: &'static str,
}

/// Library headings in display order.
pub const LIBRARIES: [&str; 7] = [
    "Sources",
    "Sinks",
    "Math Operations",
    "Continuous",
    "Discrete",
    "Ports & Subsystems",
    "Signal Routing",
];

const fn p(
    name: &'static str,
    label: &'static str,
    default: &'static str,
    kind: ParamKind,
) -> ParamSpec {
    ParamSpec {
        name,
        label,
        default,
        kind,
    }
}
const SCALAR: ParamKind = ParamKind::Expr {
    shape: Shape::Scalar,
};
const VECTOR: ParamKind = ParamKind::Expr {
    shape: Shape::Vector,
};
const MATRIX: ParamKind = ParamKind::Expr {
    shape: Shape::Matrix,
};
const ST: ParamKind = ParamKind::SampleTime;

fn ports_0_1(_: &BTreeMap<String, String>) -> Result<PortCounts, String> {
    Ok(PortCounts {
        inputs: 0,
        outputs: 1,
    })
}
fn ports_1_1(_: &BTreeMap<String, String>) -> Result<PortCounts, String> {
    Ok(PortCounts {
        inputs: 1,
        outputs: 1,
    })
}
fn ports_1_0(_: &BTreeMap<String, String>) -> Result<PortCounts, String> {
    Ok(PortCounts {
        inputs: 1,
        outputs: 0,
    })
}
fn ports_0_0(_: &BTreeMap<String, String>) -> Result<PortCounts, String> {
    Ok(PortCounts {
        inputs: 0,
        outputs: 0,
    })
}
fn ports_3_1(_: &BTreeMap<String, String>) -> Result<PortCounts, String> {
    Ok(PortCounts {
        inputs: 3,
        outputs: 1,
    })
}

/// Signs of a Sum block's Inputs parameter (+1 / -1 per input port).
pub fn sum_signs(inputs: &str) -> Result<Vec<i8>, String> {
    let s = inputs.trim();
    let bad = || "Inputs must be like '++', '+-', '|+-' or a number of inputs".to_string();
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        let n: usize = s.parse().map_err(|_| bad())?;
        if n == 0 || n > 64 {
            return Err(bad());
        }
        return Ok(vec![1; n]);
    }
    let mut v = vec![];
    for c in s.chars() {
        match c {
            '+' => v.push(1),
            '-' => v.push(-1),
            '|' => {}
            _ => return Err(bad()),
        }
    }
    if v.is_empty() {
        return Err(bad());
    }
    Ok(v)
}

/// Operations of a Product block's Inputs parameter ('*' multiply / '/' divide).
pub fn product_ops(inputs: &str) -> Result<Vec<char>, String> {
    let s = inputs.trim();
    let bad = || "Number of inputs must be like '**', '*/' or a number".to_string();
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        let n: usize = s.parse().map_err(|_| bad())?;
        if n == 0 || n > 64 {
            return Err(bad());
        }
        return Ok(vec!['*'; n]);
    }
    let v: Vec<char> = s.chars().collect();
    if v.is_empty() || v.iter().any(|c| *c != '*' && *c != '/') {
        return Err(bad());
    }
    Ok(v)
}

fn sum_ports(p: &BTreeMap<String, String>) -> Result<PortCounts, String> {
    let s = p.get("Inputs").map(String::as_str).unwrap_or("|++");
    Ok(PortCounts {
        inputs: sum_signs(s)?.len() as u32,
        outputs: 1,
    })
}
fn product_ports(p: &BTreeMap<String, String>) -> Result<PortCounts, String> {
    let s = p.get("Inputs").map(String::as_str).unwrap_or("2");
    Ok(PortCounts {
        inputs: product_ops(s)?.len() as u32,
        outputs: 1,
    })
}
fn mux_ports(p: &BTreeMap<String, String>) -> Result<PortCounts, String> {
    let n = match p.get("Inputs") {
        None => 2,
        Some(s) => s
            .trim()
            .parse::<u32>()
            .map_err(|_| "must be a positive integer".to_string())?,
    };
    if n == 0 {
        return Err("must be a positive integer".into());
    }
    Ok(PortCounts {
        inputs: n,
        outputs: 1,
    })
}

/// Output widths of a Demux: a scalar `Outputs` gives that many equal parts.
pub fn demux_widths(outputs: &str) -> Result<Vec<usize>, String> {
    let bad = || "Outputs must be a positive integer or a vector of positive integers".to_string();
    let v = parse_value(outputs).map_err(|_| bad())?;
    let widths: Vec<usize> = match (v.as_scalar(), v.as_vector()) {
        (Some(p), _) => {
            if p < 1.0 || p.fract() != 0.0 {
                return Err(bad());
            }
            vec![p as usize]
        }
        (None, Some(items)) => items
            .iter()
            .map(|x| {
                if *x < 1.0 || x.fract() != 0.0 {
                    Err(bad())
                } else {
                    Ok(*x as usize)
                }
            })
            .collect::<Result<Vec<usize>, String>>()?,
        _ => return Err(bad()),
    };
    if widths.is_empty() {
        return Err(bad());
    }
    Ok(widths)
}

fn demux_ports(p: &BTreeMap<String, String>) -> Result<PortCounts, String> {
    let raw = p.get("Outputs").map(String::as_str).unwrap_or("2");
    let widths = demux_widths(raw)?;
    // A scalar `Outputs` is the number of equal outputs; a vector lists each width.
    let outputs = match parse_value(raw) {
        Ok(v) => match v.as_scalar() {
            Some(n) => n as u32,
            None => widths.len() as u32,
        },
        Err(_) => return Err("Outputs must be a positive integer or a vector".into()),
    };
    Ok(PortCounts { inputs: 1, outputs })
}

fn scope_ports(p: &BTreeMap<String, String>) -> Result<PortCounts, String> {
    let n = match p.get("NumInputPorts") {
        None => 1,
        Some(s) => s
            .trim()
            .parse::<u32>()
            .map_err(|_| "must be a positive integer".to_string())?,
    };
    if n == 0 {
        return Err("must be a positive integer".into());
    }
    Ok(PortCounts {
        inputs: n,
        outputs: 0,
    })
}

static REGISTRY: &[BlockSpec] = &[
    BlockSpec {
        block_type: "Constant",
        display_name: "Constant",
        library: "Sources",
        default_size: (30.0, 30.0),
        params: &[
            p("Value", "Constant value", "1", MATRIX),
            p("SampleTime", "Sample time", "inf", ST),
        ],
        ports: ports_0_1,
        base_name: "Constant",
        description: "Output a constant value",
    },
    BlockSpec {
        block_type: "Step",
        display_name: "Step",
        library: "Sources",
        default_size: (30.0, 30.0),
        params: &[
            p("Time", "Step time", "1", SCALAR),
            p("Before", "Initial value", "0", SCALAR),
            p("After", "Final value", "1", SCALAR),
            p("SampleTime", "Sample time", "0", ST),
        ],
        ports: ports_0_1,
        base_name: "Step",
        description: "Step from initial to final value at step time",
    },
    BlockSpec {
        block_type: "Gain",
        display_name: "Gain",
        library: "Math Operations",
        default_size: (30.0, 30.0),
        params: &[
            p("Gain", "Gain", "1", MATRIX),
            p("SampleTime", "Sample time (-1 for inherited)", "-1", ST),
        ],
        ports: ports_1_1,
        base_name: "Gain",
        description: "Multiply the input by a constant",
    },
    BlockSpec {
        block_type: "Sum",
        display_name: "Sum",
        library: "Math Operations",
        default_size: (20.0, 20.0),
        params: &[
            p("Inputs", "List of signs", "|++", ParamKind::SumSigns),
            p(
                "IconShape",
                "Icon shape",
                "round",
                ParamKind::Enum(&["round", "rectangular"]),
            ),
            p("SampleTime", "Sample time (-1 for inherited)", "-1", ST),
        ],
        ports: sum_ports,
        base_name: "Sum",
        description: "Add or subtract inputs",
    },
    BlockSpec {
        block_type: "Scope",
        display_name: "Scope",
        library: "Sinks",
        default_size: (30.0, 32.0),
        params: &[p(
            "NumInputPorts",
            "Number of input ports",
            "1",
            ParamKind::PositiveInt,
        )],
        ports: scope_ports,
        base_name: "Scope",
        description: "Display signals",
    },
    BlockSpec {
        block_type: "Inport",
        display_name: "In1",
        library: "Ports & Subsystems",
        default_size: (30.0, 14.0),
        params: &[p("Port", "Port number", "1", ParamKind::PositiveInt)],
        ports: ports_0_1,
        base_name: "In",
        description: "Input port",
    },
    BlockSpec {
        block_type: "Outport",
        display_name: "Out1",
        library: "Ports & Subsystems",
        default_size: (30.0, 14.0),
        params: &[p("Port", "Port number", "1", ParamKind::PositiveInt)],
        ports: ports_1_0,
        base_name: "Out",
        description: "Output port (logged at the root level)",
    },
    BlockSpec {
        block_type: "SubSystem",
        display_name: "Subsystem",
        library: "Ports & Subsystems",
        default_size: (60.0, 40.0),
        params: &[],
        ports: ports_0_0,
        base_name: "Subsystem",
        description: "Virtual subsystem (ports follow its In/Out blocks)",
    },
    BlockSpec {
        block_type: "Integrator",
        display_name: "Integrator",
        library: "Continuous",
        default_size: (30.0, 32.0),
        params: &[p("InitialCondition", "Initial condition", "0", SCALAR)],
        ports: ports_1_1,
        base_name: "Integrator",
        description: "Continuous-time integration",
    },
    BlockSpec {
        block_type: "TransferFcn",
        display_name: "Transfer Fcn",
        library: "Continuous",
        default_size: (50.0, 40.0),
        params: &[
            p("Numerator", "Numerator coefficients", "[1]", VECTOR),
            p("Denominator", "Denominator coefficients", "[1 1]", VECTOR),
        ],
        ports: ports_1_1,
        base_name: "Transfer Fcn",
        description: "Transfer function num(s)/den(s)",
    },
    BlockSpec {
        block_type: "StateSpace",
        display_name: "State-Space",
        library: "Continuous",
        default_size: (50.0, 40.0),
        params: &[
            p("A", "A", "1", MATRIX),
            p("B", "B", "1", MATRIX),
            p("C", "C", "1", MATRIX),
            p("D", "D", "1", MATRIX),
            p("InitialCondition", "Initial conditions", "0", MATRIX),
        ],
        ports: ports_1_1,
        base_name: "State-Space",
        description: "dx/dt = Ax + Bu, y = Cx + Du",
    },
    BlockSpec {
        block_type: "UnitDelay",
        display_name: "Unit Delay",
        library: "Discrete",
        default_size: (30.0, 30.0),
        params: &[
            p("InitialCondition", "Initial condition", "0", SCALAR),
            p("SampleTime", "Sample time (-1 for inherited)", "-1", ST),
        ],
        ports: ports_1_1,
        base_name: "Unit Delay",
        description: "Delay the input by one sample",
    },
    BlockSpec {
        block_type: "Sin",
        display_name: "Sine Wave",
        library: "Sources",
        default_size: (30.0, 30.0),
        params: &[
            p(
                "SineType",
                "Sine type",
                "Time based",
                ParamKind::Enum(&["Time based", "Sample based"]),
            ),
            p("Amplitude", "Amplitude", "1", SCALAR),
            p("Bias", "Bias", "0", SCALAR),
            p("Frequency", "Frequency (rad/sec)", "1", SCALAR),
            p("Phase", "Phase (rad)", "0", SCALAR),
            p("SampleTime", "Sample time", "0", ST),
        ],
        ports: ports_0_1,
        base_name: "Sine Wave",
        description: "Amplitude*sin(Frequency*t+Phase)+Bias",
    },
    BlockSpec {
        block_type: "Clock",
        display_name: "Clock",
        library: "Sources",
        default_size: (30.0, 30.0),
        params: &[],
        ports: ports_0_1,
        base_name: "Clock",
        description: "Output the simulation time",
    },
    BlockSpec {
        block_type: "DiscreteIntegrator",
        display_name: "Discrete-Time Integrator",
        library: "Discrete",
        default_size: (35.0, 30.0),
        params: &[
            p(
                "IntegratorMethod",
                "Integrator method",
                "Integration: Forward Euler",
                ParamKind::Enum(&[
                    "Integration: Forward Euler",
                    "Integration: Backward Euler",
                    "Integration: Trapezoidal",
                ]),
            ),
            p("gainval", "Gain value", "1.0", SCALAR),
            p("InitialCondition", "Initial condition", "0", SCALAR),
            p("SampleTime", "Sample time (-1 for inherited)", "1", ST),
        ],
        ports: ports_1_1,
        base_name: "Discrete-Time Integrator",
        description: "Discrete-time integration",
    },
    BlockSpec {
        block_type: "PIDController",
        display_name: "PID Controller",
        library: "Continuous",
        default_size: (50.0, 40.0),
        params: &[
            p(
                "Controller",
                "Controller",
                "PID",
                ParamKind::Enum(&["PID", "PI", "PD", "P", "I"]),
            ),
            p(
                "Form",
                "Form",
                "Parallel",
                ParamKind::Enum(&["Parallel", "Ideal"]),
            ),
            p(
                "TimeDomain",
                "Time domain",
                "Continuous-time",
                ParamKind::Enum(&["Continuous-time", "Discrete-time"]),
            ),
            p("P", "Proportional (P)", "1", SCALAR),
            p("I", "Integral (I)", "1", SCALAR),
            p("D", "Derivative (D)", "0", SCALAR),
            p("N", "Filter coefficient (N)", "100", SCALAR),
        ],
        ports: ports_1_1,
        base_name: "PID Controller",
        description: "Continuous PID controller (parallel form)",
    },
    BlockSpec {
        block_type: "Product",
        display_name: "Product",
        library: "Math Operations",
        default_size: (30.0, 30.0),
        params: &[
            p("Inputs", "Number of inputs", "2", ParamKind::ProductOps),
            p("SampleTime", "Sample time (-1 for inherited)", "-1", ST),
        ],
        ports: product_ports,
        base_name: "Product",
        description: "Multiply or divide inputs",
    },
    BlockSpec {
        block_type: "Abs",
        display_name: "Abs",
        library: "Math Operations",
        default_size: (30.0, 30.0),
        params: &[p("SampleTime", "Sample time (-1 for inherited)", "-1", ST)],
        ports: ports_1_1,
        base_name: "Abs",
        description: "Absolute value",
    },
    BlockSpec {
        block_type: "Bias",
        display_name: "Bias",
        library: "Math Operations",
        default_size: (30.0, 30.0),
        params: &[p("Bias", "Bias", "0", SCALAR)],
        ports: ports_1_1,
        base_name: "Bias",
        description: "Add a bias to the input",
    },
    BlockSpec {
        block_type: "Saturate",
        display_name: "Saturation",
        library: "Math Operations",
        default_size: (30.0, 30.0),
        params: &[
            p("UpperLimit", "Upper limit", "0.5", SCALAR),
            p("LowerLimit", "Lower limit", "-0.5", SCALAR),
        ],
        ports: ports_1_1,
        base_name: "Saturation",
        description: "Limit the input (no zero-crossing detection)",
    },
    BlockSpec {
        block_type: "Switch",
        display_name: "Switch",
        library: "Math Operations",
        default_size: (30.0, 40.0),
        params: &[
            p(
                "Criteria",
                "Criteria for passing first input",
                "u2 >= Threshold",
                ParamKind::Enum(&["u2 >= Threshold", "u2 > Threshold", "u2 ~= 0"]),
            ),
            p("Threshold", "Threshold", "0", SCALAR),
        ],
        ports: ports_3_1,
        base_name: "Switch",
        description: "Pass input 1 or input 3 depending on input 2 (no zero-crossing detection)",
    },
    BlockSpec {
        block_type: "Ground",
        display_name: "Ground",
        library: "Sources",
        default_size: (20.0, 20.0),
        params: &[],
        ports: ports_0_1,
        base_name: "Ground",
        description: "Output a constant zero",
    },
    BlockSpec {
        block_type: "Terminator",
        display_name: "Terminator",
        library: "Sinks",
        default_size: (20.0, 20.0),
        params: &[],
        ports: ports_1_0,
        base_name: "Terminator",
        description: "Terminate an unused signal",
    },
    BlockSpec {
        block_type: "ZeroOrderHold",
        display_name: "Zero-Order Hold",
        library: "Discrete",
        default_size: (35.0, 34.0),
        params: &[p("SampleTime", "Sample time (-1 for inherited)", "-1", ST)],
        ports: ports_1_1,
        base_name: "Zero-Order Hold",
        description: "Sample the input at the sample time and hold it",
    },
    BlockSpec {
        block_type: "Mux",
        display_name: "Mux",
        library: "Signal Routing",
        default_size: (5.0, 40.0),
        params: &[p("Inputs", "Number of inputs", "2", ParamKind::PositiveInt)],
        ports: mux_ports,
        base_name: "Mux",
        description: "Combine inputs into one vector",
    },
    BlockSpec {
        block_type: "Demux",
        display_name: "Demux",
        library: "Signal Routing",
        default_size: (5.0, 40.0),
        params: &[p("Outputs", "Number of outputs", "2", VECTOR)],
        ports: demux_ports,
        base_name: "Demux",
        description: "Split a vector into several outputs",
    },
    BlockSpec {
        block_type: "Goto",
        display_name: "Goto",
        library: "Signal Routing",
        default_size: (40.0, 20.0),
        params: &[
            p("GotoTag", "Tag", "A", ParamKind::Text),
            p(
                "TagVisibility",
                "Tag visibility",
                "local",
                ParamKind::Enum(&["local", "scoped", "global"]),
            ),
        ],
        ports: ports_1_0,
        base_name: "Goto",
        description: "Route a signal using a tag",
    },
    BlockSpec {
        block_type: "From",
        display_name: "From",
        library: "Signal Routing",
        default_size: (40.0, 20.0),
        params: &[p("GotoTag", "Goto tag", "A", ParamKind::Text)],
        ports: ports_0_1,
        base_name: "From",
        description: "Receive a signal using a tag",
    },
    BlockSpec {
        block_type: "GotoTagVisibility",
        display_name: "Goto Tag Visibility",
        library: "Signal Routing",
        default_size: (40.0, 20.0),
        params: &[p("GotoTag", "Goto tag", "A", ParamKind::Text)],
        ports: ports_0_0,
        base_name: "Goto Tag Visibility",
        description: "Define the scope of a Goto tag",
    },
];

/// All block specs.
pub fn registry() -> &'static [BlockSpec] {
    REGISTRY
}

/// Spec by block type.
pub fn spec(block_type: &str) -> Option<&'static BlockSpec> {
    REGISTRY.iter().find(|s| s.block_type == block_type)
}

/// Port counts of a block.
pub fn port_counts(block: &Block) -> Result<PortCounts, String> {
    let Some(s) = spec(&block.block_type) else {
        let input = block
            .params
            .get(OPAQUE_INPUTS)
            .map(|v| {
                v.parse::<u32>()
                    .map_err(|_| format!("{OPAQUE_INPUTS} must be a non-negative integer"))
            })
            .transpose()?;
        let output = block
            .params
            .get(OPAQUE_OUTPUTS)
            .map(|v| {
                v.parse::<u32>()
                    .map_err(|_| format!("{OPAQUE_OUTPUTS} must be a non-negative integer"))
            })
            .transpose()?;
        if input.is_some() || output.is_some() {
            return Ok(PortCounts {
                inputs: input.unwrap_or(0),
                outputs: output.unwrap_or(0),
            });
        }
        return Err(format!("unknown block type '{}'", block.block_type));
    };
    if let Some(sys) = &block.system {
        let n = |t: &str| sys.blocks.iter().filter(|b| b.block_type == t).count() as u32;
        return Ok(PortCounts {
            inputs: n("Inport"),
            outputs: n("Outport"),
        });
    }
    (s.ports)(&block.params)
}

/// Validates a parameter string.
pub fn validate_param(kind: ParamKind, value: &str) -> Result<(), String> {
    match kind {
        ParamKind::Text => Ok(()),
        ParamKind::Expr { shape } => {
            let v = parse_value(value).map_err(|e| e.to_string())?;
            match shape {
                Shape::Scalar if v.as_scalar().is_none() => Err("expected a scalar".into()),
                Shape::Vector if v.as_vector().map(|x| x.is_empty()).unwrap_or(true) => {
                    Err("expected a vector".into())
                }
                _ => Ok(()),
            }
        }
        ParamKind::SampleTime => parse_sample_time_value(value).map(|_| ()),
        ParamKind::SumSigns => sum_signs(value).map(|_| ()),
        ParamKind::ProductOps => product_ops(value).map(|_| ()),
        ParamKind::Enum(list) => {
            if list.contains(&value) {
                Ok(())
            } else {
                Err(format!("must be one of: {}", list.join(", ")))
            }
        }
        ParamKind::PositiveInt => match value.trim().parse::<u32>() {
            Ok(n) if n >= 1 => Ok(()),
            _ => Err("must be a positive integer".into()),
        },
    }
}

/// Parsed sample time parameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SampleTimeParam {
    Inherited,
    Continuous,
    Constant,
    Discrete { period: f64, offset: f64 },
}

/// Parses "-1", "0", "inf", "Ts" or "[Ts, offset]".
pub fn parse_sample_time_value(src: &str) -> Result<SampleTimeParam, String> {
    let bad = || "invalid sample time: use -1, 0, inf, Ts or [Ts, offset]".to_string();
    let v = parse_value(src).map_err(|_| bad())?;
    if let Some(x) = v.as_scalar() {
        if x == -1.0 {
            return Ok(SampleTimeParam::Inherited);
        }
        if x == 0.0 {
            return Ok(SampleTimeParam::Continuous);
        }
        if x == f64::INFINITY {
            return Ok(SampleTimeParam::Constant);
        }
        if x > 0.0 && x.is_finite() {
            return Ok(SampleTimeParam::Discrete {
                period: x,
                offset: 0.0,
            });
        }
        return Err(bad());
    }
    if let Value::Matrix(m) = &v {
        if m.rows == 1 && m.cols == 2 {
            let (ts, off) = (m.data[0], m.data[1]);
            if ts > 0.0 && ts.is_finite() && off >= 0.0 && off < ts {
                return Ok(SampleTimeParam::Discrete {
                    period: ts,
                    offset: off,
                });
            }
        }
    }
    Err(bad())
}

/// Inserts missing parameters with their defaults.
pub fn fill_default_params(block: &mut Block) {
    if let Some(s) = spec(&block.block_type) {
        for ps in s.params {
            block
                .params
                .entry(ps.name.to_string())
                .or_insert_with(|| ps.default.to_string());
        }
    }
}

/// "Gain", "Gain1", "Gain2", ...
pub fn unique_block_name(system: &System, base: &str) -> String {
    unique_name_with(|n| system.block_by_name(n).is_some(), base)
}

/// Unique name given a predicate telling whether a name is taken.
pub fn unique_name_with(taken: impl Fn(&str) -> bool, base: &str) -> String {
    if !taken(base) {
        return base.to_string();
    }
    (1..)
        .map(|n| format!("{base}{n}"))
        .find(|c| !taken(c))
        .unwrap_or_else(|| base.to_string())
}

/// Default name for a new block of `spec` in `system`.
pub fn default_name(system: &System, s: &BlockSpec) -> String {
    if s.block_type == "Inport" || s.block_type == "Outport" {
        let base = s.base_name;
        return (1..)
            .map(|n| format!("{base}{n}"))
            .find(|c| system.block_by_name(c).is_none())
            .unwrap_or_default();
    }
    unique_block_name(system, s.base_name)
}

/// New block with default parameters at `top_left`.
pub fn new_block(
    system: &System,
    block_type: &str,
    sid: BlockId,
    top_left: Point,
) -> Result<Block, String> {
    let s = spec(block_type).ok_or_else(|| format!("unknown block type '{block_type}'"))?;
    let params = s
        .params
        .iter()
        .map(|p| (p.name.to_string(), p.default.to_string()))
        .collect();
    let mut b = Block {
        sid,
        block_type: block_type.to_string(),
        name: default_name(system, s),
        position: Rect::from_min_size(top_left, s.default_size.0, s.default_size.1),
        orientation: Orientation::Right,
        params,
        system: None,
    };
    if block_type == "Inport" || block_type == "Outport" {
        let n = system
            .blocks
            .iter()
            .filter(|x| x.block_type == block_type)
            .count()
            + 1;
        b.params.insert("Port".into(), n.to_string());
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blk(t: &str, params: &[(&str, &str)]) -> Block {
        Block {
            sid: BlockId(1),
            block_type: t.into(),
            name: "b".into(),
            position: Rect::new(0.0, 0.0, 1.0, 1.0),
            orientation: Orientation::Right,
            params: params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            system: None,
        }
    }
    #[test]
    fn registry_and_new_block() {
        assert_eq!(registry()[0].block_type, "Constant");
        assert!(registry().iter().all(|s| LIBRARIES.contains(&s.library)));
        assert_eq!(spec("Gain").unwrap().default_size, (30.0, 30.0));
        let mut sys = System::default();
        let g = new_block(&sys, "Gain", BlockId(1), Point::new(100.0, 40.0)).unwrap();
        assert_eq!(g.name, "Gain");
        assert_eq!(g.position, Rect::new(100.0, 40.0, 130.0, 70.0));
        assert_eq!(g.params.get("Gain").map(String::as_str), Some("1"));
        assert_eq!(g.params.get("SampleTime").map(String::as_str), Some("-1"));
        sys.insert_block_sorted(g);
        let g2 = new_block(&sys, "Gain", BlockId(2), Point::new(0.0, 0.0)).unwrap();
        assert_eq!(g2.name, "Gain1");
        sys.insert_block_sorted(g2);
        assert_eq!(unique_block_name(&sys, "Gain"), "Gain2");
        let i1 = new_block(&sys, "Inport", BlockId(3), Point::new(0.0, 0.0)).unwrap();
        assert_eq!(i1.name, "In1");
        sys.insert_block_sorted(i1);
        assert_eq!(
            new_block(&sys, "Inport", BlockId(4), Point::new(0.0, 0.0))
                .unwrap()
                .name,
            "In2"
        );
        assert!(new_block(&sys, "Nope", BlockId(5), Point::default()).is_err());
    }
    #[test]
    fn ports() {
        let pc = |b: Block| port_counts(&b).map(|p| (p.inputs, p.outputs));
        assert_eq!(pc(blk("Sum", &[("Inputs", "|++")])), Ok((2, 1)));
        assert_eq!(pc(blk("Sum", &[("Inputs", "+-")])), Ok((2, 1)));
        assert_eq!(pc(blk("Sum", &[("Inputs", "+-+")])), Ok((3, 1)));
        assert_eq!(pc(blk("Sum", &[("Inputs", "3")])), Ok((3, 1)));
        assert_eq!(pc(blk("Sum", &[])), Ok((2, 1)));
        assert_eq!(pc(blk("Scope", &[("NumInputPorts", "3")])), Ok((3, 0)));
        assert_eq!(pc(blk("Constant", &[])), Ok((0, 1)));
        assert_eq!(pc(blk("Outport", &[])), Ok((1, 0)));
        assert!(pc(blk("Sum", &[("Inputs", "+*")])).is_err());
        assert!(pc(blk("Scope", &[("NumInputPorts", "0")])).is_err());
        assert_eq!(
            pc(blk("Nope", &[])),
            Err("unknown block type 'Nope'".into())
        );
    }

    #[test]
    fn ground_and_terminator_specs() {
        let pc = |b: Block| port_counts(&b).map(|p| (p.inputs, p.outputs));
        let ground = spec("Ground").unwrap();
        assert_eq!(ground.library, "Sources");
        assert_eq!(ground.default_size, (20.0, 20.0));
        assert!(ground.params.is_empty());
        assert_eq!(ground.base_name, "Ground");
        assert_eq!(pc(blk("Ground", &[])), Ok((0, 1)));

        let terminator = spec("Terminator").unwrap();
        assert_eq!(terminator.library, "Sinks");
        assert_eq!(terminator.default_size, (20.0, 20.0));
        assert!(terminator.params.is_empty());
        assert_eq!(terminator.base_name, "Terminator");
        assert_eq!(pc(blk("Terminator", &[])), Ok((1, 0)));
    }
    #[test]
    fn params() {
        let ok = |k, v| validate_param(k, v).is_ok();
        assert!(ok(SCALAR, "2.5"));
        assert_eq!(
            validate_param(SCALAR, "[1 2]"),
            Err("expected a scalar".into())
        );
        assert!(ok(VECTOR, "[1 2 1]"));
        for v in ["-1", "0", "inf", "0.1", "[0.5, 0.1]"] {
            assert!(ok(ST, v), "{v}");
        }
        for v in ["-2", "[0.5, 0.5]", "[0, 0]", "abc"] {
            assert!(!ok(ST, v), "{v}");
        }
        for v in ["|+-", "++", "2"] {
            assert!(ok(ParamKind::SumSigns, v));
        }
        for v in ["", "|", "+*", "0", "65"] {
            assert!(!ok(ParamKind::SumSigns, v), "{v}");
        }
        let e = ParamKind::Enum(&["round", "rectangular"]);
        assert!(ok(e, "round"));
        assert_eq!(
            validate_param(e, "Round"),
            Err("must be one of: round, rectangular".into())
        );
        assert!(ok(ParamKind::PositiveInt, "1"));
        assert!(!ok(ParamKind::PositiveInt, "0"));
        assert!(!ok(ParamKind::PositiveInt, "1.5"));
        let mut g = blk("Gain", &[("Gain", "5")]);
        fill_default_params(&mut g);
        assert_eq!(g.params.len(), 2);
        assert_eq!(g.params["Gain"], "5");
        assert_eq!(g.params["SampleTime"], "-1");
    }
}
