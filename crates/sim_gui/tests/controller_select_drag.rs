//! Controller selection / rubber band / block drag tests (Issue #46 §H/§J).
use sim_core::edit::Command;
use sim_core::geom::{Point, Rect};
use sim_core::ids::BlockId;
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

fn shift() -> Modifiers {
    Modifiers {
        ctrl: false,
        shift: true,
        alt: false,
    }
}

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

fn down_m(x: f32, y: f32, mods: Modifiers) -> CanvasEvent {
    CanvasEvent::PointerDown {
        pos: sp(x, y),
        button: PointerButton::Primary,
        mods,
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

fn mv_m(x: f32, y: f32, mods: Modifiers) -> CanvasEvent {
    CanvasEvent::PointerMove {
        pos: sp(x, y),
        mods,
    }
}

fn up(x: f32, y: f32) -> CanvasEvent {
    CanvasEvent::PointerUp {
        pos: sp(x, y),
        button: PointerButton::Primary,
        mods: NONE,
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

fn mvb(blocks: &[u32], dx: f64, dy: f64) -> Vec<CanvasAction> {
    vec![CanvasAction::Execute(Command::MoveBlocks {
        path: SystemPath::root(),
        blocks: blocks.iter().map(|b| BlockId(*b)).collect(),
        dx,
        dy,
    })]
}

fn sel(c: &CanvasController) -> Vec<u32> {
    c.selection.blocks.iter().map(|b| b.0).collect()
}

fn line_sel(c: &CanvasController) -> Vec<u32> {
    c.selection.lines.iter().map(|l| l.0).collect()
}

fn click(c: &mut CanvasController, model: &Model, x: f32, y: f32) {
    feed(c, model, vec![down(x, y), up(x, y)]);
}

// ---------------- §H ----------------

#[test]
fn h01_click_selects_block() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert!(feed(&mut c, &model, vec![down(145.0, 65.0), up(145.0, 65.0)]).is_empty());
    assert_eq!(sel(&c), vec![2]);
}

#[test]
fn h02_click_empty_clears_selection() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    click(&mut c, &model, 145.0, 65.0);
    assert!(feed(&mut c, &model, vec![down(300.0, 300.0), up(300.0, 300.0)]).is_empty());
    assert!(c.selection.is_empty());
}

#[test]
fn h03_shift_click_adds_to_selection() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    click(&mut c, &model, 145.0, 65.0);
    assert!(feed(
        &mut c,
        &model,
        vec![down_m(65.0, 65.0, shift()), up(65.0, 65.0)]
    )
    .is_empty());
    assert_eq!(sel(&c), vec![1, 2]);
}

#[test]
fn h04_shift_click_removes_from_selection() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    click(&mut c, &model, 145.0, 65.0);
    feed(
        &mut c,
        &model,
        vec![down_m(65.0, 65.0, shift()), up(65.0, 65.0)],
    );
    assert!(feed(
        &mut c,
        &model,
        vec![down_m(65.0, 65.0, shift()), up(65.0, 65.0)]
    )
    .is_empty());
    assert_eq!(sel(&c), vec![2]);
}

#[test]
fn h05_drag_below_threshold_has_no_preview() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert!(feed(&mut c, &model, vec![down(145.0, 65.0), mv(147.0, 65.0)]).is_empty());
    assert_eq!(c.preview().drag, None);
}

#[test]
fn h06_drag_preview_uses_model_delta() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    feed(&mut c, &model, vec![down(145.0, 65.0), mv(165.0, 75.0)]);
    assert_eq!(c.preview().drag, Some((vec![BlockId(2)], 20.0, 10.0)));
}

#[test]
fn h07_drag_up_moves_blocks() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    feed(&mut c, &model, vec![down(145.0, 65.0), mv(165.0, 75.0)]);
    assert_eq!(
        feed(&mut c, &model, vec![up(165.0, 75.0)]),
        mvb(&[2], 20.0, 10.0)
    );
    assert_eq!(sel(&c), vec![2]);
    assert_eq!(c.preview(), CanvasPreview::default());
}

#[test]
fn h08_drag_at_zoom_two_uses_model_delta() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    c.view.zoom = 2.0;
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![down(290.0, 130.0), mv(330.0, 150.0), up(330.0, 150.0)]
        ),
        mvb(&[2], 20.0, 10.0)
    );
}

#[test]
fn h09_drag_moves_whole_selection() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    click(&mut c, &model, 145.0, 65.0);
    feed(
        &mut c,
        &model,
        vec![down_m(65.0, 65.0, shift()), up(65.0, 65.0)],
    );
    assert_eq!(sel(&c), vec![1, 2]);
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![down(65.0, 65.0), mv(85.0, 65.0), up(85.0, 65.0)]
        ),
        mvb(&[1, 2], 20.0, 0.0)
    );
}

#[test]
fn h10_shift_during_drag_locks_axis() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![
                down(145.0, 65.0),
                mv(150.0, 66.0),
                mv_m(175.0, 70.0, shift()),
                up(175.0, 70.0)
            ]
        ),
        mvb(&[2], 30.0, 0.0)
    );
}

#[test]
fn h11_drag_up_rounds_delta() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![down(145.0, 65.0), mv(165.6, 75.4), up(165.6, 75.4)]
        ),
        mvb(&[2], 21.0, 10.0)
    );
}

#[test]
fn h12_rubber_band_preview() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert!(feed(&mut c, &model, vec![down(70.0, 40.0), mv(135.0, 90.0)]).is_empty());
    assert_eq!(
        c.preview().rubber_band,
        Some(Rect::new(70.0, 40.0, 135.0, 90.0))
    );
}

#[test]
fn h13_rubber_band_up_selects_blocks() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    feed(&mut c, &model, vec![down(70.0, 40.0), mv(135.0, 90.0)]);
    assert!(feed(&mut c, &model, vec![up(135.0, 90.0)]).is_empty());
    assert_eq!(sel(&c), vec![1, 2]);
}

#[test]
fn h14_escape_cancels_drag() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert!(feed(
        &mut c,
        &model,
        vec![down(145.0, 65.0), mv(165.0, 75.0), escape()]
    )
    .is_empty());
    assert!(c.is_idle());
    assert_eq!(c.preview(), CanvasPreview::default());
}

#[test]
fn h15_up_after_escape_does_nothing() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    feed(
        &mut c,
        &model,
        vec![down(145.0, 65.0), mv(165.0, 75.0), escape()],
    );
    assert!(feed(&mut c, &model, vec![up(165.0, 75.0)]).is_empty());
}

#[test]
fn h16_ctrl_drag_duplicates() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![
                down_m(145.0, 65.0, ctrl()),
                mv(165.0, 75.0),
                up(165.0, 75.0)
            ]
        ),
        vec![CanvasAction::DuplicateBlocks {
            blocks: vec![BlockId(2)],
            dx: 20.0,
            dy: 10.0,
        }]
    );
}

#[test]
fn h17_secondary_drag_duplicates() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![
                down_secondary(145.0, 65.0),
                mv(165.0, 75.0),
                up_secondary(165.0, 75.0)
            ]
        ),
        vec![CanvasAction::DuplicateBlocks {
            blocks: vec![BlockId(2)],
            dx: 20.0,
            dy: 10.0,
        }]
    );
}

#[test]
fn h18_double_click_block_opens_dialog() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![CanvasEvent::DoubleClick {
                pos: sp(145.0, 65.0),
                mods: NONE,
            }]
        ),
        vec![CanvasAction::OpenBlockDialog(BlockId(2))]
    );
}

#[test]
fn h19_double_click_empty_opens_quick_insert() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![CanvasEvent::DoubleClick {
                pos: sp(300.0, 300.0),
                mods: NONE,
            }]
        ),
        vec![CanvasAction::QuickInsert {
            at: pt(300.0, 300.0)
        }]
    );
}

#[test]
fn h20_click_line_selects_line_only() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert!(feed(&mut c, &model, vec![down(105.0, 65.0), up(105.0, 65.0)]).is_empty());
    assert_eq!(line_sel(&c), vec![1]);
    assert!(c.selection.blocks.is_empty());
}

#[test]
fn h21_back_and_forth_below_threshold_keeps_selection() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert!(feed(
        &mut c,
        &model,
        vec![
            down(145.0, 65.0),
            mv(146.0, 65.0),
            mv(145.0, 65.0),
            up(145.0, 65.0)
        ]
    )
    .is_empty());
    assert_eq!(sel(&c), vec![2]);
}

// ---------------- §J ----------------

#[test]
fn j01_missing_path_returns_empty_and_stays_idle() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    let missing = SystemPath(vec![BlockId(99)]);
    assert!(c.handle(down(145.0, 65.0), &model, &missing).is_empty());
    assert!(c.is_idle());
}

#[test]
fn j02_up_without_down_does_not_panic() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert!(feed(&mut c, &model, vec![up(10.0, 10.0)]).is_empty());
}

#[test]
fn j03_move_only_updates_last_pointer() {
    let model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    assert!(feed(&mut c, &model, vec![mv(12.0, 34.0)]).is_empty());
    assert_eq!(c.last_pointer, Some(sp(12.0, 34.0)));
}

#[test]
fn j04_retain_existing_drops_deleted_selection() {
    let mut model = fx("step_gain_scope");
    let mut c = CanvasController::new();
    click(&mut c, &model, 145.0, 65.0);
    assert_eq!(sel(&c), vec![2]);
    model.root.blocks.retain(|b| b.sid != BlockId(2));
    c.retain_existing(&model.root);
    assert!(c.selection.is_empty());
}
