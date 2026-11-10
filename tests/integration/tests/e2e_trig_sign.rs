//! Trigonometric Function and Sign E2E tests (Issue #167 §H/§J).
use sim_core::diag::codes::E_UNSUPPORTED_FEATURE;
use sim_core::geom::Point;
use sim_core::ids::{BlockId, LineId};
use sim_core::library::new_block;
use sim_core::model::{Line, Model, PortRef, System};
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

#[test]
fn trig_and_sign_values() {
    let r = simulate_model(&load_fixture("trig_sign")).unwrap();
    assert_close(&r.time, &[0.0, 0.5, 1.0], 1e-12);
    assert_close(
        &r.channels[0].values,
        &[0.0, 0.479425538604203, 0.8414709848078965],
        1e-12,
    );
    assert_close(&r.channels[1].values, &[0.4636476090008061; 3], 1e-12);
    assert_close(&r.channels[2].values, &[-1.0, 0.0, 1.0], 1e-12);
    assert_close(
        &r.channels[3].values,
        &[0.0, 0.46211715726000974, 0.7615941559557649],
        1e-12,
    );
}

#[test]
fn sincos_is_rejected() {
    let mut m = load_fixture("trig_sign");
    set(&mut m, 2, "Operator", "sincos");
    let err = sim_ir::compile(&m).unwrap_err();
    assert!(
        err.diagnostics
            .iter()
            .any(|d| d.code == E_UNSUPPORTED_FEATURE
                && d.message.contains("operator 'sincos' is not supported")),
        "{:?}",
        err.diagnostics
    );
}

/// Constant(2) → asin → Scope.
fn asin_out_of_domain() -> Model {
    let mut m = Model::new("asin_domain");
    m.config.stop_time = 0.5;
    m.config.output_step = 0.5;
    let mut constant = new_block(
        &System::default(),
        "Constant",
        BlockId(1),
        Point::new(0.0, 0.0),
    )
    .unwrap();
    constant.name = "Constant".into();
    constant.params.insert("Value".into(), "2".into());
    let mut trig = new_block(
        &System::default(),
        "Trigonometry",
        BlockId(2),
        Point::new(0.0, 0.0),
    )
    .unwrap();
    trig.name = "Trigonometric Function".into();
    trig.params.insert("Operator".into(), "asin".into());
    let mut scope = new_block(
        &System::default(),
        "Scope",
        BlockId(3),
        Point::new(0.0, 0.0),
    )
    .unwrap();
    scope.name = "Scope".into();
    m.root.blocks = vec![constant, trig, scope];
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
    m
}

#[test]
fn asin_outside_its_domain_fails_at_runtime() {
    let err = simulate_model(&asin_out_of_domain()).unwrap_err();
    // Python 3.10 reports "math domain error"; newer versions name the range.
    assert!(
        err.contains("ValueError")
            && (err.contains("math domain error") || err.contains("range from -1")),
        "{err}"
    );
}

#[test]
fn cordic_approximation_is_ignored_with_a_warning() {
    let mut m = load_fixture("trig_sign");
    set(&mut m, 2, "ApproximationMethod", "CORDIC");
    let compiled = sim_ir::compile(&m).unwrap();
    assert!(compiled
        .diagnostics
        .iter()
        .any(|d| d.code == "W_DATA_TYPE_IGNORED" && d.message.contains("CORDIC")));
}

#[test]
fn block_ports_follow_the_operator() {
    let spec = sim_core::library::spec("Trigonometry").unwrap();
    let mut params = std::collections::BTreeMap::new();
    params.insert("Operator".to_string(), "atan2".to_string());
    assert_eq!((spec.ports)(&params).unwrap().inputs, 2);
    params.insert("Operator".to_string(), "sin".to_string());
    assert_eq!((spec.ports)(&params).unwrap().inputs, 1);
}
