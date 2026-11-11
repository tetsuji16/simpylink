//! Regression coverage for recursive clipboard ID allocation (#195).
use sim_core::clipboard::{copy_selection, paste_payload};
use sim_core::edit::{EditError, History};
use sim_core::geom::Point;
use sim_core::ids::{BlockId, LineId};
use sim_core::library::new_block;
use sim_core::model::{Block, Branch, Line, Model, PortRef, System, TapPoint};
use sim_core::path::SystemPath;
use sim_core::validate::validate_model;
use std::collections::BTreeSet;

/// Builds a registered block with default parameters for the fixture.
fn block(id: u32, kind: &str) -> Block {
    new_block(
        &System::default(),
        kind,
        BlockId(id),
        Point::new(10.0, 20.0),
    )
    .unwrap()
}

/// Builds a reference to the first port of a fixture block.
fn port(id: u32) -> PortRef {
    PortRef {
        block: BlockId(id),
        port: 1,
    }
}

/// Builds a subsystem containing a source, sink and connecting line.
fn subsystem(id: u32) -> Block {
    let mut outer = block(id, "SubSystem");
    outer.system = Some(System {
        blocks: vec![block(id + 1, "Constant"), block(id + 2, "Scope")],
        lines: vec![Line {
            id: LineId(id),
            src: port(id + 1),
            dst: Some(port(id + 2)),
            points: vec![Point::new(42.0, 12.0)],
            branches: vec![],
            name: "signal".into(),
        }],
    });
    outer
}

/// Builds a fixture with one or two levels and valid global counters.
fn model(nested: bool) -> Model {
    let mut m = Model::new("nested");
    let mut outer = subsystem(1);
    if nested {
        outer.system.as_mut().unwrap().blocks.push(subsystem(4));
    }
    m.root.blocks.push(outer);
    m.next_sid = m.max_sid() + 1;
    m.next_line_id = m.max_line_id() + 1;
    m
}

/// Checks global ID uniqueness, local endpoints and next-ID counters.
fn assert_ids(m: &Model) {
    fn walk(sys: &System, blocks: &mut BTreeSet<BlockId>, lines: &mut BTreeSet<LineId>) {
        for b in &sys.blocks {
            assert!(blocks.insert(b.sid), "duplicate block {:?}", b.sid);
            if let Some(child) = &b.system {
                walk(child, blocks, lines);
            }
        }
        for l in &sys.lines {
            assert!(lines.insert(l.id), "duplicate line {:?}", l.id);
            assert!(sys.block(l.src.block).is_some());
            for d in l.destinations() {
                assert!(sys.block(d.block).is_some());
            }
        }
    }
    walk(&m.root, &mut BTreeSet::new(), &mut BTreeSet::new());
    assert!(m.next_sid > m.max_sid());
    assert!(m.next_line_id > m.max_line_id());
    for d in validate_model(m) {
        assert_ne!(d.code, "E_DUPLICATE_SID");
        assert_ne!(d.code, "E_DUPLICATE_LINE_ID");
    }
}

/// Checks repeated paste, content restoration and redo without ID reuse.
fn paste_and_undo(nested: bool, repeats: usize) {
    let mut m = model(nested);
    let before = m.clone();
    let p = copy_selection(&m.root, &BTreeSet::from([BlockId(1)]));
    let mut h = History::new(10);
    for _ in 0..repeats {
        let start = m.next_sid;
        let (cmd, ids) = paste_payload(&m, &SystemPath::root(), &p, (100.0, 200.0)).unwrap();
        assert_eq!(ids, vec![BlockId(start)]);
        h.execute(&mut m, cmd).unwrap();
        assert_ids(&m);
        let pasted = m.root.block(ids[0]).unwrap().system.as_ref().unwrap();
        assert_eq!(pasted.blocks[0].sid, BlockId(start + 1));
        assert_eq!(pasted.lines[0].src.block, pasted.blocks[0].sid);
        assert_eq!(pasted.lines[0].dst.unwrap().block, pasted.blocks[1].sid);
        assert_eq!(
            pasted.lines[0].points,
            p.blocks[0].system.as_ref().unwrap().lines[0].points
        );
    }
    let after = m.clone();
    for _ in 0..repeats {
        assert!(h.undo(&mut m).unwrap());
    }
    // DES-02 §7: undo restores contents, while counters never decrease.
    let mut expected = before;
    expected.next_sid = after.next_sid;
    expected.next_line_id = after.next_line_id;
    assert_eq!(m, expected);
    for _ in 0..repeats {
        assert!(h.redo(&mut m).unwrap());
    }
    assert_eq!(m, after);
}

/// Pasting a subsystem gives every contained object a fresh ID.
#[test]
fn paste_subsystem() {
    paste_and_undo(false, 1);
}
/// Nested subsystems receive fresh IDs at every level.
#[test]
fn paste_two_levels() {
    paste_and_undo(true, 1);
}
/// Repeated paste does not reuse nested block or line IDs.
#[test]
fn paste_same_payload_twice() {
    paste_and_undo(true, 2);
}

/// Malformed sources, destinations and nested branches leave the model intact.
#[test]
fn missing_nested_endpoints_are_rejected_without_changes() {
    let m = model(false);
    let before = m.clone();
    let p = copy_selection(&m.root, &BTreeSet::from([BlockId(1)]));
    for target in 0..3 {
        let mut bad = p.clone();
        let l = &mut bad.blocks[0].system.as_mut().unwrap().lines[0];
        match target {
            0 => l.src = port(999),
            1 => l.dst = Some(port(999)),
            _ => l.branches.push(Branch {
                tap: TapPoint { segment: 0, t: 0.5 },
                points: vec![],
                dst: None,
                branches: vec![Branch {
                    tap: TapPoint { segment: 0, t: 0.5 },
                    points: vec![],
                    dst: Some(port(999)),
                    branches: vec![],
                }],
            }),
        }
        assert!(matches!(
            paste_payload(&m, &SystemPath::root(), &bad, (0.0, 0.0)),
            Err(EditError::BlockNotFound(BlockId(999)))
        ));
        assert_eq!(m, before);
    }
}

/// Outer IDs precede depth-first nested IDs; local branch geometry is preserved.
#[test]
fn outer_ids_are_reserved_before_depth_first_nested_ids() {
    let mut m = model(true);
    let mut second = subsystem(7);
    second.name = "Other".into();
    m.root.blocks.push(second);
    m.root.lines.push(Line {
        id: LineId(8),
        src: port(1),
        dst: Some(port(7)),
        points: vec![],
        branches: vec![],
        name: String::new(),
    });
    m.next_sid = 10;
    m.next_line_id = 9;
    let mut p = copy_selection(&m.root, &BTreeSet::from([BlockId(1), BlockId(7)]));
    let inner = p.blocks[0].system.as_mut().unwrap();
    inner.lines[0].dst = None;
    inner.lines[0].branches.push(Branch {
        tap: TapPoint { segment: 0, t: 0.5 },
        points: vec![Point::new(40.0, 12.0)],
        dst: Some(port(3)),
        branches: vec![],
    });
    let (cmd, ids) = paste_payload(&m, &SystemPath::root(), &p, (50.0, 50.0)).unwrap();
    assert_eq!(ids, vec![BlockId(10), BlockId(11)]);
    History::new(10).execute(&mut m, cmd).unwrap();
    assert_ids(&m);
    let first = m.root.block(BlockId(10)).unwrap().system.as_ref().unwrap();
    assert_eq!(
        first.blocks.iter().map(|b| b.sid.0).collect::<Vec<_>>(),
        vec![12, 13, 14]
    );
    let deeper = first.blocks[2].system.as_ref().unwrap();
    assert_eq!(
        deeper.blocks.iter().map(|b| b.sid.0).collect::<Vec<_>>(),
        vec![15, 16]
    );
    let second = m.root.block(BlockId(11)).unwrap().system.as_ref().unwrap();
    assert_eq!(
        second.blocks.iter().map(|b| b.sid.0).collect::<Vec<_>>(),
        vec![17, 18]
    );
    assert_eq!(m.root.lines.last().unwrap().id, LineId(9));
    assert!(first.lines[0].id.0 > 9);
    assert_eq!(first.lines[0].branches[0].dst, Some(port(13)));
    assert_eq!(
        first.lines[0].branches[0].points,
        vec![Point::new(40.0, 12.0)]
    );
}

/// ID exhaustion is atomic, while the last representable next counter is accepted.
#[test]
fn exhausted_block_and_line_ids_return_errors_without_changes() {
    let base = model(false);
    let p = copy_selection(&base.root, &BTreeSet::from([BlockId(1)]));
    for (sid, line) in [(u32::MAX, 2), (u32::MAX - 2, 2), (4, u32::MAX)] {
        let mut m = base.clone();
        m.next_sid = sid;
        m.next_line_id = line;
        let before = m.clone();
        assert_eq!(
            paste_payload(&m, &SystemPath::root(), &p, (0.0, 0.0)),
            Err(EditError::IdSpaceExhausted)
        );
        assert_eq!(m, before);
    }
    let mut m = base;
    m.next_sid = u32::MAX - 3;
    m.next_line_id = u32::MAX - 1;
    let (cmd, _) = paste_payload(&m, &SystemPath::root(), &p, (0.0, 0.0)).unwrap();
    History::new(10).execute(&mut m, cmd).unwrap();
    assert_eq!(m.next_sid, u32::MAX);
    assert_eq!(m.next_line_id, u32::MAX);
    assert_ids(&m);
}
