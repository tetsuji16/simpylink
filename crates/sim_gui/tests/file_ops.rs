//! File operation tests (M1-12, issue #53).

use sim_core::edit::Command;
use sim_core::ids::BlockId;
use sim_core::path::SystemPath;
use sim_gui::canvas::controller::CanvasAction;
use sim_gui::state::{AppState, PendingAction, SaveResult};

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("models")
        .join("step_gain_scope.json")
}

fn opened() -> AppState {
    let mut st = AppState::new();
    st.open_path(&fixture_path()).expect("fixture opens");
    st
}

fn nudge(st: &mut AppState) {
    st.handle_action(CanvasAction::Execute(Command::MoveBlocks {
        path: SystemPath::root(),
        blocks: vec![BlockId(2)],
        dx: 10.0,
        dy: 0.0,
    }));
}

#[test]
fn open_fixture_is_clean_and_has_file() {
    let st = opened();
    assert_eq!(st.doc.model.root.blocks.len(), 3);
    assert!(!st.doc.history.is_dirty());
    assert!(st.doc.file.is_some());
}

#[test]
fn save_as_writes_file_and_clears_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = opened();
    nudge(&mut st);
    assert!(st.doc.history.is_dirty());
    st.save_as(&dir.path().join("a")).expect("save_as");
    let written = dir.path().join("a.json");
    assert!(written.is_file());
    assert!(!st.doc.history.is_dirty());
    let reloaded = sim_core::json::load_from_path(&written).unwrap();
    assert_eq!(reloaded, st.doc.model);
}

#[test]
fn second_save_uses_the_same_path() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = opened();
    st.save_as(&dir.path().join("b.json")).expect("save_as");
    nudge(&mut st);
    assert_eq!(st.save(), Ok(SaveResult::Saved));
}

#[test]
fn untitled_document_needs_a_path() {
    let mut st = AppState::new();
    st.new_document();
    assert_eq!(st.doc.model.name, "untitled");
    assert_eq!(st.doc.model.root.blocks.len(), 0);
    assert_eq!(st.save(), Ok(SaveResult::NeedsPath));
}

#[test]
fn request_new_when_dirty_asks_for_confirmation() {
    let mut st = opened();
    nudge(&mut st);
    assert!(!st.request(PendingAction::New));
    assert_eq!(st.confirm, Some(PendingAction::New));
    assert_eq!(st.doc.model.root.blocks.len(), 3);

    let mut clean = opened();
    assert!(clean.request(PendingAction::New));
    assert_eq!(clean.doc.model.name, "untitled");
}

#[test]
fn open_missing_file_keeps_current_model() {
    let mut st = opened();
    let before = st.doc.model.clone();
    let err = st.open_path(std::path::Path::new("nope.json")).unwrap_err();
    assert!(err.contains("Could not open"), "error was {err:?}");
    assert_eq!(st.doc.model, before);
}

#[test]
fn open_invalid_json_reports_parse_error() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.json");
    std::fs::write(&bad, "{").unwrap();
    let mut st = opened();
    let err = st.open_path(&bad).unwrap_err();
    assert!(err.contains("JSON parse error"), "error was {err:?}");
}

#[test]
fn missing_params_are_filled_with_defaults() {
    let dir = tempfile::tempdir().unwrap();
    // A Gain block whose params omit SampleTime.
    let json = r#"{
  "format": "simpylink-model",
  "format_version": 1,
  "name": "no_sample_time",
  "config": {
    "start_time": 0.0,
    "stop_time": 10.0,
    "solver": "auto",
    "rel_tol": 0.001,
    "abs_tol": 1e-6,
    "max_step": null,
    "output_step": 0.01
  },
  "next_sid": 2,
  "next_line_id": 1,
  "root": {
    "blocks": [
      {
        "sid": 1,
        "block_type": "Gain",
        "name": "Gain",
        "position": [0.0, 0.0, 30.0, 30.0],
        "orientation": "right",
        "params": { "Gain": "2" }
      }
    ],
    "lines": []
  }
}"#;
    let path = dir.path().join("no_sample_time.json");
    std::fs::write(&path, json).unwrap();

    let mut st = AppState::new();
    st.open_path(&path).expect("opens");
    let gain = st.doc.model.root.block(BlockId(1)).expect("gain");
    assert_eq!(gain.params.get("SampleTime").map(String::as_str), Some("-1"));
    assert!(!st.doc.history.is_dirty());
}
