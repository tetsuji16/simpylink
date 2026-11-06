//! Model → SimIr compiler.
use crate::graph::{allocate_signals, build_connectivity, flat_blocks};
use crate::lower::lower_block;
use crate::naming::{log_name, var_base};
use crate::sample_time::{discrete_rates, rate_index, resolve_sample_times, StInput};
use crate::sort::{df_edges, loop_message, sorted_order};
use crate::types::*;
use sim_core::diag::{codes::*, has_errors, Diagnostic, Location, Severity};
use sim_core::ids::BlockId;
use sim_core::model::{Model, PortRef, SolverKind};
use sim_core::validate::validate_model;
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
    let flat = flat_blocks(model);
    // lower
    let mut lowered = BTreeMap::new();
    let mut names = BTreeMap::new();
    for fb in &flat {
        names.insert(fb.block.sid, fb.block.name.clone());
        match lower_block(fb.block, fb.path.is_root(), &fb.path) {
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
    let (signals, sigmap) = allocate_signals(model, &conn);
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
