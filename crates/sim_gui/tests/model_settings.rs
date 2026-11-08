//! Model Settings dialog state (Issue #79).
use sim_core::edit::Command;
use sim_core::model::{SimConfig, SolverKind};
use sim_gui::dialogs::model_settings::SettingsDialogState;
use sim_gui::dialogs::{DialogButton, DialogOutcome};
use sim_gui::state::AppState;

fn stop_of(outcome: DialogOutcome) -> SimConfig {
    match outcome {
        DialogOutcome::CloseAndExecute(Command::SetConfig { config }) => config,
        other => panic!("expected CloseAndExecute(SetConfig), got {other:?}"),
    }
}

#[test]
fn open_uses_default_values() {
    let d = SettingsDialogState::open(&SimConfig::default());
    assert_eq!(d.stop, "10.0");
    assert_eq!(d.start, "0.0");
    assert_eq!(d.max_step, "auto");
    assert_eq!(d.solver, SolverKind::Auto);
}

#[test]
fn ok_with_new_stop_time_executes_set_config() {
    let mut d = SettingsDialogState::open(&SimConfig::default());
    d.stop = "2".into();
    let cfg = stop_of(d.press(DialogButton::Ok));
    let expected = SimConfig {
        stop_time: 2.0,
        ..SimConfig::default()
    };
    assert_eq!(cfg, expected);
}

#[test]
fn stop_equal_to_start_is_rejected() {
    let mut d = SettingsDialogState::open(&SimConfig::default());
    d.stop = "0".into();
    assert!(d.validate().is_none());
    assert_eq!(
        d.errors.get("stop").map(String::as_str),
        Some("stop_time must be greater than start_time")
    );
    assert_eq!(d.press(DialogButton::Ok), DialogOutcome::KeepOpen);
}

#[test]
fn tolerances_and_max_step_are_parsed() {
    let mut d = SettingsDialogState::open(&SimConfig::default());
    d.rel_tol = "1e-6".into();
    d.max_step = "0.01".into();
    let cfg = stop_of(d.press(DialogButton::Ok));
    assert!((cfg.rel_tol - 1e-6).abs() < 1e-15);
    assert_eq!(cfg.max_step, Some(0.01));
    assert!((cfg.stop_time - 10.0).abs() < 1e-12);
}

#[test]
fn max_step_is_case_insensitive_auto() {
    let mut d = SettingsDialogState::open(&SimConfig::default());
    d.max_step = "AUTO".into();
    let cfg = d.validate().expect("valid");
    assert_eq!(cfg.max_step, None);
}

#[test]
fn negative_output_step_is_rejected() {
    let mut d = SettingsDialogState::open(&SimConfig::default());
    d.output_step = "-1".into();
    assert!(d.validate().is_none());
    assert_eq!(
        d.errors.get("output_step").map(String::as_str),
        Some("output_step must be positive")
    );
}

#[test]
fn unchanged_ok_closes_without_a_command() {
    let mut d = SettingsDialogState::open(&SimConfig::default());
    assert_eq!(d.press(DialogButton::Ok), DialogOutcome::Close);
}

#[test]
fn cancel_closes_without_a_command() {
    let mut d = SettingsDialogState::open(&SimConfig::default());
    d.stop = "2".into();
    assert_eq!(d.press(DialogButton::Cancel), DialogOutcome::Close);
}

#[test]
fn apply_keeps_the_dialog_open_and_updates_the_baseline() {
    let mut d = SettingsDialogState::open(&SimConfig::default());
    d.stop = "2".into();
    assert!(matches!(
        d.press(DialogButton::Apply),
        DialogOutcome::Execute(Command::SetConfig { .. })
    ));
    assert_eq!(d.press(DialogButton::Ok), DialogOutcome::Close);
}

#[test]
fn app_state_ok_can_be_undone() {
    let mut st = AppState::new();
    let before = st.doc.model.config.clone();
    st.settings_dialog = Some(SettingsDialogState::open(&before));
    st.settings_dialog.as_mut().expect("dialog").stop = "2".into();
    st.settings_dialog_press(DialogButton::Ok);
    assert_eq!(st.doc.model.config.stop_time, 2.0);
    assert!(st.settings_dialog.is_none());
    st.undo();
    assert_eq!(st.doc.model.config, before);
}
