//! Connectivity across virtual subsystems, and signal allocation.
use crate::naming::{signal_var_name, var_base};
use crate::types::{IrSignal, SignalId};
use sim_core::ids::BlockId;
use sim_core::library::{port_counts, PortCounts};
use sim_core::model::{Block, Model, PortRef, System};
use sim_core::path::SystemPath;
use std::collections::BTreeMap;

/// True for blocks that only route signals (not present in the IR).
pub fn is_virtual(b: &Block, path: &SystemPath) -> bool {
    b.system.is_some()
        || b.block_type == "SubSystem"
        || (!path.is_root() && (b.block_type == "Inport" || b.block_type == "Outport"))
}

/// A non-virtual block with its location.
#[derive(Debug, Clone)]
pub struct FlatBlock<'a> {
    pub path: SystemPath,
    pub block: &'a Block,
}

/// Flattened connectivity of the whole model.
#[derive(Debug, Clone, Default)]
pub struct Connectivity {
    /// key: input port of a non-virtual block → output port of a non-virtual block.
    pub driver: BTreeMap<PortRef, PortRef>,
    /// Port counts of non-virtual blocks (only when valid).
    pub counts: BTreeMap<BlockId, PortCounts>,
}

/// Local (per-system) input → source map.
fn local_drivers(sys: &System) -> BTreeMap<PortRef, PortRef> {
    let mut m = BTreeMap::new();
    for l in &sys.lines {
        for d in l.destinations() {
            m.insert(d, l.src);
        }
    }
    m
}

struct Ctx<'a> {
    model: &'a Model,
    locals: BTreeMap<SystemPath, BTreeMap<PortRef, PortRef>>,
}

impl<'a> Ctx<'a> {
    fn port_param(b: &Block) -> u32 {
        b.params
            .get("Port")
            .and_then(|p| p.trim().parse().ok())
            .unwrap_or(1)
    }
    /// Follows a source through virtual blocks to a non-virtual output port.
    fn resolve(&self, path: &SystemPath, src: PortRef, depth: usize) -> Option<PortRef> {
        if depth > 256 {
            return None;
        }
        let sys = self.model.system(path)?;
        let b = sys.block(src.block)?;
        if b.system.is_some() {
            let child = path.child(b.sid);
            let csys = b.system.as_ref()?;
            let out = csys
                .blocks
                .iter()
                .find(|x| x.block_type == "Outport" && Self::port_param(x) == src.port)?;
            let d = self.locals.get(&child)?.get(&PortRef {
                block: out.sid,
                port: 1,
            })?;
            return self.resolve(&child, *d, depth + 1);
        }
        if b.block_type == "Inport" && !path.is_root() {
            let mut parent = path.clone();
            let ss = parent.0.pop()?;
            let d = self.locals.get(&parent)?.get(&PortRef {
                block: ss,
                port: Self::port_param(b),
            })?;
            return self.resolve(&parent, *d, depth + 1);
        }
        Some(src)
    }
}

/// All non-virtual blocks, depth first, each system's blocks in sid order.
pub fn flat_blocks(model: &Model) -> Vec<FlatBlock<'_>> {
    fn rec<'a>(sys: &'a System, path: &SystemPath, out: &mut Vec<FlatBlock<'a>>) {
        for b in &sys.blocks {
            if !is_virtual(b, path) {
                out.push(FlatBlock {
                    path: path.clone(),
                    block: b,
                });
            }
        }
        for b in &sys.blocks {
            if let Some(c) = &b.system {
                rec(c, &path.child(b.sid), out);
            }
        }
    }
    let mut out = vec![];
    rec(&model.root, &SystemPath::root(), &mut out);
    out.sort_by_key(|f| f.block.sid);
    out
}

/// Builds the flattened connectivity.
pub fn build_connectivity(model: &Model) -> Connectivity {
    let mut locals = BTreeMap::new();
    fn collect(
        sys: &System,
        path: &SystemPath,
        out: &mut BTreeMap<SystemPath, BTreeMap<PortRef, PortRef>>,
    ) {
        out.insert(path.clone(), local_drivers(sys));
        for b in &sys.blocks {
            if let Some(c) = &b.system {
                collect(c, &path.child(b.sid), out);
            }
        }
    }
    collect(&model.root, &SystemPath::root(), &mut locals);
    let ctx = Ctx { model, locals };
    let mut conn = Connectivity::default();
    for fb in flat_blocks(model) {
        let Ok(c) = port_counts(fb.block) else {
            continue;
        };
        conn.counts.insert(fb.block.sid, c);
        let local = &ctx.locals[&fb.path];
        for port in 1..=c.inputs {
            let key = PortRef {
                block: fb.block.sid,
                port,
            };
            if let Some(src) = local.get(&key) {
                if let Some(r) = ctx.resolve(&fb.path, *src, 0) {
                    conn.driver.insert(key, r);
                }
            }
        }
    }
    conn
}

/// One SignalId per output port of non-virtual blocks (sid, port order).
pub fn allocate_signals(
    model: &Model,
    conn: &Connectivity,
) -> (Vec<IrSignal>, BTreeMap<PortRef, SignalId>) {
    let mut sigs = vec![];
    let mut map = BTreeMap::new();
    for fb in flat_blocks(model) {
        let Some(c) = conn.counts.get(&fb.block.sid) else {
            continue;
        };
        let vb = var_base(&fb.block.name, fb.block.sid);
        for port in 1..=c.outputs {
            let id = SignalId(sigs.len() as u32);
            let src = PortRef {
                block: fb.block.sid,
                port,
            };
            sigs.push(IrSignal {
                id,
                source: src,
                var_name: signal_var_name(&vb, port, c.outputs),
            });
            map.insert(src, id);
        }
    }
    (sigs, map)
}
