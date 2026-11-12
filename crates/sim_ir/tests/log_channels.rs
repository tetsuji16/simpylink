//! Log channel names must stay unique across subsystems (#203).
use sim_core::geom::Point;
use sim_core::ids::{BlockId, LineId};
use sim_core::json::load_from_path;
use sim_core::library::new_block;
use sim_core::model::{Block, Line, Model, PortRef, System};
use std::collections::BTreeSet;
use std::path::Path;

fn fixture(name: &str) -> Model {
    let path = format!(
        "{}/../../tests/fixtures/models/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    load_from_path(Path::new(&path)).expect("fixture loads")
}

fn block(id: u32, kind: &str, name: &str) -> Block {
    let mut b = new_block(
        &System::default(),
        kind,
        BlockId(id),
        Point::new(10.0, 20.0),
    )
    .expect("known block type");
    b.name = name.into();
    b
}

fn line(id: u32, src: u32, dst: u32) -> Line {
    Line {
        id: LineId(id),
        src: PortRef {
            block: BlockId(src),
            port: 1,
        },
        points: vec![],
        dst: Some(PortRef {
            block: BlockId(dst),
            port: 1,
        }),
        branches: vec![],
        name: String::new(),
    }
}

fn channels(model: &Model) -> Vec<String> {
    let compiled = sim_ir::compile(model).expect("model compiles");
    compiled.ir.logs.iter().map(|l| l.name.clone()).collect()
}

fn sorted(model: &Model) -> Vec<String> {
    let mut names = channels(model);
    names.sort();
    names
}

#[test]
fn scopes_in_different_subsystems_get_distinct_channel_names() {
    let m = fixture("two_scopes_in_subsystems");
    // Both subsystems hold a Scope called "Scope": the path keeps them apart.
    assert_eq!(sorted(&m), ["A/Scope/1", "B/Scope/1"]);
    // Deterministic: a second compile produces the same names.
    assert_eq!(channels(&m), channels(&m));
    let names = channels(&m);
    assert_eq!(
        names.iter().collect::<BTreeSet<_>>().len(),
        names.len(),
        "{names:?}"
    );
}

#[test]
fn a_block_named_like_a_path_gets_a_suffix() {
    // Subsystem A holds a Scope named "B" (-> "A/B/1") and the root holds a Scope
    // literally named "A/B" (-> "A/B/1"): the later SID gets the suffix.
    let mut m = Model::new("path_name");
    let mut outer = block(1, "SubSystem", "A");
    let mut inner = System::default();
    inner.blocks.push(block(3, "Scope", "B"));
    outer.system = Some(inner);
    m.root.blocks.push(outer);
    m.root.blocks.push(block(4, "Scope", "A/B"));
    m.next_sid = 5;

    let names = channels(&m);
    assert_eq!(names.iter().collect::<BTreeSet<_>>().len(), 2, "{names:?}");
    assert!(names.contains(&"A/B/1".to_string()), "{names:?}");
    assert!(names.contains(&"A/B/1_2".to_string()), "{names:?}");
}

#[test]
fn three_identical_names_are_numbered_in_sid_order() {
    // Three blocks that all end up as "A/B/Scope/1":
    //   * Scope inside subsystem A > B            (sid 3)
    //   * root Scope named "A/B/Scope"            (sid 4)
    //   * Scope named "B/Scope" inside subsystem A (sid 5)
    let mut m = Model::new("three");
    let mut inner = System::default();
    inner.blocks.push(block(3, "Scope", "Scope"));
    let mut middle = block(2, "SubSystem", "B");
    middle.system = Some(inner);
    let mut outer = block(1, "SubSystem", "A");
    outer.system = Some(System {
        blocks: vec![middle, block(5, "Scope", "B/Scope")],
        lines: vec![],
    });
    m.root.blocks.push(outer);
    m.root.blocks.push(block(4, "Scope", "A/B/Scope"));
    m.next_sid = 6;

    assert_eq!(
        sorted(&m),
        ["A/B/Scope/1", "A/B/Scope/1_2", "A/B/Scope/1_3"],
        "{:?}",
        channels(&m)
    );
}

#[test]
fn the_new_fixture_round_trips() {
    // Keeps the hand-written fixture in step with the serializer.
    let path = format!(
        "{}/../../tests/fixtures/models/two_scopes_in_subsystems.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).expect("fixture is readable");
    assert_eq!(
        sim_core::json::to_json_string(&fixture("two_scopes_in_subsystems")),
        text
    );
}

#[test]
fn root_channels_keep_their_plain_names() {
    // Rule 1 of #203: existing result files stay readable.
    let mut m = Model::new("root");
    let mut sys = System::default();
    sys.blocks.push(block(1, "Constant", "Constant"));
    sys.blocks.push(block(2, "Scope", "Scope"));
    sys.lines.push(line(1, 1, 2));
    m.root = sys;
    m.next_sid = 3;
    m.next_line_id = 2;
    assert_eq!(channels(&m), ["Scope/1"]);
}
