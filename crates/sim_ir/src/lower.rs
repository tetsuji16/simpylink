//! Block lowering: parameter evaluation into IrBlockKind.
use crate::realize::tf_to_ss;
use crate::types::{IrBlockKind, SampleTime, TrigOp};
use sim_core::diag::{codes::*, Diagnostic, Location};
use sim_core::expr::{parse_value_in, Matrix};
use sim_core::library::{
    demux_widths, parse_sample_time_value, product_ops, sum_signs, SampleTimeParam,
};
use sim_core::model::Block;
use sim_core::path::SystemPath;
use sim_core::workspace::Workspace;

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
    ws: &'a Workspace,
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
        let v = parse_value_in(self.raw(name)?, self.ws).map_err(|e| self.eval_err(name, e))?;
        v.as_scalar()
            .ok_or_else(|| self.eval_err(name, "expected a scalar"))
    }
    fn vector(&self, name: &str) -> Result<Vec<f64>, Diagnostic> {
        let v = parse_value_in(self.raw(name)?, self.ws).map_err(|e| self.eval_err(name, e))?;
        v.as_vector()
            .ok_or_else(|| self.eval_err(name, "expected a vector"))
    }
    fn matrix(&self, name: &str) -> Result<Matrix, Diagnostic> {
        let v = parse_value_in(self.raw(name)?, self.ws).map_err(|e| self.eval_err(name, e))?;
        Ok(v.as_matrix())
    }
    /// Elements of a scalar-or-vector parameter; matrices are rejected (#161).
    fn elements(&self, name: &str) -> Result<Vec<f64>, Diagnostic> {
        let v = parse_value_in(self.raw(name)?, self.ws).map_err(|e| self.eval_err(name, e))?;
        let m = v.as_matrix();
        if m.rows > 1 && m.cols > 1 {
            return Err(self.unsupported("matrix signals are not supported"));
        }
        let items = v.as_vector().unwrap_or_default();
        if items.is_empty() {
            return Err(self.eval_err(name, "expected a scalar or vector"));
        }
        Ok(items)
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

/// Lowers one non-virtual block (no model variables).
pub fn lower_block(block: &Block, is_root: bool, path: &SystemPath) -> Result<Lowered, Diagnostic> {
    lower_block_in(block, is_root, path, &Workspace::new())
}

/// Lowers one non-virtual block; parameter expressions may use model variables in `ws` (#94).
pub fn lower_block_in(
    block: &Block,
    is_root: bool,
    path: &SystemPath,
    ws: &Workspace,
) -> Result<Lowered, Diagnostic> {
    let l = L {
        b: block,
        loc: Location::block(path, block.sid),
        ws,
    };
    let cont = Some(SampleTime::Continuous);
    let (kind, st) = match block.block_type.as_str() {
        "Constant" => {
            let v = l.elements("Value")?;
            let kind = if v.len() == 1 {
                IrBlockKind::Constant { value: v[0] }
            } else {
                IrBlockKind::ConstantVector { values: v }
            };
            (kind, l.st()?)
        }
        "Ground" => (
            IrBlockKind::Constant { value: 0.0 },
            Some(SampleTime::Constant),
        ),
        "Step" => (
            IrBlockKind::Step {
                time: l.scalar("Time")?,
                before: l.scalar("Before")?,
                after: l.scalar("After")?,
            },
            l.st()?,
        ),
        "Gain" => {
            let g = l.elements("Gain")?;
            let kind = if g.len() == 1 {
                IrBlockKind::Gain { gain: g[0] }
            } else {
                IrBlockKind::GainVector { gains: g }
            };
            (kind, l.st()?)
        }
        "ZeroOrderHold" => (IrBlockKind::ZeroOrderHold, l.st()?),
        "Mux" => (IrBlockKind::Mux { widths: vec![] }, None),
        "Demux" => {
            let raw = l.raw("Outputs")?;
            let v = parse_value_in(raw, l.ws).map_err(|e| l.eval_err("Outputs", e))?;
            let specified = demux_widths(raw).map_err(|e| l.eval_err("Outputs", e))?;
            // A scalar `Outputs` only gives the port count; the widths follow from the input.
            let widths = if v.as_scalar().is_some() {
                vec![]
            } else {
                specified
            };
            (IrBlockKind::Demux { widths }, None)
        }
        "Sum" => {
            let signs = sum_signs(l.raw("Inputs")?).map_err(|e| l.eval_err("Inputs", e))?;
            (IrBlockKind::Sum { signs }, l.st()?)
        }
        "Product" => {
            let ops = product_ops(l.raw("Inputs")?).map_err(|e| l.eval_err("Inputs", e))?;
            (IrBlockKind::Product { ops }, l.st()?)
        }
        "Abs" => (IrBlockKind::Abs, l.st()?),
        "Signum" => (IrBlockKind::Signum, l.st()?),
        "Trigonometry" => {
            let op = match l.text("Operator")? {
                "sin" => TrigOp::Sin,
                "cos" => TrigOp::Cos,
                "tan" => TrigOp::Tan,
                "asin" => TrigOp::Asin,
                "acos" => TrigOp::Acos,
                "atan" => TrigOp::Atan,
                "atan2" => TrigOp::Atan2,
                "sinh" => TrigOp::Sinh,
                "cosh" => TrigOp::Cosh,
                "tanh" => TrigOp::Tanh,
                "asinh" => TrigOp::Asinh,
                "acosh" => TrigOp::Acosh,
                "atanh" => TrigOp::Atanh,
                other => {
                    return Err(l.unsupported(format!(
                        "Trigonometric Function '{}': operator '{other}' is not supported",
                        block.name
                    )))
                }
            };
            (IrBlockKind::Trig { op }, l.st()?)
        }
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
        "Terminator" => (IrBlockKind::Terminator, None),
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
            let x0v = parse_value_in(l.raw("InitialCondition")?, l.ws)
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

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::ids::BlockId;

    fn block(sid: u32, name: &str, block_type: &str, params: &[(&str, &str)]) -> Block {
        Block {
            sid: BlockId(sid),
            name: name.into(),
            block_type: block_type.into(),
            position: sim_core::geom::Rect::new(0.0, 0.0, 30.0, 30.0),
            orientation: sim_core::model::Orientation::Right,
            params: params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            system: None,
        }
    }

    #[test]
    fn parse_sample_time_cases() {
        assert_eq!(parse_sample_time("-1").unwrap(), None);
        assert_eq!(
            parse_sample_time("0").unwrap(),
            Some(SampleTime::Continuous)
        );
        assert_eq!(
            parse_sample_time("inf").unwrap(),
            Some(SampleTime::Constant)
        );
        assert_eq!(
            parse_sample_time("Inf").unwrap(),
            Some(SampleTime::Constant)
        );
        assert_eq!(
            parse_sample_time("0.1").unwrap(),
            Some(SampleTime::Discrete {
                period: 0.1,
                offset: 0.0
            })
        );
        assert_eq!(
            parse_sample_time("[0.5, 0.1]").unwrap(),
            Some(SampleTime::Discrete {
                period: 0.5,
                offset: 0.1
            })
        );

        // Invalid cases
        assert!(parse_sample_time("-2").is_err());
        assert!(parse_sample_time("[1, 1]").is_err());
        assert!(parse_sample_time("x").is_err());
    }

    #[test]
    fn direct_feedthrough_checks() {
        assert!(IrBlockKind::Gain { gain: 1.0 }.direct_feedthrough());
        assert!(IrBlockKind::Sum { signs: vec![1, -1] }.direct_feedthrough());
        assert!(IrBlockKind::Scope.direct_feedthrough());
        assert!(IrBlockKind::Outport { port: 1 }.direct_feedthrough());
        assert!(!IrBlockKind::Constant { value: 1.0 }.direct_feedthrough());
        assert!(!IrBlockKind::Step {
            time: 1.0,
            before: 0.0,
            after: 1.0
        }
        .direct_feedthrough());
    }

    #[test]
    fn lower_mvp_blocks() {
        let root = SystemPath::root();

        // Constant
        let b = block(
            1,
            "Const",
            "Constant",
            &[("Value", "2.5"), ("SampleTime", "inf")],
        );
        let l = lower_block(&b, true, &root).unwrap();
        assert_eq!(l.kind, IrBlockKind::Constant { value: 2.5 });
        assert_eq!(l.sample_time, Some(SampleTime::Constant));

        // Step
        let b = block(
            2,
            "Step",
            "Step",
            &[
                ("Time", "1"),
                ("Before", "0"),
                ("After", "2"),
                ("SampleTime", "0"),
            ],
        );
        let l = lower_block(&b, true, &root).unwrap();
        assert_eq!(
            l.kind,
            IrBlockKind::Step {
                time: 1.0,
                before: 0.0,
                after: 2.0
            }
        );
        assert_eq!(l.sample_time, Some(SampleTime::Continuous));

        // Gain scalar and expr
        let b = block(3, "Gain", "Gain", &[("Gain", "3"), ("SampleTime", "-1")]);
        let l = lower_block(&b, true, &root).unwrap();
        assert_eq!(l.kind, IrBlockKind::Gain { gain: 3.0 });
        assert_eq!(l.sample_time, None);

        let b = block(
            4,
            "GainPi",
            "Gain",
            &[("Gain", "2*pi"), ("SampleTime", "-1")],
        );
        let l = lower_block(&b, true, &root).unwrap();
        let IrBlockKind::Gain { gain } = l.kind else {
            panic!()
        };
        assert!((gain - 2.0 * std::f64::consts::PI).abs() < 1e-12);

        // Sum signs
        let b = block(5, "Sum1", "Sum", &[("Inputs", "|+-")]);
        let l = lower_block(&b, true, &root).unwrap();
        assert_eq!(l.kind, IrBlockKind::Sum { signs: vec![1, -1] });

        let b = block(6, "Sum2", "Sum", &[("Inputs", "+-+")]);
        let l = lower_block(&b, true, &root).unwrap();
        assert_eq!(
            l.kind,
            IrBlockKind::Sum {
                signs: vec![1, -1, 1]
            }
        );

        let b = block(7, "Sum3", "Sum", &[("Inputs", "3")]);
        let l = lower_block(&b, true, &root).unwrap();
        assert_eq!(
            l.kind,
            IrBlockKind::Sum {
                signs: vec![1, 1, 1]
            }
        );

        // Scope
        let b = block(8, "Scope", "Scope", &[]);
        let l = lower_block(&b, true, &root).unwrap();
        assert_eq!(l.kind, IrBlockKind::Scope);
        assert_eq!(l.sample_time, None);

        let b = block(9, "Ground", "Ground", &[]);
        let l = lower_block(&b, true, &root).unwrap();
        assert_eq!(l.kind, IrBlockKind::Constant { value: 0.0 });
        assert_eq!(l.sample_time, Some(SampleTime::Constant));

        let b = block(10, "Terminator", "Terminator", &[]);
        let l = lower_block(&b, true, &root).unwrap();
        assert_eq!(l.kind, IrBlockKind::Terminator);
        assert!(l.kind.direct_feedthrough());

        // Outport root
        let b = block(9, "Out1", "Outport", &[("Port", "2")]);
        let l = lower_block(&b, true, &root).unwrap();
        assert_eq!(l.kind, IrBlockKind::Outport { port: 2 });
    }

    #[test]
    fn lower_abnormal_cases() {
        let root = SystemPath::root();

        // Unknown identifier in Gain
        let b = block(1, "Gain", "Gain", &[("Gain", "K")]);
        let err = lower_block(&b, true, &root).unwrap_err();
        assert_eq!(err.code, E_PARAM_EVAL);
        assert!(err.message.contains("unknown identifier 'K'"));
        assert_eq!(err.location.block, Some(BlockId(1)));

        // Matrix gain (vector gains are supported since #161)
        let b = block(2, "Gain", "Gain", &[("Gain", "[1 2; 3 4]")]);
        let err = lower_block(&b, true, &root).unwrap_err();
        assert_eq!(err.code, E_UNSUPPORTED_FEATURE);
        assert!(err.message.contains("matrix signals are not supported"));

        // Missing param in Gain
        let b = block(3, "Gain", "Gain", &[]);
        let err = lower_block(&b, true, &root).unwrap_err();
        assert_eq!(err.code, E_PARAM_EVAL);
        assert!(err.message.contains("is missing"));

        // Inport root unsupported
        let b = block(4, "In1", "Inport", &[("Port", "1")]);
        let err = lower_block(&b, true, &root).unwrap_err();
        assert_eq!(err.code, E_UNSUPPORTED_FEATURE);

        // Unsupported block type
        let b = block(5, "Unknown", "NoSuchBlock", &[]);
        let err = lower_block(&b, true, &root).unwrap_err();
        assert_eq!(err.code, E_UNSUPPORTED_BLOCK);
    }
}
