//! Copy / paste of blocks and the lines between them.
use crate::edit::{prune_line, Command, EditError};
use crate::ids::{BlockId, LineId};
use crate::library::unique_name_with;
use crate::model::{Block, Branch, Line, Model, PortRef, System};
use crate::path::SystemPath;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Clipboard content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardPayload {
    pub format: String,
    pub version: u32,
    pub blocks: Vec<Block>,
    pub lines: Vec<Line>,
}

const CLIP_FORMAT: &str = "simpylink-clipboard";

/// Copies the selected blocks and the lines between them.
pub fn copy_selection(
    system: &crate::model::System,
    blocks: &BTreeSet<BlockId>,
) -> ClipboardPayload {
    let bs: Vec<Block> = system
        .blocks
        .iter()
        .filter(|b| blocks.contains(&b.sid))
        .cloned()
        .collect();
    let outside: BTreeSet<BlockId> = system
        .blocks
        .iter()
        .map(|b| b.sid)
        .filter(|s| !blocks.contains(s))
        .collect();
    let mut lines = vec![];
    for l in &system.lines {
        if !blocks.contains(&l.src.block) {
            continue;
        }
        let pruned = prune_line(l, &outside).unwrap_or_else(|| l.clone());
        if !pruned.destinations().is_empty() {
            lines.push(pruned);
        }
    }
    ClipboardPayload {
        format: CLIP_FORMAT.into(),
        version: 1,
        blocks: bs,
        lines,
    }
}

/// Reserves an ID while keeping the next-ID counter representable.
fn allocate_id(next: &mut u32) -> Result<u32, EditError> {
    let id = *next;
    *next = next.checked_add(1).ok_or(EditError::IdSpaceExhausted)?;
    Ok(id)
}

/// Resolves an endpoint in its own system, rejecting missing blocks.
fn remap_port(port: PortRef, map: &BTreeMap<BlockId, BlockId>) -> Result<PortRef, EditError> {
    Ok(PortRef {
        block: *map
            .get(&port.block)
            .ok_or(EditError::BlockNotFound(port.block))?,
        port: port.port,
    })
}

/// Remaps branch endpoints recursively and translates their geometry.
fn remap_branches(
    bs: &mut [Branch],
    map: &BTreeMap<BlockId, BlockId>,
    dx: f64,
    dy: f64,
) -> Result<(), EditError> {
    for b in bs {
        b.dst = b.dst.map(|d| remap_port(d, map)).transpose()?;
        for p in &mut b.points {
            p.x += dx;
            p.y += dy;
        }
        remap_branches(&mut b.branches, map, dx, dy)?;
    }
    Ok(())
}

/// Remaps all endpoints of a line and translates its geometry.
fn remap_line(
    line: &mut Line,
    map: &BTreeMap<BlockId, BlockId>,
    dx: f64,
    dy: f64,
) -> Result<(), EditError> {
    line.src = remap_port(line.src, map)?;
    line.dst = line.dst.map(|d| remap_port(d, map)).transpose()?;
    for p in &mut line.points {
        p.x += dx;
        p.y += dy;
    }
    remap_branches(&mut line.branches, map, dx, dy)
}

/// Gives every nested block and line a fresh ID, rewriting local endpoints.
fn renumber_nested(
    sys: &mut System,
    next_sid: &mut u32,
    next_line_id: &mut u32,
) -> Result<(), EditError> {
    let mut map = BTreeMap::new();
    for b in &mut sys.blocks {
        let sid = BlockId(allocate_id(next_sid)?);
        if map.insert(b.sid, sid).is_some() {
            return Err(EditError::DuplicateBlock(b.sid));
        }
        b.sid = sid;
        if let Some(child) = &mut b.system {
            renumber_nested(child, next_sid, next_line_id)?;
        }
    }
    for line in &mut sys.lines {
        line.id = LineId(allocate_id(next_line_id)?);
        remap_line(line, &map, 0.0, 0.0)?;
    }
    Ok(())
}

/// Builds the paste command (does not modify the model). Returns the new SIDs.
pub fn paste_payload(
    model: &Model,
    path: &SystemPath,
    payload: &ClipboardPayload,
    offset: (f64, f64),
) -> Result<(Command, Vec<BlockId>), EditError> {
    let sys = model.system(path).ok_or(EditError::SystemNotFound)?;
    let (dx, dy) = offset;
    let mut map = BTreeMap::new();
    let mut taken: BTreeSet<String> = sys.blocks.iter().map(|b| b.name.clone()).collect();
    let mut cmds = vec![];
    let mut new_ids = vec![];
    let mut next_sid = model.next_sid;
    let mut next_line_id = model.next_line_id;
    // Reserve all outer IDs before allocating any nested IDs.
    for b in &payload.blocks {
        let sid = BlockId(allocate_id(&mut next_sid)?);
        if map.insert(b.sid, sid).is_some() {
            return Err(EditError::DuplicateBlock(b.sid));
        }
        new_ids.push(sid);
    }
    let outer_line_ids = payload
        .lines
        .iter()
        .map(|_| allocate_id(&mut next_line_id).map(LineId))
        .collect::<Result<Vec<_>, _>>()?;
    for (b, sid) in payload.blocks.iter().zip(new_ids.iter().copied()) {
        let name = unique_name_with(|n| taken.contains(n), &b.name);
        taken.insert(name.clone());
        let mut nb = b.clone();
        nb.sid = sid;
        nb.name = name;
        nb.position = b.position.translated(dx, dy);
        if let Some(child) = &mut nb.system {
            renumber_nested(child, &mut next_sid, &mut next_line_id)?;
        }
        cmds.push(Command::AddBlock {
            path: path.clone(),
            block: nb,
        });
    }
    for (l, id) in payload.lines.iter().zip(outer_line_ids) {
        if !map.contains_key(&l.src.block)
            || l.destinations().iter().any(|d| !map.contains_key(&d.block))
        {
            continue;
        }
        let mut nl = l.clone();
        nl.id = id;
        remap_line(&mut nl, &map, dx, dy)?;
        cmds.push(Command::AddLine {
            path: path.clone(),
            line: nl,
        });
    }
    Ok((
        Command::Batch {
            label: "Paste".into(),
            commands: cmds,
        },
        new_ids,
    ))
}

/// JSON text for the system clipboard.
pub fn payload_to_text(p: &ClipboardPayload) -> String {
    serde_json::to_string(p).unwrap_or_default()
}

/// Parses clipboard text; None if it is not a simpylink payload.
pub fn payload_from_text(s: &str) -> Option<ClipboardPayload> {
    let p: ClipboardPayload = serde_json::from_str(s).ok()?;
    (p.format == CLIP_FORMAT && p.version == 1).then_some(p)
}

/// Top-left corner of all payload blocks.
pub fn payload_origin(p: &ClipboardPayload) -> Option<crate::geom::Point> {
    let r = p
        .blocks
        .iter()
        .map(|b| b.position)
        .reduce(|a, b| a.union(&b))?;
    Some(crate::geom::Point::new(r.left, r.top))
}
