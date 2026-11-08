//! Discrete-Time Integrator E2E tests (Issue #76 §H/§J).
use sim_core::model::Model;
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
fn discrete_integrator_gain_and_sample_time_error() {
    let mut m = load_fixture("dti_constant");
    set(&mut m, 2, "gainval", "2");
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[0.0, 1.0, 2.0, 3.0, 4.0],
        1e-12,
    );

    let mut m = load_fixture("dti_constant");
    set(&mut m, 2, "SampleTime", "0");
    let err = simulate_model(&m).unwrap_err();
    assert!(err.contains("E_SAMPLE_TIME"), "{err}");
}
