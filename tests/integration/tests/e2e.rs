//! End-to-end numerical tests (analytic or design-defined expectations; not Simulink-verified).
use sim_integration::e2e::{assert_close, simulate_fixture, simulate_model};
use sim_integration::load_fixture;

fn ch(name: &str, ch: &str) -> (Vec<f64>, Vec<f64>) {
    let r = simulate_fixture(name);
    (
        r.time.clone(),
        r.channel(ch)
            .unwrap_or_else(|| panic!("{name}: no {ch}"))
            .values
            .clone(),
    )
}
fn set(m: &mut sim_core::model::Model, sid: u32, k: &str, v: &str) {
    m.root
        .blocks
        .iter_mut()
        .find(|b| b.sid.0 == sid)
        .unwrap()
        .params
        .insert(k.into(), v.into());
}

#[test]
fn basic_mvp() {
    let (t, y) = ch("gain_constant", "Scope/1");
    assert_close(&t, &[0.0, 0.5, 1.0], 1e-12);
    assert_close(&y, &[6.0; 3], 1e-12);
    assert_close(&ch("sum_constants", "Out1").1, &[2.5, 2.5], 1e-12);
    let (t, y) = ch("step_gain_scope", "Scope/1");
    assert_close(&t, &[0.0, 0.5, 1.0, 1.5, 2.0], 1e-12);
    assert_close(&y, &[0.0, 0.0, 6.0, 6.0, 6.0], 1e-12);
    let mut m = load_fixture("step_gain_scope");
    set(&mut m, 1, "Time", "0.75");
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[0.0, 0.0, 6.0, 6.0, 6.0],
        1e-12,
    );
    let mut m = load_fixture("step_gain_scope");
    m.config.output_step = 0.3;
    let r = simulate_model(&m).unwrap();
    assert_close(&r.time, &[0.0, 0.3, 0.6, 0.9, 1.2, 1.5, 1.8, 2.0], 1e-12);
    assert_close(
        &r.channels[0].values,
        &[0.0, 0.0, 0.0, 0.0, 6.0, 6.0, 6.0, 6.0],
        1e-12,
    );
    let e = simulate_model(&load_fixture("algebraic_loop")).unwrap_err();
    assert!(
        e.contains("E_ALGEBRAIC_LOOP") && e.contains("Gain(2) -> Sum(3) -> Gain(2)"),
        "{e}"
    );
    let mut m = load_fixture("step_gain_scope");
    set(&mut m, 2, "Gain", "K");
    assert!(simulate_model(&m).unwrap_err().contains("E_INVALID_PARAM"));
}

#[test]
fn continuous() {
    assert_close(
        &ch("const_integrator", "Scope/1").1,
        &[0.0, 0.5, 1.0, 1.5, 2.0],
        1e-9,
    );
    let (t, y) = ch("first_order_lag", "Scope/1");
    let exact: Vec<f64> = t.iter().map(|t| 1.0 - (-t).exp()).collect();
    assert_close(&y, &exact, 1e-4);
    assert!((y[10] - 0.6321205588).abs() < 1e-4);
    assert_close(
        &ch("step_integrator", "Scope/1").1,
        &[0.0, 0.0, 0.0, 0.5, 1.0],
        1e-9,
    );
    let mut m = load_fixture("const_integrator");
    set(&mut m, 2, "InitialCondition", "2.5");
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[2.5, 3.0, 3.5, 4.0, 4.5],
        1e-9,
    );
}

#[test]
fn transfer_fcn_and_state_space() {
    let e1 = |t: f64| 1.0 - (-t).exp();
    let e2 = |t: f64| 0.5 - (-t).exp() + 0.5 * (-2.0 * t).exp();
    let at1 = |name: &str| ch(name, "Scope/1").1[2];
    assert!((at1("tf_first_order") - e1(1.0)).abs() < 1e-5);
    assert!((at1("tf_second_order") - e2(1.0)).abs() < 1e-5);
    let y = ch("tf_direct", "Scope/1").1;
    assert!((y[0] - 1.0).abs() < 1e-9 && (y[2] - (-1.0f64).exp()).abs() < 1e-5);
    assert!((at1("ss_first_order") - e1(1.0)).abs() < 1e-5);
    assert!((at1("ss_second_order") - e2(1.0)).abs() < 1e-5);
    let mut m = load_fixture("ss_first_order");
    set(&mut m, 2, "InitialCondition", "1");
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[1.0; 5],
        1e-6,
    );
    let e = simulate_model(&load_fixture("algebraic_loop_tf")).unwrap_err();
    assert!(
        e.contains("E_ALGEBRAIC_LOOP") && e.contains("Sum(2)") && e.contains("Transfer Fcn(3)"),
        "{e}"
    );
}

#[test]
fn discrete() {
    assert_close(
        &ch("accumulator", "Scope/1").1,
        &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        1e-12,
    );
    let mut m = load_fixture("accumulator");
    set(&mut m, 3, "InitialCondition", "10");
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0],
        1e-12,
    );
    let mut m = load_fixture("accumulator");
    set(&mut m, 3, "SampleTime", "[1, 0.5]");
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        1e-12,
    );
    let mut m = load_fixture("accumulator");
    set(&mut m, 3, "SampleTime", "-1");
    assert!(simulate_model(&m).unwrap_err().contains("E_SAMPLE_TIME"));
    assert_close(
        &ch("dti_constant", "Scope/1").1,
        &[0.0, 0.5, 1.0, 1.5, 2.0],
        1e-12,
    );
    let mut m = load_fixture("dti_constant");
    set(&mut m, 2, "InitialCondition", "1");
    m.config.output_step = 0.25;
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[1.0, 1.5, 1.5, 2.0, 2.0, 2.5, 2.5, 3.0, 3.0],
        1e-12,
    );
    let mut m = load_fixture("dti_constant");
    set(&mut m, 2, "IntegratorMethod", "Integration: Trapezoidal");
    assert!(simulate_model(&m)
        .unwrap_err()
        .contains("E_UNSUPPORTED_FEATURE"));
}

#[test]
fn pid_and_feedback() {
    let y = ch("pid_open_loop", "Scope/1").1;
    assert_close(&y, &[12.0, 3.5673794700, 5.0004539993], 1e-5);
    let mut m = load_fixture("pid_open_loop");
    set(&mut m, 2, "Controller", "PI");
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[2.0, 3.5, 5.0],
        1e-6,
    );
    let (t, y) = ch("pi_loop", "Scope/1");
    let exact: Vec<f64> = t.iter().map(|t| 1.0 - (-t).exp()).collect();
    assert_close(&y, &exact, 1e-5);
    assert_close(
        &ch("hybrid_zoh_loop", "Scope/1").1,
        &[0.0, 0.5, 0.75, 0.875, 0.9375],
        1e-7,
    );
}

#[test]
fn sources() {
    let y = ch("sine_integrator", "Scope/1").1;
    assert_close(&y, &[0.0, 1.0 - 1f64.cos(), 1.0 - 2f64.cos()], 1e-6);
    let mut m = load_fixture("sine_integrator");
    m.root.blocks.retain(|b| b.sid.0 != 2);
    m.root.lines = vec![sim_core::model::Line {
        id: sim_core::ids::LineId(1),
        src: sim_core::model::PortRef {
            block: sim_core::ids::BlockId(1),
            port: 1,
        },
        points: vec![],
        dst: Some(sim_core::model::PortRef {
            block: sim_core::ids::BlockId(3),
            port: 1,
        }),
        branches: vec![],
        name: String::new(),
    }];
    set(&mut m, 1, "Amplitude", "2");
    set(&mut m, 1, "Bias", "1");
    set(&mut m, 1, "Frequency", "pi");
    m.config.output_step = 0.5;
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[1.0, 3.0, 1.0, -1.0, 1.0],
        1e-12,
    );
    assert_close(&ch("clock_gain", "Scope/1").1, &[0.0, 1.0, 2.0], 1e-12);
}

#[test]
fn math_blocks_and_subsystem() {
    let r = simulate_fixture("math_blocks");
    let got: Vec<f64> = (1..=5)
        .map(|p| r.channel(&format!("Scope/{p}")).unwrap().values[0])
        .collect();
    // Abs(-2), 3 / -2, -2 + 5, sat(3, -1, 1), switch(u2=-2 >= 0 ? 3 : Abs)
    assert_close(&got, &[2.0, -1.5, 3.0, 1.0, 2.0], 1e-12);
    assert_close(&ch("subsystem_gain", "Scope/1").1, &[6.0, 6.0, 6.0], 1e-12);
}
