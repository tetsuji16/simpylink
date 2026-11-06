//! sim_gui: egui application.
//!
//! Pure logic (testable without egui): `canvas::{view, geometry, route, hit, controller}`,
//! `state`, `sim`, `scope::scope_series`, dialog states. The egui layer is `app`,
//! `canvas::widget`, `panels` and the `show_*` functions.
pub mod app;
pub mod canvas;
pub mod dialogs;
pub mod panels;
pub mod scope;
pub mod sim;
pub mod state;
pub mod theme;
pub mod toolstrip;
pub mod ui_icons;

pub use app::{run_gui, SimpylinkApp};

/// Returns the crate name.
pub fn crate_name() -> &'static str {
    "sim_gui"
}
