//! Python export from `AppState` (Issue #70 §H).
use sim_gui::sim::SimSession;
use sim_gui::state::AppState;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/models")
        .join(name)
}

#[test]
fn export_python_writes_the_generated_source() {
    let mut st = AppState::new();
    st.open_path(&fixture("step_gain_scope.json"))
        .expect("open fixture");
    let expected = SimSession::update_diagram(&st.doc.model)
        .1
        .expect("generated code")
        .source;
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a");
    st.export_python(&path).expect("export");
    let written = std::fs::read_to_string(dir.path().join("a.py")).expect("read export");
    assert_eq!(written, expected);
    assert_eq!(
        st.status,
        format!("Exported {}", dir.path().join("a.py").display())
    );
}

#[test]
fn export_python_refuses_models_with_compile_errors() {
    let mut st = AppState::new();
    st.open_path(&fixture("algebraic_loop.json"))
        .expect("open fixture");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("b.py");
    let err = st.export_python(&path).expect_err("compile error");
    assert!(err.contains("E_ALGEBRAIC_LOOP"), "{err}");
    assert!(!path.exists(), "no file is written on error");
}

#[test]
fn moving_blocks_increments_the_revision() {
    let mut st = AppState::new();
    st.open_path(&fixture("step_gain_scope.json"))
        .expect("open fixture");
    let before = st.revision;
    assert!(st.execute(sim_core::edit::Command::MoveBlocks {
        path: st.doc.path.clone(),
        blocks: vec![sim_core::ids::BlockId(1)],
        dx: 10.0,
        dy: 5.0,
    }));
    assert_eq!(st.revision, before + 1);
}
