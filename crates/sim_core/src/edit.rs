//! Editing commands with inverse commands, and the Undo/Redo history.
use crate::geom::Rect;
use crate::ids::{BlockId, LineId};
use crate::library::port_counts;
use crate::model::{Block, Branch, Line, Model, Orientation, PortRef, SimConfig, System};
use crate::path::SystemPath;
use std::collections::BTreeSet;

/// A model edit. `apply` returns its inverse.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddBlock {
        path: SystemPath,
        block: Block,
    },
    DeleteItems {
        path: SystemPath,
        blocks: Vec<BlockId>,
        lines: Vec<LineId>,
    },
    RestoreItems {
        path: SystemPath,
        blocks: Vec<Block>,
        lines: Vec<Line>,
        modified_lines: Vec<Line>,
    },
    MoveBlocks {
        path: SystemPath,
        blocks: Vec<BlockId>,
        dx: f64,
        dy: f64,
    },
    SetBlockPosition {
        path: SystemPath,
        block: BlockId,
        position: Rect,
    },
    SetOrientation {
        path: SystemPath,
        block: BlockId,
        orientation: Orientation,
    },
    SetParams {
        path: SystemPath,
        block: BlockId,
        params: Vec<(String, Option<String>)>,
    },
    RenameBlock {
        path: SystemPath,
        block: BlockId,
        name: String,
    },
    AddLine {
        path: SystemPath,
        line: Line,
    },
    RemoveLine {
        path: SystemPath,
        line: LineId,
    },
    ReplaceLine {
        path: SystemPath,
        line: Line,
    },
    SetConfig {
        config: SimConfig,
    },
    Batch {
        label: String,
        commands: Vec<Command>,
    },
}

/// Edit failure. The model is unchanged when an error is returned.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EditError {
    /// No ID can be allocated while keeping the next-ID counter representable.
    #[error("no free block or line id left")]
    IdSpaceExhausted,
    #[error("system not found")]
    SystemNotFound,
    #[error("block {0:?} not found")]
    BlockNotFound(BlockId),
    #[error("line {0:?} not found")]
    LineNotFound(LineId),
    #[error("block {0:?} already exists")]
    DuplicateBlock(BlockId),
    #[error("line {0:?} already exists")]
    DuplicateLine(LineId),
    #[error("block name {0:?} already used")]
    DuplicateName(String),
    #[error("input port {0:?} is already connected")]
    InputAlreadyConnected(PortRef),
    /// A line endpoint names a port the block does not have.
    #[error("port {0:?} does not exist")]
    PortNotFound(PortRef),
}

/// Menu label of a command.
pub fn command_label(cmd: &Command) -> String {
    match cmd {
        Command::AddBlock { .. } => "Add Block",
        Command::DeleteItems { .. } => "Delete",
        Command::RestoreItems { .. } => "Restore",
        Command::MoveBlocks { .. } => "Move",
        Command::SetBlockPosition { .. } => "Resize",
        Command::SetOrientation { .. } => "Rotate",
        Command::SetParams { .. } => "Edit Parameters",
        Command::RenameBlock { .. } => "Rename",
        Command::AddLine { .. } => "Connect",
        Command::RemoveLine { .. } => "Delete Line",
        Command::ReplaceLine { .. } => "Edit Line",
        Command::SetConfig { .. } => "Edit Configuration",
        Command::Batch { label, .. } => return label.clone(),
    }
    .to_string()
}

fn prune_branches(bs: &[Branch], removed: &BTreeSet<BlockId>, changed: &mut bool) -> Vec<Branch> {
    let mut out = vec![];
    for b in bs {
        let children = prune_branches(&b.branches, removed, changed);
        let dst = match b.dst {
            Some(d) if removed.contains(&d.block) => {
                *changed = true;
                None
            }
            other => other,
        };
        if dst.is_none() && children.is_empty() {
            if b.dst.is_some() || !b.branches.is_empty() {
                *changed = true;
            }
            if b.dst.is_none() && b.branches.is_empty() {
                // already dangling: keep as it was
                out.push(b.clone());
            }
            continue;
        }
        out.push(Branch {
            tap: b.tap,
            points: b.points.clone(),
            dst,
            branches: children,
        });
    }
    out
}

/// Removes destinations pointing to `removed` blocks. None if unchanged.
pub fn prune_line(line: &Line, removed: &BTreeSet<BlockId>) -> Option<Line> {
    let mut changed = false;
    let branches = prune_branches(&line.branches, removed, &mut changed);
    let dst = match line.dst {
        Some(d) if removed.contains(&d.block) => {
            changed = true;
            None
        }
        other => other,
    };
    if !changed {
        return None;
    }
    let mut l = line.clone();
    l.dst = dst;
    l.branches = branches;
    Some(l)
}

fn sys_mut<'a>(m: &'a mut Model, p: &SystemPath) -> Result<&'a mut System, EditError> {
    m.system_mut(p).ok_or(EditError::SystemNotFound)
}

fn connected_inputs(sys: &System, except: Option<LineId>) -> BTreeSet<PortRef> {
    sys.lines
        .iter()
        .filter(|l| Some(l.id) != except)
        .flat_map(|l| l.destinations())
        .collect()
}

/// One endpoint of a line: the block must exist and must have that port.
///
/// Blocks whose port counts cannot be computed (unknown type, invalid parameter) are
/// left to `validate_model`, which reports them as its own diagnostics.
fn check_endpoint(sys: &System, p: PortRef, is_output: bool) -> Result<(), EditError> {
    let block = sys
        .block(p.block)
        .ok_or(EditError::BlockNotFound(p.block))?;
    let Ok(counts) = port_counts(block) else {
        return Ok(());
    };
    let available = if is_output {
        counts.outputs
    } else {
        counts.inputs
    };
    if p.port == 0 || p.port > available {
        return Err(EditError::PortNotFound(p));
    }
    Ok(())
}

/// No destination input of `line` may already be driven by another line.
/// `ignore` is the id of a line that is being replaced (its own inputs do not count).
fn check_inputs_free(sys: &System, line: &Line, ignore: Option<LineId>) -> Result<(), EditError> {
    let used = connected_inputs(sys, ignore);
    for d in line.destinations() {
        if used.contains(&d) {
            return Err(EditError::InputAlreadyConnected(d));
        }
    }
    Ok(())
}

/// Checks that every endpoint of `line` names an existing block and port of `sys`
/// and that no destination input is already driven by another line.
/// `ignore` is the id of a line that is being replaced (its own inputs do not count).
fn check_line_endpoints(
    sys: &System,
    line: &Line,
    ignore: Option<LineId>,
) -> Result<(), EditError> {
    check_endpoint(sys, line.src, true)?;
    for d in line.destinations() {
        check_endpoint(sys, d, false)?;
    }
    check_inputs_free(sys, line, ignore)
}

fn bump_ids(m: &mut Model) {
    m.next_sid = m.next_sid.max(m.max_sid() + 1);
    m.next_line_id = m.next_line_id.max(m.max_line_id() + 1);
}

fn translate_line(l: &mut Line, dx: f64, dy: f64) {
    fn rec(bs: &mut [Branch], dx: f64, dy: f64) {
        for b in bs {
            for p in &mut b.points {
                p.x += dx;
                p.y += dy;
            }
            rec(&mut b.branches, dx, dy);
        }
    }
    for p in &mut l.points {
        p.x += dx;
        p.y += dy;
    }
    rec(&mut l.branches, dx, dy);
}

/// Applies a command and returns its inverse.
pub fn apply(model: &mut Model, cmd: Command) -> Result<Command, EditError> {
    match cmd {
        Command::AddBlock { path, block } => {
            let sys = sys_mut(model, &path)?;
            if sys.block(block.sid).is_some() {
                return Err(EditError::DuplicateBlock(block.sid));
            }
            if sys.block_by_name(&block.name).is_some() {
                return Err(EditError::DuplicateName(block.name));
            }
            let sid = block.sid;
            sys.insert_block_sorted(block);
            bump_ids(model);
            Ok(Command::DeleteItems {
                path,
                blocks: vec![sid],
                lines: vec![],
            })
        }
        Command::DeleteItems {
            path,
            blocks,
            lines,
        } => {
            let sys = sys_mut(model, &path)?;
            for id in &lines {
                if sys.line(*id).is_none() {
                    return Err(EditError::LineNotFound(*id));
                }
            }
            for sid in &blocks {
                if sys.block(*sid).is_none() {
                    return Err(EditError::BlockNotFound(*sid));
                }
            }
            let bset: BTreeSet<BlockId> = blocks.iter().copied().collect();
            let lset: BTreeSet<LineId> = lines.iter().copied().collect();
            let removed_blocks: Vec<Block> = sys
                .blocks
                .iter()
                .filter(|b| bset.contains(&b.sid))
                .cloned()
                .collect();
            let removed_lines: Vec<Line> = sys
                .lines
                .iter()
                .filter(|l| lset.contains(&l.id) || bset.contains(&l.src.block))
                .cloned()
                .collect();
            let rl: BTreeSet<LineId> = removed_lines.iter().map(|l| l.id).collect();
            sys.blocks.retain(|b| !bset.contains(&b.sid));
            sys.lines.retain(|l| !rl.contains(&l.id));
            let mut modified = vec![];
            for l in sys.lines.iter_mut() {
                if let Some(n) = prune_line(l, &bset) {
                    modified.push(l.clone());
                    *l = n;
                }
            }
            Ok(Command::RestoreItems {
                path,
                blocks: removed_blocks,
                lines: removed_lines,
                modified_lines: modified,
            })
        }
        Command::RestoreItems {
            path,
            blocks,
            lines,
            modified_lines,
        } => {
            let sys = sys_mut(model, &path)?;
            for b in &blocks {
                if sys.block(b.sid).is_some() {
                    return Err(EditError::DuplicateBlock(b.sid));
                }
            }
            for l in &lines {
                if sys.line(l.id).is_some() {
                    return Err(EditError::DuplicateLine(l.id));
                }
            }
            for l in &modified_lines {
                if sys.line(l.id).is_none() {
                    return Err(EditError::LineNotFound(l.id));
                }
            }
            // Check the lines against the system as it will look once restored, before
            // touching the model: a single bad line must leave the model unchanged.
            let mut probe = sys.clone();
            for b in &blocks {
                probe.insert_block_sorted(b.clone());
            }
            for l in &lines {
                check_line_endpoints(&probe, l, None)?;
                probe.insert_line_sorted(l.clone());
            }
            let inv = Command::DeleteItems {
                path: path.clone(),
                blocks: blocks.iter().map(|b| b.sid).collect(),
                lines: lines.iter().map(|l| l.id).collect(),
            };
            for b in blocks {
                sys.insert_block_sorted(b);
            }
            for l in lines {
                sys.insert_line_sorted(l);
            }
            for l in modified_lines {
                if let Some(x) = sys.line_mut(l.id) {
                    *x = l;
                }
            }
            bump_ids(model);
            Ok(inv)
        }
        Command::MoveBlocks {
            path,
            blocks,
            dx,
            dy,
        } => {
            let sys = sys_mut(model, &path)?;
            for sid in &blocks {
                if sys.block(*sid).is_none() {
                    return Err(EditError::BlockNotFound(*sid));
                }
            }
            let set: BTreeSet<BlockId> = blocks.iter().copied().collect();
            for b in sys.blocks.iter_mut().filter(|b| set.contains(&b.sid)) {
                b.position = b.position.translated(dx, dy);
            }
            for l in sys.lines.iter_mut() {
                if set.contains(&l.src.block)
                    && l.destinations().iter().all(|d| set.contains(&d.block))
                {
                    translate_line(l, dx, dy);
                }
            }
            Ok(Command::MoveBlocks {
                path,
                blocks,
                dx: -dx,
                dy: -dy,
            })
        }
        Command::SetBlockPosition {
            path,
            block,
            position,
        } => {
            let sys = sys_mut(model, &path)?;
            let b = sys
                .block_mut(block)
                .ok_or(EditError::BlockNotFound(block))?;
            let old = b.position;
            b.position = position;
            Ok(Command::SetBlockPosition {
                path,
                block,
                position: old,
            })
        }
        Command::SetOrientation {
            path,
            block,
            orientation,
        } => {
            let sys = sys_mut(model, &path)?;
            let b = sys
                .block_mut(block)
                .ok_or(EditError::BlockNotFound(block))?;
            let old = b.orientation;
            b.orientation = orientation;
            Ok(Command::SetOrientation {
                path,
                block,
                orientation: old,
            })
        }
        Command::SetParams {
            path,
            block,
            params,
        } => {
            let sys = sys_mut(model, &path)?;
            let b = sys
                .block_mut(block)
                .ok_or(EditError::BlockNotFound(block))?;
            let mut inv = vec![];
            for (k, v) in params {
                let old = b.params.get(&k).cloned();
                match v {
                    Some(v) => {
                        b.params.insert(k.clone(), v);
                    }
                    None => {
                        b.params.remove(&k);
                    }
                }
                inv.push((k, old));
            }
            inv.reverse();
            Ok(Command::SetParams {
                path,
                block,
                params: inv,
            })
        }
        Command::RenameBlock { path, block, name } => {
            let sys = sys_mut(model, &path)?;
            if sys.blocks.iter().any(|b| b.name == name && b.sid != block) {
                return Err(EditError::DuplicateName(name));
            }
            let b = sys
                .block_mut(block)
                .ok_or(EditError::BlockNotFound(block))?;
            let old = std::mem::replace(&mut b.name, name);
            Ok(Command::RenameBlock {
                path,
                block,
                name: old,
            })
        }
        Command::AddLine { path, line } => {
            let sys = sys_mut(model, &path)?;
            if sys.line(line.id).is_some() {
                return Err(EditError::DuplicateLine(line.id));
            }
            // `AddLine` keeps the checks it has always had: an out-of-range port is a
            // parameter problem that `validate_model` reports, so a line may be drawn
            // before the block's port count is fixed (#197 §F).
            if sys.block(line.src.block).is_none() {
                return Err(EditError::BlockNotFound(line.src.block));
            }
            for d in line.destinations() {
                if sys.block(d.block).is_none() {
                    return Err(EditError::BlockNotFound(d.block));
                }
            }
            check_inputs_free(sys, &line, None)?;
            let id = line.id;
            sys.insert_line_sorted(line);
            bump_ids(model);
            Ok(Command::RemoveLine { path, line: id })
        }
        Command::RemoveLine { path, line } => {
            let sys = sys_mut(model, &path)?;
            let idx = sys
                .lines
                .iter()
                .position(|l| l.id == line)
                .ok_or(EditError::LineNotFound(line))?;
            let l = sys.lines.remove(idx);
            Ok(Command::AddLine { path, line: l })
        }
        Command::ReplaceLine { path, line } => {
            let sys = sys_mut(model, &path)?;
            if sys.line(line.id).is_none() {
                return Err(EditError::LineNotFound(line.id));
            }
            check_line_endpoints(sys, &line, Some(line.id))?;
            let x = sys
                .line_mut(line.id)
                .ok_or(EditError::LineNotFound(line.id))?;
            let old = std::mem::replace(x, line);
            bump_ids(model);
            Ok(Command::ReplaceLine { path, line: old })
        }
        Command::SetConfig { config } => {
            let old = std::mem::replace(&mut model.config, config);
            Ok(Command::SetConfig { config: old })
        }
        Command::Batch { label, commands } => {
            let mut done = vec![];
            for c in commands {
                match apply(model, c) {
                    Ok(inv) => done.push(inv),
                    Err(e) => {
                        for inv in done.into_iter().rev() {
                            apply(model, inv)?;
                        }
                        return Err(e);
                    }
                }
            }
            done.reverse();
            Ok(Command::Batch {
                label,
                commands: done,
            })
        }
    }
}

/// Undo/Redo stacks with dirty tracking.
#[derive(Debug, Clone)]
pub struct History {
    undo: Vec<(Command, u64, String)>,
    redo: Vec<(Command, u64, String)>,
    current: u64,
    next_id: u64,
    saved: u64,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        History::new(200)
    }
}

impl History {
    /// New history with a size limit.
    pub fn new(limit: usize) -> History {
        History {
            undo: vec![],
            redo: vec![],
            current: 0,
            next_id: 1,
            saved: 0,
            limit: limit.max(1),
        }
    }
    /// Applies and records a command.
    pub fn execute(&mut self, model: &mut Model, cmd: Command) -> Result<(), EditError> {
        let label = command_label(&cmd);
        let inv = apply(model, cmd)?;
        self.undo.push((inv, self.current, label));
        self.current = self.next_id;
        self.next_id += 1;
        self.redo.clear();
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        Ok(())
    }
    /// Undo the last command. Ok(false) if nothing to undo.
    pub fn undo(&mut self, model: &mut Model) -> Result<bool, EditError> {
        let Some((inv, prev, label)) = self.undo.pop() else {
            return Ok(false);
        };
        match apply(model, inv.clone()) {
            Ok(redo_cmd) => {
                self.redo.push((redo_cmd, self.current, label));
                self.current = prev;
                Ok(true)
            }
            Err(e) => {
                self.undo.push((inv, prev, label));
                Err(e)
            }
        }
    }
    /// Redo. Ok(false) if nothing to redo.
    pub fn redo(&mut self, model: &mut Model) -> Result<bool, EditError> {
        let Some((cmd, next, label)) = self.redo.pop() else {
            return Ok(false);
        };
        match apply(model, cmd.clone()) {
            Ok(inv) => {
                self.undo.push((inv, self.current, label));
                self.current = next;
                Ok(true)
            }
            Err(e) => {
                self.redo.push((cmd, next, label));
                Err(e)
            }
        }
    }
    /// True if undo is possible.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    /// True if redo is possible.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// Label of the operation that would be undone.
    pub fn undo_label(&self) -> Option<String> {
        self.undo.last().map(|x| x.2.clone())
    }
    /// Label of the operation that would be redone.
    pub fn redo_label(&self) -> Option<String> {
        self.redo.last().map(|x| x.2.clone())
    }
    /// Marks the current state as saved.
    pub fn mark_saved(&mut self) {
        self.saved = self.current;
    }
    /// True if the model differs from the last saved state.
    pub fn is_dirty(&self) -> bool {
        self.current != self.saved
    }
    /// Clears all history (used when opening a file).
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.current = 0;
        self.saved = 0;
        self.next_id = 1;
    }
}
