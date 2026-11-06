//! Dialog states (egui independent) and their egui views.
pub mod block_params;
pub mod model_settings;
pub mod quick_insert;

/// Dialog buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogButton {
    Ok,
    Cancel,
    Apply,
}

/// Result of pressing a dialog button.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogOutcome {
    KeepOpen,
    Close,
    CloseAndExecute(sim_core::edit::Command),
    Execute(sim_core::edit::Command),
}
