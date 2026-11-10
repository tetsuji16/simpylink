//! Math Function and Sqrt E2E tests (Issue #168 §H/§J).
use sim_core::diag::codes::E_INVALID_PARAM;
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

/// Constant(value) → Math(operator) → Scope.
fn constant_through_math(value: &str, operator: &str) -> Model {
    let mut m = Model::new("math_runtime");
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
    constant.params.insert("Value".into(), value.into());
    let mut math = new_block(&System::default(), "Math", BlockId(2), Point::new(0.0, 0.0)).unwrap();
    math.name = "Math Function".into();
    math.params.insert("Operator".into(), operator.into());
    let mut scope = new_block(
        &System::default(),
        "Scope",
        BlockId(3),
        Point::new(0.0, 0.0),
    )
    .unwrap();
    scope.name = "Scope".into();
    m.root.blocks = vec![constant, math, scope];
    m.root.lines = vec![line(1, 1, 2), line(2, 2, 3)];
    m
}

#[test]
fn math_and_sqrt_values() {
    let r = simulate_model(&load_fixture("math_sqrt")).unwrap();
    assert_close(&r.time, &[0.0, 1.0], 1e-12);
    let expected = [
        1.6487212707001282,
        std::f64::consts::LN_2,
        3.1622776601683795,
        5.0,
        -1.0,
        2.0,
        -2.0,
        9.0,
    ];
    for (channel, want) in r.channels.iter().zip(expected) {
        assert_close(&channel.values, &[want, want], 1e-12);
    }
}

/// rem follows the sign of the dividend and mod the sign of the divisor (§K).
#[test]
fn rem_and_mod_differ_for_a_negative_dividend() {
    let r = simulate_model(&load_fixture("math_sqrt")).unwrap();
    assert_close(&r.channels[4].values, &[-1.0, -1.0], 1e-12);
    assert_close(&r.channels[5].values, &[2.0, 2.0], 1e-12);
}

#[test]
fn log_of_a_negative_value_fails_at_runtime() {
    let err = simulate_model(&constant_through_math("-1", "log")).unwrap_err();
    // Python 3.10 reports "math domain error"; newer versions name the input.
    assert!(
        err.contains("ValueError")
            && (err.contains("math domain error")
                || err.contains("range from")
                || err.contains("expected a positive input")),
        "{err}"
    );
}

/// Simulink returns inf for a reciprocal of zero; Python raises (§J).
#[test]
fn reciprocal_of_zero_fails_at_runtime() {
    let err = simulate_model(&constant_through_math("0", "reciprocal")).unwrap_err();
    assert!(err.contains("ZeroDivisionError"), "{err}");
}

#[test]
fn rem_by_zero_returns_the_dividend() {
    let mut m = constant_through_math("-7", "rem");
    let mut zero = new_block(
        &System::default(),
        "Constant",
        BlockId(4),
        Point::new(0.0, 0.0),
    )
    .unwrap();
    zero.name = "Constant1".into();
    zero.params.insert("Value".into(), "0".into());
    m.root.blocks.push(zero);
    m.root.lines.push(Line {
        id: LineId(3),
        src: PortRef {
            block: BlockId(4),
            port: 1,
        },
        points: vec![],
        dst: Some(PortRef {
            block: BlockId(2),
            port: 2,
        }),
        branches: vec![],
        name: String::new(),
    });
    let r = simulate_model(&m).unwrap();
    assert_close(&r.channels[0].values, &[-7.0, -7.0], 1e-12);
}

#[test]
fn block_ports_follow_the_operator() {
    let spec = sim_core::library::spec("Math").unwrap();
    let mut params = std::collections::BTreeMap::new();
    params.insert("Operator".to_string(), "hypot".to_string());
    assert_eq!((spec.ports)(&params).unwrap().inputs, 2);
    params.insert("Operator".to_string(), "exp".to_string());
    assert_eq!((spec.ports)(&params).unwrap().inputs, 1);
    let sqrt = sim_core::library::spec("Sqrt").unwrap();
    assert_eq!(
        (sqrt.ports)(&std::collections::BTreeMap::new())
            .unwrap()
            .inputs,
        1
    );
}

/// The enum is checked by validation before lowering, so an unknown value is E_INVALID_PARAM.
#[test]
fn unknown_operators_are_rejected() {
    let mut m = load_fixture("math_sqrt");
    set(&mut m, 2, "Operator", "cube");
    let err = sim_ir::compile(&m).unwrap_err();
    assert!(
        err.diagnostics
            .iter()
            .any(|d| d.code == E_INVALID_PARAM && d.message.contains("Operator")),
        "{:?}",
        err.diagnostics
    );
}
