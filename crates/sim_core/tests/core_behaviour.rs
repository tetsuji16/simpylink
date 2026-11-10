//! Validation, editing, history and clipboard tests (Issues #37, #42, #52).
use sim_core::clipboard::*;
use sim_core::diag::Severity;
use sim_core::edit::*;
use sim_core::geom::{Point, Rect};
use sim_core::ids::{BlockId, LineId};
use sim_core::json::from_json_str;
use sim_core::library::{new_block, port_counts, PortCounts, OPAQUE_INPUTS, OPAQUE_OUTPUTS};
use sim_core::model::*;
use sim_core::path::SystemPath;
use sim_core::validate::validate_model;
use std::collections::BTreeSet;

fn fixture(name: &str) -> Model {
    let p = format!(
        "{}/../../tests/fixtures/models/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    from_json_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}
fn codes(m: &Model) -> Vec<String> {
    validate_model(m).into_iter().map(|d| d.code).collect()
}
fn root() -> SystemPath {
    SystemPath::root()
}
fn pr(b: u32, p: u32) -> PortRef {
    PortRef {
        block: BlockId(b),
        port: p,
    }
}

#[test]
fn opaque_block_port_counts_are_used_for_validation() {
    let mut block = Block {
        sid: BlockId(1),
        block_type: "UnknownOpaqueBlock".into(),
        name: "T".into(),
        position: Rect::new(0.0, 0.0, 30.0, 30.0),
        orientation: Orientation::Right,
        params: [(OPAQUE_INPUTS.to_string(), "1".to_string())]
            .into_iter()
            .collect(),
        system: None,
    };
    assert_eq!(
        port_counts(&block).unwrap(),
        PortCounts {
            inputs: 1,
            outputs: 0
        }
    );
    block.params.insert(OPAQUE_OUTPUTS.into(), "x".into());
    assert!(port_counts(&block).is_err());
    block.params.clear();
    assert!(port_counts(&block).is_err());

    let source = Block {
        sid: BlockId(2),
        block_type: "Constant".into(),
        name: "C".into(),
        position: Rect::new(0.0, 0.0, 30.0, 30.0),
        orientation: Orientation::Right,
        params: [
            ("Value".into(), "1".into()),
            ("SampleTime".into(), "inf".into()),
        ]
        .into_iter()
        .collect(),
        system: None,
    };
    let mut model = Model::new("opaque");
    model.root.blocks = vec![
        source,
        Block {
            sid: BlockId(1),
            block_type: "UnknownOpaqueBlock".into(),
            name: "T".into(),
            position: Rect::new(40.0, 0.0, 70.0, 30.0),
            orientation: Orientation::Right,
            params: [(OPAQUE_INPUTS.into(), "1".into())].into_iter().collect(),
            system: None,
        },
    ];
    model.root.lines.push(Line {
        id: LineId(1),
        src: pr(2, 1),
        points: vec![],
        dst: Some(pr(1, 1)),
        branches: vec![],
        name: String::new(),
    });
    let diagnostics = validate_model(&model);
    assert!(diagnostics.iter().any(|d| d.code == "E_UNKNOWN_BLOCK_TYPE"));
    assert!(!diagnostics.iter().any(|d| d.code == "E_PORT_OUT_OF_RANGE"));
}

#[test]
fn validate_cases() {
    let m0 = fixture("step_gain_scope");
    assert!(validate_model(&m0).is_empty());
    let mut m = m0.clone();
    m.config.stop_time = 0.0;
    assert_eq!(codes(&m), vec!["E_INVALID_CONFIG"]);
    let mut m = m0.clone();
    m.config.output_step = -1.0;
    m.config.rel_tol = 0.0;
    let d = validate_model(&m);
    assert_eq!(d[0].message, "output_step must be positive");
    assert_eq!(d[1].message, "rel_tol must be positive");
    let mut m = m0.clone();
    m.root.blocks[1].sid = BlockId(1);
    assert_eq!(codes(&m)[0], "E_DUPLICATE_SID");
    let mut m = m0.clone();
    m.root.blocks[1].name = "Step".into();
    assert!(codes(&m).contains(&"E_DUPLICATE_BLOCK_NAME".into()));
    let mut m = m0.clone();
    m.root.blocks[1].block_type = "Foo".into();
    let c = codes(&m);
    assert!(c.contains(&"E_UNKNOWN_BLOCK_TYPE".into()) && !c.contains(&"E_INVALID_PARAM".into()));
    let mut m = m0.clone();
    m.root.blocks[1].params.remove("Gain");
    assert!(validate_model(&m)
        .iter()
        .any(|d| d.message == "parameter 'Gain' is missing"));
    let mut m = m0.clone();
    m.root.blocks[1]
        .params
        .insert("Gain".into(), "[1 2]".into());
    assert!(validate_model(&m)
        .iter()
        .any(|d| d.message == "parameter 'Gain': expected a scalar"));
    let mut m = m0.clone();
    m.root.lines[1].src.block = BlockId(99);
    assert!(codes(&m).contains(&"E_LINE_SRC_MISSING".into()));
    let mut m = m0.clone();
    m.root.lines[0].dst = Some(pr(2, 2));
    let c = codes(&m);
    assert!(c.contains(&"E_PORT_OUT_OF_RANGE".into()) && c.contains(&"W_UNCONNECTED_INPUT".into()));
    let mut m = m0.clone();
    let mut l3 = m.root.lines[0].clone();
    l3.id = LineId(3);
    m.root.lines.push(l3);
    assert!(codes(&m).contains(&"E_INPUT_MULTIPLY_DRIVEN".into()));
    let mut m = m0.clone();
    m.root.lines[1].dst = None;
    let c = codes(&m);
    assert!(c.contains(&"W_DANGLING_LINE".into()) && c.contains(&"W_UNCONNECTED_INPUT".into()));
    let mut m = fixture("sum_constants");
    m.root.lines.retain(|l| l.id != LineId(2));
    let d = validate_model(&m);
    assert_eq!(d.len(), 2);
    assert_eq!(d[0].severity, Severity::Warning);
    assert_eq!(
        d[0].message,
        "input port 2 of 'Sum' is not connected; it will read 0"
    );
    assert_eq!(d[0].to_string(), "warning[W_UNCONNECTED_INPUT]: input port 2 of 'Sum' is not connected; it will read 0 (block 3)");
}

#[test]
fn add_delete_undo_redo() {
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();
    let mut h = History::new(200);
    assert!(!h.is_dirty());
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
    assert_eq!(m.next_sid, 5);
    assert!(h.is_dirty());
    assert!(h.undo(&mut m).unwrap());
    assert_eq!(m.root, m0.root);
    assert_eq!(m.next_sid, 5);
    assert!(!h.is_dirty());
    assert!(h.redo(&mut m).unwrap());
    assert_eq!(m.root.blocks.len(), 4);
    // delete gain
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
    assert_eq!(m.root.blocks.len(), 2);
    assert_eq!(m.root.lines.len(), 1);
    assert_eq!(m.root.lines[0].dst, None);
    h.undo(&mut m).unwrap();
    assert_eq!(m.root, m0.root);
    assert!(!h.undo(&mut m).unwrap());
    assert!(!History::new(5).redo(&mut m).unwrap());
}

#[test]
fn prune_and_move_and_params() {
    let l = Line {
        id: LineId(1),
        src: pr(1, 1),
        points: vec![],
        dst: Some(pr(2, 1)),
        branches: vec![Branch {
            tap: TapPoint { segment: 0, t: 0.5 },
            points: vec![],
            dst: Some(pr(3, 1)),
            branches: vec![],
        }],
        name: String::new(),
    };
    let p = prune_line(&l, &BTreeSet::from([BlockId(3)])).unwrap();
    assert_eq!(p.dst, Some(pr(2, 1)));
    assert!(p.branches.is_empty());
    let p = prune_line(&l, &BTreeSet::from([BlockId(2)])).unwrap();
    assert_eq!(p.dst, None);
    assert_eq!(p.branches.len(), 1);
    assert!(prune_line(&l, &BTreeSet::from([BlockId(9)])).is_none());

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
    assert_eq!(h.undo_label(), Some("Move".into()));
    h.undo(&mut m).unwrap();
    assert_eq!(m.root, before);

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
    h.undo(&mut m).unwrap();
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
    h.undo(&mut m).unwrap();
    assert!(!m.root.blocks[1].params.contains_key("New"));
}

#[test]
fn errors_leave_model_unchanged() {
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();
    let dup = Line {
        id: LineId(9),
        src: pr(1, 1),
        points: vec![],
        dst: Some(pr(2, 1)),
        branches: vec![],
        name: String::new(),
    };
    assert_eq!(
        apply(
            &mut m,
            Command::AddLine {
                path: root(),
                line: dup.clone()
            }
        ),
        Err(EditError::InputAlreadyConnected(pr(2, 1)))
    );
    assert_eq!(m.root, m0.root);
    let extra = new_block(&m.root, "Constant", BlockId(10), Point::new(0.0, 0.0)).unwrap();
    let r = apply(
        &mut m,
        Command::Batch {
            label: "x".into(),
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
    assert!(r.is_err());
    assert_eq!(m.root, m0.root);
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
    let g = m.root.blocks[1].clone();
    assert_eq!(
        apply(
            &mut m,
            Command::AddBlock {
                path: root(),
                block: g
            }
        ),
        Err(EditError::DuplicateBlock(BlockId(2)))
    );
    assert!(matches!(
        apply(
            &mut m,
            Command::RenameBlock {
                path: root(),
                block: BlockId(1),
                name: "Gain".into()
            }
        ),
        Err(EditError::DuplicateName(_))
    ));
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
    assert_eq!(m.root, m0.root);
}

#[test]
fn history_limit_and_dirty() {
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
    assert!(!h.undo(&mut m).unwrap());
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
    h.mark_saved();
    assert!(!h.is_dirty());
    h.undo(&mut m).unwrap();
    assert!(h.is_dirty());
    h.redo(&mut m).unwrap();
    assert!(!h.is_dirty());
}

#[test]
fn clipboard_roundtrip() {
    let m = fixture("step_gain_scope");
    let p = copy_selection(&m.root, &BTreeSet::from([BlockId(2), BlockId(3)]));
    assert_eq!(p.blocks.len(), 2);
    assert_eq!(p.lines.len(), 1);
    assert_eq!(p.lines[0].id, LineId(2));
    let (cmd, ids) = paste_payload(&m, &root(), &p, (0.0, 100.0)).unwrap();
    assert_eq!(ids, vec![BlockId(4), BlockId(5)]);
    let Command::Batch { commands, .. } = &cmd else {
        panic!()
    };
    let Command::AddBlock { block, .. } = &commands[0] else {
        panic!()
    };
    assert_eq!(block.name, "Gain1");
    assert_eq!(block.position, Rect::new(130.0, 150.0, 160.0, 180.0));
    let Command::AddLine { line, .. } = &commands[2] else {
        panic!()
    };
    assert_eq!(
        (line.id, line.src, line.dst),
        (LineId(3), pr(4, 1), Some(pr(5, 1)))
    );
    assert_eq!(
        copy_selection(&m.root, &BTreeSet::from([BlockId(1), BlockId(2)]))
            .lines
            .len(),
        1
    );
    assert_eq!(
        copy_selection(&m.root, &BTreeSet::from([BlockId(1)]))
            .lines
            .len(),
        0
    );
    assert_eq!(payload_from_text(&payload_to_text(&p)), Some(p.clone()));
    assert_eq!(payload_from_text("{\"format\":\"other\"}"), None);
    let mut m2 = m.clone();
    let mut h = History::new(10);
    h.execute(&mut m2, cmd).unwrap();
    assert_eq!(m2.root.blocks.len(), 5);
    h.undo(&mut m2).unwrap();
    assert_eq!(m2.root, m.root);
}

#[test]
fn create_subsystem_from_gain() {
    use sim_core::subsystem::create_subsystem_command;
    let m0 = fixture("step_gain_scope");
    let mut m = m0.clone();
    let (cmd, sid) = create_subsystem_command(&m, &root(), &BTreeSet::from([BlockId(2)])).unwrap();
    let mut h = History::new(10);
    h.execute(&mut m, cmd).unwrap();
    assert!(validate_model(&m).is_empty(), "{:?}", validate_model(&m));
    let ss = m.root.block(sid).unwrap();
    let inner = ss.system.as_ref().unwrap();
    assert_eq!(
        inner
            .blocks
            .iter()
            .map(|b| b.block_type.as_str())
            .collect::<Vec<_>>(),
        vec!["Gain", "Inport", "Outport"]
    );
    assert_eq!(inner.lines.len(), 2);
    assert_eq!(m.root.blocks.len(), 3);
    let dsts: Vec<_> = m.root.lines.iter().flat_map(|l| l.destinations()).collect();
    assert!(dsts.contains(&pr(sid.0, 1)) && dsts.contains(&pr(3, 1)));
    h.undo(&mut m).unwrap();
    assert_eq!(m.root.blocks, m0.root.blocks);
}
