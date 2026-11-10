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
        || b.block_type == "Goto"
        || b.block_type == "From"
        || b.block_type == "GotoTagVisibility"
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
        if b.block_type == "From" {
            let tag = b.params.get("GotoTag").map(String::as_str).unwrap_or("A");
            let local = sys.blocks.iter().find(|g| {
                g.block_type == "Goto"
                    && g.params.get("GotoTag").map(String::as_str).unwrap_or("A") == tag
                    && g.params
                        .get("TagVisibility")
                        .map(String::as_str)
                        .unwrap_or("local")
                        == "local"
            });
            let scope_for = |p: &SystemPath| {
                let mut p = p.clone();
                loop {
                    let system = self.model.system(&p)?;
                    if system.blocks.iter().any(|g| {
                        g.block_type == "GotoTagVisibility"
                            && g.params.get("GotoTag").map(String::as_str).unwrap_or("A") == tag
                    }) {
                        return Some(p);
                    }
                    let sid = p.0.pop()?;
                    let parent = self.model.system(&p)?;
                    if parent
                        .block(sid)?
                        .params
                        .get("TreatAsAtomicUnit")
                        .is_some_and(|v| v == "on")
                    {
                        return None;
                    }
                }
            };
            let scoped_target = || {
                let scope = scope_for(path)?;
                self.locals.keys().find_map(|p| {
                    if !p.0.starts_with(&scope.0) || scope_for(p).as_ref() != Some(&scope) {
                        return None;
                    }
                    let system = self.model.system(p)?;
                    system
                        .blocks
                        .iter()
                        .find(|g| {
                            g.block_type == "Goto"
                                && g.params.get("GotoTag").map(String::as_str).unwrap_or("A") == tag
                                && g.params.get("TagVisibility").map(String::as_str)
                                    == Some("scoped")
                        })
                        .map(|g| (p.clone(), g))
                })
            };
            let target = local
                .map(|g| (path.clone(), g))
                .or_else(scoped_target)
                .or_else(|| {
                    self.locals.keys().find_map(|p| {
                        let system = self.model.system(p)?;
                        system
                            .blocks
                            .iter()
                            .find(|g| {
                                g.block_type == "Goto"
                                    && g.params.get("GotoTag").map(String::as_str).unwrap_or("A")
                                        == tag
                                    && g.params.get("TagVisibility").map(String::as_str)
                                        == Some("global")
                            })
                            .map(|g| (p.clone(), g))
                    })
                })?;
            let d = self.locals.get(&target.0)?.get(&PortRef {
                block: target.1.sid,
                port: 1,
            })?;
            return self.resolve(&target.0, *d, depth + 1);
        }
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
                width: 1,
            });
            map.insert(src, id);
        }
    }
    (sigs, map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::ids::{BlockId, LineId};
    use sim_core::json::from_json_str;
    use sim_core::model::{Branch, Line, PortRef, TapPoint};

    fn fixture(name: &str) -> Model {
        let p = format!(
            "{}/../../tests/fixtures/models/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        from_json_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    }

    fn pr(b: u32, p: u32) -> PortRef {
        PortRef {
            block: BlockId(b),
            port: p,
        }
    }

    /// H-2: Connectivity on step_gain_scope fixture:
    /// - driver == {Gain.1 -> Step.1, Scope.1 -> Gain.1}
    /// - signals == [IrSignal{0, Step.1, "step_1"}, IrSignal{1, Gain.1, "gain_2"}]
    /// - map == {Step.1 -> 0, Gain.1 -> 1}
    #[test]
    fn step_gain_scope_connectivity_and_signals() {
        let m = fixture("step_gain_scope");
        let conn = build_connectivity(&m);

        assert_eq!(conn.driver.get(&pr(2, 1)), Some(&pr(1, 1)));
        assert_eq!(conn.driver.get(&pr(3, 1)), Some(&pr(2, 1)));
        assert_eq!(conn.driver.len(), 2);

        let (signals, map) = allocate_signals(&m, &conn);
        assert_eq!(signals.len(), 2);

        assert_eq!(signals[0].id, SignalId(0));
        assert_eq!(signals[0].source, pr(1, 1));
        assert_eq!(signals[0].var_name, "step_1");

        assert_eq!(signals[1].id, SignalId(1));
        assert_eq!(signals[1].source, pr(2, 1));
        assert_eq!(signals[1].var_name, "gain_2");

        assert_eq!(map.get(&pr(1, 1)), Some(&SignalId(0)));
        assert_eq!(map.get(&pr(2, 1)), Some(&SignalId(1)));
        assert_eq!(map.len(), 2);
    }

    /// H-2: Branching lines:
    /// Line{src Step.1, dst Gain.1, branches:[Branch{dst Scope.1}]}
    /// -> driver maps both Gain.1 and Scope.1 to Step.1
    #[test]
    fn branching_line_connectivity() {
        let mut m = fixture("step_gain_scope");
        // Remove line 2 (Gain -> Scope) and add branch to line 1 (Step -> Scope)
        m.root.lines.retain(|l| l.id == LineId(1));
        m.root.lines[0].branches.push(Branch {
            tap: TapPoint { segment: 0, t: 0.5 },
            points: vec![],
            dst: Some(pr(3, 1)),
            branches: vec![],
        });

        let conn = build_connectivity(&m);
        assert_eq!(conn.driver.get(&pr(2, 1)), Some(&pr(1, 1)));
        assert_eq!(conn.driver.get(&pr(3, 1)), Some(&pr(1, 1)));
        assert_eq!(conn.driver.len(), 2);
    }

    /// J: Abnormal cases:
    /// - dst block doesn't exist -> not in driver
    /// - port_counts is Err -> not in counts, no signals allocated
    /// - empty System -> driver empty, signals empty
    #[test]
    fn abnormal_cases() {
        // Line pointing to non-existent block
        let mut m = fixture("step_gain_scope");
        m.root.lines.push(Line {
            id: LineId(99),
            src: pr(1, 1),
            points: vec![],
            dst: Some(pr(999, 1)),
            branches: vec![],
            name: String::new(),
        });
        let conn = build_connectivity(&m);
        assert!(!conn.driver.contains_key(&pr(999, 1)));

        // Block with unknown block type (port_counts Err)
        let mut bad_block = m.root.blocks[0].clone();
        bad_block.sid = BlockId(88);
        bad_block.name = "UnknownBlock".into();
        bad_block.block_type = "NonExistentBlockType".into();
        m.root.blocks.push(bad_block);

        let conn2 = build_connectivity(&m);
        assert!(!conn2.counts.contains_key(&BlockId(88)));
        let (signals, map) = allocate_signals(&m, &conn2);
        assert!(!map.contains_key(&pr(88, 1)));
        assert!(signals.iter().all(|s| s.source.block != BlockId(88)));

        // Empty System
        let mut empty_model = m.clone();
        empty_model.root.blocks.clear();
        empty_model.root.lines.clear();
        let empty_conn = build_connectivity(&empty_model);
        assert!(empty_conn.driver.is_empty());
        assert!(empty_conn.counts.is_empty());
        let (empty_signals, empty_map) = allocate_signals(&empty_model, &empty_conn);
        assert!(empty_signals.is_empty());
        assert!(empty_map.is_empty());
    }

    #[test]
    fn goto_connections_and_cycle() {
        let mut m = fixture("goto_from");
        let conn = build_connectivity(&m);
        assert_eq!(conn.driver.get(&pr(2, 1)), Some(&pr(1, 1)));
        assert_eq!(conn.driver.get(&pr(3, 2)), Some(&pr(6, 1)));
        assert!(!flat_blocks(&m)
            .iter()
            .any(|b| b.block.block_type == "Goto" || b.block.block_type == "From"));
        m.root.blocks[7].params.insert("GotoTag".into(), "u".into());
        assert_eq!(
            build_connectivity(&m).driver.get(&pr(2, 1)),
            Some(&pr(1, 1))
        );
        // Route the Goto input back from its own From; resolution must terminate.
        m.root.lines[0].src = pr(5, 1);
        assert!(!build_connectivity(&m).driver.contains_key(&pr(2, 1)));
    }
}
