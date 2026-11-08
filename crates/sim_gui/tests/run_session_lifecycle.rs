//! SimSession lifecycle tests (Issue #67 §H-1).
use sim_core::json::load_from_path;
use sim_gui::sim::{run_directory, SimSession, SimUpdate, StartError};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/models")
        .join(name)
}

#[test]
fn update_diagram_returns_code_or_diagnostics() {
    let model = load_from_path(&fixture("step_gain_scope.json")).expect("load fixture");
    let (diags, code) = SimSession::update_diagram(&model);
    assert!(diags.is_empty(), "{diags:?}");
    let code = code.expect("generated code");
    assert_eq!(code.file_name, "step_gain_scope.py");
    assert!(code.source.contains("MODEL_NAME = \"step_gain_scope\""));

    let model = load_from_path(&fixture("algebraic_loop.json")).expect("load fixture");
    let (diags, code) = SimSession::update_diagram(&model);
    assert!(code.is_none());
    let loops: Vec<_> = diags
        .iter()
        .filter(|d| d.code == "E_ALGEBRAIC_LOOP")
        .collect();
    assert_eq!(loops.len(), 1, "{diags:?}");
}

#[test]
fn start_twice_reports_already_running() {
    let mut model = load_from_path(&fixture("step_gain_scope.json")).expect("load fixture");
    model.name = "start_twice".into();
    let mut sim = SimSession::new();
    sim.start(&model).expect("first start");
    assert_eq!(sim.start(&model), Err(StartError::AlreadyRunning));
    sim.stop();
    let t0 = Instant::now();
    while sim.is_running() && t0.elapsed() < Duration::from_secs(30) {
        sim.poll();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!sim.is_running(), "run did not finish");
}

#[test]
fn start_algebraic_loop_is_a_compile_error() {
    let model = load_from_path(&fixture("algebraic_loop.json")).expect("load fixture");
    let mut sim = SimSession::new();
    match sim.start(&model) {
        Err(StartError::Compile(diags)) => {
            assert!(
                diags.iter().any(|d| d.code == "E_ALGEBRAIC_LOOP"),
                "{diags:?}"
            );
        }
        other => panic!("expected Compile error, got {other:?}"),
    }
    assert!(!sim.is_running());
}

#[test]
fn long_run_can_be_stopped() {
    let mut model = load_from_path(&fixture("step_gain_scope.json")).expect("load fixture");
    model.name = "long_run_stop".into();
    model.config.stop_time = 1e7;
    model.config.output_step = 1e-3;
    let mut sim = SimSession::new();
    sim.start(&model).expect("start long run");
    std::thread::sleep(Duration::from_secs(1));
    sim.stop();
    let t0 = Instant::now();
    let mut done = None;
    while done.is_none() && t0.elapsed() < Duration::from_secs(30) {
        if let SimUpdate::Done { summary, .. } = sim.poll() {
            done = Some(summary);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let summary = done.expect("run did not finish after stop");
    assert_eq!(summary.status, sim_runner::RunStatus::Stopped);
}

#[test]
fn run_directory_has_the_documented_suffix() {
    let dir = run_directory("m", 10, 3);
    assert!(
        dir.ends_with("simpylink/m-10-3"),
        "unexpected run directory {}",
        dir.display()
    );
}
