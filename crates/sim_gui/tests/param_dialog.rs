//! Block Parameters dialog tests (M1-10, issue #51).

use sim_core::edit::Command;
use sim_core::ids::BlockId;
use sim_core::path::SystemPath;
use sim_gui::canvas::controller::CanvasAction;
use sim_gui::dialogs::block_params::ParamDialogState;
use sim_gui::dialogs::{DialogButton, DialogOutcome};
use sim_gui::state::AppState;

fn fixture() -> sim_core::model::Model {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("models")
        .join("step_gain_scope.json");
    sim_core::json::load_from_path(&p).expect("fixture loads")
}

fn open_gain() -> ParamDialogState {
    ParamDialogState::open(&fixture(), &SystemPath::root(), BlockId(2)).expect("gain dialog")
}

#[test]
fn open_gain_lists_fields_without_errors() {
    let d = open_gain();
    assert_eq!(d.title, "Block Parameters: Gain");
    let got: Vec<(&str, &str)> = d
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.value.as_str()))
        .collect();
    assert_eq!(got, vec![("Gain", "3"), ("SampleTime", "-1")]);
    assert!(!d.has_errors());
}

#[test]
fn ok_with_change_closes_and_executes() {
    let mut d = open_gain();
    d.set_value(0, "5".into());
    assert_eq!(
        d.changed(),
        vec![("Gain".to_string(), Some("5".to_string()))]
    );
    assert_eq!(
        d.press(DialogButton::Ok),
        DialogOutcome::CloseAndExecute(Command::SetParams {
            path: SystemPath::root(),
            block: BlockId(2),
            params: vec![("Gain".to_string(), Some("5".to_string()))],
        })
    );
}

#[test]
fn invalid_value_keeps_dialog_open() {
    let mut d = open_gain();
    d.set_value(0, "[1 2]".into());
    assert_eq!(d.fields[0].error, Some("expected a scalar".to_string()));
    assert_eq!(d.press(DialogButton::Ok), DialogOutcome::KeepOpen);
}

#[test]
fn apply_executes_then_keeps_open_when_clean() {
    let mut d = open_gain();
    d.set_value(0, "4".into());
    assert_eq!(
        d.press(DialogButton::Apply),
        DialogOutcome::Execute(Command::SetParams {
            path: SystemPath::root(),
            block: BlockId(2),
            params: vec![("Gain".to_string(), Some("4".to_string()))],
        })
    );
    assert_eq!(d.press(DialogButton::Apply), DialogOutcome::KeepOpen);
}

#[test]
fn cancel_closes() {
    let mut d = open_gain();
    assert_eq!(d.press(DialogButton::Cancel), DialogOutcome::Close);
}

#[test]
fn ok_without_change_closes() {
    let mut d = open_gain();
    assert_eq!(d.press(DialogButton::Ok), DialogOutcome::Close);
}

#[test]
fn unknown_block_has_no_dialog() {
    assert!(ParamDialogState::open(&fixture(), &SystemPath::root(), BlockId(99)).is_none());
}

#[test]
fn enum_field_rejects_unknown_value() {
    // Sum is not in the fixture; build a model with one to test the Enum kind.
    let mut m = sim_core::model::Model::new("sum");
    let mut sys = sim_core::model::System::default();
    let b = sim_core::library::new_block(
        &sys,
        "Sum",
        BlockId(1),
        sim_core::geom::Point::new(0.0, 0.0),
    )
    .expect("sum block");
    sys.insert_block_sorted(b);
    m.root = sys;
    let mut d = ParamDialogState::open(&m, &SystemPath::root(), BlockId(1)).expect("sum dialog");
    assert_eq!(d.fields[1].name, "IconShape");
    d.set_value(1, "Round".into());
    assert_eq!(
        d.fields[1].error,
        Some("must be one of: round, rectangular".to_string())
    );
}

#[test]
fn app_state_applies_and_undoes_param_change() {
    let mut st = AppState::new();
    st.doc.model = fixture();
    st.handle_action(CanvasAction::OpenBlockDialog(BlockId(2)));
    let d = st.param_dialog.as_mut().expect("dialog opened");
    d.set_value(0, "5".into());
    st.param_dialog_press(DialogButton::Ok);
    assert!(st.param_dialog.is_none());
    assert_eq!(
        st.doc.model.root.block(BlockId(2)).unwrap().params["Gain"],
        "5"
    );
    st.handle_action(CanvasAction::Undo);
    assert_eq!(
        st.doc.model.root.block(BlockId(2)).unwrap().params["Gain"],
        "3"
    );
}
