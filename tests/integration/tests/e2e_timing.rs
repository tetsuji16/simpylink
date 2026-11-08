//! Output-grid timing tests (Issue #69 §H).
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
fn step_time_off_grid() {
    let mut m = load_fixture("step_gain_scope");
    set(&mut m, 1, "Time", "0.75");
    let r = simulate_model(&m).unwrap();
    assert_close(&r.time, &[0.0, 0.5, 1.0, 1.5, 2.0], 1e-12);
    assert_close(&r.channels[0].values, &[0.0, 0.0, 6.0, 6.0, 6.0], 1e-12);
}

#[test]
fn output_step_not_divisor() {
    let mut m = load_fixture("step_gain_scope");
    m.config.output_step = 0.3;
    let r = simulate_model(&m).unwrap();
    assert_close(&r.time, &[0.0, 0.3, 0.6, 0.9, 1.2, 1.5, 1.8, 2.0], 1e-12);
    assert_close(
        &r.channels[0].values,
        &[0.0, 0.0, 0.0, 0.0, 6.0, 6.0, 6.0, 6.0],
        1e-12,
    );
}
