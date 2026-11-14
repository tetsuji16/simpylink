//! Zero-Order Hold E2E tests (Issue #166 §H/§J).
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
fn zoh_holds_the_sampled_value() {
    let r = simulate_model(&load_fixture("zoh_clock")).unwrap();
    assert_close(
        &r.time,
        &[0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0],
        1e-12,
    );
    assert_close(
        &r.channels[0].values,
        &[0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.5, 1.5, 2.0],
        1e-12,
    );
}

#[test]
fn inherited_continuous_rate_passes_the_input_through() {
    let mut m = load_fixture("zoh_clock");
    set(&mut m, 2, "SampleTime", "-1");
    let r = simulate_model(&m).unwrap();
    assert_close(
        &r.channels[0].values,
        &[0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0],
        1e-12,
    );
}

#[test]
fn sample_hits_respect_the_offset() {
    let mut m = load_fixture("zoh_clock");
    set(&mut m, 2, "SampleTime", "[0.5, 0.1]");
    m.config.stop_time = 0.6;
    m.config.output_step = 0.1;
    let r = simulate_model(&m).unwrap();
    assert_close(&r.time, &[0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6], 1e-12);
    assert_close(
        &r.channels[0].values,
        &[0.0, 0.1, 0.1, 0.1, 0.1, 0.1, 0.6],
        1e-12,
    );
}
