//! JSON format v1 round-trip and normalization tests (M0-04, issue #34).

use sim_core::geom::{Point, Rect};
use sim_core::ids::{BlockId, LineId};
use sim_core::json::{from_json_str, load_from_path, save_to_path, to_json_string};
use sim_core::model::{Block, Branch, Line, Model, Orientation, PortRef, System, TapPoint};
use sim_core::path::SystemPath;
use std::collections::BTreeMap;

fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<BTreeMap<_, _>>()
}

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("models")
        .join("step_gain_scope.json")
}

fn build_step_gain_scope() -> Model {
    let mut m = Model::new("step_gain_scope");
    m.config.stop_time = 2.0;
    m.config.output_step = 0.5;
    m.root.blocks = vec![
        Block {
            sid: BlockId(1),
            block_type: "Step".into(),
            name: "Step".into(),
            position: Rect::new(50.0, 50.0, 80.0, 80.0),
            orientation: Orientation::Right,
            params: params(&[
                ("Time", "1"),
                ("Before", "0"),
                ("After", "2"),
                ("SampleTime", "0"),
            ]),
            system: None,
        },
        Block {
            sid: BlockId(2),
            block_type: "Gain".into(),
            name: "Gain".into(),
            position: Rect::new(130.0, 50.0, 160.0, 80.0),
            orientation: Orientation::Right,
            params: params(&[("Gain", "3"), ("SampleTime", "-1")]),
            system: None,
        },
        Block {
            sid: BlockId(3),
            block_type: "Scope".into(),
            name: "Scope".into(),
            position: Rect::new(210.0, 49.0, 240.0, 81.0),
            orientation: Orientation::Right,
            params: params(&[("NumInputPorts", "1")]),
            system: None,
        },
    ];
    m.root.lines = vec![
        Line {
            id: LineId(1),
            src: PortRef {
                block: BlockId(1),
                port: 1,
            },
            points: vec![],
            dst: Some(PortRef {
                block: BlockId(2),
                port: 1,
            }),
            branches: vec![],
            name: String::new(),
        },
        Line {
            id: LineId(2),
            src: PortRef {
                block: BlockId(2),
                port: 1,
            },
            points: vec![],
            dst: Some(PortRef {
                block: BlockId(3),
                port: 1,
            }),
            branches: vec![],
            name: String::new(),
        },
    ];
    m.next_sid = 4;
    m.next_line_id = 3;
    m
}

#[test]
fn roundtrip_equal() {
    let m = build_step_gain_scope();
    let s = to_json_string(&m);
    assert_eq!(from_json_str(&s).unwrap(), m);
}

#[test]
fn fixture_is_fixed_point() {
    let s = std::fs::read_to_string(fixture_path()).unwrap();
    let m = from_json_str(&s).unwrap();
    assert_eq!(to_json_string(&m), s);
}

#[test]
fn fixture_matches_builder() {
    let s = std::fs::read_to_string(fixture_path()).unwrap();
    assert_eq!(from_json_str(&s).unwrap(), build_step_gain_scope());
}

#[test]
fn deterministic() {
    let m = build_step_gain_scope();
    assert_eq!(to_json_string(&m), to_json_string(&m));
}

#[test]
fn sorts_on_load() {
    let m = build_step_gain_scope();
    let mut v: serde_json::Value = serde_json::from_str(&to_json_string(&m)).unwrap();
    let blocks = v["root"]["blocks"].as_array_mut().unwrap();
    blocks.reverse();
    let loaded = from_json_str(&serde_json::to_string(&v).unwrap()).unwrap();
    let sids: Vec<u32> = loaded.root.blocks.iter().map(|b| b.sid.0).collect();
    assert_eq!(sids, vec![1, 2, 3]);
}

#[test]
fn fixes_next_sid() {
    let m = build_step_gain_scope();
    let mut v: serde_json::Value = serde_json::from_str(&to_json_string(&m)).unwrap();
    v["next_sid"] = serde_json::json!(1);
    v["next_line_id"] = serde_json::json!(0);
    let loaded = from_json_str(&serde_json::to_string(&v).unwrap()).unwrap();
    assert_eq!(loaded.next_sid, 4);
    assert_eq!(loaded.next_line_id, 3);
}

#[test]
fn destinations_order() {
    let a = PortRef {
        block: BlockId(20),
        port: 1,
    };
    let b = PortRef {
        block: BlockId(21),
        port: 1,
    };
    let c = PortRef {
        block: BlockId(22),
        port: 1,
    };
    let d = PortRef {
        block: BlockId(23),
        port: 1,
    };
    let tap = TapPoint { segment: 0, t: 0.5 };
    let line = Line {
        id: LineId(1),
        src: PortRef {
            block: BlockId(1),
            port: 1,
        },
        points: vec![],
        dst: Some(a),
        branches: vec![
            Branch {
                tap,
                points: vec![],
                dst: Some(b),
                branches: vec![Branch {
                    tap,
                    points: vec![],
                    dst: Some(c),
                    branches: vec![],
                }],
            },
            Branch {
                tap,
                points: vec![],
                dst: Some(d),
                branches: vec![],
            },
        ],
        name: String::new(),
    };
    assert_eq!(line.destinations(), vec![a, b, c, d]);
}

#[test]
fn system_path() {
    let mut m = Model::new("nested");
    let inner = System {
        blocks: vec![Block {
            sid: BlockId(11),
            block_type: "Gain".into(),
            name: "Gain".into(),
            position: Rect::new(0.0, 0.0, 30.0, 30.0),
            orientation: Orientation::Right,
            params: params(&[("Gain", "1")]),
            system: None,
        }],
        lines: vec![],
    };
    m.root.blocks = vec![Block {
        sid: BlockId(10),
        block_type: "Subsystem".into(),
        name: "Subsystem".into(),
        position: Rect::new(0.0, 0.0, 30.0, 30.0),
        orientation: Orientation::Right,
        params: params(&[]),
        system: Some(inner),
    }];
    m.next_sid = 12;
    let found = m.system(&SystemPath(vec![BlockId(10)])).unwrap();
    assert!(found.block(BlockId(11)).is_some());
    assert!(m.system(&SystemPath(vec![BlockId(99)])).is_none());
    assert_eq!(m.max_sid(), 11);
}

#[test]
fn save_and_load_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    let m = build_step_gain_scope();
    save_to_path(&m, &path).unwrap();
    assert_eq!(load_from_path(&path).unwrap(), m);
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json.tmp"))
        .collect();
    assert!(leftovers.is_empty(), "temp file left behind: {leftovers:?}");
    assert_eq!(m.root.blocks[0].position, Rect::new(50.0, 50.0, 80.0, 80.0));
    assert_eq!(m.root.lines[0].points, Vec::<Point>::new());
}
