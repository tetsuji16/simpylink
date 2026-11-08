//! Clipboard unit and integration tests (M1-11, issue #52).

use sim_core::clipboard::*;
use sim_core::edit::{Command, History};
use sim_core::geom::{Point, Rect};
use sim_core::ids::{BlockId, LineId};
use sim_core::json::from_json_str;
use sim_core::model::{Block, Branch, Model, PortRef, TapPoint};
use sim_core::path::SystemPath;
use std::collections::{BTreeMap, BTreeSet};

fn fixture(name: &str) -> Model {
    let p = format!(
        "{}/../../tests/fixtures/models/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    from_json_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn pr(b: u32, p: u32) -> PortRef {
    PortRef {
        block: BlockId(b),
        port: p,
    }
}

/// H-1: copy_selection(root, {2,3}) -> blocks [Gain, Scope], lines [Line2]
/// (Line1 is not included because src Step is not in selection).
#[test]
fn copy_selection_between_selected_blocks() {
    let m = fixture("step_gain_scope");
    let p = copy_selection(&m.root, &BTreeSet::from([BlockId(2), BlockId(3)]));
    assert_eq!(p.blocks.len(), 2);
    assert_eq!(p.blocks[0].sid, BlockId(2));
    assert_eq!(p.blocks[0].name, "Gain");
    assert_eq!(p.blocks[1].sid, BlockId(3));
    assert_eq!(p.blocks[1].name, "Scope");
    assert_eq!(p.lines.len(), 1);
    assert_eq!(p.lines[0].id, LineId(2));
    assert_eq!(p.lines[0].src, pr(2, 1));
    assert_eq!(p.lines[0].dst, Some(pr(3, 1)));
}

/// H-2: paste_payload(m, root, &p, (0, 100)) ->
/// Batch[AddBlock{sid 4, name "Gain1", pos [130,150,160,180]},
///       AddBlock{sid 5, name "Scope1", pos [210,149,240,181]},
///       AddLine{id 3, src{4,1}, dst{5,1}}], new SIDs [4, 5].
#[test]
fn paste_payload_assigns_new_sids_and_names() {
    let m = fixture("step_gain_scope");
    let p = copy_selection(&m.root, &BTreeSet::from([BlockId(2), BlockId(3)]));
    let (cmd, ids) = paste_payload(&m, &SystemPath::root(), &p, (0.0, 100.0)).unwrap();
    assert_eq!(ids, vec![BlockId(4), BlockId(5)]);

    let Command::Batch { label, commands } = cmd else {
        panic!("expected Batch command");
    };
    assert_eq!(label, "Paste");
    assert_eq!(commands.len(), 3);

    // 1st AddBlock: Gain1
    let Command::AddBlock { path, block } = &commands[0] else {
        panic!("expected AddBlock");
    };
    assert_eq!(path, &SystemPath::root());
    assert_eq!(block.sid, BlockId(4));
    assert_eq!(block.name, "Gain1");
    assert_eq!(block.position, Rect::new(130.0, 150.0, 160.0, 180.0));

    // 2nd AddBlock: Scope1
    let Command::AddBlock { path, block } = &commands[1] else {
        panic!("expected AddBlock");
    };
    assert_eq!(path, &SystemPath::root());
    assert_eq!(block.sid, BlockId(5));
    assert_eq!(block.name, "Scope1");
    assert_eq!(block.position, Rect::new(210.0, 149.0, 240.0, 181.0));

    // AddLine: id 3, src {4, 1}, dst {5, 1}
    let Command::AddLine { path, line } = &commands[2] else {
        panic!("expected AddLine");
    };
    assert_eq!(path, &SystemPath::root());
    assert_eq!(line.id, LineId(3));
    assert_eq!(line.src, pr(4, 1));
    assert_eq!(line.dst, Some(pr(5, 1)));
}

/// H-3: copy_selection(root, {1,2}) -> lines [Line1] (dst Gain is in selection).
///      copy_selection(root, {1}) -> lines [] (dst not in selection).
#[test]
fn copy_selection_source_and_destination_filtering() {
    let m = fixture("step_gain_scope");
    let p12 = copy_selection(&m.root, &BTreeSet::from([BlockId(1), BlockId(2)]));
    assert_eq!(p12.blocks.len(), 2);
    assert_eq!(p12.lines.len(), 1);
    assert_eq!(p12.lines[0].id, LineId(1));
    assert_eq!(p12.lines[0].src, pr(1, 1));
    assert_eq!(p12.lines[0].dst, Some(pr(2, 1)));

    let p1 = copy_selection(&m.root, &BTreeSet::from([BlockId(1)]));
    assert_eq!(p1.blocks.len(), 1);
    assert_eq!(p1.lines.len(), 0);
}

/// H-6: payload_from_text("{\"format\":\"other\"}") -> None.
///      往復 payload_from_text(&payload_to_text(&p)) == Some(p).
#[test]
fn payload_serialization_roundtrip_and_validation() {
    let m = fixture("step_gain_scope");
    let p = copy_selection(&m.root, &BTreeSet::from([BlockId(2), BlockId(3)]));
    let text = payload_to_text(&p);
    assert_eq!(payload_from_text(&text), Some(p.clone()));

    // Invalid formats or versions
    assert_eq!(payload_from_text("{\"format\":\"other\"}"), None);
    assert_eq!(payload_from_text("not json at all"), None);
    assert_eq!(
        payload_from_text(
            "{\"format\":\"simpylink-clipboard\",\"version\":999,\"blocks\":[],\"lines\":[]}"
        ),
        None
    );
}

/// Paste execution and undo via History:
/// - Next SID / LineId increments correctly
/// - Undo completely restores initial model blocks and lines
#[test]
fn paste_execute_and_undo() {
    let m = fixture("step_gain_scope");
    let p = copy_selection(&m.root, &BTreeSet::from([BlockId(2), BlockId(3)]));
    let (cmd, ids) = paste_payload(&m, &SystemPath::root(), &p, (0.0, 100.0)).unwrap();
    assert_eq!(ids, vec![BlockId(4), BlockId(5)]);

    let mut m2 = m.clone();
    let mut h = History::new(10);
    h.execute(&mut m2, cmd).unwrap();

    assert_eq!(m2.root.blocks.len(), 5);
    assert_eq!(m2.root.lines.len(), 3);
    assert_eq!(m2.next_sid, 6);
    assert_eq!(m2.next_line_id, 4);

    // Undo restores original blocks and lines
    h.undo(&mut m2).unwrap();
    assert_eq!(m2.root, m.root);
}

/// Paste multiple times generates sequentially unique block names
/// (Gain -> Gain1 -> Gain2).
#[test]
fn paste_multiple_times_generates_unique_names() {
    let mut m = fixture("step_gain_scope");
    let mut h = History::new(10);
    let p = copy_selection(&m.root, &BTreeSet::from([BlockId(2)]));

    // First paste: "Gain1"
    let (cmd1, ids1) = paste_payload(&m, &SystemPath::root(), &p, (20.0, 10.0)).unwrap();
    assert_eq!(ids1, vec![BlockId(4)]);
    h.execute(&mut m, cmd1).unwrap();
    assert_eq!(m.root.block(BlockId(4)).unwrap().name, "Gain1");

    // Second paste: "Gain2"
    let (cmd2, ids2) = paste_payload(&m, &SystemPath::root(), &p, (40.0, 20.0)).unwrap();
    assert_eq!(ids2, vec![BlockId(5)]);
    h.execute(&mut m, cmd2).unwrap();
    assert_eq!(m.root.block(BlockId(5)).unwrap().name, "Gain2");
}

/// Duplicate blocks offset and positioning:
/// Verify translation offset applies to positions and points.
#[test]
fn duplicate_blocks_positioning() {
    let m = fixture("step_gain_scope");
    let p = copy_selection(&m.root, &BTreeSet::from([BlockId(2)]));
    let (cmd, ids) = paste_payload(&m, &SystemPath::root(), &p, (20.0, 10.0)).unwrap();
    assert_eq!(ids, vec![BlockId(4)]);

    let Command::Batch { commands, .. } = cmd else {
        panic!("expected Batch");
    };
    let Command::AddBlock { block, .. } = &commands[0] else {
        panic!("expected AddBlock");
    };
    // Gain original position: [130, 50, 160, 80]
    // with offset (20, 10): [150, 60, 180, 90]
    assert_eq!(block.position, Rect::new(150.0, 60.0, 180.0, 90.0));
    assert_eq!(block.name, "Gain1");
}

/// Branch line remapping during copy and paste:
/// If a line has branches connecting to multiple selected blocks,
/// all branch destinations and points are remapped and offset properly.
#[test]
fn copy_paste_with_branches() {
    let mut m = fixture("step_gain_scope");
    // Add a second Gain block to receive the branched connection from Step
    let gain2 = Block {
        sid: BlockId(4),
        name: "GainB".into(),
        block_type: "Gain".into(),
        position: Rect::new(130.0, 120.0, 160.0, 150.0),
        orientation: sim_core::model::Orientation::Right,
        params: BTreeMap::from([("Gain".into(), "1".into())]),
        system: None,
    };
    m.root.blocks.push(gain2);
    m.next_sid = 5;

    // Add a branch from line 1 to GainB(4):1
    m.root.lines[0].branches.push(Branch {
        tap: TapPoint { segment: 0, t: 0.5 },
        points: vec![Point::new(100.0, 65.0), Point::new(100.0, 135.0)],
        dst: Some(pr(4, 1)),
        branches: vec![],
    });

    // Copy Step (1), Gain (2), and GainB (4)
    let p = copy_selection(
        &m.root,
        &BTreeSet::from([BlockId(1), BlockId(2), BlockId(4)]),
    );
    assert_eq!(p.blocks.len(), 3);
    assert_eq!(p.lines.len(), 1);

    let (cmd, ids) = paste_payload(&m, &SystemPath::root(), &p, (50.0, 50.0)).unwrap();
    assert_eq!(ids, vec![BlockId(5), BlockId(6), BlockId(7)]);

    let mut m2 = m.clone();
    let mut h = History::new(10);
    h.execute(&mut m2, cmd).unwrap();

    // Verify pasted branched line
    let pasted_line = m2.root.lines.iter().find(|l| l.id == LineId(3)).unwrap();
    assert_eq!(pasted_line.src, pr(5, 1));
    assert_eq!(pasted_line.dst, Some(pr(6, 1)));
    assert_eq!(pasted_line.branches.len(), 1);
    assert_eq!(pasted_line.branches[0].dst, Some(pr(7, 1)));
    assert_eq!(pasted_line.branches[0].points[0], Point::new(150.0, 115.0));
}

/// payload_origin calculation:
/// Returns top-left of the bounding box of all blocks in payload.
#[test]
fn payload_origin_bounds() {
    let m = fixture("step_gain_scope");
    let p = copy_selection(&m.root, &BTreeSet::from([BlockId(2), BlockId(3)]));
    // Gain: [130, 50, 160, 80]
    // Scope: [210, 49, 240, 81]
    // Bounding box: left 130, top 49, right 240, bottom 81
    let origin = payload_origin(&p).unwrap();
    assert_eq!(origin, Point::new(130.0, 49.0));

    // Empty payload has None origin
    let empty_payload = ClipboardPayload {
        format: "simpylink-clipboard".into(),
        version: 1,
        blocks: vec![],
        lines: vec![],
    };
    assert_eq!(payload_origin(&empty_payload), None);
}
