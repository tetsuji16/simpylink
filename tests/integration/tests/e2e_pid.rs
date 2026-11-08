//! PID Controller E2E tests (Issue #77 §H/§J).
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

fn retype(m: &mut Model, sid: u32, block_type: &str, params: &[(&str, &str)]) {
    let b = m.root.blocks.iter_mut().find(|b| b.sid.0 == sid).unwrap();
    b.block_type = block_type.into();
    b.params.clear();
    for (k, v) in params {
        b.params.insert((*k).into(), (*v).into());
    }
}

#[test]
fn pid_proportional_integral_and_disabled_terms() {
    let mut m = load_fixture("pid_open_loop");
    set(&mut m, 2, "P", "2");
    set(&mut m, 2, "I", "3");
    set(&mut m, 2, "D", "0");
    set(&mut m, 2, "N", "100");
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[2.0, 3.5, 5.0],
        1e-6,
    );

    let mut m = load_fixture("pid_open_loop");
    set(&mut m, 2, "Controller", "P");
    set(&mut m, 2, "P", "2");
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[2.0, 2.0, 2.0],
        1e-9,
    );
}

#[test]
fn pid_unsupported_form_and_invalid_filter() {
    let mut m = load_fixture("pid_open_loop");
    set(&mut m, 2, "Form", "Ideal");
    assert!(simulate_model(&m)
        .unwrap_err()
        .contains("E_UNSUPPORTED_FEATURE"));

    let mut m = load_fixture("pid_open_loop");
    set(&mut m, 2, "N", "0");
    assert!(simulate_model(&m).unwrap_err().contains("E_PARAM_DOMAIN"));
}

#[test]
fn pid_direct_feedback_is_an_algebraic_loop() {
    let mut m = load_fixture("pi_loop");
    retype(&mut m, 4, "Gain", &[("Gain", "0.5"), ("SampleTime", "-1")]);
    let err = simulate_model(&m).unwrap_err();
    assert!(err.contains("E_ALGEBRAIC_LOOP"), "{err}");
}
