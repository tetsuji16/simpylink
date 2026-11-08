//! Unit Delay E2E tests (Issue #75 §H/§J).
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
fn unit_delay_holds_the_previous_sample() {
    let mut m = load_fixture("step_gain_scope");
    set(&mut m, 1, "Time", "0.5");
    set(&mut m, 1, "After", "1");
    retype(
        &mut m,
        2,
        "UnitDelay",
        &[("InitialCondition", "0"), ("SampleTime", "1")],
    );
    m.config.stop_time = 3.0;
    let r = simulate_model(&m).unwrap();
    assert_close(&r.time, &[0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0], 1e-12);
    assert_close(
        &r.channels[0].values,
        &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        1e-12,
    );
}

#[test]
fn inherited_sample_time_from_constant_is_an_error() {
    let mut m = load_fixture("gain_constant");
    retype(
        &mut m,
        2,
        "UnitDelay",
        &[("InitialCondition", "0"), ("SampleTime", "-1")],
    );
    let err = simulate_model(&m).unwrap_err();
    assert!(err.contains("E_SAMPLE_TIME"), "{err}");
    assert!(err.contains("needs a discrete sample time"), "{err}");
}

#[test]
fn zero_sample_time_is_an_error() {
    let mut m = load_fixture("accumulator");
    set(&mut m, 3, "SampleTime", "0");
    let err = simulate_model(&m).unwrap_err();
    assert!(err.contains("E_SAMPLE_TIME"), "{err}");
}
