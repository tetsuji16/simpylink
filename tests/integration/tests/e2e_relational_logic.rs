//! Relational Operator and Logical Operator E2E tests (Issue #170 §H/§J).
use sim_core::diag::codes::W_DATA_TYPE_IGNORED;
use sim_core::geom::Point;
use sim_core::ids::{BlockId, LineId};
use sim_core::library::new_block;
use sim_core::model::{Block, Line, Model, PortRef, System};
use sim_integration::e2e::{assert_close, simulate_model};
use sim_integration::load_fixture;

fn set(m: &mut Model, sid: u32, k: &str, v: &str) {
    m.root
        .blocks
        .iter_mut()
        .find(|b| b.sid.0 == sid)
        .unwrap()
        .params
        .insert(k.into(), v.into());
}

fn line(id: u32, src: u32, dst: u32, port: u32) -> Line {
    Line {
        id: LineId(id),
        src: PortRef {
            block: BlockId(src),
            port: 1,
        },
        points: vec![],
        dst: Some(PortRef {
            block: BlockId(dst),
            port,
        }),
        branches: vec![],
        name: String::new(),
    }
}

fn constant(sid: u32, value: f64) -> Block {
    let mut b = new_block(
        &System::default(),
        "Constant",
        BlockId(sid),
        Point::new(0.0, 0.0),
    )
    .unwrap();
    b.name = format!("Constant{sid}");
    b.params.insert("Value".into(), value.to_string());
    b
}

fn scope(sid: u32) -> Block {
    let mut b = new_block(
        &System::default(),
        "Scope",
        BlockId(sid),
        Point::new(0.0, 0.0),
    )
    .unwrap();
    b.name = "Scope".into();
    b
}

fn logic(sid: u32, operator: &str, inputs: &str) -> Block {
    let mut b = new_block(
        &System::default(),
        "Logic",
        BlockId(sid),
        Point::new(0.0, 0.0),
    )
    .unwrap();
    b.name = "Logical Operator".into();
    b.params.insert("Operator".into(), operator.into());
    b.params.insert("Inputs".into(), inputs.into());
    b
}

fn two_sample_model(blocks: Vec<Block>, lines: Vec<Line>, name: &str) -> Model {
    let mut m = Model::new(name);
    m.config.stop_time = 0.5;
    m.config.output_step = 0.5;
    m.root.blocks = blocks;
    m.root.lines = lines;
    m
}

/// `Logic(op)` of two constants, evaluated at t = 0.
fn logic_of(a: f64, b: f64, operator: &str) -> f64 {
    let m = two_sample_model(
        vec![
            constant(1, a),
            constant(2, b),
            logic(3, operator, "2"),
            scope(4),
        ],
        vec![line(1, 1, 3, 1), line(2, 2, 3, 2), line(3, 3, 4, 1)],
        "logic",
    );
    simulate_model(&m).unwrap().channels[0].values[0]
}

/// `Logic(NOT)` of one constant, evaluated at t = 0.
fn not_of(a: f64) -> f64 {
    let m = two_sample_model(
        vec![constant(1, a), logic(3, "NOT", "1"), scope(4)],
        vec![line(1, 1, 3, 1), line(2, 3, 4, 1)],
        "not",
    );
    simulate_model(&m).unwrap().channels[0].values[0]
}

#[test]
fn relational_and_logic_values() {
    let r = simulate_model(&load_fixture("relational_logic")).unwrap();
    assert_close(&r.time, &[0.0, 0.5, 1.0, 1.5, 2.0], 1e-12);
    // t >= 1
    assert_close(&r.channels[0].values, &[0.0, 0.0, 1.0, 1.0, 1.0], 1e-12);
    // (t < 0.5) OR (t >= 1)
    assert_close(&r.channels[1].values, &[1.0, 0.0, 1.0, 1.0, 1.0], 1e-12);
    // NOT (t >= 1)
    assert_close(&r.channels[2].values, &[1.0, 1.0, 0.0, 0.0, 0.0], 1e-12);
}

/// NOT always takes one port even if `Inputs` says otherwise (§F).
#[test]
fn not_ignores_the_inputs_count() {
    let mut m = load_fixture("relational_logic");
    set(&mut m, 7, "Inputs", "3");
    let r = simulate_model(&m).unwrap();
    assert_close(&r.channels[2].values, &[1.0, 1.0, 0.0, 0.0, 0.0], 1e-12);
}

#[test]
fn the_logical_operators_follow_their_truth_tables() {
    // Order: (0,0), (0,1), (1,0), (1,1).
    let cases = [
        ("AND", [0.0, 0.0, 0.0, 1.0]),
        ("OR", [0.0, 1.0, 1.0, 1.0]),
        ("NAND", [1.0, 1.0, 1.0, 0.0]),
        ("NOR", [1.0, 0.0, 0.0, 0.0]),
        ("XOR", [0.0, 1.0, 1.0, 0.0]),
        ("NXOR", [1.0, 0.0, 0.0, 1.0]),
    ];
    let combos = [(0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)];
    for (op, want) in cases {
        for (i, (a, b)) in combos.iter().enumerate() {
            assert_eq!(logic_of(*a, *b, op), want[i], "{op}({a}, {b})");
        }
    }
}

#[test]
fn not_inverts_the_truth_value() {
    assert_eq!(not_of(0.0), 1.0);
    assert_eq!(not_of(1.0), 0.0);
    assert_eq!(not_of(-2.5), 0.0);
}

#[test]
fn the_type_checks_take_one_input() {
    let spec = sim_core::library::spec("RelationalOperator").unwrap();
    for op in ["isInf", "isNaN", "isFinite"] {
        let mut params = std::collections::BTreeMap::new();
        params.insert("Operator".to_string(), op.to_string());
        assert_eq!((spec.ports)(&params).unwrap().inputs, 1, "{op}");
    }
    let mut params = std::collections::BTreeMap::new();
    params.insert("Operator".to_string(), ">=".to_string());
    assert_eq!((spec.ports)(&params).unwrap().inputs, 2);
}

#[test]
fn the_type_checks_agree_with_python() {
    for (op, want) in [("isFinite", 1.0), ("isInf", 0.0), ("isNaN", 0.0)] {
        let mut m = load_fixture("relational_logic");
        set(&mut m, 3, "Operator", op);
        // The type checks read input 1 only, so the second input is removed.
        m.root.lines.retain(|l| l.id.0 != 2);
        let r = simulate_model(&m).unwrap();
        assert_close(&r.channels[0].values, &[want; 5], 1e-12);
    }
}

/// A non-boolean output data type is reported but does not stop compilation (§D).
#[test]
fn a_non_boolean_output_data_type_warns() {
    let mut m = load_fixture("relational_logic");
    set(&mut m, 3, "OutDataTypeStr", "double");
    let compiled = sim_ir::compile(&m).unwrap();
    assert!(compiled
        .diagnostics
        .iter()
        .any(|d| d.code == W_DATA_TYPE_IGNORED && d.message.contains("double")));

    set(
        &mut m,
        3,
        "OutDataTypeStr",
        "Inherit: Logical (see Configuration Parameters)",
    );
    let compiled = sim_ir::compile(&m).unwrap();
    assert!(!compiled
        .diagnostics
        .iter()
        .any(|d| d.code == W_DATA_TYPE_IGNORED));
}

#[test]
fn defaults_match_the_reference() {
    let relational = sim_core::library::spec("RelationalOperator").unwrap();
    let params: Vec<(&str, &str)> = relational
        .params
        .iter()
        .map(|p| (p.name, p.default))
        .collect();
    assert_eq!(params, vec![("Operator", ">=")]);

    let logic = sim_core::library::spec("Logic").unwrap();
    let params: Vec<(&str, &str)> = logic.params.iter().map(|p| (p.name, p.default)).collect();
    assert_eq!(params, vec![("Operator", "AND"), ("Inputs", "2")]);
}

#[test]
fn parity_counts_truth_values_from_numpy_states() {
    for (op, want) in [("XOR", 0.0), ("NXOR", 1.0)] {
        let mut a = constant(1, 1.0);
        a.block_type = "Integrator".into();
        a.params = std::collections::BTreeMap::from([("InitialCondition".into(), "1".into())]);
        let mut b = a.clone();
        b.sid = BlockId(2);
        b.name = "Integrator2".into();
        let m = two_sample_model(
            vec![a, b, logic(3, op, "2"), scope(4)],
            vec![line(1, 1, 3, 1), line(2, 2, 3, 2), line(3, 3, 4, 1)],
            "numpy_parity",
        );
        let r = simulate_model(&m).unwrap();
        assert_close(&r.channels[0].values, &[want; 2], 1e-12);
    }
}
