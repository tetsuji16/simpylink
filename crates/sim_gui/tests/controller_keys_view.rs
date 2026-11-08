//! Controller keyboard / zoom / pan / fit tests (Issue #48 §H/§J).
use sim_core::edit::Command;
use sim_core::geom::Point;
use sim_core::ids::{BlockId, LineId};
use sim_core::json::load_from_path;
use sim_core::model::*;
use sim_core::path::SystemPath;
use sim_gui::canvas::controller::*;
use sim_gui::canvas::view::{Modifiers, ScreenPos, ScreenVec, ViewTransform};

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

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        shift: false,
        alt: false,
    }
}

fn ctrl_shift() -> Modifiers {
    Modifiers {
        ctrl: true,
        shift: true,
        alt: false,
    }
}

fn ctrl_alt() -> Modifiers {
    Modifiers {
        ctrl: true,
        shift: false,
        alt: true,
    }
}

fn shift() -> Modifiers {
    Modifiers {
        ctrl: false,
        shift: true,
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

fn down_middle(x: f32, y: f32) -> CanvasEvent {
    CanvasEvent::PointerDown {
        pos: sp(x, y),
        button: PointerButton::Middle,
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

fn up_middle(x: f32, y: f32) -> CanvasEvent {
    CanvasEvent::PointerUp {
        pos: sp(x, y),
        button: PointerButton::Middle,
        mods: NONE,
    }
}

fn key(k: CanvasKey, mods: Modifiers) -> CanvasEvent {
    CanvasEvent::Key {
        key: k,
        pressed: true,
        mods,
    }
}

fn key_up(k: CanvasKey) -> CanvasEvent {
    CanvasEvent::Key {
        key: k,
        pressed: false,
        mods: NONE,
    }
}

fn scroll(pos: (f32, f32), delta: (f32, f32), mods: Modifiers) -> CanvasEvent {
    CanvasEvent::Scroll {
        pos: sp(pos.0, pos.1),
        delta: ScreenVec::new(delta.0, delta.1),
        mods,
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

fn new_controller() -> CanvasController {
    let mut c = CanvasController::new();
    c.viewport = ScreenVec::new(800.0, 600.0);
    c
}

// ---------------- §H ----------------

#[test]
fn h01_delete_selected_block() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    click(&mut c, &model, 145.0, 65.0);
    assert_eq!(
        feed(&mut c, &model, vec![key(CanvasKey::Delete, NONE)]),
        vec![CanvasAction::Execute(Command::DeleteItems {
            path: SystemPath::root(),
            blocks: vec![BlockId(2)],
            lines: vec![],
        })]
    );
    assert!(c.selection.is_empty());
}

#[test]
fn h02_backspace_deletes_selected_line() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    click(&mut c, &model, 105.0, 65.0);
    assert_eq!(
        feed(&mut c, &model, vec![key(CanvasKey::Backspace, NONE)]),
        vec![CanvasAction::Execute(Command::DeleteItems {
            path: SystemPath::root(),
            blocks: vec![],
            lines: vec![LineId(1)],
        })]
    );
}

#[test]
fn h03_delete_with_empty_selection_does_nothing() {
    let model = fx("step_gain_scope");
    assert!(feed(
        &mut new_controller(),
        &model,
        vec![key(CanvasKey::Delete, NONE)]
    )
    .is_empty());
}

#[test]
fn h04_ctrl_z_undoes() {
    let model = fx("step_gain_scope");
    assert_eq!(
        feed(
            &mut new_controller(),
            &model,
            vec![key(CanvasKey::Z, ctrl())]
        ),
        vec![CanvasAction::Undo]
    );
}

#[test]
fn h05_ctrl_y_and_ctrl_shift_z_redo() {
    let model = fx("step_gain_scope");
    assert_eq!(
        feed(
            &mut new_controller(),
            &model,
            vec![key(CanvasKey::Y, ctrl())]
        ),
        vec![CanvasAction::Redo]
    );
    assert_eq!(
        feed(
            &mut new_controller(),
            &model,
            vec![key(CanvasKey::Z, ctrl_shift())]
        ),
        vec![CanvasAction::Redo]
    );
}

#[test]
fn h06_ctrl_a_selects_all() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    assert!(feed(&mut c, &model, vec![key(CanvasKey::A, ctrl())]).is_empty());
    assert_eq!(sel(&c), vec![1, 2, 3]);
    assert_eq!(line_sel(&c), vec![1, 2]);
}

#[test]
fn h07_ctrl_alt_a_clears_selection() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    feed(&mut c, &model, vec![key(CanvasKey::A, ctrl())]);
    assert!(feed(&mut c, &model, vec![key(CanvasKey::A, ctrl_alt())]).is_empty());
    assert!(c.selection.is_empty());
}

#[test]
fn h08_arrow_right_moves_block() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    click(&mut c, &model, 145.0, 65.0);
    assert_eq!(
        feed(&mut c, &model, vec![key(CanvasKey::ArrowRight, NONE)]),
        mvb(&[2], 1.0, 0.0)
    );
}

#[test]
fn h09_arrow_up_moves_block() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    click(&mut c, &model, 145.0, 65.0);
    assert_eq!(
        feed(&mut c, &model, vec![key(CanvasKey::ArrowUp, NONE)]),
        mvb(&[2], 0.0, -1.0)
    );
}

#[test]
fn h10_ctrl_v_pastes_at_last_pointer() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![mv(100.0, 100.0), key(CanvasKey::V, ctrl())]
        ),
        vec![CanvasAction::Paste {
            at: pt(100.0, 100.0)
        }]
    );
}

#[test]
fn h11_wheel_zooms_about_pointer() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    assert!(feed(
        &mut c,
        &model,
        vec![scroll((100.0, 100.0), (0.0, 100.0), NONE)]
    )
    .is_empty());
    assert!((c.view.zoom - 1.0015f32.powf(100.0)).abs() < 1e-4);
    assert!(c.view.to_model(sp(100.0, 100.0)).distance(pt(100.0, 100.0)) < 1e-3);
}

#[test]
fn h12_ctrl_wheel_pans_vertically() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    assert!(feed(
        &mut c,
        &model,
        vec![scroll((1.0, 1.0), (0.0, 50.0), ctrl())]
    )
    .is_empty());
    assert_eq!(
        (c.view.offset, c.view.zoom),
        (ScreenVec::new(0.0, 50.0), 1.0)
    );
}

#[test]
fn h13_shift_wheel_pans_horizontally() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    assert!(feed(
        &mut c,
        &model,
        vec![scroll((1.0, 1.0), (0.0, 50.0), shift())]
    )
    .is_empty());
    assert_eq!(c.view.offset, ScreenVec::new(50.0, 0.0));
}

#[test]
fn h14_middle_drag_pans() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    assert!(feed(
        &mut c,
        &model,
        vec![
            down_middle(100.0, 100.0),
            mv(130.0, 90.0),
            up_middle(130.0, 90.0),
        ]
    )
    .is_empty());
    assert_eq!(c.view.offset, ScreenVec::new(30.0, -10.0));
}

#[test]
fn h15_space_release_fits_all() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![key(CanvasKey::Space, NONE), key_up(CanvasKey::Space)]
        ),
        vec![CanvasAction::FitToView]
    );
    assert_eq!(
        (c.view.zoom, c.view.offset),
        (4.0, ScreenVec::new(-180.0, 40.0))
    );
}

#[test]
fn h16_space_pan_does_not_fit() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    assert!(feed(
        &mut c,
        &model,
        vec![
            key(CanvasKey::Space, NONE),
            down(100.0, 100.0),
            mv(150.0, 100.0),
            up(150.0, 100.0),
            key_up(CanvasKey::Space),
        ]
    )
    .is_empty());
    assert_eq!(
        (c.view.offset, c.view.zoom),
        (ScreenVec::new(50.0, 0.0), 1.0)
    );
}

#[test]
fn h17_ctrl_plus_zooms_about_center() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    assert!(feed(&mut c, &model, vec![key(CanvasKey::Plus, ctrl())]).is_empty());
    assert_eq!(c.view.zoom, 1.25);
    assert!(c.view.to_model(sp(400.0, 300.0)).distance(pt(400.0, 300.0)) < 1e-3);
}

#[test]
fn h18_ctrl_zero_resets_zoom() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    feed(&mut c, &model, vec![key(CanvasKey::Plus, ctrl())]);
    assert!(feed(&mut c, &model, vec![key(CanvasKey::Zero, ctrl())]).is_empty());
    assert!((c.view.zoom - 1.0).abs() < 1e-6);
}

#[test]
fn h19_delete_during_drag_is_ignored() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    assert!(feed(
        &mut c,
        &model,
        vec![
            down(145.0, 65.0),
            mv(165.0, 75.0),
            key(CanvasKey::Delete, NONE)
        ]
    )
    .is_empty());
    assert!(!c.is_idle());
}

// ---------------- §J ----------------

#[test]
fn j01_space_fit_on_empty_system_resets_view() {
    let mut model = fx("step_gain_scope");
    model.root.blocks.clear();
    model.root.lines.clear();
    let mut c = new_controller();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![key(CanvasKey::Space, NONE), key_up(CanvasKey::Space)]
        ),
        vec![CanvasAction::FitToView]
    );
    assert_eq!(c.view, ViewTransform::identity());
}

#[test]
fn j02_delete_key_release_is_ignored() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    click(&mut c, &model, 145.0, 65.0);
    assert!(feed(&mut c, &model, vec![key_up(CanvasKey::Delete)]).is_empty());
    assert_eq!(sel(&c), vec![2]);
}

#[test]
fn j03_zero_wheel_delta_changes_nothing() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    assert!(feed(
        &mut c,
        &model,
        vec![scroll((100.0, 100.0), (0.0, 0.0), NONE)]
    )
    .is_empty());
    assert_eq!(c.view, ViewTransform::identity());
}

#[test]
fn j04_ctrl_plus_at_max_zoom_does_not_exceed_limit() {
    let model = fx("step_gain_scope");
    let mut c = new_controller();
    c.view.zoom = ViewTransform::MAX_ZOOM;
    assert!(feed(&mut c, &model, vec![key(CanvasKey::Plus, ctrl())]).is_empty());
    assert_eq!(c.view.zoom, ViewTransform::MAX_ZOOM);
}
