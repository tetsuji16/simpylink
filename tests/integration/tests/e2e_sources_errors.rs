//! Sine Wave unsupported modes (Issue #80 §J).
use sim_core::model::Model;
use sim_integration::e2e::simulate_model;
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
fn sine_wave_unsupported_modes() {
    let mut m = load_fixture("sine_integrator");
    set(&mut m, 1, "SineType", "Sample based");
    assert!(simulate_model(&m)
        .unwrap_err()
        .contains("E_UNSUPPORTED_FEATURE"));

    let mut m = load_fixture("sine_integrator");
    set(&mut m, 1, "SampleTime", "0.1");
    assert!(simulate_model(&m)
        .unwrap_err()
        .contains("E_UNSUPPORTED_FEATURE"));
}
