//! GUI run path without egui: AppState → SimSession → Python → results → scope series.
use sim_gui::scope::scope_series;
use sim_gui::state::AppState;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/models")
        .join(name)
}

#[test]
fn run_step_gain_scope_from_app_state() {
    let mut st = AppState::new();
    st.open_path(&fixture("step_gain_scope.json"))
        .expect("open fixture");
    st.start_run();
    assert!(
        st.sim.is_running(),
        "status: {} / python: {:?}",
        st.status,
        st.python_error
    );
    let t0 = Instant::now();
    while st.sim.is_running() && t0.elapsed() < Duration::from_secs(60) {
        st.poll_sim();
        std::thread::sleep(Duration::from_millis(50));
    }
    st.poll_sim();
    let r = st
        .results
        .as_ref()
        .unwrap_or_else(|| panic!("no results: {}", st.status));
    assert_eq!(r.time, vec![0.0, 0.5, 1.0, 1.5, 2.0]);
    let s = scope_series(r, sim_core::ids::BlockId(3));
    let ys: Vec<f64> = s[0].points.iter().map(|p| p[1]).collect();
    assert_eq!(ys, vec![0.0, 0.0, 6.0, 6.0, 6.0]);
    assert!(
        st.open_scopes.contains(&sim_core::ids::BlockId(3)),
        "scope opens after the first run"
    );
}

#[test]
fn compile_errors_go_to_diagnostics() {
    let mut st = AppState::new();
    st.open_path(&fixture("algebraic_loop.json"))
        .expect("open fixture");
    st.start_run();
    assert!(!st.sim.is_running());
    assert!(st.diagnostics.iter().any(|d| d.code == "E_ALGEBRAIC_LOOP"));
}
