//! AppState behaviour tests (M1-09, issue #50).

use sim_core::edit::{Command, History};
use sim_core::geom::{Point, Rect};
use sim_core::ids::{BlockId, LineId};
use sim_core::model::{Line, Model, PortRef};
use sim_core::path::SystemPath;
use sim_gui::canvas::controller::{CanvasAction, CanvasController, CanvasEvent};
use sim_gui::canvas::view::{ScreenPos, ViewTransform};
use sim_gui::state::AppState;

fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("models")
}

fn state_with_fixture() -> AppState {
    let mut st = AppState::new();
    let path = fixtures_dir().join("step_gain_scope.json");
    st.doc.model = sim_core::json::load_from_path(&path).expect("fixture loads");
    st.doc.history = History::new(200);
    st.doc.path = SystemPath::root();
    st
}

fn gain_left(st: &AppState) -> f64 {
    st.doc
        .model
        .root
        .block(BlockId(2))
        .expect("gain")
        .position
        .left
}

#[test]
fn move_block_marks_dirty_and_updates_title() {
    let mut st = state_with_fixture();
    st.handle_action(CanvasAction::Execute(Command::MoveBlocks {
        path: SystemPath::root(),
        blocks: vec![BlockId(2)],
        dx: 10.0,
        dy: 0.0,
    }));
    assert_eq!(gain_left(&st), 140.0);
    assert!(st.doc.history.is_dirty());
    assert_eq!(st.window_title(), "*step_gain_scope - Simpylink");
}

#[test]
fn undo_restores_position_and_clears_dirty() {
    let mut st = state_with_fixture();
    st.handle_action(CanvasAction::Execute(Command::MoveBlocks {
        path: SystemPath::root(),
        blocks: vec![BlockId(2)],
        dx: 10.0,
        dy: 0.0,
    }));
    st.handle_action(CanvasAction::Undo);
    assert_eq!(gain_left(&st), 130.0);
    assert_eq!(st.window_title(), "step_gain_scope - Simpylink");
}

#[test]
fn undo_on_empty_history_reports_status() {
    let mut st = state_with_fixture();
    st.handle_action(CanvasAction::Undo);
    assert_eq!(st.status, "Nothing to undo");
}

#[test]
fn multiply_driven_input_reports_edit_failure() {
    let mut st = state_with_fixture();
    // Gain input port 1 is already driven by the Step block.
    let line = Line {
        id: LineId(99),
        src: PortRef {
            block: BlockId(3),
            port: 1,
        },
        points: vec![],
        dst: Some(PortRef {
            block: BlockId(2),
            port: 1,
        }),
        branches: vec![],
        name: String::new(),
    };
    let ok = st.execute(Command::AddLine {
        path: SystemPath::root(),
        line,
    });
    assert!(!ok);
    assert!(
        st.status.starts_with("Edit failed: input port"),
        "status was {:?}",
        st.status
    );
}

#[test]
fn delete_removes_stale_selection() {
    let mut st = state_with_fixture();
    st.canvas.selection.blocks.insert(BlockId(2));
    st.handle_action(CanvasAction::Execute(Command::DeleteItems {
        path: SystemPath::root(),
        blocks: vec![BlockId(2)],
        lines: vec![],
    }));
    assert!(st.canvas.selection.blocks.is_empty());
}

#[test]
fn add_block_at_centers_and_names() {
    let mut st = state_with_fixture();
    let sid = st.add_block_at("Gain", Point::new(300.0, 200.0));
    assert_eq!(sid, Some(BlockId(4)));
    let b = st.doc.model.root.block(BlockId(4)).expect("new block");
    assert_eq!(b.position, Rect::new(285.0, 185.0, 315.0, 215.0));
    assert_eq!(b.name, "Gain1");
}

#[test]
fn add_unknown_block_reports_status() {
    let mut st = state_with_fixture();
    assert_eq!(st.add_block_at("Nope", Point::new(0.0, 0.0)), None);
    assert_eq!(st.status, "unknown block type 'Nope'");
}

#[test]
fn drop_block_places_centered_at_pointer() {
    let model = Model::new("empty");
    let mut c = CanvasController::default();
    let actions = c.handle(
        CanvasEvent::DropBlock {
            pos: ScreenPos::new(100.0, 100.0),
            block_type: "Constant".into(),
        },
        &model,
        &SystemPath::root(),
    );
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        CanvasAction::Execute(Command::AddBlock { path, block }) => {
            assert_eq!(*path, SystemPath::root());
            assert_eq!(block.sid, BlockId(1));
            assert_eq!(block.name, "Constant");
            assert_eq!(block.position, Rect::new(85.0, 85.0, 115.0, 115.0));
        }
        other => panic!("unexpected action: {other:?}"),
    }
    assert_eq!(
        c.selection.blocks.iter().copied().collect::<Vec<_>>(),
        vec![BlockId(1)]
    );
}

#[test]
fn drop_block_respects_zoom() {
    let model = Model::new("empty");
    let mut c = CanvasController::default();
    c.view = ViewTransform {
        offset: Default::default(),
        zoom: 2.0,
    };
    let actions = c.handle(
        CanvasEvent::DropBlock {
            pos: ScreenPos::new(200.0, 200.0),
            block_type: "Constant".into(),
        },
        &model,
        &SystemPath::root(),
    );
    match &actions[0] {
        CanvasAction::Execute(Command::AddBlock { block, .. }) => {
            assert_eq!(block.position, Rect::new(85.0, 85.0, 115.0, 115.0));
        }
        other => panic!("unexpected action: {other:?}"),
    }
}
