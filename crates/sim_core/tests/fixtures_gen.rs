//! Builds the shared fixture models in tests/fixtures/models.
//! Run with SIMPYLINK_WRITE_FIXTURES=1 to (re)write them; otherwise checks they match.
#![allow(clippy::field_reassign_with_default, clippy::vec_init_then_push)]
use sim_core::geom::{Point, Rect};
use sim_core::ids::{BlockId, LineId};
use sim_core::json::{from_json_str, to_json_string};
use sim_core::library::{fill_default_params, spec};
use sim_core::model::{Block, Branch, Line, Model, Orientation, PortRef, TapPoint};
use std::path::PathBuf;

fn blk(sid: u32, t: &str, name: &str, x: f64, y: f64, params: &[(&str, &str)]) -> Block {
    let s = spec(t).expect("known type");
    let (w, h) = s.default_size;
    let mut b = Block {
        sid: BlockId(sid),
        block_type: t.into(),
        name: name.into(),
        position: Rect::from_min_size(Point::new(x, y), w, h),
        orientation: Orientation::Right,
        params: params
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        system: None,
    };
    fill_default_params(&mut b);
    b
}
fn pr(b: u32, p: u32) -> PortRef {
    PortRef {
        block: BlockId(b),
        port: p,
    }
}
fn ln(id: u32, src: (u32, u32), dst: (u32, u32)) -> Line {
    Line {
        id: LineId(id),
        src: pr(src.0, src.1),
        points: vec![],
        dst: Some(pr(dst.0, dst.1)),
        branches: vec![],
        name: String::new(),
    }
}
fn ln_branch(id: u32, src: (u32, u32), dst: (u32, u32), bdst: (u32, u32)) -> Line {
    let mut l = ln(id, src, dst);
    l.branches.push(Branch {
        tap: TapPoint { segment: 0, t: 0.5 },
        points: vec![],
        dst: Some(pr(bdst.0, bdst.1)),
        branches: vec![],
    });
    l
}
fn model(
    name: &str,
    stop: f64,
    step: f64,
    rtol: f64,
    atol: f64,
    blocks: Vec<Block>,
    lines: Vec<Line>,
) -> Model {
    let mut m = Model::new(name);
    m.config.stop_time = stop;
    m.config.output_step = step;
    m.config.rel_tol = rtol;
    m.config.abs_tol = atol;
    m.root.blocks = blocks;
    m.root.lines = lines;
    m.next_sid = m.max_sid() + 1;
    m.next_line_id = m.max_line_id() + 1;
    m
}

/// All fixtures.
pub fn fixtures() -> Vec<Model> {
    let step = |sid, t: &str, a: &str| {
        blk(
            sid,
            "Step",
            "Step",
            50.0,
            50.0,
            &[
                ("Time", t),
                ("Before", "0"),
                ("After", a),
                ("SampleTime", "0"),
            ],
        )
    };
    let scope = |sid, x| blk(sid, "Scope", "Scope", x, 49.0, &[]);
    let c = |sid, name: &str, v: &str, y| blk(sid, "Constant", name, 50.0, y, &[("Value", v)]);
    let mut v = vec![];
    // Step -> Gain -> Scope (DES-02 §3.1)
    v.push(model(
        "step_gain_scope",
        2.0,
        0.5,
        1e-3,
        1e-6,
        vec![
            step(1, "1", "2"),
            blk(2, "Gain", "Gain", 130.0, 50.0, &[("Gain", "3")]),
            scope(3, 210.0),
        ],
        vec![ln(1, (1, 1), (2, 1)), ln(2, (2, 1), (3, 1))],
    ));
    v.push(model(
        "gain_constant",
        1.0,
        0.5,
        1e-3,
        1e-6,
        vec![
            c(1, "Constant", "2", 50.0),
            blk(2, "Gain", "Gain", 130.0, 50.0, &[("Gain", "3")]),
            scope(3, 210.0),
        ],
        vec![ln(1, (1, 1), (2, 1)), ln(2, (2, 1), (3, 1))],
    ));
    v.push(model(
        "sum_constants",
        1.0,
        1.0,
        1e-3,
        1e-6,
        vec![
            c(1, "Constant", "4", 30.0),
            c(2, "Constant1", "1.5", 90.0),
            blk(3, "Sum", "Sum", 130.0, 55.0, &[("Inputs", "+-")]),
            blk(4, "Outport", "Out1", 210.0, 58.0, &[("Port", "1")]),
        ],
        vec![
            ln(1, (1, 1), (3, 1)),
            ln(2, (2, 1), (3, 2)),
            ln(3, (3, 1), (4, 1)),
        ],
    ));
    v.push(model(
        "algebraic_loop",
        1.0,
        0.5,
        1e-3,
        1e-6,
        vec![
            c(1, "Constant", "1", 50.0),
            blk(2, "Gain", "Gain", 130.0, 120.0, &[("Gain", "0.5")]),
            blk(3, "Sum", "Sum", 130.0, 55.0, &[("Inputs", "++")]),
            scope(4, 210.0),
        ],
        vec![
            ln(1, (1, 1), (3, 1)),
            ln(2, (2, 1), (3, 2)),
            ln_branch(3, (3, 1), (4, 1), (2, 1)),
        ],
    ));
    v.push(model(
        "const_integrator",
        2.0,
        0.5,
        1e-6,
        1e-9,
        vec![
            c(1, "Constant", "1", 50.0),
            blk(2, "Integrator", "Integrator", 130.0, 49.0, &[]),
            scope(3, 210.0),
        ],
        vec![ln(1, (1, 1), (2, 1)), ln(2, (2, 1), (3, 1))],
    ));
    v.push(model(
        "first_order_lag",
        1.0,
        0.1,
        1e-6,
        1e-9,
        vec![
            c(1, "Constant", "1", 50.0),
            blk(2, "Sum", "Sum", 120.0, 55.0, &[("Inputs", "+-")]),
            blk(3, "Integrator", "Integrator", 180.0, 49.0, &[]),
            scope(4, 260.0),
        ],
        vec![
            ln(1, (1, 1), (2, 1)),
            ln(2, (2, 1), (3, 1)),
            ln_branch(3, (3, 1), (4, 1), (2, 2)),
        ],
    ));
    v.push(model(
        "step_integrator",
        2.0,
        0.5,
        1e-6,
        1e-9,
        vec![
            step(1, "1", "1"),
            blk(2, "Integrator", "Integrator", 130.0, 49.0, &[]),
            scope(3, 210.0),
        ],
        vec![ln(1, (1, 1), (2, 1)), ln(2, (2, 1), (3, 1))],
    ));
    v.push(model(
        "accumulator",
        5.0,
        1.0,
        1e-3,
        1e-6,
        vec![
            c(1, "Constant", "1", 50.0),
            blk(2, "Sum", "Sum", 120.0, 55.0, &[("Inputs", "++")]),
            blk(
                3,
                "UnitDelay",
                "Unit Delay",
                180.0,
                50.0,
                &[("SampleTime", "1")],
            ),
            scope(4, 260.0),
        ],
        vec![
            ln(1, (1, 1), (2, 1)),
            ln(2, (2, 1), (3, 1)),
            ln_branch(3, (3, 1), (4, 1), (2, 2)),
        ],
    ));
    let tf = |name: &str, num: &str, den: &str| {
        model(
            name,
            2.0,
            0.5,
            1e-6,
            1e-9,
            vec![
                step(1, "0", "1"),
                blk(
                    2,
                    "TransferFcn",
                    "Transfer Fcn",
                    120.0,
                    45.0,
                    &[("Numerator", num), ("Denominator", den)],
                ),
                scope(3, 220.0),
            ],
            vec![ln(1, (1, 1), (2, 1)), ln(2, (2, 1), (3, 1))],
        )
    };
    v.push(tf("tf_first_order", "[1]", "[1 1]"));
    v.push(tf("tf_second_order", "[1]", "[1 3 2]"));
    v.push(tf("tf_direct", "[1 0]", "[1 1]"));
    let ss = |name: &str, a: &str, b: &str, cc: &str, d: &str| {
        model(
            name,
            2.0,
            0.5,
            1e-6,
            1e-9,
            vec![
                c(1, "Constant", "1", 50.0),
                blk(
                    2,
                    "StateSpace",
                    "State-Space",
                    120.0,
                    45.0,
                    &[
                        ("A", a),
                        ("B", b),
                        ("C", cc),
                        ("D", d),
                        ("InitialCondition", "0"),
                    ],
                ),
                scope(3, 220.0),
            ],
            vec![ln(1, (1, 1), (2, 1)), ln(2, (2, 1), (3, 1))],
        )
    };
    v.push(ss("ss_first_order", "-1", "1", "1", "0"));
    v.push(ss(
        "ss_second_order",
        "[0 1; -2 -3]",
        "[0; 1]",
        "[1 0]",
        "0",
    ));
    v.push(model(
        "pid_open_loop",
        1.0,
        0.5,
        1e-8,
        1e-10,
        vec![
            c(1, "Constant", "1", 50.0),
            blk(
                2,
                "PIDController",
                "PID Controller",
                120.0,
                45.0,
                &[("P", "2"), ("I", "3"), ("D", "1"), ("N", "10")],
            ),
            scope(3, 220.0),
        ],
        vec![ln(1, (1, 1), (2, 1)), ln(2, (2, 1), (3, 1))],
    ));
    v.push(model(
        "pi_loop",
        2.0,
        0.5,
        1e-8,
        1e-10,
        vec![
            step(1, "0", "1"),
            blk(2, "Sum", "Sum", 110.0, 55.0, &[("Inputs", "+-")]),
            blk(
                3,
                "PIDController",
                "PID Controller",
                160.0,
                45.0,
                &[("P", "1"), ("I", "1"), ("D", "0")],
            ),
            blk(
                4,
                "TransferFcn",
                "Transfer Fcn",
                250.0,
                45.0,
                &[("Numerator", "[1]"), ("Denominator", "[1 1]")],
            ),
            scope(5, 340.0),
        ],
        vec![
            ln(1, (1, 1), (2, 1)),
            ln(2, (2, 1), (3, 1)),
            ln(3, (3, 1), (4, 1)),
            ln_branch(4, (4, 1), (5, 1), (2, 2)),
        ],
    ));
    v.push(model(
        "hybrid_zoh_loop",
        2.0,
        0.5,
        1e-8,
        1e-10,
        vec![
            c(1, "Constant", "1", 50.0),
            blk(2, "Sum", "Sum", 110.0, 55.0, &[("Inputs", "+-")]),
            blk(3, "Integrator", "Integrator", 160.0, 49.0, &[]),
            blk(
                4,
                "UnitDelay",
                "Unit Delay",
                160.0,
                130.0,
                &[("SampleTime", "0.5")],
            ),
            scope(5, 250.0),
        ],
        vec![
            ln(1, (1, 1), (2, 1)),
            ln(2, (2, 1), (3, 1)),
            ln_branch(3, (3, 1), (5, 1), (4, 1)),
            ln(4, (4, 1), (2, 2)),
        ],
    ));
    v.push(model(
        "algebraic_loop_tf",
        2.0,
        0.5,
        1e-6,
        1e-9,
        vec![
            step(1, "0", "1"),
            blk(2, "Sum", "Sum", 110.0, 55.0, &[("Inputs", "+-")]),
            blk(
                3,
                "TransferFcn",
                "Transfer Fcn",
                160.0,
                45.0,
                &[("Numerator", "[1 0]"), ("Denominator", "[1 1]")],
            ),
            scope(4, 250.0),
        ],
        vec![
            ln(1, (1, 1), (2, 1)),
            ln(2, (2, 1), (3, 1)),
            ln_branch(3, (3, 1), (4, 1), (2, 2)),
        ],
    ));
    v.push(model(
        "dti_constant",
        2.0,
        0.5,
        1e-3,
        1e-6,
        vec![
            c(1, "Constant", "1", 50.0),
            blk(
                2,
                "DiscreteIntegrator",
                "Discrete-Time Integrator",
                120.0,
                50.0,
                &[("SampleTime", "0.5")],
            ),
            scope(3, 220.0),
        ],
        vec![ln(1, (1, 1), (2, 1)), ln(2, (2, 1), (3, 1))],
    ));
    v.push(model(
        "sine_integrator",
        2.0,
        1.0,
        1e-8,
        1e-10,
        vec![
            blk(1, "Sin", "Sine Wave", 50.0, 50.0, &[]),
            blk(2, "Integrator", "Integrator", 130.0, 49.0, &[]),
            scope(3, 210.0),
        ],
        vec![ln(1, (1, 1), (2, 1)), ln(2, (2, 1), (3, 1))],
    ));
    v.push(model(
        "clock_gain",
        1.0,
        0.5,
        1e-3,
        1e-6,
        vec![
            blk(1, "Clock", "Clock", 50.0, 50.0, &[]),
            blk(2, "Gain", "Gain", 130.0, 50.0, &[("Gain", "2")]),
            scope(3, 210.0),
        ],
        vec![ln(1, (1, 1), (2, 1)), ln(2, (2, 1), (3, 1))],
    ));
    v.push(model(
        "math_blocks",
        1.0,
        1.0,
        1e-3,
        1e-6,
        vec![
            c(1, "Constant", "-2", 30.0),
            c(2, "Constant1", "3", 110.0),
            blk(3, "Abs", "Abs", 120.0, 20.0, &[]),
            blk(4, "Product", "Product", 120.0, 70.0, &[("Inputs", "*/")]),
            blk(5, "Bias", "Bias", 120.0, 120.0, &[("Bias", "5")]),
            blk(
                6,
                "Saturate",
                "Saturation",
                120.0,
                170.0,
                &[("UpperLimit", "1"), ("LowerLimit", "-1")],
            ),
            blk(7, "Switch", "Switch", 200.0, 220.0, &[("Threshold", "0")]),
            blk(8, "Scope", "Scope", 300.0, 100.0, &[("NumInputPorts", "5")]),
        ],
        vec![
            Line {
                id: LineId(1),
                src: pr(1, 1),
                points: vec![],
                dst: Some(pr(3, 1)),
                name: String::new(),
                branches: vec![
                    Branch {
                        tap: TapPoint { segment: 0, t: 0.5 },
                        points: vec![],
                        dst: Some(pr(4, 2)),
                        branches: vec![],
                    },
                    Branch {
                        tap: TapPoint { segment: 0, t: 0.5 },
                        points: vec![],
                        dst: Some(pr(5, 1)),
                        branches: vec![],
                    },
                    Branch {
                        tap: TapPoint { segment: 0, t: 0.5 },
                        points: vec![],
                        dst: Some(pr(7, 2)),
                        branches: vec![],
                    },
                ],
            },
            Line {
                id: LineId(2),
                src: pr(2, 1),
                points: vec![],
                dst: Some(pr(4, 1)),
                name: String::new(),
                branches: vec![
                    Branch {
                        tap: TapPoint { segment: 0, t: 0.5 },
                        points: vec![],
                        dst: Some(pr(6, 1)),
                        branches: vec![],
                    },
                    Branch {
                        tap: TapPoint { segment: 0, t: 0.5 },
                        points: vec![],
                        dst: Some(pr(7, 1)),
                        branches: vec![],
                    },
                ],
            },
            ln_branch(3, (3, 1), (8, 1), (7, 3)),
            ln(4, (4, 1), (8, 2)),
            ln(5, (5, 1), (8, 3)),
            ln(6, (6, 1), (8, 4)),
            ln(7, (7, 1), (8, 5)),
        ],
    ));
    // Constant(1) -> Subsystem(2){ In1(3) -> Gain(4) -> Out1(5) } -> Scope(6)
    let mut inner = sim_core::model::System::default();
    inner.blocks = vec![
        blk(3, "Inport", "In1", 20.0, 50.0, &[("Port", "1")]),
        blk(4, "Gain", "Gain", 80.0, 42.0, &[("Gain", "3")]),
        blk(5, "Outport", "Out1", 150.0, 50.0, &[("Port", "1")]),
    ];
    inner.lines = vec![ln(3, (3, 1), (4, 1)), ln(4, (4, 1), (5, 1))];
    let mut ss = blk(2, "SubSystem", "Subsystem", 120.0, 45.0, &[]);
    ss.system = Some(inner);
    v.push(model(
        "subsystem_gain",
        1.0,
        0.5,
        1e-3,
        1e-6,
        vec![c(1, "Constant", "2", 50.0), ss, scope(6, 230.0)],
        vec![ln(1, (1, 1), (2, 1)), ln(2, (2, 1), (6, 1))],
    ));
    v
}

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/models")
}

#[test]
fn fixtures_are_up_to_date() {
    let write = std::env::var("SIMPYLINK_WRITE_FIXTURES").is_ok();
    for m in fixtures() {
        let path = dir().join(format!("{}.json", m.name));
        let text = to_json_string(&m);
        if write {
            std::fs::write(&path, &text).unwrap();
        }
        let on_disk = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!("missing fixture {path:?}; run with SIMPYLINK_WRITE_FIXTURES=1")
        });
        assert_eq!(on_disk, text, "fixture {} is stale", m.name);
        assert_eq!(from_json_str(&on_disk).unwrap(), m);
        assert_eq!(to_json_string(&from_json_str(&on_disk).unwrap()), on_disk);
    }
}
