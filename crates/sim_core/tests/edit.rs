//! Edit commands and Undo/Redo history tests (Issue #42, sections H and J).
//!
//! The expected values are taken verbatim from the issue's section H / J tables.
//! The model identity after an undo is checked with `m.root` and `m.config`,
//! because the ID counters are never decreased (DES-02 section 7).
use sim_core::edit::*;
use sim_core::geom::{Point, Rect};
use sim_core::ids::{BlockId, LineId};
use sim_core::json::from_json_str;
use sim_core::library::new_block;
use sim_core::model::*;
use sim_core::path::SystemPath;
use std::collections::BTreeSet;

fn fixture(name: &str) -> Model {
    let p = format!(
        "{}/../../tests/fixtures/models/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    from_json_str(&std::fs::read_to_string(p).unwrap()).unwrap()
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

fn line(id: u32, src: PortRef, dst: Option<PortRef>, branches: Vec<Branch>) -> Line {
    Line {
        id: LineId(id),
        src,
        points: vec![],
        dst,
        branches,
        name: String::new(),
    }
}

fn branch(dst: Option<PortRef>) -> Branch {
    Branch {
        tap: TapPoint { segment: 0, t: 0.5 },
        points: vec![],
        dst,
        branches: vec![],
    }
}

/// H-1: AddBlock -> undo -> redo.
#[test]
fn h1_add_block_undo_redo() {
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();
    let mut h = History::new(200);

    let b = new_block(&m.root, "Constant", BlockId(4), Point::new(0.0, 120.0)).unwrap();
    h.execute(
        &mut m,
        Command::AddBlock {
            path: root(),
            block: b,
        },
    )
    .unwrap();

    assert_eq!(m.root.blocks.len(), 4);
    assert_eq!(m.root.blocks[3].name, "Constant");
    assert_eq!(m.next_sid, 5);

    assert!(h.undo(&mut m).unwrap());
    assert_eq!(m.root, m0.root);
    assert_eq!(m.config, m0.config);
    assert_eq!(m.next_sid, 5);

    assert!(h.redo(&mut m).unwrap());
    assert_eq!(m.root.blocks.len(), 4);
    assert_eq!(m.root.blocks[3].name, "Constant");
}

/// H-2: DeleteItems(Gain). Line 1 keeps its source and loses its destination;
/// line 2 disappears together with its source.
#[test]
fn h2_delete_items_prunes_lines() {
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();
    let mut h = History::new(200);

    h.execute(
        &mut m,
        Command::DeleteItems {
            path: root(),
            blocks: vec![BlockId(2)],
            lines: vec![],
        },
    )
    .unwrap();

    let names: Vec<&str> = m.root.blocks.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, vec!["Step", "Scope"]);
    assert_eq!(m.root.lines.len(), 1);
    assert_eq!(m.root.lines[0].id, LineId(1));
    assert_eq!(m.root.lines[0].dst, None);
    assert_eq!(m.root.lines[0].src, pr(1, 1));

    assert!(h.undo(&mut m).unwrap());
    assert_eq!(m.root, m0.root);
    assert_eq!(m.config, m0.config);
}

/// H-3: Branch pruning.
#[test]
fn h3_branch_pruning() {
    let l = line(1, pr(1, 1), Some(pr(2, 1)), vec![branch(Some(pr(3, 1)))]);

    // Removing the branch destination drops the branch entirely.
    let p = prune_line(&l, &BTreeSet::from([BlockId(3)])).unwrap();
    assert_eq!(p.dst, Some(pr(2, 1)));
    assert!(p.branches.is_empty());

    // Removing the trunk destination keeps the branch.
    let p = prune_line(&l, &BTreeSet::from([BlockId(2)])).unwrap();
    assert_eq!(p.dst, None);
    assert_eq!(p.branches.len(), 1);
    assert_eq!(p.branches[0].dst, Some(pr(3, 1)));

    // Nothing removed -> no change.
    assert!(prune_line(&l, &BTreeSet::from([BlockId(9)])).is_none());
}

/// H-4: MoveBlocks moves the blocks and the lines that connect only moved blocks.
#[test]
fn h4_move_blocks() {
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();
    m.root.lines[0].points = vec![Point::new(100.0, 65.0)];
    let before = m.root.clone();
    let mut h = History::new(10);

    h.execute(
        &mut m,
        Command::MoveBlocks {
            path: root(),
            blocks: vec![BlockId(1), BlockId(2)],
            dx: 10.0,
            dy: -5.0,
        },
    )
    .unwrap();

    assert_eq!(m.root.blocks[0].position, Rect::new(60.0, 45.0, 90.0, 75.0));
    assert_eq!(m.root.lines[0].points, vec![Point::new(110.0, 60.0)]);
    // Line 2 ends at the (not moved) Scope, so its points stay as they were.
    assert_eq!(m.root.lines[1].points, before.lines[1].points);

    assert!(h.undo(&mut m).unwrap());
    assert_eq!(m.root, before);
}

/// H-5: SetParams sets a value, restores it on undo, and removes added keys.
#[test]
fn h5_set_params() {
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();
    let mut h = History::new(10);

    h.execute(
        &mut m,
        Command::SetParams {
            path: root(),
            block: BlockId(2),
            params: vec![("Gain".into(), Some("5".into()))],
        },
    )
    .unwrap();
    assert_eq!(m.root.blocks[1].params["Gain"], "5");

    assert!(h.undo(&mut m).unwrap());
    assert_eq!(m.root.blocks[1].params["Gain"], "3");

    h.execute(
        &mut m,
        Command::SetParams {
            path: root(),
            block: BlockId(2),
            params: vec![("New".into(), Some("1".into()))],
        },
    )
    .unwrap();
    assert_eq!(m.root.blocks[1].params["New"], "1");

    assert!(h.undo(&mut m).unwrap());
    assert!(!m.root.blocks[1].params.contains_key("New"));
    assert_eq!(m.root, m0.root);
}

/// H-6: AddLine on an already connected input port fails and changes nothing.
#[test]
fn h6_add_line_duplicate_input() {
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();

    let dup = line(9, pr(1, 1), Some(pr(2, 1)), vec![]);
    assert_eq!(
        apply(
            &mut m,
            Command::AddLine {
                path: root(),
                line: dup
            }
        ),
        Err(EditError::InputAlreadyConnected(pr(2, 1)))
    );
    assert_eq!(m.root, m0.root);
}

/// H-7: A failing Batch rolls back the commands that already succeeded.
#[test]
fn h7_batch_rollback() {
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();

    let extra = new_block(&m.root, "Constant", BlockId(10), Point::new(0.0, 0.0)).unwrap();
    let dup = line(9, pr(1, 1), Some(pr(2, 1)), vec![]);
    let r = apply(
        &mut m,
        Command::Batch {
            label: "Paste".into(),
            commands: vec![
                Command::AddBlock {
                    path: root(),
                    block: extra,
                },
                Command::AddLine {
                    path: root(),
                    line: dup,
                },
            ],
        },
    );

    assert_eq!(r, Err(EditError::InputAlreadyConnected(pr(2, 1))));
    assert_eq!(m.root, m0.root);
    assert!(m.root.block(BlockId(10)).is_none());
}

/// H-8: dirty tracking across execute / mark_saved / undo / redo.
#[test]
fn h8_history_dirty() {
    let mut m = fixture("step_gain_scope");
    let mut h = History::new(10);
    assert!(!h.is_dirty());

    h.execute(
        &mut m,
        Command::MoveBlocks {
            path: root(),
            blocks: vec![BlockId(1)],
            dx: 1.0,
            dy: 0.0,
        },
    )
    .unwrap();
    assert!(h.is_dirty());

    h.mark_saved();
    assert!(!h.is_dirty());

    assert!(h.undo(&mut m).unwrap());
    assert!(h.is_dirty());

    assert!(h.redo(&mut m).unwrap());
    assert!(!h.is_dirty());
}

/// H-9: the history keeps at most `limit` undo entries.
#[test]
fn h9_history_limit() {
    let mut m = fixture("step_gain_scope");
    let mut h = History::new(3);

    for _ in 0..4 {
        h.execute(
            &mut m,
            Command::MoveBlocks {
                path: root(),
                blocks: vec![BlockId(1)],
                dx: 1.0,
                dy: 0.0,
            },
        )
        .unwrap();
    }
    for _ in 0..3 {
        assert!(h.undo(&mut m).unwrap());
    }
    assert_eq!(h.undo(&mut m), Ok(false));
    assert!(!h.can_undo());
}

/// H-10: undo labels come from the original command (Batch uses its own label).
#[test]
fn h10_undo_label() {
    let mut m = fixture("step_gain_scope");
    let mut h = History::new(10);

    h.execute(
        &mut m,
        Command::MoveBlocks {
            path: root(),
            blocks: vec![BlockId(1)],
            dx: 1.0,
            dy: 0.0,
        },
    )
    .unwrap();
    assert_eq!(h.undo_label(), Some("Move".into()));

    h.execute(
        &mut m,
        Command::Batch {
            label: "Paste".into(),
            commands: vec![Command::MoveBlocks {
                path: root(),
                blocks: vec![BlockId(2)],
                dx: 1.0,
                dy: 0.0,
            }],
        },
    )
    .unwrap();
    assert_eq!(h.undo_label(), Some("Paste".into()));

    assert!(h.undo(&mut m).unwrap());
    assert_eq!(h.redo_label(), Some("Paste".into()));
    assert_eq!(h.undo_label(), Some("Move".into()));
}

/// E: menu labels of every documented command (Batch uses its own label).
#[test]
fn e_command_labels() {
    let path = || SystemPath::root();
    let b = Block {
        sid: BlockId(1),
        block_type: "Gain".into(),
        name: "Gain".into(),
        position: Rect::new(0.0, 0.0, 30.0, 30.0),
        orientation: Orientation::Right,
        params: Default::default(),
        system: None,
    };
    let l = line(1, pr(1, 1), Some(pr(2, 1)), vec![]);
    let cases = vec![
        (
            Command::AddBlock {
                path: path(),
                block: b,
            },
            "Add Block",
        ),
        (
            Command::DeleteItems {
                path: path(),
                blocks: vec![],
                lines: vec![],
            },
            "Delete",
        ),
        (
            Command::RestoreItems {
                path: path(),
                blocks: vec![],
                lines: vec![],
                modified_lines: vec![],
            },
            "Restore",
        ),
        (
            Command::MoveBlocks {
                path: path(),
                blocks: vec![],
                dx: 0.0,
                dy: 0.0,
            },
            "Move",
        ),
        (
            Command::SetBlockPosition {
                path: path(),
                block: BlockId(1),
                position: Rect::new(0.0, 0.0, 1.0, 1.0),
            },
            "Resize",
        ),
        (
            Command::SetParams {
                path: path(),
                block: BlockId(1),
                params: vec![],
            },
            "Edit Parameters",
        ),
        (
            Command::RenameBlock {
                path: path(),
                block: BlockId(1),
                name: "x".into(),
            },
            "Rename",
        ),
        (
            Command::AddLine {
                path: path(),
                line: l.clone(),
            },
            "Connect",
        ),
        (
            Command::RemoveLine {
                path: path(),
                line: LineId(1),
            },
            "Delete Line",
        ),
        (
            Command::ReplaceLine {
                path: path(),
                line: l,
            },
            "Edit Line",
        ),
        (
            Command::SetConfig {
                config: SimConfig::default(),
            },
            "Edit Configuration",
        ),
        (
            Command::Batch {
                label: "Paste".into(),
                commands: vec![],
            },
            "Paste",
        ),
    ];
    for (cmd, label) in cases {
        assert_eq!(command_label(&cmd), label);
    }
}

/// F.5b: the ID counters are raised by AddLine and never lowered by undo.
#[test]
fn f5b_id_counters() {
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();
    let mut h = History::new(10);
    assert_eq!(m.next_line_id, 3);

    // A dangling line from the Step output (all input ports are taken in the fixture).
    h.execute(
        &mut m,
        Command::AddLine {
            path: root(),
            line: line(3, pr(1, 1), None, vec![]),
        },
    )
    .unwrap();
    assert_eq!(m.root.lines.len(), 3);
    assert_eq!(m.next_line_id, 4);

    assert!(h.undo(&mut m).unwrap());
    assert_eq!(m.root, m0.root);
    assert_eq!(m.next_line_id, 4);
}

/// J-1: deleting a missing block fails and leaves the model unchanged.
#[test]
fn j1_delete_missing_block() {
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();

    assert_eq!(
        apply(
            &mut m,
            Command::DeleteItems {
                path: root(),
                blocks: vec![BlockId(99)],
                lines: vec![]
            }
        ),
        Err(EditError::BlockNotFound(BlockId(99)))
    );
    assert_eq!(m.root, m0.root);
    assert_eq!(m.config, m0.config);
}

/// J-2: removing a missing line fails.
#[test]
fn j2_remove_missing_line() {
    let mut m = fixture("step_gain_scope");
    assert_eq!(
        apply(
            &mut m,
            Command::RemoveLine {
                path: root(),
                line: LineId(99)
            }
        ),
        Err(EditError::LineNotFound(LineId(99)))
    );
}

/// J-3: adding a block with an existing SID fails.
#[test]
fn j3_add_duplicate_block() {
    let mut m = fixture("step_gain_scope");
    let gain = m.root.blocks[1].clone();

    assert_eq!(
        apply(
            &mut m,
            Command::AddBlock {
                path: root(),
                block: gain
            }
        ),
        Err(EditError::DuplicateBlock(BlockId(2)))
    );
}

/// J-4: adding a block with an existing name fails.
#[test]
fn j4_add_duplicate_name() {
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();
    let mut b = new_block(&m.root, "Constant", BlockId(10), Point::new(0.0, 120.0)).unwrap();
    b.name = "Gain".into();

    assert_eq!(
        apply(
            &mut m,
            Command::AddBlock {
                path: root(),
                block: b
            }
        ),
        Err(EditError::DuplicateName("Gain".into()))
    );
    assert_eq!(m.root, m0.root);
}

/// J-5: renaming to an existing name fails.
#[test]
fn j5_rename_duplicate_name() {
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();

    assert_eq!(
        apply(
            &mut m,
            Command::RenameBlock {
                path: root(),
                block: BlockId(1),
                name: "Gain".into()
            }
        ),
        Err(EditError::DuplicateName("Gain".into()))
    );
    assert_eq!(m.root, m0.root);
}

/// J-6: an unknown system path fails.
#[test]
fn j6_unknown_system_path() {
    let mut m = fixture("step_gain_scope");

    assert_eq!(
        apply(
            &mut m,
            Command::MoveBlocks {
                path: SystemPath(vec![BlockId(99)]),
                blocks: vec![],
                dx: 0.0,
                dy: 0.0
            }
        ),
        Err(EditError::SystemNotFound)
    );
}

/// J-7 / J-8: undo and redo on an empty history report "nothing to do".
#[test]
fn j7_j8_empty_history() {
    let mut m = fixture("step_gain_scope");
    let mut h = History::new(10);

    assert!(!h.can_undo());
    assert!(!h.can_redo());
    assert_eq!(h.undo_label(), None);
    assert_eq!(h.undo(&mut m), Ok(false));
    assert_eq!(h.redo(&mut m), Ok(false));
}
