//! Signal width propagation (#161, DES-07 §3.2).
use crate::graph::{Connectivity, FlatBlock};
use crate::lower::Lowered;
use crate::types::IrBlockKind;
use sim_core::diag::{codes::E_DIMENSION, Diagnostic, Location};
use sim_core::ids::BlockId;
use sim_core::library::PortCounts;
use sim_core::model::PortRef;
use std::collections::BTreeMap;

/// Width of every output port of non-virtual blocks. Unknown widths default to 1.
pub fn propagate_widths(
    conn: &Connectivity,
    lowered: &BTreeMap<BlockId, (Lowered, &FlatBlock)>,
) -> Result<BTreeMap<PortRef, usize>, Vec<Diagnostic>> {
    let mut widths: BTreeMap<PortRef, usize> = BTreeMap::new();
    // State blocks start from their initial condition (scalar until #162) so that
    // feedback loops have a width before their input is known.
    for (sid, (l, _)) in lowered {
        let c = counts(conn, *sid);
        if !matches!(
            l.kind,
            IrBlockKind::Integrator { .. }
                | IrBlockKind::UnitDelay { .. }
                | IrBlockKind::DiscreteIntegrator { .. }
        ) {
            continue;
        }
        for port in 1..=c.outputs {
            widths.insert(PortRef { block: *sid, port }, 1);
        }
    }
    // Inputs that are not connected count as width 1; widths still unknown are skipped.
    for _ in 0..=lowered.len() {
        let mut changed = false;
        for (sid, (l, fb)) in lowered {
            let c = counts(conn, *sid);
            if c.outputs == 0 {
                continue;
            }
            let Some(ins) = known_input_widths(*sid, &c, conn, &widths) else {
                continue;
            };
            // Mismatches are reported once every width is final (see below).
            let Ok(ws) = out_widths(&l.kind, c.outputs as usize, &ins, &fb.block.name) else {
                continue;
            };
            for (i, width) in ws.into_iter().enumerate() {
                let key = PortRef {
                    block: *sid,
                    port: i as u32 + 1,
                };
                if widths.get(&key) != Some(&width) {
                    widths.insert(key, width);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for sid in lowered.keys() {
        let c = counts(conn, *sid);
        for port in 1..=c.outputs {
            widths.entry(PortRef { block: *sid, port }).or_insert(1);
        }
    }
    let mut diags = vec![];
    for (sid, (l, fb)) in lowered {
        let c = counts(conn, *sid);
        if c.outputs == 0 {
            continue;
        }
        let ins = input_widths(*sid, &c, conn, &widths);
        if let Err(message) = out_widths(&l.kind, c.outputs as usize, &ins, &fb.block.name) {
            diags.push(Diagnostic::error(
                E_DIMENSION,
                message,
                Location::block(&fb.path, *sid),
            ));
        }
    }
    if diags.is_empty() {
        Ok(widths)
    } else {
        Err(diags)
    }
}

fn counts(conn: &Connectivity, sid: BlockId) -> PortCounts {
    conn.counts.get(&sid).copied().unwrap_or(PortCounts {
        inputs: 0,
        outputs: 0,
    })
}

/// Input widths, or `None` while a connected driver's width is still unknown.
fn known_input_widths(
    sid: BlockId,
    c: &PortCounts,
    conn: &Connectivity,
    widths: &BTreeMap<PortRef, usize>,
) -> Option<Vec<usize>> {
    (1..=c.inputs)
        .map(
            |port| match conn.driver.get(&PortRef { block: sid, port }) {
                None => Some(1),
                Some(src) => widths.get(src).copied(),
            },
        )
        .collect()
}

/// Input widths after propagation (unknown → 1).
fn input_widths(
    sid: BlockId,
    c: &PortCounts,
    conn: &Connectivity,
    widths: &BTreeMap<PortRef, usize>,
) -> Vec<usize> {
    (1..=c.inputs)
        .map(|port| {
            conn.driver
                .get(&PortRef { block: sid, port })
                .and_then(|src| widths.get(src).copied())
                .unwrap_or(1)
        })
        .collect()
}

/// Largest input width; every input must be 1 or that width (element-wise blocks).
fn element_wise(name: &str, ins: &[usize]) -> Result<usize, String> {
    let mut w = 1;
    for (i, x) in ins.iter().enumerate() {
        if *x == 1 {
            continue;
        }
        if w == 1 {
            w = *x;
        } else if *x != w {
            return Err(format!(
                "'{name}' input {} has width {x}, expected 1 or {w}",
                i + 1
            ));
        }
    }
    Ok(w)
}

/// Output widths of one block, one entry per output port.
fn out_widths(
    kind: &IrBlockKind,
    outputs: usize,
    ins: &[usize],
    name: &str,
) -> Result<Vec<usize>, String> {
    let w = match kind {
        IrBlockKind::Constant { .. } => 1,
        IrBlockKind::ConstantVector { values } => values.len(),
        IrBlockKind::Step { .. } | IrBlockKind::SineWave { .. } | IrBlockKind::Clock => 1,
        IrBlockKind::Gain { .. } => ins.first().copied().unwrap_or(1),
        IrBlockKind::GainVector { gains } => {
            let k = gains.len();
            let w1 = ins.first().copied().unwrap_or(1);
            if k != 1 && w1 != 1 && k != w1 {
                return Err(format!(
                    "'{name}' gain has {k} elements but the input has width {w1}"
                ));
            }
            k.max(w1)
        }
        IrBlockKind::Sum { .. } | IrBlockKind::Product { .. } if ins.len() == 1 => 1,
        IrBlockKind::Sum { .. }
        | IrBlockKind::Product { .. }
        | IrBlockKind::Bias { .. }
        | IrBlockKind::Abs
        | IrBlockKind::Saturate { .. }
        | IrBlockKind::Switch { .. } => element_wise(name, ins)?,
        IrBlockKind::Integrator { .. }
        | IrBlockKind::UnitDelay { .. }
        | IrBlockKind::DiscreteIntegrator { .. } => ins.first().copied().unwrap_or(1),
        IrBlockKind::Mux { .. } => ins.iter().sum(),
        IrBlockKind::Demux { widths } => {
            let w1 = ins.first().copied().unwrap_or(1);
            if widths.is_empty() {
                if w1 % outputs != 0 {
                    return Err(format!(
                        "'{name}' cannot split width {w1} into {outputs} outputs"
                    ));
                }
                let part = w1 / outputs;
                return Ok(vec![part; outputs]);
            }
            let total: usize = widths.iter().sum();
            if total != w1 {
                return Err(format!(
                    "'{name}' cannot split width {w1} into {outputs} outputs"
                ));
            }
            return Ok(widths.clone());
        }
        // Scalar-only blocks: TransferFcn, StateSpace, PIDController (#161 §F-4).
        IrBlockKind::TransferFcn { .. }
        | IrBlockKind::StateSpace { .. }
        | IrBlockKind::Pid { .. } => 1,
        _ => 1,
    };
    Ok(vec![w; outputs])
}
