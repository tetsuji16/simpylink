//! Ctrl+G: group selected blocks into a virtual subsystem.
use crate::edit::Command;
use crate::geom::{Point, Rect};
use crate::ids::{BlockId, LineId};
use crate::library::{fill_default_params, spec};
use crate::model::{Block, Branch, Line, Model, Orientation, PortRef, System, TapPoint};
use crate::path::SystemPath;
use std::collections::{BTreeMap, BTreeSet};

fn lines_from_pairs(src: PortRef, dsts: &[PortRef], id: LineId) -> Line {
    Line {
        id,
        src,
        points: vec![],
        dst: dsts.first().copied(),
        branches: dsts[1..]
            .iter()
            .map(|d| Branch {
                tap: TapPoint { segment: 0, t: 0.5 },
                points: vec![],
                dst: Some(*d),
                branches: vec![],
            })
            .collect(),
        name: String::new(),
    }
}

fn port_block(kind: &str, sid: u32, n: usize, at: Point) -> Block {
    let s = spec(kind).expect("Inport/Outport registered");
    let mut b = Block {
        sid: BlockId(sid),
        block_type: kind.into(),
        name: format!("{}{}", if kind == "Inport" { "In" } else { "Out" }, n),
        position: Rect::from_min_size(at, s.default_size.0, s.default_size.1),
        orientation: Orientation::Right,
        params: Default::default(),
        system: None,
    };
    fill_default_params(&mut b);
    b.params.insert("Port".into(), n.to_string());
    b
}

/// Builds the command that replaces `blocks` with a subsystem. Returns (command, subsystem sid).
pub fn create_subsystem_command(
    model: &Model,
    path: &SystemPath,
    blocks: &BTreeSet<BlockId>,
) -> Result<(Command, BlockId), String> {
    let sys: &System = model.system(path).ok_or("system not found")?;
    if blocks.is_empty() {
        return Err("select blocks to create a subsystem".into());
    }
    let inner_blocks: Vec<Block> = sys
        .blocks
        .iter()
        .filter(|b| blocks.contains(&b.sid))
        .cloned()
        .collect();
    if inner_blocks.len() != blocks.len() {
        return Err("selection contains unknown blocks".into());
    }
    let bbox = inner_blocks
        .iter()
        .map(|b| b.position)
        .reduce(|a, b| a.union(&b))
        .ok_or("empty")?;
    // flatten all lines touching the selection into (src, dst) pairs
    let mut touched: Vec<LineId> = vec![];
    let mut inside: BTreeMap<PortRef, Vec<PortRef>> = BTreeMap::new();
    let mut in_from_out: BTreeMap<PortRef, Vec<PortRef>> = BTreeMap::new(); // external src -> inner dsts
    let mut out_to_out: BTreeMap<PortRef, Vec<PortRef>> = BTreeMap::new(); // inner src -> external dsts
    let mut untouched_outer: BTreeMap<PortRef, Vec<PortRef>> = BTreeMap::new(); // external src -> external dsts (of touched lines)
    for l in &sys.lines {
        let src_in = blocks.contains(&l.src.block);
        let dsts = l.destinations();
        if !src_in && !dsts.iter().any(|d| blocks.contains(&d.block)) {
            continue;
        }
        touched.push(l.id);
        for d in dsts {
            match (src_in, blocks.contains(&d.block)) {
                (true, true) => inside.entry(l.src).or_default().push(d),
                (false, true) => in_from_out.entry(l.src).or_default().push(d),
                (true, false) => out_to_out.entry(l.src).or_default().push(d),
                (false, false) => untouched_outer.entry(l.src).or_default().push(d),
            }
        }
    }
    let ss_sid = model.next_sid;
    let mut next_sid = ss_sid + 1;
    let n_in = in_from_out.len();
    let n_out = out_to_out.len();
    let mut inner = System {
        blocks: inner_blocks,
        lines: vec![],
    };
    let mut next_line = model.next_line_id;
    let mut alloc_line = || {
        let id = LineId(next_line);
        next_line += 1;
        id
    };
    // inner lines
    for (src, dsts) in &inside {
        let mut d = dsts.clone();
        if let Some(k) = out_to_out.keys().position(|s| s == src) {
            d.push(PortRef {
                block: BlockId(ss_sid + 1 + n_in as u32 + k as u32),
                port: 1,
            });
        }
        inner.lines.push(lines_from_pairs(*src, &d, alloc_line()));
    }
    for (k, (_, dsts)) in in_from_out.iter().enumerate() {
        let ib = port_block(
            "Inport",
            next_sid,
            k + 1,
            Point::new(bbox.left - 80.0, bbox.top + 40.0 * k as f64),
        );
        next_sid += 1;
        inner.lines.push(lines_from_pairs(
            PortRef {
                block: ib.sid,
                port: 1,
            },
            dsts,
            alloc_line(),
        ));
        inner.insert_block_sorted(ib);
    }
    for (k, src) in out_to_out.keys().enumerate() {
        let ob = port_block(
            "Outport",
            next_sid,
            k + 1,
            Point::new(bbox.right + 60.0, bbox.top + 40.0 * k as f64),
        );
        next_sid += 1;
        if !inside.contains_key(src) {
            inner.lines.push(lines_from_pairs(
                *src,
                &[PortRef {
                    block: ob.sid,
                    port: 1,
                }],
                alloc_line(),
            ));
        }
        inner.insert_block_sorted(ob);
    }
    inner.lines.sort_by_key(|l| l.id);
    let h = (20.0 * n_in.max(n_out) as f64).max(40.0);
    let c = bbox.center();
    let ss = Block {
        sid: BlockId(ss_sid),
        block_type: "SubSystem".into(),
        name: crate::library::unique_block_name(sys, "Subsystem"),
        position: Rect::new(
            (c.x - 30.0).round(),
            (c.y - h / 2.0).round(),
            (c.x + 30.0).round(),
            (c.y + h / 2.0).round(),
        ),
        orientation: Orientation::Right,
        params: Default::default(),
        system: Some(inner),
    };
    // outer lines
    let mut outer: BTreeMap<PortRef, Vec<PortRef>> = untouched_outer;
    for (k, src) in in_from_out.keys().enumerate() {
        outer.entry(*src).or_default().push(PortRef {
            block: BlockId(ss_sid),
            port: k as u32 + 1,
        });
    }
    for (k, dsts) in out_to_out.values().enumerate() {
        outer
            .entry(PortRef {
                block: BlockId(ss_sid),
                port: k as u32 + 1,
            })
            .or_default()
            .extend(dsts.iter().copied());
    }
    let mut cmds = vec![
        Command::DeleteItems {
            path: path.clone(),
            blocks: blocks.iter().copied().collect(),
            lines: touched,
        },
        Command::AddBlock {
            path: path.clone(),
            block: ss,
        },
    ];
    for (src, dsts) in outer {
        cmds.push(Command::AddLine {
            path: path.clone(),
            line: lines_from_pairs(src, &dsts, alloc_line()),
        });
    }
    Ok((
        Command::Batch {
            label: "Create Subsystem".into(),
            commands: cmds,
        },
        BlockId(ss_sid),
    ))
}
