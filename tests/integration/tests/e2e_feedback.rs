//! Feedback loop E2E tests (Issue #78 §H/§J).
use sim_core::model::Model;
use sim_integration::e2e::{assert_close, simulate_fixture, simulate_model};
use sim_integration::load_fixture;

fn retype(m: &mut Model, sid: u32, block_type: &str, params: &[(&str, &str)]) {
    let b = m.root.blocks.iter_mut().find(|b| b.sid.0 == sid).unwrap();
    b.block_type = block_type.into();
    b.params.clear();
    for (k, v) in params {
        b.params.insert((*k).into(), (*v).into());
    }
}

/// §H: first_order_lag: 全点 1 - e^{-t} (解析解, 許容誤差 1e-5)
#[test]
fn first_order_lag() {
    let r = simulate_fixture("first_order_lag");
    let exact: Vec<f64> = r.time.iter().map(|t| 1.0 - (-t).exp()).collect();
    assert_close(&r.channels[0].values, &exact, 1e-5);
}

/// §H: pi_loop: t=[0, 0.5, 1, 1.5, 2] で 1 - e^{-t} (解析解, 許容誤差 1e-5)
#[test]
fn pi_loop() {
    let r = simulate_fixture("pi_loop");
    assert_close(&r.time, &[0.0, 0.5, 1.0, 1.5, 2.0], 1e-12);
    let exact = [0.0, 0.3934693403, 0.6321205588, 0.7768698399, 0.8646647168];
    assert_close(&r.channels[0].values, &exact, 1e-5);
}

/// §H: hybrid_zoh_loop: [0, 0.5, 0.75, 0.875, 0.9375] (許容誤差 1e-7)
#[test]
fn hybrid_zoh_loop() {
    let r = simulate_fixture("hybrid_zoh_loop");
    assert_close(
        &r.channels[0].values,
        &[0.0, 0.5, 0.75, 0.875, 0.9375],
        1e-7,
    );
}

/// §H: algebraic_tf: algebraic_loop_tf は E_ALGEBRAIC_LOOP かつ "Sum(2)" と "Transfer Fcn(3)" を含む
#[test]
fn algebraic_tf() {
    let m = load_fixture("algebraic_loop_tf");
    let err = simulate_model(&m).unwrap_err();
    assert!(err.contains("E_ALGEBRAIC_LOOP"), "{err}");
    assert!(err.contains("Sum(2)"), "{err}");
    assert!(err.contains("Transfer Fcn(3)"), "{err}");
}

/// §J: pi_loop で PID の P 項をそのまま返す（TF を Gain 0.5 に置換）と E_ALGEBRAIC_LOOP
#[test]
fn pi_loop_gain_replacement_is_algebraic_loop() {
    let mut m = load_fixture("pi_loop");
    retype(&mut m, 4, "Gain", &[("Gain", "0.5"), ("SampleTime", "-1")]);
    let err = simulate_model(&m).unwrap_err();
    assert!(err.contains("E_ALGEBRAIC_LOOP"), "{err}");
}
