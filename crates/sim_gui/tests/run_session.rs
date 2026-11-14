//! GUI run path without egui: AppState → SimSession → Python → results → scope series.
use sim_core::edit::Command;
use sim_core::ids::BlockId;
use sim_core::path::SystemPath;
use sim_gui::canvas::controller::{CanvasEvent, CanvasKey};
use sim_gui::canvas::view::Modifiers;
use sim_gui::scope::scope_series;
use sim_gui::state::AppState;
use sim_runner::{Channel, SimResults};
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

// ---------------------------------------------------------------- stale results

fn dummy_results() -> SimResults {
    SimResults {
        model: "step_gain_scope".into(),
        time: vec![0.0, 1.0],
        channels: vec![Channel {
            name: "Scope/1".into(),
            block_sid: 3,
            port: 1,
            values: vec![0.0, 6.0],
        }],
    }
}

fn move_gain(st: &mut AppState) {
    assert!(st.execute(Command::MoveBlocks {
        path: SystemPath::root(),
        blocks: vec![BlockId(2)],
        dx: 10.0,
        dy: 0.0,
    }));
}

fn run_to_completion(st: &mut AppState) {
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
}

#[test]
fn editing_after_a_run_marks_results_stale() {
    let mut st = AppState::new();
    st.open_path(&fixture("step_gain_scope.json"))
        .expect("open fixture");
    st.results = Some(dummy_results());
    st.results_revision = Some(st.revision);
    assert!(!st.results_are_stale());
    move_gain(&mut st);
    assert!(
        st.results_are_stale(),
        "moving a block must mark results stale"
    );
    assert!(st.results.is_some(), "results must not be cleared");
}

#[test]
fn undo_does_not_clear_stale() {
    let mut st = AppState::new();
    st.open_path(&fixture("step_gain_scope.json"))
        .expect("open fixture");
    st.results = Some(dummy_results());
    st.results_revision = Some(st.revision);
    move_gain(&mut st);
    st.undo();
    assert!(st.results_are_stale(), "undo keeps the simple stale rule");
    assert!(st.results.is_some());
}

#[test]
fn no_results_is_never_stale() {
    let mut st = AppState::new();
    st.open_path(&fixture("step_gain_scope.json"))
        .expect("open fixture");
    assert!(!st.results_are_stale());
    move_gain(&mut st);
    assert!(!st.results_are_stale());
}

#[test]
fn selection_and_zoom_do_not_mark_results_stale() {
    let mut st = AppState::new();
    st.open_path(&fixture("step_gain_scope.json"))
        .expect("open fixture");
    st.results = Some(dummy_results());
    st.results_revision = Some(st.revision);
    let rev = st.revision;
    for key in [CanvasKey::A, CanvasKey::Plus, CanvasKey::Minus] {
        let actions = st.canvas.handle(
            CanvasEvent::Key {
                key,
                pressed: true,
                mods: Modifiers {
                    ctrl: true,
                    ..Default::default()
                },
            },
            &st.doc.model,
            &st.doc.path,
        );
        st.handle_actions(actions);
    }
    assert_eq!(st.revision, rev, "selection/zoom must not bump revision");
    assert!(!st.results_are_stale());
}

#[test]
fn new_document_clears_results_revision() {
    let mut st = AppState::new();
    st.open_path(&fixture("step_gain_scope.json"))
        .expect("open fixture");
    st.results = Some(dummy_results());
    st.results_revision = Some(st.revision);
    st.new_document();
    assert!(st.results.is_none());
    assert_eq!(st.results_revision, None);
    assert!(!st.results_are_stale());
}

#[test]
fn run_records_revision_and_rerun_clears_stale() {
    let mut st = AppState::new();
    st.open_path(&fixture("step_gain_scope.json"))
        .expect("open fixture");
    // Unique run directory (pid + per-session counter), like the lifecycle tests.
    st.doc.model.name = "stale_rerun".into();
    run_to_completion(&mut st);
    assert!(st.results.is_some(), "no results: {}", st.status);
    assert_eq!(st.results_revision, Some(st.revision));
    assert!(!st.results_are_stale());
    move_gain(&mut st);
    assert!(st.results_are_stale());
    run_to_completion(&mut st);
    assert!(
        !st.results_are_stale(),
        "a new run must clear the stale flag"
    );
    assert_eq!(st.results_revision, Some(st.revision));
}
