//! Endpoint checks shared by `AddLine`, `ReplaceLine` and `RestoreItems` (#197).
use sim_core::edit::{apply, Command, EditError, History};
use sim_core::ids::{BlockId, LineId};
use sim_core::json::load_from_path;
use sim_core::model::{Line, Model, PortRef};
use sim_core::path::SystemPath;
use std::path::PathBuf;

/// Step(1) -> Gain(2) -> Scope(3); lines 1 and 2.
fn fixture() -> Model {
    let p = PathBuf::from(format!(
        "{}/../../tests/fixtures/models/step_gain_scope.json",
        env!("CARGO_MANIFEST_DIR")
    ));
    load_from_path(&p).expect("fixture loads")
}

fn root() -> SystemPath {
    SystemPath::root()
}

fn pr(block: u32, port: u32) -> PortRef {
    PortRef {
        block: BlockId(block),
        port,
    }
}

fn line_of(m: &Model, id: u32) -> Line {
    m.root
        .line(LineId(id))
        .unwrap_or_else(|| panic!("line {id} exists"))
        .clone()
}

/// Applies `cmd`, expects an error and expects the model to be untouched.
fn expect_rejected(m: &mut Model, cmd: Command) -> EditError {
    let before = m.clone();
    let err = apply(m, cmd).expect_err("command must be rejected");
    assert_eq!(*m, before, "a rejected command must not change the model");
    err
}

#[test]
fn replace_line_rejects_a_missing_destination_block() {
    let mut m = fixture();
    let mut line = line_of(&m, 2);
    line.dst = Some(pr(99, 1));
    let err = expect_rejected(&mut m, Command::ReplaceLine { path: root(), line });
    assert_eq!(err, EditError::BlockNotFound(BlockId(99)));
}

#[test]
fn replace_line_rejects_a_missing_source_block() {
    let mut m = fixture();
    let mut line = line_of(&m, 2);
    line.src = pr(98, 1);
    let err = expect_rejected(&mut m, Command::ReplaceLine { path: root(), line });
    assert_eq!(err, EditError::BlockNotFound(BlockId(98)));
}

#[test]
fn replace_line_rejects_an_out_of_range_port() {
    let mut m = fixture();
    // Scope has a single input, so input 2 does not exist.
    let mut line = line_of(&m, 2);
    line.dst = Some(pr(3, 2));
    let err = expect_rejected(&mut m, Command::ReplaceLine { path: root(), line });
    assert_eq!(err, EditError::PortNotFound(pr(3, 2)));

    // Gain has a single output, so output 2 does not exist.
    let mut line = line_of(&m, 2);
    line.src = pr(2, 2);
    let err = expect_rejected(&mut m, Command::ReplaceLine { path: root(), line });
    assert_eq!(err, EditError::PortNotFound(pr(2, 2)));
}

#[test]
fn replace_line_rejects_an_input_driven_by_another_line() {
    let mut m = fixture();
    // Line 2 already drives Scope.1; line 1 must not take it over.
    let mut line = line_of(&m, 1);
    line.dst = Some(pr(3, 1));
    let err = expect_rejected(&mut m, Command::ReplaceLine { path: root(), line });
    assert_eq!(err, EditError::InputAlreadyConnected(pr(3, 1)));
}

#[test]
fn replace_line_keeps_its_own_inputs_connected() {
    let mut m = fixture();
    // Replacing line 2 with the same destination must be accepted: its own input is
    // ignored by the check.
    let mut line = line_of(&m, 2);
    line.points = vec![sim_core::geom::Point::new(170.0, 65.0)];
    let before = m.clone();
    let mut h = History::new(10);
    h.execute(
        &mut m,
        Command::ReplaceLine {
            path: root(),
            line: line.clone(),
        },
    )
    .expect("moving the breakpoints is allowed");
    assert_eq!(m.root.line(LineId(2)), Some(&line));
    assert!(h.undo(&mut m).expect("undo"));
    assert_eq!(m, before);
}

#[test]
fn add_line_preserves_existing_out_of_range_port_behavior() {
    let mut m = fixture();
    let line = Line {
        id: LineId(3),
        src: pr(2, 99),
        points: vec![],
        dst: None,
        branches: vec![],
        name: String::new(),
    };
    assert!(apply(&mut m, Command::AddLine { path: root(), line }).is_ok());
}

#[test]
fn restore_items_rejects_a_line_to_a_missing_block() {
    let mut m = fixture();
    let ghost = Line {
        id: LineId(7),
        src: pr(1, 1),
        points: vec![],
        dst: Some(pr(99, 1)),
        branches: vec![],
        name: String::new(),
    };
    let err = expect_rejected(
        &mut m,
        Command::RestoreItems {
            path: root(),
            blocks: vec![],
            lines: vec![ghost],
            modified_lines: vec![],
        },
    );
    assert_eq!(err, EditError::BlockNotFound(BlockId(99)));
}

#[test]
fn restore_items_rejects_a_line_that_steals_a_connected_input() {
    let mut m = fixture();
    let stolen = Line {
        id: LineId(7),
        src: pr(1, 1),
        points: vec![],
        dst: Some(pr(2, 1)),
        branches: vec![],
        name: String::new(),
    };
    let err = expect_rejected(
        &mut m,
        Command::RestoreItems {
            path: root(),
            blocks: vec![],
            lines: vec![stolen],
            modified_lines: vec![],
        },
    );
    assert_eq!(err, EditError::InputAlreadyConnected(pr(2, 1)));
}

#[test]
fn restore_items_rejects_a_modified_line_to_a_missing_block_atomically() {
    let mut m = fixture();
    let mut invalid = line_of(&m, 1);
    invalid.dst = Some(pr(99, 1));
    let err = expect_rejected(
        &mut m,
        Command::RestoreItems {
            path: root(),
            blocks: vec![],
            lines: vec![],
            modified_lines: vec![invalid],
        },
    );
    assert_eq!(err, EditError::BlockNotFound(BlockId(99)));
}

#[test]
fn restore_items_rejects_a_modified_line_that_steals_an_input_atomically() {
    let mut m = fixture();
    let mut stolen = line_of(&m, 1);
    stolen.dst = Some(pr(3, 1));
    let err = expect_rejected(
        &mut m,
        Command::RestoreItems {
            path: root(),
            blocks: vec![],
            lines: vec![],
            modified_lines: vec![stolen],
        },
    );
    assert_eq!(err, EditError::InputAlreadyConnected(pr(3, 1)));
}

#[test]
fn undo_delete_restores_a_preexisting_out_of_range_line() {
    let mut m = fixture();
    // AddLine historically permits invalid port numbers; deleting a destination
    // prunes the line and Undo must restore the original line verbatim.
    m.root.lines[1].src = pr(2, 99);
    let before = m.clone();
    let mut history = History::new(10);
    history
        .execute(
            &mut m,
            Command::DeleteItems {
                path: root(),
                blocks: vec![BlockId(3)],
                lines: vec![],
            },
        )
        .expect("delete destination block");
    assert!(history.undo(&mut m).expect("undo deletion"));
    assert_eq!(m, before);
}

#[test]
fn delete_and_undo_still_restores_lines() {
    let mut m = fixture();
    let before = m.clone();
    let mut h = History::new(10);
    h.execute(
        &mut m,
        Command::DeleteItems {
            path: root(),
            blocks: vec![BlockId(2)],
            lines: vec![],
        },
    )
    .expect("delete Gain");
    assert!(m.root.block(BlockId(2)).is_none());
    assert!(h.undo(&mut m).expect("undo"));
    assert_eq!(m, before);
}
