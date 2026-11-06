//! Canvas interaction state machine (DES-05 §5). egui independent: driven by `CanvasEvent`s.
use super::geometry::{block_ports, system_bounds};
use super::hit::{blocks_in_rect, hit_test, input_connected, HitTarget};
use super::route::{line_path, node_at, tap_position};
use super::view::{Modifiers, ScreenPos, ScreenVec, ViewTransform};
use sim_core::edit::Command;
use sim_core::geom::{Point, Rect};
use sim_core::ids::{BlockId, LineId};
use sim_core::library::{new_block, port_counts, spec};
use sim_core::model::{Branch, Line, Model, PortRef, System, TapPoint};
use sim_core::path::SystemPath;
use std::collections::BTreeSet;

pub const DRAG_THRESHOLD_PX: f32 = 4.0;
pub const WHEEL_ZOOM_BASE: f32 = 1.0015;
pub const KEY_ZOOM_STEP: f32 = 1.25;
pub const FIT_MARGIN_PX: f32 = 20.0;

/// Input events (canvas-local screen coordinates).
#[derive(Debug, Clone, PartialEq)]
pub enum CanvasEvent {
    PointerDown {
        pos: ScreenPos,
        button: PointerButton,
        mods: Modifiers,
    },
    PointerMove {
        pos: ScreenPos,
        mods: Modifiers,
    },
    PointerUp {
        pos: ScreenPos,
        button: PointerButton,
        mods: Modifiers,
    },
    DoubleClick {
        pos: ScreenPos,
        mods: Modifiers,
    },
    Scroll {
        pos: ScreenPos,
        delta: ScreenVec,
        mods: Modifiers,
    },
    Key {
        key: CanvasKey,
        pressed: bool,
        mods: Modifiers,
    },
    PointerLeft,
    DropBlock {
        pos: ScreenPos,
        block_type: String,
    },
}

/// Pointer buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

/// Keys handled by the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasKey {
    Delete,
    Backspace,
    Escape,
    Space,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    A,
    Z,
    Y,
    C,
    X,
    V,
    D,
    G,
    R,
    I,
    Plus,
    Minus,
    Zero,
    Enter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

/// Where a line being drawn starts.
#[derive(Debug, Clone, PartialEq)]
pub enum LineStart {
    Output(PortRef),
    Input(PortRef),
    Branch {
        line: LineId,
        branch: Vec<usize>,
        tap: TapPoint,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum Interaction {
    Idle,
    Pressed {
        start: ScreenPos,
        target: HitTarget,
        button: PointerButton,
        mods: Modifiers,
        deselected: bool,
    },
    DraggingBlocks {
        start: ScreenPos,
        current: ScreenPos,
        blocks: Vec<BlockId>,
        copy: bool,
        axis_lock: Option<Axis>,
    },
    RubberBand {
        start: ScreenPos,
        current: ScreenPos,
        additive: bool,
    },
    DrawingLine {
        from: LineStart,
        current: ScreenPos,
    },
    Panning {
        last: ScreenPos,
        button: PointerButton,
    },
    SpaceHeld {
        used_for_pan: bool,
    },
}

/// Actions the application must perform.
#[derive(Debug, Clone, PartialEq)]
pub enum CanvasAction {
    Execute(Command),
    OpenBlockDialog(BlockId),
    Undo,
    Redo,
    Copy,
    Cut,
    Paste {
        at: Point,
    },
    DuplicateBlocks {
        blocks: Vec<BlockId>,
        dx: f64,
        dy: f64,
    },
    FitToView,
    QuickInsert {
        at: Point,
    },
    OpenSubsystem(BlockId),
    GoToParent,
    CreateSubsystem,
    Status(String),
}

/// Selected items.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Selection {
    pub blocks: BTreeSet<BlockId>,
    pub lines: BTreeSet<LineId>,
}

impl Selection {
    /// Nothing selected.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty() && self.lines.is_empty()
    }
    /// Select exactly one block.
    pub fn select_only_block(&mut self, sid: BlockId) {
        self.blocks = BTreeSet::from([sid]);
        self.lines.clear();
    }
    /// Toggle a block.
    pub fn toggle_block(&mut self, sid: BlockId) {
        if !self.blocks.remove(&sid) {
            self.blocks.insert(sid);
        }
    }
}

/// Data for drawing transient feedback.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CanvasPreview {
    pub drag: Option<(Vec<BlockId>, f64, f64)>,
    pub rubber_band: Option<Rect>,
    pub drawing_line: Option<(Point, Point)>,
}

/// The controller.
#[derive(Debug, Clone)]
pub struct CanvasController {
    pub view: ViewTransform,
    pub selection: Selection,
    pub viewport: ScreenVec,
    pub last_pointer: Option<ScreenPos>,
    interaction: Interaction,
}

impl Default for CanvasController {
    fn default() -> Self {
        Self::new()
    }
}

fn first_free_input(sys: &System, block: BlockId) -> Option<PortRef> {
    let b = sys.block(block)?;
    let n = port_counts(b).ok()?.inputs;
    (1..=n)
        .map(|p| PortRef { block, port: p })
        .find(|p| !input_connected(sys, *p))
}

fn first_unused_output(sys: &System, block: BlockId) -> Option<PortRef> {
    let b = sys.block(block)?;
    let n = port_counts(b).ok()?.outputs;
    if n == 0 {
        return None;
    }
    (1..=n)
        .map(|p| PortRef { block, port: p })
        .find(|p| !sys.lines.iter().any(|l| l.src == *p))
        .or(Some(PortRef { block, port: 1 }))
}

fn new_line(model: &Model, src: PortRef, dst: PortRef) -> Line {
    Line {
        id: LineId(model.next_line_id),
        src,
        points: vec![],
        dst: Some(dst),
        branches: vec![],
        name: String::new(),
    }
}

fn add_branch(line: &Line, path: &[usize], tap: TapPoint, dst: PortRef) -> Option<Line> {
    let mut l = line.clone();
    let nb = Branch {
        tap,
        points: vec![],
        dst: Some(dst),
        branches: vec![],
    };
    if path.is_empty() {
        l.branches.push(nb);
        return Some(l);
    }
    let mut cur = l.branches.get_mut(path[0])?;
    for i in &path[1..] {
        cur = cur.branches.get_mut(*i)?;
    }
    cur.branches.push(nb);
    Some(l)
}

fn block_name(sys: &System, sid: BlockId) -> String {
    sys.block(sid).map(|b| b.name.clone()).unwrap_or_default()
}

impl CanvasController {
    /// New controller (identity view, 800x600 viewport).
    pub fn new() -> Self {
        CanvasController {
            view: ViewTransform::identity(),
            selection: Selection::default(),
            viewport: ScreenVec::new(800.0, 600.0),
            last_pointer: None,
            interaction: Interaction::Idle,
        }
    }
    /// True when no gesture is in progress.
    pub fn is_idle(&self) -> bool {
        matches!(self.interaction, Interaction::Idle)
    }
    /// Clears the selection.
    pub fn clear_selection(&mut self) {
        self.selection = Selection::default();
    }
    /// Drops selected ids that no longer exist.
    pub fn retain_existing(&mut self, sys: &System) {
        self.selection.blocks.retain(|b| sys.block(*b).is_some());
        self.selection.lines.retain(|l| sys.line(*l).is_some());
    }
    /// Fits all blocks into the viewport.
    pub fn fit_to_view(&mut self, sys: &System) {
        match system_bounds(sys) {
            None => self.view = ViewTransform::identity(),
            Some(b) => self.view.fit(b, self.viewport, FIT_MARGIN_PX),
        }
    }
    /// Fits all blocks but never zooms in beyond `max_zoom`.
    pub fn fit_to_view_limited(&mut self, sys: &System, max_zoom: f32) {
        self.fit_to_view(sys);
        if self.view.zoom > max_zoom {
            let c = self.center();
            self.view.zoom_about(c, max_zoom / self.view.zoom);
        }
    }
    /// Resets transient state (used when switching systems).
    pub fn reset_interaction(&mut self) {
        self.interaction = Interaction::Idle;
    }

    fn center(&self) -> ScreenPos {
        ScreenPos::new(self.viewport.x / 2.0, self.viewport.y / 2.0)
    }

    fn drag_delta(&self, start: ScreenPos, cur: ScreenPos, axis: Option<Axis>) -> (f64, f64) {
        let (mut dx, mut dy) = self.view.screen_delta_to_model(cur - start);
        match axis {
            Some(Axis::X) => dy = 0.0,
            Some(Axis::Y) => dx = 0.0,
            None => {}
        }
        (dx, dy)
    }

    /// Preview for drawing.
    pub fn preview(&self) -> CanvasPreview {
        self.preview_with(None)
    }

    /// Preview, resolving line start points with the system.
    pub fn preview_with(&self, sys: Option<&System>) -> CanvasPreview {
        let mut p = CanvasPreview::default();
        match &self.interaction {
            Interaction::DraggingBlocks {
                start,
                current,
                blocks,
                axis_lock,
                ..
            } => {
                let (dx, dy) = self.drag_delta(*start, *current, *axis_lock);
                p.drag = Some((blocks.clone(), dx, dy));
            }
            Interaction::RubberBand { start, current, .. } => {
                p.rubber_band = Some(Rect::normalized(
                    self.view.to_model(*start),
                    self.view.to_model(*current),
                ));
            }
            Interaction::DrawingLine { from, current } => {
                if let Some(sys) = sys {
                    if let Some(s) = line_start_point(sys, from) {
                        p.drawing_line = Some((s, self.view.to_model(*current)));
                    }
                }
            }
            _ => {}
        }
        p
    }

    /// Handles one event.
    pub fn handle(
        &mut self,
        ev: CanvasEvent,
        model: &Model,
        path: &SystemPath,
    ) -> Vec<CanvasAction> {
        match &ev {
            CanvasEvent::PointerDown { pos, .. }
            | CanvasEvent::PointerMove { pos, .. }
            | CanvasEvent::PointerUp { pos, .. }
            | CanvasEvent::DoubleClick { pos, .. }
            | CanvasEvent::Scroll { pos, .. } => self.last_pointer = Some(*pos),
            _ => {}
        }
        let Some(sys) = model.system(path) else {
            return vec![];
        };
        if let CanvasEvent::Key {
            key: CanvasKey::Escape,
            pressed: true,
            ..
        } = ev
        {
            if !self.is_idle() && !matches!(self.interaction, Interaction::SpaceHeld { .. }) {
                self.interaction = Interaction::Idle;
                return vec![];
            }
        }
        let state = std::mem::replace(&mut self.interaction, Interaction::Idle);
        let (next, actions) = self.step(state, ev, model, sys, path);
        self.interaction = next;
        actions
    }

    fn step(
        &mut self,
        state: Interaction,
        ev: CanvasEvent,
        model: &Model,
        sys: &System,
        path: &SystemPath,
    ) -> (Interaction, Vec<CanvasAction>) {
        use CanvasEvent as E;
        use Interaction as I;
        match (state, ev) {
            // ---------------- Idle
            (I::Idle, E::PointerDown { pos, button, mods }) => self.on_down(pos, button, mods, sys),
            (I::Idle, E::DoubleClick { pos, .. }) => {
                let a = match hit_test(sys, &self.view, pos) {
                    HitTarget::Block(b) => {
                        let is_ss = sys.block(b).map(|x| x.system.is_some()).unwrap_or(false);
                        if is_ss {
                            CanvasAction::OpenSubsystem(b)
                        } else {
                            CanvasAction::OpenBlockDialog(b)
                        }
                    }
                    HitTarget::Empty => CanvasAction::QuickInsert {
                        at: self.view.to_model(pos),
                    },
                    _ => return (I::Idle, vec![]),
                };
                (I::Idle, vec![a])
            }
            (I::Idle, E::Scroll { pos, delta, mods }) => {
                if mods.ctrl {
                    self.view.pan(ScreenVec::new(0.0, delta.y));
                } else if mods.shift {
                    let d = if delta.y != 0.0 { delta.y } else { delta.x };
                    self.view.pan(ScreenVec::new(d, 0.0));
                } else if delta.y != 0.0 {
                    self.view.zoom_about(pos, WHEEL_ZOOM_BASE.powf(delta.y));
                }
                (I::Idle, vec![])
            }
            (
                I::Idle,
                E::Key {
                    key,
                    pressed: true,
                    mods,
                },
            ) => self.on_key(key, mods, model, sys, path),
            (I::Idle, E::DropBlock { pos, block_type }) => {
                let m = self.view.to_model(pos);
                let Some(s) = spec(&block_type) else {
                    return (
                        I::Idle,
                        vec![CanvasAction::Status(format!(
                            "unknown block type '{block_type}'"
                        ))],
                    );
                };
                let (w, h) = s.default_size;
                let tl = Point::new((m.x - w / 2.0).round(), (m.y - h / 2.0).round());
                match new_block(sys, &block_type, BlockId(model.next_sid), tl) {
                    Ok(b) => {
                        self.selection.select_only_block(b.sid);
                        (
                            I::Idle,
                            vec![CanvasAction::Execute(Command::AddBlock {
                                path: path.clone(),
                                block: b,
                            })],
                        )
                    }
                    Err(e) => (I::Idle, vec![CanvasAction::Status(e)]),
                }
            }
            (I::Idle, _) => (I::Idle, vec![]),
            // ---------------- Pressed
            (
                I::Pressed {
                    start,
                    target,
                    button,
                    mods,
                    deselected,
                },
                E::PointerMove { pos, mods: mmods },
            ) => {
                if (pos - start).length() < DRAG_THRESHOLD_PX {
                    return (
                        I::Pressed {
                            start,
                            target,
                            button,
                            mods,
                            deselected,
                        },
                        vec![],
                    );
                }
                let next = match &target {
                    HitTarget::OutputPort(p) if button == PointerButton::Primary => {
                        I::DrawingLine {
                            from: LineStart::Output(*p),
                            current: pos,
                        }
                    }
                    HitTarget::InputPort(p) if button == PointerButton::Primary => {
                        if input_connected(sys, *p) {
                            return (
                                I::Pressed {
                                    start,
                                    target,
                                    button,
                                    mods,
                                    deselected,
                                },
                                vec![],
                            );
                        }
                        I::DrawingLine {
                            from: LineStart::Input(*p),
                            current: pos,
                        }
                    }
                    HitTarget::Block(b) => {
                        if deselected {
                            return (
                                I::Pressed {
                                    start,
                                    target,
                                    button,
                                    mods,
                                    deselected,
                                },
                                vec![],
                            );
                        }
                        let copy = mods.ctrl || button == PointerButton::Secondary;
                        let blocks = if self.selection.blocks.contains(b) {
                            self.selection.blocks.iter().copied().collect()
                        } else {
                            vec![*b]
                        };
                        let axis_lock = None;
                        let mut st = I::DraggingBlocks {
                            start,
                            current: pos,
                            blocks,
                            copy,
                            axis_lock,
                        };
                        if mmods.shift {
                            st = self.lock_axis(st);
                        }
                        st
                    }
                    HitTarget::Line {
                        line,
                        branch,
                        segment,
                        t,
                    } if (button == PointerButton::Primary && mods.ctrl)
                        || button == PointerButton::Secondary =>
                    {
                        I::DrawingLine {
                            from: LineStart::Branch {
                                line: *line,
                                branch: branch.clone(),
                                tap: TapPoint {
                                    segment: *segment,
                                    t: *t,
                                },
                            },
                            current: pos,
                        }
                    }
                    HitTarget::Empty if button == PointerButton::Primary => I::RubberBand {
                        start,
                        current: pos,
                        additive: mods.shift,
                    },
                    _ => I::Pressed {
                        start,
                        target,
                        button,
                        mods,
                        deselected,
                    },
                };
                (next, vec![])
            }
            (I::Pressed { target, mods, .. }, E::PointerUp { .. }) => {
                if let HitTarget::Block(b) = target {
                    if mods.ctrl {
                        return (I::Idle, self.ctrl_click(b, model, sys, path));
                    }
                }
                (I::Idle, vec![])
            }
            (s @ I::Pressed { .. }, _) => (s, vec![]),
            // ---------------- DraggingBlocks
            (
                I::DraggingBlocks {
                    start,
                    blocks,
                    copy,
                    axis_lock,
                    ..
                },
                E::PointerMove { pos, mods },
            ) => {
                let st = I::DraggingBlocks {
                    start,
                    current: pos,
                    blocks,
                    copy,
                    axis_lock,
                };
                (if mods.shift { self.lock_axis(st) } else { st }, vec![])
            }
            (
                I::DraggingBlocks {
                    start,
                    blocks,
                    copy,
                    axis_lock,
                    ..
                },
                E::PointerUp { pos, .. },
            ) => {
                let (dx, dy) = self.drag_delta(start, pos, axis_lock);
                let (dx, dy) = (dx.round(), dy.round());
                if dx == 0.0 && dy == 0.0 {
                    return (I::Idle, vec![]);
                }
                if copy {
                    (
                        I::Idle,
                        vec![CanvasAction::DuplicateBlocks { blocks, dx, dy }],
                    )
                } else {
                    (
                        I::Idle,
                        vec![CanvasAction::Execute(Command::MoveBlocks {
                            path: path.clone(),
                            blocks,
                            dx,
                            dy,
                        })],
                    )
                }
            }
            (s @ I::DraggingBlocks { .. }, _) => (s, vec![]),
            // ---------------- RubberBand
            (
                I::RubberBand {
                    start, additive, ..
                },
                E::PointerMove { pos, .. },
            ) => (
                I::RubberBand {
                    start,
                    current: pos,
                    additive,
                },
                vec![],
            ),
            (
                I::RubberBand {
                    start, additive, ..
                },
                E::PointerUp { pos, .. },
            ) => {
                let r = Rect::normalized(self.view.to_model(start), self.view.to_model(pos));
                let hits = blocks_in_rect(sys, r);
                if !additive {
                    self.selection = Selection::default();
                }
                self.selection.blocks.extend(hits);
                (I::Idle, vec![])
            }
            (s @ I::RubberBand { .. }, _) => (s, vec![]),
            // ---------------- DrawingLine
            (I::DrawingLine { from, .. }, E::PointerMove { pos, .. }) => {
                (I::DrawingLine { from, current: pos }, vec![])
            }
            (I::DrawingLine { from, .. }, E::PointerUp { pos, .. }) => {
                (I::Idle, self.finish_line(from, pos, model, sys, path))
            }
            (s @ I::DrawingLine { .. }, _) => (s, vec![]),
            // ---------------- Panning
            (I::Panning { last, button }, E::PointerMove { pos, .. }) => {
                self.view.pan(pos - last);
                (I::Panning { last: pos, button }, vec![])
            }
            (I::Panning { button, .. }, E::PointerUp { .. }) => {
                if button == PointerButton::Primary {
                    (I::SpaceHeld { used_for_pan: true }, vec![])
                } else {
                    (I::Idle, vec![])
                }
            }
            (
                I::Panning { last, button },
                E::Key {
                    key: CanvasKey::Space,
                    pressed: false,
                    ..
                },
            ) => {
                if button == PointerButton::Primary {
                    (
                        I::Panning {
                            last,
                            button: PointerButton::Middle,
                        },
                        vec![],
                    )
                } else {
                    (I::Panning { last, button }, vec![])
                }
            }
            (s @ I::Panning { .. }, _) => (s, vec![]),
            // ---------------- SpaceHeld
            (
                I::SpaceHeld { .. },
                E::PointerDown {
                    pos,
                    button: PointerButton::Primary,
                    ..
                },
            ) => (
                I::Panning {
                    last: pos,
                    button: PointerButton::Primary,
                },
                vec![],
            ),
            (
                I::SpaceHeld { used_for_pan },
                E::Key {
                    key: CanvasKey::Space,
                    pressed: false,
                    ..
                },
            ) => {
                if used_for_pan {
                    (I::Idle, vec![])
                } else {
                    self.fit_to_view(sys);
                    (I::Idle, vec![CanvasAction::FitToView])
                }
            }
            (
                I::SpaceHeld { .. },
                E::Key {
                    key,
                    pressed: true,
                    mods,
                },
            ) if key != CanvasKey::Space => {
                // Space+Plus / Space+Minus / Space+0 zoom shortcuts (R2026b).
                match key {
                    CanvasKey::Plus => self.view.zoom_about(self.center(), KEY_ZOOM_STEP),
                    CanvasKey::Minus => self.view.zoom_about(self.center(), 1.0 / KEY_ZOOM_STEP),
                    CanvasKey::Zero => {
                        let z = self.view.zoom;
                        self.view.zoom_about(self.center(), 1.0 / z)
                    }
                    _ => {
                        let _ = mods;
                    }
                }
                (I::SpaceHeld { used_for_pan: true }, vec![])
            }
            (s @ I::SpaceHeld { .. }, _) => (s, vec![]),
        }
    }

    fn lock_axis(&self, st: Interaction) -> Interaction {
        if let Interaction::DraggingBlocks {
            start,
            current,
            blocks,
            copy,
            axis_lock: None,
        } = st
        {
            let (dx, dy) = self.view.screen_delta_to_model(current - start);
            let axis = if dx.abs() >= dy.abs() {
                Axis::X
            } else {
                Axis::Y
            };
            return Interaction::DraggingBlocks {
                start,
                current,
                blocks,
                copy,
                axis_lock: Some(axis),
            };
        }
        st
    }

    fn on_down(
        &mut self,
        pos: ScreenPos,
        button: PointerButton,
        mods: Modifiers,
        sys: &System,
    ) -> (Interaction, Vec<CanvasAction>) {
        if button == PointerButton::Middle {
            return (Interaction::Panning { last: pos, button }, vec![]);
        }
        let target = hit_test(sys, &self.view, pos);
        let mut deselected = false;
        if button == PointerButton::Primary {
            match &target {
                HitTarget::Block(b) if !mods.ctrl => {
                    if !self.selection.blocks.contains(b) {
                        if mods.shift {
                            self.selection.blocks.insert(*b);
                        } else {
                            self.selection.select_only_block(*b);
                        }
                    } else if mods.shift {
                        self.selection.blocks.remove(b);
                        deselected = true;
                    }
                }
                HitTarget::Line { line, .. } if !mods.ctrl => {
                    if mods.shift {
                        if !self.selection.lines.remove(line) {
                            self.selection.lines.insert(*line);
                        }
                    } else {
                        self.selection = Selection {
                            blocks: BTreeSet::new(),
                            lines: BTreeSet::from([*line]),
                        };
                    }
                }
                HitTarget::Empty if !mods.shift => self.selection = Selection::default(),
                _ => {}
            }
        }
        (
            Interaction::Pressed {
                start: pos,
                target,
                button,
                mods,
                deselected,
            },
            vec![],
        )
    }

    fn ctrl_click(
        &mut self,
        b: BlockId,
        model: &Model,
        sys: &System,
        path: &SystemPath,
    ) -> Vec<CanvasAction> {
        let prev: Vec<BlockId> = self.selection.blocks.iter().copied().collect();
        self.selection.select_only_block(b);
        if prev.len() != 1 || prev[0] == b {
            return vec![];
        }
        let a = prev[0];
        let Some(src) = first_unused_output(sys, a) else {
            return vec![CanvasAction::Status(format!(
                "Block '{}' has no output port",
                block_name(sys, a)
            ))];
        };
        let Some(dst) = first_free_input(sys, b) else {
            return vec![CanvasAction::Status(format!(
                "No free input port on '{}'",
                block_name(sys, b)
            ))];
        };
        vec![CanvasAction::Execute(Command::AddLine {
            path: path.clone(),
            line: new_line(model, src, dst),
        })]
    }

    fn finish_line(
        &mut self,
        from: LineStart,
        pos: ScreenPos,
        model: &Model,
        sys: &System,
        path: &SystemPath,
    ) -> Vec<CanvasAction> {
        let hit = hit_test(sys, &self.view, pos);
        let connected = || vec![CanvasAction::Status("Input port already connected".into())];
        let target_input = |hit: &HitTarget| -> Result<Option<PortRef>, Vec<CanvasAction>> {
            match hit {
                HitTarget::InputPort(p) => {
                    if input_connected(sys, *p) {
                        Err(connected())
                    } else {
                        Ok(Some(*p))
                    }
                }
                HitTarget::Block(b) => match first_free_input(sys, *b) {
                    Some(p) => Ok(Some(p)),
                    None => Err(vec![CanvasAction::Status(format!(
                        "No free input port on '{}'",
                        block_name(sys, *b)
                    ))]),
                },
                _ => Ok(None),
            }
        };
        match from {
            LineStart::Output(src) => match target_input(&hit) {
                Ok(Some(dst)) => vec![CanvasAction::Execute(Command::AddLine {
                    path: path.clone(),
                    line: new_line(model, src, dst),
                })],
                Ok(None) => vec![],
                Err(a) => a,
            },
            LineStart::Input(dst) => match hit {
                HitTarget::OutputPort(src) => vec![CanvasAction::Execute(Command::AddLine {
                    path: path.clone(),
                    line: new_line(model, src, dst),
                })],
                HitTarget::Block(b) => match sys.block(b).and_then(|x| port_counts(x).ok()) {
                    Some(c) if c.outputs > 0 => {
                        vec![CanvasAction::Execute(Command::AddLine {
                            path: path.clone(),
                            line: new_line(model, PortRef { block: b, port: 1 }, dst),
                        })]
                    }
                    _ => vec![CanvasAction::Status(format!(
                        "Block '{}' has no output port",
                        block_name(sys, b)
                    ))],
                },
                _ => vec![],
            },
            LineStart::Branch { line, branch, tap } => {
                let Some(l) = sys.line(line) else {
                    return vec![];
                };
                match target_input(&hit) {
                    Ok(Some(dst)) => match add_branch(l, &branch, tap, dst) {
                        Some(nl) => vec![CanvasAction::Execute(Command::ReplaceLine {
                            path: path.clone(),
                            line: nl,
                        })],
                        None => vec![],
                    },
                    Ok(None) => vec![],
                    Err(a) => a,
                }
            }
        }
    }

    fn on_key(
        &mut self,
        key: CanvasKey,
        mods: Modifiers,
        model: &Model,
        sys: &System,
        path: &SystemPath,
    ) -> (Interaction, Vec<CanvasAction>) {
        use CanvasKey as K;
        let sel_blocks: Vec<BlockId> = self.selection.blocks.iter().copied().collect();
        let mv = |dx: f64, dy: f64| {
            if sel_blocks.is_empty() {
                vec![]
            } else {
                vec![CanvasAction::Execute(Command::MoveBlocks {
                    path: path.clone(),
                    blocks: sel_blocks.clone(),
                    dx,
                    dy,
                })]
            }
        };
        let a = match (key, mods.ctrl, mods.shift, mods.alt) {
            (K::Delete | K::Backspace, false, _, _) => {
                if self.selection.is_empty() {
                    vec![]
                } else {
                    let c = Command::DeleteItems {
                        path: path.clone(),
                        blocks: sel_blocks.clone(),
                        lines: self.selection.lines.iter().copied().collect(),
                    };
                    self.selection = Selection::default();
                    vec![CanvasAction::Execute(c)]
                }
            }
            (K::Z, true, false, _) => vec![CanvasAction::Undo],
            (K::Y, true, _, _) | (K::Z, true, true, _) => vec![CanvasAction::Redo],
            (K::A, true, _, true) => {
                self.selection = Selection::default();
                vec![]
            }
            (K::A, true, _, false) => {
                self.selection = Selection {
                    blocks: sys.blocks.iter().map(|b| b.sid).collect(),
                    lines: sys.lines.iter().map(|l| l.id).collect(),
                };
                vec![]
            }
            (K::C, true, _, _) => {
                if sel_blocks.is_empty() {
                    vec![]
                } else {
                    vec![CanvasAction::Copy]
                }
            }
            (K::X, true, _, _) => {
                if sel_blocks.is_empty() {
                    vec![]
                } else {
                    vec![CanvasAction::Cut]
                }
            }
            (K::V, true, _, _) => {
                let p = self.last_pointer.unwrap_or(self.center());
                vec![CanvasAction::Paste {
                    at: self.view.to_model(p),
                }]
            }
            (K::G, true, _, _) => {
                if sel_blocks.is_empty() {
                    vec![]
                } else {
                    vec![CanvasAction::CreateSubsystem]
                }
            }
            (K::R, true, shift, _) | (K::I, true, shift, _) => {
                let mut cmds = vec![];
                for sid in &sel_blocks {
                    if let Some(b) = sys.block(*sid) {
                        let o = match (key, shift) {
                            (K::R, false) => b.orientation.rotated_cw(),
                            (K::R, true) => b.orientation.rotated_ccw(),
                            _ => b.orientation.flipped(),
                        };
                        cmds.push(Command::SetOrientation {
                            path: path.clone(),
                            block: *sid,
                            orientation: o,
                        });
                    }
                }
                if cmds.is_empty() {
                    vec![]
                } else {
                    vec![CanvasAction::Execute(Command::Batch {
                        label: if key == K::R {
                            "Rotate".into()
                        } else {
                            "Flip".into()
                        },
                        commands: cmds,
                    })]
                }
            }
            (K::ArrowLeft, false, false, _) => mv(-1.0, 0.0),
            (K::ArrowRight, false, false, _) => mv(1.0, 0.0),
            (K::ArrowUp, false, false, _) => mv(0.0, -1.0),
            (K::ArrowDown, false, false, _) => mv(0.0, 1.0),
            (K::Plus, true, _, _) => {
                self.view.zoom_about(self.center(), KEY_ZOOM_STEP);
                vec![]
            }
            (K::Minus, true, _, _) => {
                self.view.zoom_about(self.center(), 1.0 / KEY_ZOOM_STEP);
                vec![]
            }
            (K::Zero, true, _, _) => {
                let z = self.view.zoom;
                self.view.zoom_about(self.center(), 1.0 / z);
                vec![]
            }
            (K::Space, false, _, _) => {
                return (
                    Interaction::SpaceHeld {
                        used_for_pan: false,
                    },
                    vec![],
                )
            }
            (K::Enter, false, _, _) => {
                if sel_blocks.len() == 1
                    && sys
                        .block(sel_blocks[0])
                        .map(|b| b.system.is_some())
                        .unwrap_or(false)
                {
                    vec![CanvasAction::OpenSubsystem(sel_blocks[0])]
                } else {
                    vec![]
                }
            }
            (K::Escape, false, _, _) => {
                if path.is_root() {
                    vec![]
                } else {
                    vec![CanvasAction::GoToParent]
                }
            }
            _ => vec![],
        };
        let _ = model;
        (Interaction::Idle, a)
    }
}

/// Start point of a line being drawn.
fn line_start_point(sys: &System, from: &LineStart) -> Option<Point> {
    match from {
        LineStart::Output(p) => block_ports(sys.block(p.block)?)
            .outputs
            .get(p.port as usize - 1)
            .copied(),
        LineStart::Input(p) => block_ports(sys.block(p.block)?)
            .inputs
            .get(p.port as usize - 1)
            .copied(),
        LineStart::Branch { line, branch, tap } => {
            let lp = line_path(sys, sys.line(*line)?)?;
            Some(tap_position(&node_at(&lp.root, branch)?.points, *tap))
        }
    }
}
