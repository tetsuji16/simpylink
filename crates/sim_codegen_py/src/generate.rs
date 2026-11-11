//! SimIr → Python source (DES-04 §2).
use crate::emit::{py_comment, py_float, py_matrix, py_str, py_vector, PyWriter};
use crate::runtime::RUNTIME_FORMAT1;
use crate::{CodegenError, CodegenOptions, GeneratedCode};
use sim_ir::naming::script_file_name;
use sim_ir::{IrBlock, IrBlockKind, MathOp, SignalId, SimIr, SqrtOp, TrigOp};
use std::collections::BTreeMap;

type R<T> = Result<T, CodegenError>;

struct Ctx<'a> {
    names: BTreeMap<SignalId, &'a str>,
}

impl Ctx<'_> {
    /// Input variable (or 0.0) inside compute_outputs.
    fn u(&self, b: &IrBlock, i: usize) -> String {
        b.inputs
            .get(i)
            .copied()
            .flatten()
            .map(|s| self.names[&s].to_string())
            .unwrap_or_else(|| "0.0".into())
    }
    /// Input as y["..."] (or 0.0) in derivatives/updates.
    fn uy(&self, b: &IrBlock, i: usize) -> String {
        b.inputs
            .get(i)
            .copied()
            .flatten()
            .map(|s| format!("y[\"{}\"]", self.names[&s]))
            .unwrap_or_else(|| "0.0".into())
    }
}

fn params(w: &mut PyWriter, b: &IrBlock) -> R<()> {
    let vb = b.var_base.to_uppercase();
    let mut items: Vec<(&str, String)> = vec![];
    match &b.kind {
        IrBlockKind::Constant { value } => items.push(("VALUE", py_float(*value)?)),
        IrBlockKind::Step {
            time,
            before,
            after,
        } => {
            items.push(("TIME", py_float(*time)?));
            items.push(("BEFORE", py_float(*before)?));
            items.push(("AFTER", py_float(*after)?));
        }
        IrBlockKind::Gain { gain } => items.push(("GAIN", py_float(*gain)?)),
        IrBlockKind::Bias { bias } => items.push(("BIAS", py_float(*bias)?)),
        IrBlockKind::Saturate { upper, lower } => {
            items.push(("UPPER_LIMIT", py_float(*upper)?));
            items.push(("LOWER_LIMIT", py_float(*lower)?));
        }
        IrBlockKind::Switch { threshold, .. } => items.push(("THRESHOLD", py_float(*threshold)?)),
        IrBlockKind::DeadZone { lower, upper } => {
            items.push(("LOWER_VALUE", py_float(*lower)?));
            items.push(("UPPER_VALUE", py_float(*upper)?));
        }
        IrBlockKind::Integrator { x0 } => items.push(("INITIAL_CONDITION", py_float(*x0)?)),
        IrBlockKind::TransferFcn { a, b: bv, c, d } => {
            if a.rows > 0 {
                items.push(("A", py_matrix(a)?));
                items.push(("B", py_vector(bv)?));
                items.push(("C", py_vector(c)?));
            }
            items.push(("D", py_float(*d)?));
        }
        IrBlockKind::StateSpace { a, b: bv, c, d, x0 } => {
            if a.rows > 0 {
                items.push(("A", py_matrix(a)?));
                items.push(("B", py_vector(bv)?));
                items.push(("C", py_vector(c)?));
            }
            items.push(("D", py_float(*d)?));
            items.push(("X0", py_vector(x0)?));
        }
        IrBlockKind::UnitDelay { x0 } => {
            items.push(("INITIAL_CONDITION", py_float(*x0)?));
            items.push(("SAMPLE_TIME", period(b)?));
        }
        IrBlockKind::DiscreteIntegrator { gain, x0 } => {
            items.push(("GAIN_VALUE", py_float(*gain)?));
            items.push(("INITIAL_CONDITION", py_float(*x0)?));
            items.push(("SAMPLE_TIME", period(b)?));
        }
        IrBlockKind::Pid { p, i, d, n } => {
            items.push(("P", py_float(*p)?));
            items.push(("I", py_float(*i)?));
            items.push(("D", py_float(*d)?));
            items.push(("N", py_float(*n)?));
        }
        IrBlockKind::ZeroOrderHold => {
            if let sim_ir::SampleTime::Discrete { period, offset } = b.sample_time {
                items.push(("PERIOD", py_float(period)?));
                items.push(("OFFSET", py_float(offset)?));
            }
        }
        IrBlockKind::SineWave {
            amplitude,
            bias,
            frequency,
            phase,
        } => {
            items.push(("AMPLITUDE", py_float(*amplitude)?));
            items.push(("BIAS", py_float(*bias)?));
            items.push(("FREQUENCY", py_float(*frequency)?));
            items.push(("PHASE", py_float(*phase)?));
        }
        _ => {}
    }
    if items.is_empty() {
        return Ok(());
    }
    w.line(&format!("# {} (SID {})", py_comment(&b.name), b.sid.0));
    for (k, v) in items {
        w.line(&format!("{vb}_{k} = {v}"));
    }
    Ok(())
}

fn period(b: &IrBlock) -> R<String> {
    match b.sample_time {
        sim_ir::SampleTime::Discrete { period, .. } => py_float(period),
        _ => Ok("0.0".into()),
    }
}

fn output_expr(c: &Ctx, b: &IrBlock) -> R<Option<String>> {
    let vb = b.var_base.to_uppercase();
    let s0 = b.cont_state.as_ref().map(|r| r.start).unwrap_or(0);
    let range = b
        .cont_state
        .as_ref()
        .map(|r| format!("{}:{}", r.start, r.end))
        .unwrap_or_default();
    let d0 = b.disc_state.as_ref().map(|r| r.start).unwrap_or(0);
    Ok(Some(match &b.kind {
        IrBlockKind::Constant { .. } => format!("{vb}_VALUE"),
        IrBlockKind::Step { .. } => format!("{vb}_AFTER if t_seg >= {vb}_TIME else {vb}_BEFORE"),
        IrBlockKind::Gain { .. } => format!("{vb}_GAIN * {}", c.u(b, 0)),
        IrBlockKind::Sum { signs } => {
            let mut s = String::new();
            for (i, sg) in signs.iter().enumerate() {
                let u = c.u(b, i);
                if i == 0 {
                    s.push_str(&if *sg > 0 { u } else { format!("-{u}") });
                } else {
                    s.push_str(&format!(" {} {u}", if *sg > 0 { '+' } else { '-' }));
                }
            }
            s
        }
        IrBlockKind::Product { ops } => {
            let mut s = String::new();
            for (i, op) in ops.iter().enumerate() {
                let u = c.u(b, i);
                if i == 0 {
                    s.push_str(&if *op == '*' { u } else { format!("1.0 / {u}") });
                } else {
                    s.push_str(&format!(" {op} {u}"));
                }
            }
            s
        }
        IrBlockKind::Abs => format!("abs({})", c.u(b, 0)),
        IrBlockKind::Trig { op } => match op {
            TrigOp::Atan2 => format!("math.atan2({}, {})", c.u(b, 0), c.u(b, 1)),
            other => format!("math.{}({})", other.py_name(), c.u(b, 0)),
        },
        IrBlockKind::Signum => {
            let u = c.u(b, 0);
            format!("float(({u} > 0) - ({u} < 0))")
        }
        IrBlockKind::MathFn { op } => {
            let (u1, u2) = (c.u(b, 0), c.u(b, 1));
            match op {
                MathOp::Exp => format!("math.exp({u1})"),
                MathOp::Log => format!("math.log({u1})"),
                MathOp::TenToU => format!("10.0 ** {u1}"),
                MathOp::Log10 => format!("math.log10({u1})"),
                MathOp::MagnitudeSquared | MathOp::Square => format!("{u1} * {u1}"),
                MathOp::Sqrt => format!("math.sqrt({u1})"),
                MathOp::Pow => format!("{u1} ** {u2}"),
                MathOp::Conj | MathOp::Transpose | MathOp::Hermitian => u1,
                MathOp::Reciprocal => format!("1.0 / {u1}"),
                MathOp::Hypot => format!("math.hypot({u1}, {u2})"),
                MathOp::Rem => format!("({u1} if {u2} == 0.0 else math.fmod({u1}, {u2}))"),
                MathOp::Mod => {
                    format!("({u1} if {u2} == 0.0 else {u1} - math.floor({u1} / {u2}) * {u2})")
                }
            }
        }
        IrBlockKind::Sqrt { op } => {
            let u = c.u(b, 0);
            match op {
                SqrtOp::Sqrt => format!("math.sqrt({u})"),
                SqrtOp::SignedSqrt => format!("math.copysign(math.sqrt(abs({u})), {u})"),
                SqrtOp::RSqrt => format!("1.0 / math.sqrt({u})"),
            }
        }
        IrBlockKind::MinMax { is_max, inputs } => {
            let f = if *is_max { "max" } else { "min" };
            if *inputs <= 1 {
                // A single scalar input is its own minimum and maximum.
                c.u(b, 0)
            } else {
                let args: Vec<String> = (0..*inputs as usize).map(|i| c.u(b, i)).collect();
                format!("{f}({})", args.join(", "))
            }
        }
        IrBlockKind::DeadZone { .. } => format!(
            "({u} - {vb}_UPPER_VALUE) if {u} > {vb}_UPPER_VALUE else \
             (({u} - {vb}_LOWER_VALUE) if {u} < {vb}_LOWER_VALUE else 0.0)",
            u = c.u(b, 0)
        ),
        IrBlockKind::Bias { .. } => format!("{} + {vb}_BIAS", c.u(b, 0)),
        IrBlockKind::Saturate { .. } => format!(
            "min(max({}, {vb}_LOWER_LIMIT), {vb}_UPPER_LIMIT)",
            c.u(b, 0)
        ),
        IrBlockKind::Switch { criteria, .. } => {
            let cond = match criteria {
                0 => format!("{} >= {vb}_THRESHOLD", c.u(b, 1)),
                1 => format!("{} > {vb}_THRESHOLD", c.u(b, 1)),
                _ => format!("{} != 0.0", c.u(b, 1)),
            };
            format!("{} if {cond} else {}", c.u(b, 0), c.u(b, 2))
        }
        IrBlockKind::Integrator { .. } => format!("x[{s0}]"),
        IrBlockKind::TransferFcn { a, d, .. } | IrBlockKind::StateSpace { a, d, .. } => {
            if a.rows == 0 {
                format!("{vb}_D * {}", c.u(b, 0))
            } else if *d == 0.0 {
                format!("float({vb}_C @ x[{range}])")
            } else {
                format!("float({vb}_C @ x[{range}]) + {vb}_D * {}", c.u(b, 0))
            }
        }
        IrBlockKind::UnitDelay { .. } | IrBlockKind::DiscreteIntegrator { .. } => {
            format!("d[{d0}]")
        }
        IrBlockKind::Pid { .. } => {
            let u = c.u(b, 0);
            format!(
                "{vb}_P * {u} + x[{s0}] + {vb}_D * {vb}_N * ({u} - x[{}])",
                s0 + 1
            )
        }
        IrBlockKind::SineWave { .. } => {
            format!("{vb}_AMPLITUDE * math.sin({vb}_FREQUENCY * t + {vb}_PHASE) + {vb}_BIAS")
        }
        IrBlockKind::Clock => "t".into(),
        IrBlockKind::ZeroOrderHold => match b.sample_time {
            // The runtime samples d before integrating the interval. Only major
            // outputs may pass the input through; solver stages use that hold,
            // including a stage at the next sample hit.
            sim_ir::SampleTime::Discrete { .. } => format!(
                "{} if (sample_inputs and t == t_seg and _sample_hit(t_seg, {vb}_PERIOD, {vb}_OFFSET)) else d[{d0}]",
                c.u(b, 0)
            ),
            _ => c.u(b, 0),
        },
        IrBlockKind::Scope | IrBlockKind::Terminator | IrBlockKind::Outport { .. } => {
            return Ok(None)
        }
        // Vector code generation lands in #162.
        IrBlockKind::ConstantVector { .. }
        | IrBlockKind::GainVector { .. }
        | IrBlockKind::Mux { .. }
        | IrBlockKind::Demux { .. } => {
            return Err(CodegenError::UnsupportedBlock {
                name: b.name.clone(),
                block_type: b.block_type.clone(),
            })
        }
    }))
}

fn by_sid(ir: &SimIr) -> Vec<&IrBlock> {
    let mut v: Vec<&IrBlock> = ir.blocks.iter().collect();
    v.sort_by_key(|b| b.sid);
    v
}

/// Generates the Python file.
pub fn generate(ir: &SimIr, opts: &CodegenOptions) -> Result<GeneratedCode, CodegenError> {
    let names: BTreeMap<SignalId, &str> = ir
        .signals
        .iter()
        .map(|s| (s.id, s.var_name.as_str()))
        .collect();
    let c = Ctx { names };
    let file_name = script_file_name(&ir.model_name);
    let bar = format!("# {}", "-".repeat(75));
    let mut w = PyWriter::new();
    let has_discrete_zoh = ir.blocks.iter().any(|b| {
        matches!(b.kind, IrBlockKind::ZeroOrderHold)
            && matches!(b.sample_time, sim_ir::SampleTime::Discrete { .. })
    });
    let doc_name = ir.model_name.replace('\\', "\\\\").replace('"', "'");
    w.line("#!/usr/bin/env python3");
    w.line("\"\"\"Simulation code generated by simpylink.");
    w.blank();
    w.line(&format!("Model: {doc_name}"));
    w.line(&format!(
        "Generator: simpylink-codegen-py {} (format 1)",
        opts.generator_version
    ));
    w.blank();
    w.line("Standalone: requires only Python >= 3.10, NumPy and SciPy.");
    w.line(&format!("Run: python {file_name} --out-dir results"));
    w.line("\"\"\"");
    w.blank();
    for m in ["argparse", "json", "math", "os", "sys", "traceback"] {
        w.line(&format!("import {m}"));
    }
    w.blank();
    w.line("import numpy as np");
    w.line("from scipy.integrate import solve_ivp");
    w.blank();
    w.line(&format!("MODEL_NAME = {}", py_str(&ir.model_name)));
    w.blank();
    w.line(&bar);
    w.line("# Simulation settings");
    w.line(&bar);
    let cfg = &ir.config;
    w.line(&format!("START_TIME = {}", py_float(cfg.start_time)?));
    w.line(&format!("STOP_TIME = {}", py_float(cfg.stop_time)?));
    w.line(&format!("OUTPUT_STEP = {}", py_float(cfg.output_step)?));
    w.line(&format!("SOLVER = {}", py_str(&cfg.solver_method)));
    w.line(&format!("REL_TOL = {}", py_float(cfg.rel_tol)?));
    w.line(&format!("ABS_TOL = {}", py_float(cfg.abs_tol)?));
    w.line(&format!(
        "MAX_STEP = {}",
        match cfg.max_step {
            None => "math.inf".to_string(),
            Some(v) => py_float(v)?,
        }
    ));
    w.line(&format!(
        "FIXED_STEP = {}",
        match cfg.fixed_step {
            None => "None".to_string(),
            Some(v) => py_float(v)?,
        }
    ));
    w.blank();
    w.line(&bar);
    w.line("# Block parameters");
    w.line(&bar);
    let sorted = by_sid(ir);
    for b in &sorted {
        params(&mut w, b)?;
    }
    w.blank();
    w.line(&bar);
    w.line("# States, rates and logging");
    w.line(&bar);
    w.line(&format!("NUM_CONT_STATES = {}", ir.num_cont_states));
    let mut x0 = vec![];
    let mut d0 = vec![];
    for b in &sorted {
        let vb = b.var_base.to_uppercase();
        match &b.kind {
            IrBlockKind::Integrator { .. } => x0.push(format!("{vb}_INITIAL_CONDITION")),
            IrBlockKind::TransferFcn { a, .. } => {
                x0.extend(std::iter::repeat_n("0.0".to_string(), a.rows))
            }
            IrBlockKind::StateSpace { a, .. } if a.rows > 0 => x0.push(format!("*{vb}_X0")),
            IrBlockKind::Pid { .. } => x0.extend(["0.0".to_string(), "0.0".to_string()]),
            IrBlockKind::UnitDelay { .. } | IrBlockKind::DiscreteIntegrator { .. } => {
                d0.push(format!("{vb}_INITIAL_CONDITION"))
            }
            IrBlockKind::ZeroOrderHold if b.disc_state.is_some() => d0.push("0.0".to_string()),
            _ => {}
        }
    }
    w.line(&format!("X0 = [{}]", x0.join(", ")));
    w.line(&format!("NUM_DISC_STATES = {}", ir.num_disc_states));
    w.line(&format!("D0 = [{}]", d0.join(", ")));
    let rates: R<Vec<String>> = ir
        .discrete_rates
        .iter()
        .map(|(p, o)| Ok(format!("({}, {})", py_float(*p)?, py_float(*o)?)))
        .collect();
    w.line(&format!("DISCRETE_RATES = [{}]", rates?.join(", ")));
    let bps: R<Vec<String>> = ir.breakpoints.iter().map(|v| py_float(*v)).collect();
    w.line(&format!("BREAKPOINTS = [{}]", bps?.join(", ")));
    if ir.logs.is_empty() {
        w.line("LOG_CHANNELS = []");
    } else {
        w.line("LOG_CHANNELS = [");
        for l in &ir.logs {
            w.line(&format!(
                "    {{\"name\": {}, \"block_sid\": {}, \"port\": {}}},",
                py_str(&l.name),
                l.block_sid.0,
                l.port
            ));
        }
        w.line("]");
    }
    w.blank();
    w.blank();
    if ir
        .blocks
        .iter()
        .any(|b| matches!(b.kind, IrBlockKind::ZeroOrderHold))
    {
        w.line("def _sample_hit(t, period, offset):");
        w.indent();
        w.line("\"\"\"True if t is a sample hit of the rate (period, offset).\"\"\"");
        w.line("k = round((t - offset) / period)");
        w.line("return k >= 0 and abs(t - (offset + k * period)) <= _time_tolerance()");
        w.dedent();
        w.blank();
        w.blank();
    }
    // compute_outputs
    w.line("def compute_outputs(t, x, d, t_seg):");
    w.indent();
    w.line("\"\"\"Compute every block output at time t in execution order.\"\"\"");
    if has_discrete_zoh {
        w.line("return _compute_outputs(t, x, d, t_seg, True)");
        w.dedent();
        w.blank();
        w.blank();
        w.line("def _compute_outputs(t, x, d, t_seg, sample_inputs):");
        w.indent();
        w.line("\"\"\"Solver stages use the stored ZOH samples, even at the interval start.\"\"\"");
    }
    let mut assigned = vec![];
    for b in &ir.blocks {
        let Some(expr) = output_expr(&c, b)? else {
            continue;
        };
        let Some(out) = b.outputs.first() else {
            continue;
        };
        let var = c.names[out];
        w.line(&format!("# {} (SID {})", py_comment(&b.name), b.sid.0));
        w.line(&format!("{var} = {expr}"));
        assigned.push(var);
    }
    if assigned.is_empty() {
        w.line("return {}");
    } else {
        w.line("return {");
        for v in &assigned {
            w.line(&format!("    \"{v}\": {v},"));
        }
        w.line("}");
    }
    w.dedent();
    w.blank();
    w.blank();
    // compute_derivatives
    w.line("def compute_derivatives(t, x, d, t_seg):");
    w.indent();
    w.line("\"\"\"Return dx/dt for the continuous states.\"\"\"");
    w.line(if has_discrete_zoh {
        "y = _compute_outputs(t, x, d, t_seg, False)"
    } else {
        "y = compute_outputs(t, x, d, t_seg)"
    });
    w.line("dx = np.zeros(NUM_CONT_STATES)");
    for b in &sorted {
        let vb = b.var_base.to_uppercase();
        let Some(r) = &b.cont_state else { continue };
        w.line(&format!("# {} (SID {})", py_comment(&b.name), b.sid.0));
        match &b.kind {
            IrBlockKind::Integrator { .. } => w.line(&format!("dx[{}] = {}", r.start, c.uy(b, 0))),
            IrBlockKind::TransferFcn { .. } | IrBlockKind::StateSpace { .. } => w.line(&format!(
                "dx[{0}:{1}] = {vb}_A @ x[{0}:{1}] + {vb}_B * {2}",
                r.start,
                r.end,
                c.uy(b, 0)
            )),
            IrBlockKind::Pid { .. } => {
                w.line(&format!("dx[{}] = {vb}_I * {}", r.start, c.uy(b, 0)));
                w.line(&format!(
                    "dx[{0}] = {vb}_N * ({1} - x[{0}])",
                    r.start + 1,
                    c.uy(b, 0)
                ));
            }
            _ => {}
        }
    }
    w.line("return dx");
    w.dedent();
    w.blank();
    w.blank();
    // update_discrete_states
    w.line("def update_discrete_states(t, x, d, y, hits):");
    w.indent();
    w.line("\"\"\"Return the discrete state vector after the sample hits in `hits`.\"\"\"");
    w.line("d_next = d.copy()");
    for b in &sorted {
        let vb = b.var_base.to_uppercase();
        let (Some(r), Some(rate)) = (&b.disc_state, b.rate_index) else {
            continue;
        };
        w.line(&format!("if {rate} in hits:"));
        w.indent();
        w.line(&format!(
            "# {} (SID {}), rate {rate}",
            py_comment(&b.name),
            b.sid.0
        ));
        match &b.kind {
            IrBlockKind::UnitDelay { .. } => {
                w.line(&format!("d_next[{}] = {}", r.start, c.uy(b, 0)))
            }
            IrBlockKind::ZeroOrderHold => w.line(&format!("d_next[{}] = {}", r.start, c.uy(b, 0))),
            IrBlockKind::DiscreteIntegrator { .. } => w.line(&format!(
                "d_next[{0}] = d[{0}] + {vb}_GAIN_VALUE * {vb}_SAMPLE_TIME * {1}",
                r.start,
                c.uy(b, 0)
            )),
            _ => {}
        }
        w.dedent();
    }
    w.line("return d_next");
    w.dedent();
    w.blank();
    w.blank();
    w.line("def log_values(y):");
    w.indent();
    w.line("\"\"\"Return the logged values in LOG_CHANNELS order.\"\"\"");
    if ir.logs.is_empty() {
        w.line("return []");
    } else {
        w.line("return [");
        for l in &ir.logs {
            match l.signal {
                Some(s) => w.line(&format!("    y[\"{}\"],", c.names[&s])),
                None => w.line("    0.0,"),
            }
        }
        w.line("]");
    }
    let mut source = w.finish();
    source.push_str(RUNTIME_FORMAT1);
    Ok(GeneratedCode { file_name, source })
}
