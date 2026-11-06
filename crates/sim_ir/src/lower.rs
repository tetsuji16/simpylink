//! Block lowering: parameter evaluation into IrBlockKind.
use crate::realize::tf_to_ss;
use crate::types::{IrBlockKind, SampleTime};
use sim_core::diag::{codes::*, Diagnostic, Location};
use sim_core::expr::{parse_value, Matrix};
use sim_core::library::{parse_sample_time_value, product_ops, sum_signs, SampleTimeParam};
use sim_core::model::Block;
use sim_core::path::SystemPath;

/// Lowered block.
#[derive(Debug, Clone, PartialEq)]
pub struct Lowered {
    pub kind: IrBlockKind,
    /// None = inherited (-1).
    pub sample_time: Option<SampleTime>,
}

/// Parses a sample time parameter.
pub fn parse_sample_time(src: &str) -> Result<Option<SampleTime>, String> {
    match parse_sample_time_value(src).map_err(|_| format!("invalid sample time '{src}'"))? {
        SampleTimeParam::Inherited => Ok(None),
        SampleTimeParam::Continuous => Ok(Some(SampleTime::Continuous)),
        SampleTimeParam::Constant => Ok(Some(SampleTime::Constant)),
        SampleTimeParam::Discrete { period, offset } => {
            Ok(Some(SampleTime::Discrete { period, offset }))
        }
    }
}

struct L<'a> {
    b: &'a Block,
    loc: Location,
}

impl L<'_> {
    fn raw(&self, name: &str) -> Result<&str, Diagnostic> {
        self.b.params.get(name).map(String::as_str).ok_or_else(|| {
            Diagnostic::error(
                E_PARAM_EVAL,
                format!("parameter '{name}' of '{}' is missing", self.b.name),
                self.loc.clone(),
            )
        })
    }
    fn eval_err(&self, name: &str, e: impl std::fmt::Display) -> Diagnostic {
        Diagnostic::error(
            E_PARAM_EVAL,
            format!("parameter '{name}' of '{}': {e}", self.b.name),
            self.loc.clone(),
        )
    }
    fn domain(&self, msg: impl std::fmt::Display) -> Diagnostic {
        Diagnostic::error(
            E_PARAM_DOMAIN,
            format!("'{}': {msg}", self.b.name),
            self.loc.clone(),
        )
    }
    fn unsupported(&self, msg: impl Into<String>) -> Diagnostic {
        Diagnostic::error(E_UNSUPPORTED_FEATURE, msg, self.loc.clone())
    }
    fn scalar(&self, name: &str) -> Result<f64, Diagnostic> {
        let v = parse_value(self.raw(name)?).map_err(|e| self.eval_err(name, e))?;
        v.as_scalar()
            .ok_or_else(|| self.eval_err(name, "expected a scalar"))
    }
    fn vector(&self, name: &str) -> Result<Vec<f64>, Diagnostic> {
        let v = parse_value(self.raw(name)?).map_err(|e| self.eval_err(name, e))?;
        v.as_vector()
            .ok_or_else(|| self.eval_err(name, "expected a vector"))
    }
    fn matrix(&self, name: &str) -> Result<Matrix, Diagnostic> {
        let v = parse_value(self.raw(name)?).map_err(|e| self.eval_err(name, e))?;
        Ok(v.as_matrix())
    }
    fn st(&self) -> Result<Option<SampleTime>, Diagnostic> {
        match self.b.params.get("SampleTime") {
            None => Ok(None),
            Some(s) => parse_sample_time(s).map_err(|e| self.eval_err("SampleTime", e)),
        }
    }
    fn text(&self, name: &str) -> Result<&str, Diagnostic> {
        self.raw(name)
    }
}

fn dims(m: &Matrix) -> String {
    format!("{}x{}", m.rows, m.cols)
}

/// Lowers one non-virtual block.
pub fn lower_block(block: &Block, is_root: bool, path: &SystemPath) -> Result<Lowered, Diagnostic> {
    let l = L {
        b: block,
        loc: Location::block(path, block.sid),
    };
    let cont = Some(SampleTime::Continuous);
    let (kind, st) = match block.block_type.as_str() {
        "Constant" => (
            IrBlockKind::Constant {
                value: l.scalar("Value")?,
            },
            l.st()?,
        ),
        "Step" => (
            IrBlockKind::Step {
                time: l.scalar("Time")?,
                before: l.scalar("Before")?,
                after: l.scalar("After")?,
            },
            l.st()?,
        ),
        "Gain" => (
            IrBlockKind::Gain {
                gain: l.scalar("Gain")?,
            },
            l.st()?,
        ),
        "Sum" => {
            let signs = sum_signs(l.raw("Inputs")?).map_err(|e| l.eval_err("Inputs", e))?;
            (IrBlockKind::Sum { signs }, l.st()?)
        }
        "Product" => {
            let ops = product_ops(l.raw("Inputs")?).map_err(|e| l.eval_err("Inputs", e))?;
            (IrBlockKind::Product { ops }, l.st()?)
        }
        "Abs" => (IrBlockKind::Abs, l.st()?),
        "Bias" => (
            IrBlockKind::Bias {
                bias: l.scalar("Bias")?,
            },
            None,
        ),
        "Saturate" => {
            let (upper, lower) = (l.scalar("UpperLimit")?, l.scalar("LowerLimit")?);
            if lower > upper {
                return Err(l.domain("LowerLimit must not exceed UpperLimit"));
            }
            (IrBlockKind::Saturate { upper, lower }, None)
        }
        "Switch" => {
            let criteria = match l.text("Criteria")? {
                "u2 >= Threshold" => 0,
                "u2 > Threshold" => 1,
                "u2 ~= 0" => 2,
                other => return Err(l.eval_err("Criteria", format!("unknown criteria '{other}'"))),
            };
            (
                IrBlockKind::Switch {
                    criteria,
                    threshold: l.scalar("Threshold")?,
                },
                None,
            )
        }
        "Scope" => (IrBlockKind::Scope, None),
        "Outport" if is_root => {
            let port = l
                .raw("Port")?
                .trim()
                .parse::<u32>()
                .map_err(|e| l.eval_err("Port", e))?;
            (IrBlockKind::Outport { port }, None)
        }
        "Inport" | "Outport" => {
            return Err(l.unsupported("root-level Inport (external inputs) is not supported yet"));
        }
        "Integrator" => (
            IrBlockKind::Integrator {
                x0: l.scalar("InitialCondition")?,
            },
            cont,
        ),
        "TransferFcn" => {
            let (a, b, c, d) = tf_to_ss(&l.vector("Numerator")?, &l.vector("Denominator")?)
                .map_err(|e| l.domain(e))?;
            (IrBlockKind::TransferFcn { a, b, c, d }, cont)
        }
        "StateSpace" => {
            let a = l.matrix("A")?;
            let bm = l.matrix("B")?;
            let cm = l.matrix("C")?;
            let dm = l.matrix("D")?;
            let n = a.rows;
            if a.cols != n {
                return Err(l.domain("A must be square"));
            }
            if !(bm.rows == n && bm.cols == 1) && !(n == 0 && bm.data.is_empty()) {
                return Err(l.domain(format!("B must be {n}x1 (got {})", dims(&bm))));
            }
            if !(cm.rows == 1 && cm.cols == n) && !(n == 0 && cm.data.is_empty()) {
                return Err(l.domain(format!("C must be 1x{n} (got {})", dims(&cm))));
            }
            if !(dm.rows == 1 && dm.cols == 1) {
                return Err(l.domain(format!("D must be 1x1 (got {})", dims(&dm))));
            }
            let x0v = parse_value(l.raw("InitialCondition")?)
                .map_err(|e| l.eval_err("InitialCondition", e))?;
            let x0 = match (x0v.as_scalar(), x0v.as_vector()) {
                (Some(s), _) => vec![s; n],
                (None, Some(v)) if v.len() == n => v,
                _ => return Err(l.domain("InitialCondition must be a scalar or have n elements")),
            };
            (
                IrBlockKind::StateSpace {
                    a,
                    b: bm.data,
                    c: cm.data,
                    d: dm.data[0],
                    x0,
                },
                cont,
            )
        }
        "UnitDelay" => (
            IrBlockKind::UnitDelay {
                x0: l.scalar("InitialCondition")?,
            },
            l.st()?,
        ),
        "DiscreteIntegrator" => {
            if l.text("IntegratorMethod")? != "Integration: Forward Euler" {
                return Err(l.unsupported("only 'Integration: Forward Euler' is supported yet"));
            }
            (
                IrBlockKind::DiscreteIntegrator {
                    gain: l.scalar("gainval")?,
                    x0: l.scalar("InitialCondition")?,
                },
                l.st()?,
            )
        }
        "PIDController" => {
            if l.text("Form")? != "Parallel" {
                return Err(l.unsupported("only the Parallel PID form is supported yet"));
            }
            if l.text("TimeDomain")? != "Continuous-time" {
                return Err(l.unsupported("discrete-time PID is not supported yet"));
            }
            let (mut p, mut i, mut d, n) = (
                l.scalar("P")?,
                l.scalar("I")?,
                l.scalar("D")?,
                l.scalar("N")?,
            );
            if n <= 0.0 {
                return Err(l.domain("N must be positive"));
            }
            match l.text("Controller")? {
                "PI" => d = 0.0,
                "PD" => i = 0.0,
                "P" => {
                    i = 0.0;
                    d = 0.0
                }
                "I" => {
                    p = 0.0;
                    d = 0.0
                }
                _ => {}
            }
            (IrBlockKind::Pid { p, i, d, n }, cont)
        }
        "Sin" => {
            if l.text("SineType")? != "Time based" {
                return Err(l.unsupported("sample-based Sine Wave is not supported yet"));
            }
            if l.st()? != cont {
                return Err(l.unsupported("discrete Sine Wave is not supported yet"));
            }
            (
                IrBlockKind::SineWave {
                    amplitude: l.scalar("Amplitude")?,
                    bias: l.scalar("Bias")?,
                    frequency: l.scalar("Frequency")?,
                    phase: l.scalar("Phase")?,
                },
                cont,
            )
        }
        "Clock" => (IrBlockKind::Clock, cont),
        other => {
            return Err(Diagnostic::error(
                E_UNSUPPORTED_BLOCK,
                format!("block type '{other}' is not supported by the code generator yet"),
                l.loc.clone(),
            ))
        }
    };
    Ok(Lowered {
        kind,
        sample_time: st,
    })
}
