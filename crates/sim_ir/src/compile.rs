//! Model → SimIr compiler.
use crate::graph::{allocate_signals, build_connectivity, flat_blocks};
use crate::lower::lower_block_in;
use crate::naming::{log_name, var_base};
use crate::sample_time::{discrete_rates, rate_index, resolve_sample_times, StInput};
use crate::sort::{df_edges, loop_message, sorted_order};
use crate::types::*;
use crate::width::propagate_widths;
use sim_core::diag::{codes::*, has_errors, Diagnostic, Location, Severity};
use sim_core::ids::BlockId;
use sim_core::model::{Model, PortRef, SolverKind};
use sim_core::validate::validate_model;
use sim_core::workspace::Workspace;
use std::collections::BTreeMap;

/// Successful compilation.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledModel {
    pub ir: SimIr,
    pub diagnostics: Vec<Diagnostic>,
}

/// Failed compilation (contains at least one error).
#[derive(Debug, Clone, PartialEq)]
pub struct CompileFailure {
    pub diagnostics: Vec<Diagnostic>,
}

/// SciPy method name for a solver.
pub fn solver_method(kind: SolverKind) -> &'static str {
    match kind {
        SolverKind::Auto | SolverKind::Ode45 => "RK45",
        SolverKind::Ode23 => "RK23",
        SolverKind::Ode15s => "BDF",
        // Fixed-step methods are implemented by the generated runtime (#99).
        SolverKind::FixedDiscrete => "discrete",
        SolverKind::Ode1 => "ode1",
        SolverKind::Ode2 => "ode2",
        SolverKind::Ode3 => "ode3",
        SolverKind::Ode4 => "ode4",
        SolverKind::Ode5 => "ode5",
    }
}

/// Compiles a model.
pub fn compile(model: &Model) -> Result<CompiledModel, CompileFailure> {
    let mut diags = validate_model(model);
    let fail = |mut d: Vec<Diagnostic>| {
        d.sort_by_key(|x| x.severity != Severity::Error);
        Err(CompileFailure { diagnostics: d })
    };
    if has_errors(&diags) {
        return fail(diags);
    }
    // model variables (#94)
    let ws = match Workspace::from_definitions(&model.variables) {
        Ok(ws) => ws,
        Err(errs) => {
            for e in errs {
                diags.push(Diagnostic::error(
                    E_VARIABLE,
                    e.to_string(),
                    Location::default(),
                ));
            }
            Workspace::new()
        }
    };
    let flat = flat_blocks(model);
    // lower
    let mut lowered = BTreeMap::new();
    let mut names = BTreeMap::new();
    for fb in &flat {
        names.insert(fb.block.sid, fb.block.name.clone());
        match lower_block_in(fb.block, fb.path.is_root(), &fb.path, &ws) {
            Ok(l) => {
                lowered.insert(fb.block.sid, (l, fb));
            }
            Err(d) => diags.push(d),
        }
    }
    if has_errors(&diags) {
        return fail(diags);
    }
    let conn = build_connectivity(model);
    let (mut signals, sigmap) = allocate_signals(model, &conn);
    // signal widths (#161)
    match propagate_widths(&conn, &lowered) {
        Ok(w) => {
            for s in signals.iter_mut() {
                s.width = w.get(&s.source).copied().unwrap_or(1);
            }
            let input_width = |sid: BlockId, port: u32| {
                conn.driver
                    .get(&PortRef { block: sid, port })
                    .and_then(|src| w.get(src).copied())
                    .unwrap_or(1)
            };
            for (sid, (l, fb)) in lowered.iter_mut() {
                let c = conn
                    .counts
                    .get(sid)
                    .copied()
                    .unwrap_or(sim_core::library::PortCounts {
                        inputs: 0,
                        outputs: 0,
                    });
                match &mut l.kind {
                    IrBlockKind::Mux { widths } => {
                        *widths = (1..=c.inputs).map(|p| input_width(*sid, p)).collect();
                    }
                    IrBlockKind::Demux { widths } if widths.is_empty() => {
                        *widths = (1..=c.outputs)
                            .map(|p| {
                                w.get(&PortRef {
                                    block: *sid,
                                    port: p,
                                })
                                .copied()
                                .unwrap_or(1)
                            })
                            .collect();
                    }
                    _ => {}
                }
                if matches!(
                    l.kind,
                    IrBlockKind::TransferFcn { .. }
                        | IrBlockKind::StateSpace { .. }
                        | IrBlockKind::Pid { .. }
                ) {
                    for p in 1..=c.inputs {
                        if input_width(*sid, p) > 1 {
                            diags.push(Diagnostic::error(
                                E_UNSUPPORTED_FEATURE,
                                format!("vector input to '{}' is not supported", fb.block.name),
                                Location::block(&fb.path, *sid),
                            ));
                        }
                    }
                }
            }
            // code generation for vectors lands in #162
            if let Some(sig) = signals.iter().find(|s| s.width > 1) {
                let (_, fb) = &lowered[&sig.source.block];
                diags.push(Diagnostic::error(
                    E_UNSUPPORTED_FEATURE,
                    "vector signals are not supported in code generation yet",
                    Location::block(&fb.path, sig.source.block),
                ));
            }
        }
        Err(errs) => {
            diags.extend(errs);
            return fail(diags);
        }
    }
    if has_errors(&diags) {
        return fail(diags);
    }
    // sort
    let df: BTreeMap<BlockId, bool> = lowered
        .iter()
        .map(|(s, (l, _))| (*s, l.kind.direct_feedthrough()))
        .collect();
    let nodes: Vec<BlockId> = lowered.keys().copied().collect();
    let order = match sorted_order(&nodes, &df_edges(&conn, &df)) {
        Ok(o) => o,
        Err(loops) => {
            for lp in loops {
                let fb = lowered[&lp[0]].1;
                diags.push(Diagnostic::error(
                    E_ALGEBRAIC_LOOP,
                    loop_message(&lp, &names),
                    Location::block(&fb.path, lp[0]),
                ));
            }
            return fail(diags);
        }
    };
    // sample times
    let st_inputs: Vec<StInput> = nodes
        .iter()
        .map(|sid| {
            let (l, fb) = &lowered[sid];
            let n_in = conn.counts.get(sid).map(|c| c.inputs).unwrap_or(0);
            let drivers = (1..=n_in)
                .filter_map(|p| {
                    conn.driver
                        .get(&PortRef {
                            block: *sid,
                            port: p,
                        })
                        .map(|s| s.block)
                })
                .collect();
            StInput {
                sid: *sid,
                name: fb.block.name.clone(),
                declared: l.sample_time,
                needs_discrete: l.kind.needs_discrete(),
                drivers,
            }
        })
        .collect();
    let resolved = match resolve_sample_times(&st_inputs) {
        Ok(r) => r,
        Err(errs) => {
            for (sid, msg) in errs {
                let fb = lowered[&sid].1;
                diags.push(Diagnostic::error(
                    E_SAMPLE_TIME,
                    msg,
                    Location::block(&fb.path, sid),
                ));
            }
            return fail(diags);
        }
    };
    let rates = discrete_rates(&resolved);
    // states (sid order)
    let mut cont = BTreeMap::new();
    let mut disc = BTreeMap::new();
    let (mut co, mut dof) = (0usize, 0usize);
    for sid in &nodes {
        let k = &lowered[sid].0.kind;
        let (nc, nd) = (k.num_cont_states(), k.num_disc_states());
        if nc > 0 {
            cont.insert(*sid, co..co + nc);
            co += nc;
        }
        if nd > 0 {
            disc.insert(*sid, dof..dof + nd);
            dof += nd;
        }
    }
    // fixed-step solvers (#99): decide h and validate it against the rates
    let mut fixed_step: Option<f64> = None;
    if model.config.solver.is_fixed_step() {
        let span = model.config.stop_time - model.config.start_time;
        let h = match model.config.fixed_step {
            Some(h) if h > 0.0 && h <= span => h,
            Some(_) => {
                diags.push(Diagnostic::error(
                    E_INVALID_CONFIG,
                    "fixed step must be positive and not larger than stop - start",
                    Location::default(),
                ));
                return fail(diags);
            }
            // auto: the smallest discrete period, or 50 steps over the whole run
            None => rates
                .iter()
                .map(|r| r.0)
                .reduce(f64::min)
                .unwrap_or(span / 50.0),
        };
        if model.config.solver == SolverKind::FixedDiscrete && co > 0 {
            diags.push(Diagnostic::error(
                E_INVALID_CONFIG,
                "the discrete solver cannot integrate continuous states; choose ode1..ode5",
                Location::default(),
            ));
        }
        for (ts, off) in &rates {
            let integral = |r: f64| (r - r.round()).abs() <= 1e-9 * r.abs().max(1.0);
            if integral(ts / h) && integral(off / h) {
                continue;
            }
            // every rate comes from a block, so an owner always exists
            let Some(sid) = resolved
                .iter()
                .find(|(_, st)| {
                    matches!(
                        st,
                        SampleTime::Discrete { period, offset }
                            if (period - ts).abs() <= 1e-12 && (offset - off).abs() <= 1e-12
                    )
                })
                .map(|(sid, _)| *sid)
            else {
                continue;
            };
            let fb = lowered[&sid].1;
            diags.push(Diagnostic::error(
                E_SAMPLE_TIME,
                format!(
                    "sample time [{}, {}] of '{}' is not a multiple of the fixed step {}",
                    ts, off, fb.block.name, h
                ),
                Location::block(&fb.path, sid),
            ));
        }
        if has_errors(&diags) {
            return fail(diags);
        }
        fixed_step = Some(h);
    }
    // blocks in execution order
    let mut blocks = vec![];
    for sid in &order {
        let (l, fb) = &lowered[sid];
        let c = conn
            .counts
            .get(sid)
            .copied()
            .unwrap_or(sim_core::library::PortCounts {
                inputs: 0,
                outputs: 0,
            });
        let inputs = (1..=c.inputs)
            .map(|p| {
                conn.driver
                    .get(&PortRef {
                        block: *sid,
                        port: p,
                    })
                    .and_then(|s| sigmap.get(s).copied())
            })
            .collect();
        let outputs = (1..=c.outputs)
            .filter_map(|p| {
                sigmap
                    .get(&PortRef {
                        block: *sid,
                        port: p,
                    })
                    .copied()
            })
            .collect();
        let st = resolved[sid];
        blocks.push(IrBlock {
            sid: *sid,
            name: fb.block.name.clone(),
            block_type: fb.block.block_type.clone(),
            var_base: var_base(&fb.block.name, *sid),
            kind: l.kind.clone(),
            inputs,
            outputs,
            sample_time: st,
            cont_state: cont.get(sid).cloned(),
            disc_state: disc.get(sid).cloned(),
            rate_index: if l.kind.needs_discrete() {
                rate_index(&rates, st)
            } else {
                None
            },
        });
    }
    // breakpoints
    let cfg = &model.config;
    let mut bps: Vec<f64> = vec![];
    for sid in &nodes {
        if let IrBlockKind::Step { time, .. } = lowered[sid].0.kind {
            if time.is_finite()
                && time > cfg.start_time
                && time <= cfg.stop_time
                && !bps.iter().any(|b| (b - time).abs() <= 1e-12)
            {
                bps.push(time);
            }
        }
    }
    bps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // logs
    let by_sid: BTreeMap<BlockId, &IrBlock> = blocks.iter().map(|b| (b.sid, b)).collect();
    let mut logs = vec![];
    for sid in &nodes {
        let b = by_sid[sid];
        if b.kind == IrBlockKind::Scope {
            for (i, s) in b.inputs.iter().enumerate() {
                logs.push(IrLog {
                    name: log_name(&b.name, Some(i as u32 + 1)),
                    block_sid: *sid,
                    port: i as u32 + 1,
                    signal: *s,
                });
            }
        }
    }
    let mut outs: Vec<(u32, BlockId)> = blocks
        .iter()
        .filter_map(|b| {
            if let IrBlockKind::Outport { port } = b.kind {
                Some((port, b.sid))
            } else {
                None
            }
        })
        .collect();
    outs.sort();
    for (_, sid) in outs {
        let b = by_sid[&sid];
        logs.push(IrLog {
            name: log_name(&b.name, None),
            block_sid: sid,
            port: 1,
            signal: b.inputs.first().copied().flatten(),
        });
    }
    let ir = SimIr {
        model_name: model.name.clone(),
        config: IrConfig {
            start_time: cfg.start_time,
            stop_time: cfg.stop_time,
            output_step: cfg.output_step,
            solver_method: solver_method(cfg.solver).into(),
            rel_tol: cfg.rel_tol,
            abs_tol: cfg.abs_tol,
            max_step: cfg.max_step,
            fixed_step,
        },
        blocks,
        signals,
        num_cont_states: co,
        num_disc_states: dof,
        discrete_rates: rates,
        breakpoints: bps,
        logs,
    };
    Ok(CompiledModel {
        ir,
        diagnostics: diags,
    })
}
