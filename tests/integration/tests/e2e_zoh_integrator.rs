//! ZOH must hold its sampled input throughout continuous solver stages (#201).
use sim_core::model::SolverKind;
use sim_integration::e2e::{assert_close, simulate_model};
use sim_integration::load_fixture;

fn check_integral(solver: SolverKind, output_step: f64, tolerance: f64) {
    let mut model = load_fixture("zoh_integrator");
    model.config.solver = solver;
    model.config.fixed_step = solver.is_fixed_step().then_some(0.5);
    model.config.output_step = output_step;
    let results = simulate_model(&model).unwrap();
    let expected: Vec<f64> = results
        .time
        .iter()
        .map(|&t| {
            (0..4)
                .map(|k| {
                    let start = k as f64 * 0.5;
                    start.sin() * (t - start).clamp(0.0, 0.5)
                })
                .sum()
        })
        .collect();
    assert_close(&results.channels[0].values, &expected, tolerance);
    let held: Vec<f64> = results
        .time
        .iter()
        .map(|&t| ((t / 0.5).floor() * 0.5).sin())
        .collect();
    assert_close(&results.channels[1].values, &held, 1e-12);
    let final_value = 0.5 * (0.0_f64.sin() + 0.5_f64.sin() + 1.0_f64.sin() + 1.5_f64.sin());
    assert!((results.channels[0].values.last().unwrap() - final_value).abs() <= tolerance);
}

#[test]
fn rk4_integrates_the_held_samples() {
    check_integral(SolverKind::Ode4, 0.5, 1e-9);
}

#[test]
fn adaptive_solver_integrates_the_held_samples() {
    check_integral(SolverKind::Auto, 0.5, 1e-6);
}

#[test]
fn output_times_between_sample_hits_preserve_the_hold() {
    check_integral(SolverKind::Ode4, 0.125, 1e-9);
    check_integral(SolverKind::Auto, 0.125, 1e-6);
}

#[test]
fn simultaneous_delay_updates_do_not_change_the_sampled_input() {
    for solver in [SolverKind::Ode4, SolverKind::Auto] {
        let mut model = load_fixture("zoh_integrator");
        model.config.solver = solver;
        model.config.fixed_step = solver.is_fixed_step().then_some(0.5);
        let source = model.root.blocks.first_mut().unwrap();
        source.block_type = "UnitDelay".into();
        source.params = std::collections::BTreeMap::from([
            ("InitialCondition".into(), "1".into()),
            ("SampleTime".into(), "0.5".into()),
        ]);
        let results = simulate_model(&model).unwrap();
        // ZOH samples the delay's old output (1) before both states update at t=0.
        assert_close(
            &results.channels[0].values,
            &[0.0, 0.5, 0.5, 0.5, 0.5],
            1e-6,
        );
        assert_close(
            &results.channels[1].values,
            &[1.0, 0.0, 0.0, 0.0, 0.0],
            1e-12,
        );
    }
}
