//! Controller line drawing / branching / Ctrl+click tests (Issue #47 §H/§J).
use sim_core::edit::Command;
use sim_core::geom::Point;
use sim_core::ids::{BlockId, LineId};
use sim_core::json::load_from_path;
use sim_core::model::*;
use sim_core::path::SystemPath;
use sim_gui::canvas::controller::*;
use sim_gui::canvas::view::{Modifiers, ScreenPos};

fn fx(name: &str) -> Model {
    load_from_path(&std::path::PathBuf::from(format!(
        "{}/../../tests/fixtures/models/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    )))
    .unwrap()
}

/// Fixture `step_gain_scope` without Line2 (Gain→Scope); next_line_id = 3.
fn m1() -> Model {
    let mut m = fx("step_gain_scope");
    m.root.lines.retain(|l| l.id != LineId(2));
    m
}

fn sp(x: f32, y: f32) -> ScreenPos {
    ScreenPos::new(x, y)
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

const NONE: Modifiers = Modifiers {
    ctrl: false,
    shift: false,
    alt: false,
};

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        shift: false,
        alt: false,
    }
}

fn down(x: f32, y: f32) -> CanvasEvent {
    CanvasEvent::PointerDown {
        pos: sp(x, y),
        button: PointerButton::Primary,
        mods: NONE,
    }
}

fn down_ctrl(x: f32, y: f32) -> CanvasEvent {
    CanvasEvent::PointerDown {
        pos: sp(x, y),
        button: PointerButton::Primary,
        mods: ctrl(),
    }
}

fn down_secondary(x: f32, y: f32) -> CanvasEvent {
    CanvasEvent::PointerDown {
        pos: sp(x, y),
        button: PointerButton::Secondary,
        mods: NONE,
    }
}

fn mv(x: f32, y: f32) -> CanvasEvent {
    CanvasEvent::PointerMove {
        pos: sp(x, y),
        mods: NONE,
    }
}

fn up(x: f32, y: f32) -> CanvasEvent {
    CanvasEvent::PointerUp {
        pos: sp(x, y),
        button: PointerButton::Primary,
        mods: NONE,
    }
}

fn up_ctrl(x: f32, y: f32) -> CanvasEvent {
    CanvasEvent::PointerUp {
        pos: sp(x, y),
        button: PointerButton::Primary,
        mods: ctrl(),
    }
}

fn up_secondary(x: f32, y: f32) -> CanvasEvent {
    CanvasEvent::PointerUp {
        pos: sp(x, y),
        button: PointerButton::Secondary,
        mods: NONE,
    }
}

fn escape() -> CanvasEvent {
    CanvasEvent::Key {
        key: CanvasKey::Escape,
        pressed: true,
        mods: NONE,
    }
}

fn feed(c: &mut CanvasController, model: &Model, evs: Vec<CanvasEvent>) -> Vec<CanvasAction> {
    evs.into_iter()
        .flat_map(|e| c.handle(e, model, &SystemPath::root()))
        .collect()
}

fn pr(b: u32, p: u32) -> PortRef {
    PortRef {
        block: BlockId(b),
        port: p,
    }
}

fn line(id: u32, src: PortRef, dst: PortRef) -> Line {
    Line {
        id: LineId(id),
        src,
        points: vec![],
        dst: Some(dst),
        branches: vec![],
        name: String::new(),
    }
}

fn add_line(src: PortRef, dst: PortRef) -> Vec<CanvasAction> {
    vec![CanvasAction::Execute(Command::AddLine {
        path: SystemPath::root(),
        line: line(3, src, dst),
    })]
}

fn sel(c: &CanvasController) -> Vec<u32> {
    c.selection.blocks.iter().map(|b| b.0).collect()
}

// ---------------- §H ----------------

#[test]
fn h01_output_to_input_port_adds_line() {
    let model = m1();
    assert_eq!(
        feed(
            &mut CanvasController::new(),
            &model,
            vec![down(161.0, 65.0), mv(180.0, 65.0), up(210.0, 65.0)]
        ),
        add_line(pr(2, 1), pr(3, 1))
    );
}

#[test]
fn h02_output_to_block_adds_line() {
    let model = m1();
    assert_eq!(
        feed(
            &mut CanvasController::new(),
            &model,
            vec![down(161.0, 65.0), mv(180.0, 65.0), up(225.0, 65.0)]
        ),
        add_line(pr(2, 1), pr(3, 1))
    );
}

#[test]
fn h03_output_to_empty_does_nothing() {
    let model = m1();
    assert!(feed(
        &mut CanvasController::new(),
        &model,
        vec![down(161.0, 65.0), mv(180.0, 65.0), up(300.0, 300.0)]
    )
    .is_empty());
}

#[test]
fn h04_output_to_connected_input_reports_status() {
    let model = m1();
    assert_eq!(
        feed(
            &mut CanvasController::new(),
            &model,
            vec![down(81.0, 65.0), mv(100.0, 70.0), up(129.0, 65.0)]
        ),
        vec![CanvasAction::Status("Input port already connected".into())]
    );
}

#[test]
fn h05_input_to_output_port_adds_line() {
    let model = m1();
    assert_eq!(
        feed(
            &mut CanvasController::new(),
            &model,
            vec![down(209.0, 65.0), mv(190.0, 65.0), up(161.0, 65.0)]
        ),
        add_line(pr(2, 1), pr(3, 1))
    );
}

#[test]
fn h06_ctrl_drag_from_line_adds_branch() {
    let model = m1();
    let mut l1 = model.root.lines[0].clone();
    l1.branches.push(Branch {
        tap: TapPoint { segment: 0, t: 0.5 },
        points: vec![],
        dst: Some(pr(3, 1)),
        branches: vec![],
    });
    let want = vec![CanvasAction::Execute(Command::ReplaceLine {
        path: SystemPath::root(),
        line: l1,
    })];
    assert_eq!(
        feed(
            &mut CanvasController::new(),
            &model,
            vec![down_ctrl(105.0, 65.0), mv(110.0, 90.0), up(225.0, 65.0)]
        ),
        want
    );
}

#[test]
fn h07_secondary_drag_from_line_adds_branch() {
    let model = m1();
    let mut l1 = model.root.lines[0].clone();
    l1.branches.push(Branch {
        tap: TapPoint { segment: 0, t: 0.5 },
        points: vec![],
        dst: Some(pr(3, 1)),
        branches: vec![],
    });
    let want = vec![CanvasAction::Execute(Command::ReplaceLine {
        path: SystemPath::root(),
        line: l1,
    })];
    assert_eq!(
        feed(
            &mut CanvasController::new(),
            &model,
            vec![
                down_secondary(105.0, 65.0),
                mv(110.0, 90.0),
                up_secondary(225.0, 65.0)
            ]
        ),
        want
    );
}

#[test]
fn h08_plain_drag_from_line_only_selects_it() {
    let model = m1();
    assert!(feed(
        &mut CanvasController::new(),
        &model,
        vec![down(105.0, 65.0), mv(110.0, 90.0), up(225.0, 65.0)]
    )
    .is_empty());
}

#[test]
fn h09_ctrl_click_connects_selected_block() {
    let model = m1();
    let mut c = CanvasController::new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![
                down(145.0, 65.0),
                up(145.0, 65.0),
                down_ctrl(225.0, 65.0),
                up_ctrl(225.0, 65.0)
            ]
        ),
        add_line(pr(2, 1), pr(3, 1))
    );
    assert_eq!(sel(&c), vec![3]);
}

#[test]
fn h10_ctrl_click_without_selection_only_selects() {
    let model = m1();
    let mut c = CanvasController::new();
    assert!(feed(
        &mut c,
        &model,
        vec![down_ctrl(225.0, 65.0), up_ctrl(225.0, 65.0)]
    )
    .is_empty());
    assert_eq!(sel(&c), vec![3]);
}

#[test]
fn h11_drawing_line_preview() {
    let model = m1();
    let mut c = CanvasController::new();
    feed(&mut c, &model, vec![down(161.0, 65.0), mv(180.0, 65.0)]);
    assert_eq!(
        c.preview_with(Some(&model.root)).drawing_line,
        Some((pt(160.0, 65.0), pt(180.0, 65.0)))
    );
}

#[test]
fn h12_escape_cancels_line_drawing() {
    let model = m1();
    let mut c = CanvasController::new();
    assert!(feed(
        &mut c,
        &model,
        vec![
            down(161.0, 65.0),
            mv(180.0, 65.0),
            escape(),
            up(210.0, 65.0)
        ]
    )
    .is_empty());
}

#[test]
fn h13_ctrl_click_reports_no_free_input() {
    let model = m1();
    let mut c = CanvasController::new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![
                down(65.0, 65.0),
                up(65.0, 65.0),
                down_ctrl(145.0, 65.0),
                up_ctrl(145.0, 65.0)
            ]
        ),
        vec![CanvasAction::Status("No free input port on 'Gain'".into())]
    );
}

// ---------------- §J ----------------

#[test]
fn j01_ctrl_click_from_block_without_output_reports_status() {
    let model = m1();
    let mut c = CanvasController::new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![
                down(225.0, 65.0),
                up(225.0, 65.0),
                down_ctrl(145.0, 65.0),
                up_ctrl(145.0, 65.0)
            ]
        ),
        vec![CanvasAction::Status(
            "Block 'Scope' has no output port".into()
        )]
    );
}

#[test]
fn j02_missing_branch_path_does_nothing() {
    let model = m1();
    let mut c = CanvasController::new();
    feed(
        &mut c,
        &model,
        vec![down_ctrl(105.0, 65.0), mv(110.0, 90.0)],
    );
    let mut changed = model.clone();
    changed.root.lines.retain(|l| l.id != LineId(1));
    assert!(feed(&mut c, &changed, vec![up(225.0, 65.0)]).is_empty());
}

#[test]
fn j03_self_loop_to_connected_input_reports_status() {
    let model = m1();
    assert_eq!(
        feed(
            &mut CanvasController::new(),
            &model,
            vec![down(161.0, 65.0), mv(150.0, 70.0), up(145.0, 65.0)]
        ),
        vec![CanvasAction::Status("No free input port on 'Gain'".into())]
    );
}
