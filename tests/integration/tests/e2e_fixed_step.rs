//! Fixed-step solver E2E tests (Issue #159 §H/§I).
use sim_core::model::{Model, SolverKind};
use sim_integration::e2e::{assert_close, simulate_model};
use sim_integration::load_fixture;

/// first_order_lag (dx/dt = 1 - x, x(0) = 0) at stop 2 with h = 0.5.
fn lag(solver: SolverKind) -> Model {
    let mut m = load_fixture("first_order_lag");
    m.config.stop_time = 2.0;
    m.config.output_step = 0.5;
    m.config.solver = solver;
    m.config.fixed_step = Some(0.5);
    m
}

#[test]
fn fixed_step_solvers_match_the_design_table() {
    let cases: [(SolverKind, [f64; 4]); 5] = [
        (SolverKind::Ode1, [0.5, 0.75, 0.875, 0.9375]),
        (
            SolverKind::Ode2,
            [0.375, 0.609375, 0.755859375, 0.847412109375],
        ),
        (
            SolverKind::Ode3,
            [
                0.3958333333333333,
                0.6349826388888888,
                0.779468677662037,
                0.8667623260874807,
            ],
        ),
        (
            SolverKind::Ode4,
            [
                0.39322916666666663,
                0.6318291558159722,
                0.7766046700654207,
                0.8644502294928204,
            ],
        ),
        (
            SolverKind::Ode5,
            [
                0.3934635416666667,
                0.6321135247124567,
                0.7768634402103601,
                0.8646595413005078,
            ],
        ),
    ];
    for (solver, v) in cases {
        let r = simulate_model(&lag(solver)).unwrap_or_else(|e| panic!("{solver:?}: {e}"));
        assert_close(&r.time, &[0.0, 0.5, 1.0, 1.5, 2.0], 1e-12);
        assert_close(&r.channels[0].values, &[0.0, v[0], v[1], v[2], v[3]], 1e-12);
    }
}

#[test]
fn discrete_solver_matches_the_variable_step_values() {
    let auto = simulate_model(&load_fixture("accumulator")).unwrap();
    let mut m = load_fixture("accumulator");
    m.config.solver = SolverKind::FixedDiscrete;
    let disc = simulate_model(&m).unwrap();
    assert_close(&disc.time, &auto.time, 1e-12);
    for (a, b) in disc.channels.iter().zip(&auto.channels) {
        assert_close(&a.values, &b.values, 1e-12);
    }
    assert_close(
        &disc.channels[0].values,
        &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        1e-12,
    );
}

#[test]
fn auto_step_uses_fifty_steps_without_discrete_rates() {
    let mut m = load_fixture("first_order_lag");
    m.config.solver = SolverKind::Ode4;
    let ir = sim_ir::compile(&m).unwrap().ir;
    assert_eq!(ir.config.fixed_step, Some(0.02));
}
