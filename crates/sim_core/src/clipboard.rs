//! Copy / paste of blocks and the lines between them.
use crate::edit::{prune_line, Command, EditError};
use crate::ids::{BlockId, LineId};
use crate::library::unique_name_with;
use crate::model::{Block, Branch, Line, Model};
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

fn remap_branches(
    bs: &[Branch],
    map: &BTreeMap<BlockId, BlockId>,
    dx: f64,
    dy: f64,
) -> Vec<Branch> {
    bs.iter()
        .map(|b| Branch {
            tap: b.tap,
            points: b
                .points
                .iter()
                .map(|p| crate::geom::Point::new(p.x + dx, p.y + dy))
                .collect(),
            dst: b.dst.map(|d| crate::model::PortRef {
                block: map[&d.block],
                port: d.port,
            }),
            branches: remap_branches(&b.branches, map, dx, dy),
        })
        .collect()
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
    for (i, b) in payload.blocks.iter().enumerate() {
        let sid = BlockId(model.next_sid + i as u32);
        map.insert(b.sid, sid);
        let name = unique_name_with(|n| taken.contains(n), &b.name);
        taken.insert(name.clone());
        let mut nb = b.clone();
        nb.sid = sid;
        nb.name = name;
        nb.position = b.position.translated(dx, dy);
        new_ids.push(sid);
        cmds.push(Command::AddBlock {
            path: path.clone(),
            block: nb,
        });
    }
    for (j, l) in payload.lines.iter().enumerate() {
        if !map.contains_key(&l.src.block)
            || l.destinations().iter().any(|d| !map.contains_key(&d.block))
        {
            continue;
        }
        let nl = Line {
            id: LineId(model.next_line_id + j as u32),
            src: crate::model::PortRef {
                block: map[&l.src.block],
                port: l.src.port,
            },
            points: l
                .points
                .iter()
                .map(|p| crate::geom::Point::new(p.x + dx, p.y + dy))
                .collect(),
            dst: l.dst.map(|d| crate::model::PortRef {
                block: map[&d.block],
                port: d.port,
            }),
            branches: remap_branches(&l.branches, &map, dx, dy),
            name: l.name.clone(),
        };
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
